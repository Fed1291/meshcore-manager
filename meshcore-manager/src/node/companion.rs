//! Talking to a companion over USB or Bluetooth: the binary frames of src/proto.rs (on USB in
//! their serial wrapping, on Bluetooth one per write / notification).
//! One request at a time; frames the node pushes on its own (adverts, messages) are skipped.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};

use super::{telemetry, Answer, Fact};
use crate::i18n::{t, tf};
use crate::proto::{self as p, DeviceInfo, FrameParser, SelfInfo};
use crate::serial::Link;

/// Ask again this often during the hello: the first ask can land while the board is still booting
const HELLO_REPEAT: Duration = Duration::from_millis(1500);

/// How long a companion gets for one reply
pub const REPLY: Duration = Duration::from_secs(3);

/// A clock as shown: "2026-11-18 12:06:00 UTC"
fn clock_text(secs: i64) -> String {
    chrono::DateTime::from_timestamp(secs, 0).map_or_else(|| secs.to_string(), |d| d.format("%Y-%m-%d %H:%M:%S UTC").to_string())
}

/// The answer to `Companion::ask`
pub enum Got {
    Frame(Vec<u8>),
    /// The node said no: an error, or a feature this firmware was built without
    Refused,
    /// Nothing came in time
    Silent,
}

/// What the companion says about itself (either part can be missing on a very old firmware)
pub struct Hello {
    pub me: Option<SelfInfo>,
    pub device: Option<DeviceInfo>,
}

pub struct Companion {
    link: Link,
    parser: FrameParser,
    /// Frames received and not looked at yet
    waiting: VecDeque<Vec<u8>>,
}

impl Companion {
    pub fn new(link: Link) -> Companion {
        Companion { link, parser: FrameParser::default(), waiting: VecDeque::new() }
    }

    /// Gives the port back (to try the text console on it)
    pub fn into_link(self) -> Link {
        self.link
    }

    fn send(&mut self, frame: &[u8]) -> Result<()> {
        // the code and the size only: a frame can hold a key
        crate::debug_log!("DEBUG", "-> frame {:?}, {} bytes", frame.first(), frame.len());
        // Bluetooth: one write is one frame; USB: the frame in its serial wrapping
        let bytes = if self.link.carries_frames() { frame.to_vec() } else { p::encode(frame) };
        self.link.write(&bytes).with_context(|| t("serial-lost"))
    }

    /// The next frame, or None when none came before `deadline`
    fn next_frame(&mut self, deadline: Instant) -> Result<Option<Vec<u8>>> {
        let mut buf = [0u8; 1024];
        loop {
            if let Some(frame) = self.waiting.pop_front() {
                crate::debug_log!("DEBUG", "<- frame {:?}, {} bytes", frame.first(), frame.len());
                return Ok(Some(frame));
            }
            if Instant::now() >= deadline {
                return Ok(None);
            }
            if self.link.carries_frames() {
                let Some(frame) = self.link.read_frame(deadline)? else { return Ok(None) };
                self.waiting.push_back(frame);
                continue;
            }
            let n = self.link.read(&mut buf).with_context(|| t("serial-lost"))?;
            self.waiting.extend(self.parser.push(&buf[..n]));
        }
    }

    /// Asks who is there. None = no companion answered within `wait`.
    /// This also proves the node is ready before anything else is asked.
    pub fn hello(&mut self, wait: Duration) -> Result<Option<Hello>> {
        let deadline = Instant::now() + wait;
        let (mut me, mut device) = (None, None);
        while (me.is_none() || device.is_none()) && Instant::now() < deadline {
            self.send(&[p::CMD_DEVICE_QUERY, p::APP_VERSION])?;
            self.send(&p::app_start())?;
            let next_ask = (Instant::now() + HELLO_REPEAT).min(deadline);
            while me.is_none() || device.is_none() {
                let Some(frame) = self.next_frame(next_ask)? else { break };
                if let Some(info) = SelfInfo::parse(&frame) {
                    me = Some(info);
                } else if let Some(info) = DeviceInfo::parse(&frame) {
                    device = Some(info);
                }
            }
        }
        if me.is_none() && device.is_none() {
            return Ok(None);
        }
        // answers to a repeated ask must not be taken for the answer to the next request
        while self.next_frame(Instant::now() + Duration::from_millis(300))?.is_some() {}
        Ok(Some(Hello { me, device }))
    }

    /// Reads what only the companion's view shows: battery, storage, clock, sensors, counters,
    /// tuning, auto-add, private key (its contacts and channels come from the backup reading,
    /// backup::read). Only reads, except the clock set when behind. What an older firmware does
    /// not answer is simply not in the result.
    pub fn inside(&mut self) -> Result<Vec<Fact>> {
        let mut facts = Vec::new();
        if let Some(f) = self.request(&[p::CMD_GET_BATT_AND_STORAGE], p::RESP_BATT_AND_STORAGE, REPLY)? {
            if let Some(millivolts) = p::u16_at(&f, 1) {
                facts.push(Fact::new(t("fact-battery"), format!("{:.2} V", millivolts as f64 / 1000.0)));
            }
            if let (Some(used), Some(total)) = (p::u32_at(&f, 3), p::u32_at(&f, 7)) {
                facts.push(Fact::new(t("fact-storage"), format!("{used} / {total} kB")));
            }
        }
        if let Some(time) = self.clock()? {
            // behind this computer's (a node without a clock battery restarts in 2024): set it,
            // as the MeshCore apps do on connecting. The firmware takes only a later time.
            let off = time - chrono::Utc::now().timestamp();
            let synced = off < -super::CLOCK_SLACK && {
                let now = chrono::Utc::now().timestamp() as u32;
                let mut frame = vec![p::CMD_SET_DEVICE_TIME];
                frame.extend(now.to_le_bytes());
                matches!(self.command(&frame)?, Answer::Done)
            };
            let fact = match (synced, self.clock()?) {
                (true, Some(now)) => Fact::new(t("fact-clock"), super::clock_synced(&clock_text(now), &clock_text(time))).updated(),
                _ => Fact::new(t("fact-clock"), format!("{} ({off:+} s)", clock_text(time))),
            };
            facts.push(fact);
        }
        // its own sensors (battery, chip temperature, the board's sensors) and its counters,
        // (telemetry.rs)
        if let Some(f) = self.request(&[p::CMD_SEND_TELEMETRY_REQ, 0, 0, 0], p::PUSH_TELEMETRY_RESPONSE, REPLY)? {
            facts.extend(telemetry::sensor_facts(&f));
        }
        let core = self.request(&[p::CMD_GET_STATS, 0], p::RESP_STATS, REPLY)?;
        let radio = self.request(&[p::CMD_GET_STATS, 1], p::RESP_STATS, REPLY)?;
        let packets = self.request(&[p::CMD_GET_STATS, 2], p::RESP_STATS, REPLY)?;
        facts.extend(telemetry::stats_facts(core.as_deref(), radio.as_deref(), packets.as_deref()));
        if let Some(f) = self.request(&[p::CMD_GET_TUNING_PARAMS], p::RESP_TUNING_PARAMS, REPLY)? {
            if let (Some(rx), Some(af)) = (p::u32_at(&f, 1), p::u32_at(&f, 5)) {
                facts.extend(super::edit::tuning_settings(rx as f64 / 1000.0, af as f64 / 1000.0));
            }
        }
        // which nodes it adds to its contacts by itself (firmware 1.12+: an older one says no)
        if let Some(f) = self.request(&[p::CMD_GET_AUTOADD_CONFIG], p::RESP_AUTOADD_CONFIG, REPLY)? {
            if let (Some(config), Some(hops)) = (f.get(1), f.get(2)) {
                facts.extend(super::edit::autoadd_settings(*config, *hops));
            }
        }
        // its private key (a firmware built without key export says so: then it is left out),
        // hidden on screen until asked for
        if let Some(f) = self.request(&[p::CMD_EXPORT_PRIVATE_KEY], p::RESP_PRIVATE_KEY, REPLY)? {
            if f.len() > 1 {
                let key = p::hex(&f[1..]);
                facts.push(Fact::setting(t("fact-private-key"), key, super::edit::PRIVATE_KEY, "", true).secret());
            }
        }
        Ok(facts)
    }

    /// The node's clock, in seconds since 1970
    fn clock(&mut self) -> Result<Option<i64>> {
        let reply = self.request(&[p::CMD_GET_DEVICE_TIME], p::RESP_CURR_TIME, REPLY)?;
        Ok(reply.and_then(|f| p::u32_at(&f, 1)).map(i64::from))
    }

    /// Sends a command that changes something: the node answers OK or ERR (with an error code:
    /// ERR_CODE_* in the firmware). Sent once, whatever happens.
    pub fn command(&mut self, frame: &[u8]) -> Result<Answer> {
        self.send(frame)?;
        let deadline = Instant::now() + REPLY;
        while let Some(reply) = self.next_frame(deadline)? {
            match reply.first().copied() {
                Some(p::RESP_OK) => return Ok(Answer::Done),
                Some(p::RESP_ERR) => {
                    let code = reply.get(1).copied().unwrap_or(0);
                    return Ok(Answer::Refused(tf("edit-error-code", &[("code", code.into())])));
                }
                // the firmware was built without this feature (private key import)
                Some(p::RESP_DISABLED) => return Ok(Answer::Refused(t("edit-disabled"))),
                // pushed by the node on its own
                _ => {}
            }
        }
        Ok(Answer::Silent)
    }

    /// Sends a command and waits for the reply with code `wanted`, telling a "no" (the firmware
    /// does not have it) from silence (a problem: the link, the node)
    pub fn ask(&mut self, frame: &[u8], wanted: u8, wait: Duration) -> Result<Got> {
        self.send(frame)?;
        let deadline = Instant::now() + wait;
        while let Some(reply) = self.next_frame(deadline)? {
            match reply.first().copied() {
                Some(code) if code == wanted => return Ok(Got::Frame(reply)),
                Some(p::RESP_ERR) | Some(p::RESP_DISABLED) => return Ok(Got::Refused),
                // pushed by the node on its own, or a late reply to something else
                _ => {}
            }
        }
        Ok(Got::Silent)
    }

    /// Sends a command and waits for the reply with code `wanted`.
    /// None = the node said no (error, feature not built in) or did not answer within `wait`.
    pub fn request(&mut self, frame: &[u8], wanted: u8, wait: Duration) -> Result<Option<Vec<u8>>> {
        Ok(match self.ask(frame, wanted, wait)? {
            Got::Frame(reply) => Some(reply),
            Got::Refused | Got::Silent => None,
        })
    }

    /// Sends a command answered by many `item` frames, then one `end` frame (the contact list).
    /// None = the list did not come to its end (`wait` is per frame), it came without its first
    /// frame (the count), or it did not hold as many items as that frame announced (one lost on
    /// the way). Items before the first frame (the end of an older list) are not kept.
    pub fn request_list(&mut self, frame: &[u8], item: u8, end: u8, wait: Duration) -> Result<Option<Vec<Vec<u8>>>> {
        self.send(frame)?;
        let mut items = Vec::new();
        let mut announced: Option<u32> = None;
        while let Some(reply) = self.next_frame(Instant::now() + wait)? {
            match reply.first().copied() {
                Some(code) if code == item && announced.is_some() => items.push(reply),
                Some(code) if code == end && announced.is_some() => {
                    let whole = announced.is_some_and(|count| count as usize == items.len());
                    return Ok(whole.then_some(items));
                }
                Some(p::RESP_CONTACTS_START) => {
                    announced = p::u32_at(&reply, 1);
                    items.clear();
                }
                Some(p::RESP_ERR) => return Ok(None),
                // pushed by the node on its own
                _ => {}
            }
        }
        Ok(None)
    }
}
