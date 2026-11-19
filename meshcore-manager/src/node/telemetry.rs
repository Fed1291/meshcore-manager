//! A companion's own measurements, read with its view (only reads):
//! - its sensors: CMD_SEND_TELEMETRY_REQ with no key (firmware MyMesh.cpp "self telemetry
//!   request"): battery voltage, the chip's temperature, then every sensor of the board, in LPP
//!   records (types, sizes and dividers from helpers/sensors/LPPDataHelpers.h);
//! - its counters: CMD_GET_STATS core (uptime, queue), radio (noise floor, last packet, air
//!   time), packets (received, sent, flood / direct, errors). Firmware v8+.
//!
//! What a firmware does not answer is simply left out.

use super::Fact;
use crate::i18n::{t, tf};
use crate::proto as p;

/// One telemetry value: LPP channel (1 = the node itself), what it is, value in its unit
struct Reading {
    channel: u8,
    kind: &'static str,
    value: f64,
}

/// Big-endian LPP value
fn be(b: &[u8], signed: bool) -> f64 {
    let mut v: u64 = 0;
    for x in b {
        v = (v << 8) | *x as u64;
    }
    if signed {
        let bit = 1u64 << (b.len() * 8 - 1);
        if v & bit != 0 {
            return -(((bit << 1) - v) as f64);
        }
    }
    v as f64
}

/// LPP type -> (name, data size, divider, signed), as the firmware writes them. A wrong size
/// here shifts every value that follows in the same answer.
fn lpp_type(t: u8) -> Option<(&'static str, usize, f64, bool)> {
    Some(match t {
        0 => ("digital_in", 1, 1.0, false),
        1 => ("digital_out", 1, 1.0, false),
        2 => ("analog_in", 2, 100.0, true),
        3 => ("analog_out", 2, 100.0, true),
        100 => ("generic", 4, 1.0, false),
        101 => ("illuminance", 2, 1.0, false),
        102 => ("presence", 1, 1.0, false),
        103 => ("temperature", 2, 10.0, true),
        // 1 byte, 0.5 % steps
        104 => ("humidity", 1, 2.0, false),
        // 2 bytes per axis, 0.001 G
        113 => ("accelerometer", 6, 1000.0, true),
        115 => ("pressure", 2, 10.0, false),
        116 => ("voltage", 2, 100.0, false),
        117 => ("current", 2, 1000.0, false),
        118 => ("frequency", 4, 1.0, false),
        120 => ("percentage", 1, 1.0, false),
        121 => ("altitude", 2, 1.0, true),
        125 => ("concentration", 2, 1.0, false),
        128 => ("power", 2, 1.0, false),
        130 => ("distance", 4, 1000.0, false),
        131 => ("energy", 4, 1000.0, false),
        132 => ("direction", 2, 1.0, false),
        133 => ("unixtime", 4, 1.0, false),
        // 2 bytes per axis, 0.01 degree/s
        134 => ("gyrometer", 6, 100.0, true),
        135 => ("colour", 3, 1.0, false),
        136 => ("gps", 9, 1.0, true),
        142 => ("switch", 1, 1.0, false),
        // unknown or of variable size: the rest cannot be read
        _ => return None,
    })
}

/// LPP records: channel, type, data... (channel 0 = the end)
fn lpp(d: &[u8]) -> Vec<Reading> {
    let mut out = Vec::new();
    let mut i = 0;
    while i + 2 <= d.len() {
        let (channel, typ) = (d[i], d[i + 1]);
        if channel == 0 {
            break;
        }
        i += 2;
        let Some((kind, size, div, signed)) = lpp_type(typ) else { break };
        let Some(v) = d.get(i..i + size) else { break };
        let mut push = |kind: &'static str, value: f64| out.push(Reading { channel, kind, value });
        match typ {
            136 => {
                push("latitude", be(&v[0..3], true) / 10_000.0);
                push("longitude", be(&v[3..6], true) / 10_000.0);
                push("gps_altitude", be(&v[6..9], true) / 100.0);
            }
            113 => {
                push("accel_x", be(&v[0..2], signed) / div);
                push("accel_y", be(&v[2..4], signed) / div);
                push("accel_z", be(&v[4..6], signed) / div);
            }
            134 => {
                push("gyro_x", be(&v[0..2], signed) / div);
                push("gyro_y", be(&v[2..4], signed) / div);
                push("gyro_z", be(&v[4..6], signed) / div);
            }
            _ => push(kind, be(v, signed) / div),
        }
        i += size;
    }
    // a GPS without a fix reports 0,0
    let zero = |k: &str| out.iter().any(|r| r.kind == k && r.value == 0.0);
    if zero("latitude") && zero("longitude") {
        out.retain(|r| !matches!(r.kind, "latitude" | "longitude" | "gps_altitude"));
    }
    out
}

/// A reading's unit and decimals
fn unit(kind: &str) -> (&'static str, usize) {
    match kind {
        "temperature" => ("°C", 1),
        "humidity" => ("%", 1),
        "pressure" => ("hPa", 1),
        "voltage" => ("V", 2),
        "current" => ("A", 3),
        "illuminance" => ("lx", 0),
        "altitude" | "gps_altitude" => ("m", 1),
        "latitude" | "longitude" => ("°", 5),
        "percentage" => ("%", 0),
        "power" => ("W", 0),
        "distance" => ("m", 3),
        "energy" => ("kWh", 3),
        "direction" => ("°", 0),
        "frequency" => ("Hz", 0),
        "concentration" => ("ppm", 0),
        "accel_x" | "accel_y" | "accel_z" => ("G", 3),
        "gyro_x" | "gyro_y" | "gyro_z" => ("°/s", 2),
        "analog_in" | "analog_out" => ("", 2),
        _ => ("", 0),
    }
}

/// The sensors of a PUSH_TELEMETRY_RESPONSE as facts: "temperature  21.5 °C". A channel other
/// than the node's own (1) is named: "temperature (2)".
pub fn sensor_facts(frame: &[u8]) -> Vec<Fact> {
    if frame.first() != Some(&p::PUSH_TELEMETRY_RESPONSE) || frame.len() < 8 {
        return Vec::new();
    }
    lpp(&frame[8..])
        .into_iter()
        .map(|r| {
            let name = t(&format!("tel-{}", r.kind.replace('_', "-")));
            let label = if r.channel > 1 { format!("{name} ({})", r.channel) } else { name };
            let (unit, decimals) = unit(r.kind);
            Fact::new(label, format!("{:.decimals$} {unit}", r.value).trim_end().to_string())
        })
        .collect()
}

/// "3 d 4 h", "4 h 12 min", "12 min", "45 s"
fn span(secs: u32) -> String {
    let (d, h, m) = (secs / 86_400, secs % 86_400 / 3_600, secs % 3_600 / 60);
    match (d, h, m) {
        (0, 0, 0) => tf("span-seconds", &[("s", secs.into())]),
        (0, 0, m) => tf("span-minutes", &[("m", m.into())]),
        (0, h, m) => tf("span-hours", &[("h", h.into()), ("m", m.into())]),
        (d, h, _) => tf("span-days", &[("d", d.into()), ("h", h.into())]),
    }
}

fn i16_at(f: &[u8], at: usize) -> Option<i16> {
    f.get(at..at + 2).map(|b| i16::from_le_bytes([b[0], b[1]]))
}

/// The counters of the RESP_STATS answers (core, radio, packets: any of them may be missing)
pub fn stats_facts(core: Option<&[u8]>, radio: Option<&[u8]>, packets: Option<&[u8]>) -> Vec<Fact> {
    let mut facts = Vec::new();
    // core: battery (already read), uptime, error flags, queue
    if let Some(f) = core.filter(|f| f.get(1) == Some(&0)) {
        if let Some(up) = p::u32_at(f, 4) {
            facts.push(Fact::new(t("stat-uptime"), span(up)));
        }
        if let Some(queue) = f.get(10) {
            facts.push(Fact::new(t("stat-queue"), queue.to_string()));
        }
    }
    // radio: noise floor, the last packet heard, air time
    if let Some(f) = radio.filter(|f| f.get(1) == Some(&1)) {
        if let Some(noise) = i16_at(f, 2) {
            facts.push(Fact::new(t("stat-noise"), format!("{noise} dBm")));
        }
        if let (Some(rssi), Some(snr)) = (f.get(4), f.get(5)) {
            let snr = *snr as i8 as f64 / 4.0;
            facts.push(Fact::new(t("stat-last-packet"), format!("{} dBm, {snr:.2} dB", *rssi as i8)));
        }
        if let (Some(tx), Some(rx)) = (p::u32_at(f, 6), p::u32_at(f, 10)) {
            facts.push(Fact::new(t("stat-air-time"), tf("stat-air-time-value", &[("tx", span(tx).into()), ("rx", span(rx).into())])));
        }
    }
    // packets: received, sent; then flood / direct and errors (newer firmware)
    if let Some(f) = packets.filter(|f| f.get(1) == Some(&2)) {
        if let (Some(recv), Some(sent)) = (p::u32_at(f, 2), p::u32_at(f, 6)) {
            facts.push(Fact::new(t("stat-packets"), tf("stat-packets-value", &[("recv", recv.into()), ("sent", sent.into())])));
        }
        if let (Some(sf), Some(sd)) = (p::u32_at(f, 10), p::u32_at(f, 14)) {
            facts.push(Fact::new(t("stat-sent-by"), tf("stat-by-value", &[("flood", sf.into()), ("direct", sd.into())])));
        }
        if let (Some(rf), Some(rd)) = (p::u32_at(f, 18), p::u32_at(f, 22)) {
            facts.push(Fact::new(t("stat-recv-by"), tf("stat-by-value", &[("flood", rf.into()), ("direct", rd.into())])));
        }
        if let Some(errors) = p::u32_at(f, 26) {
            facts.push(Fact::new(t("stat-recv-errors"), errors.to_string()));
        }
    }
    facts
}
