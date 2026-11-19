//! MeshCore hardware: names a node's board from the model its firmware reports, and tells what a
//! USB serial port probably is. The list is built in (data/devices.toml); `devices.toml` in the
//! data folder adds to it and wins.

use std::path::Path;
use std::sync::OnceLock;

use serde::Deserialize;

use crate::i18n::tf;

const BUILT_IN: &str = include_str!("../data/devices.toml");

/// A board, from the model its firmware reports
#[derive(Deserialize, Clone)]
pub struct Device {
    /// The start of the reported model ("Heltec V4" for "Heltec V4 OLED")
    pub model: String,
    pub name: String,
    #[serde(default)]
    pub maker: String,
    #[serde(default)]
    pub chip: String,
}

/// What a USB serial port's vendor:product id usually is
#[derive(Deserialize, Clone)]
pub struct Usb {
    /// "303a:1001"
    pub id: String,
    pub chip: String,
    #[serde(default)]
    pub boards: String,
    /// In its bootloader: waiting to be flashed, no firmware running
    #[serde(default)]
    pub flash_mode: bool,
}

#[derive(Deserialize, Default)]
struct List {
    #[serde(default)]
    device: Vec<Device>,
    #[serde(default)]
    usb: Vec<Usb>,
}

static LIST: OnceLock<List> = OnceLock::new();

/// Reads the built-in list and the user's own file (if any). Once, at start.
/// Returns a line to show when the user's file could not be used.
pub fn init(own: &Path) -> Option<String> {
    let mut list: List = toml::from_str(BUILT_IN).unwrap_or_default();
    let mut note = None;
    if own.exists() {
        let read = std::fs::read_to_string(own)
            .map_err(|e| e.to_string())
            .and_then(|s| toml::from_str::<List>(&s).map_err(|e| e.to_string()));
        match read {
            Ok(more) => {
                // the user's entries first: same model or id = theirs is used
                list.device.retain(|d| !more.device.iter().any(|m| m.model.eq_ignore_ascii_case(&d.model)));
                list.usb.retain(|u| !more.usb.iter().any(|m| m.id.eq_ignore_ascii_case(&u.id)));
                list.device.extend(more.device);
                list.usb.extend(more.usb);
            }
            Err(e) => {
                note = Some(tf(
                    "devices-unreadable",
                    &[("file", own.display().to_string().into()), ("error", e.into())],
                ))
            }
        }
    }
    let _ = LIST.set(list);
    note
}

fn list() -> &'static List {
    LIST.get_or_init(|| toml::from_str(BUILT_IN).unwrap_or_default())
}

/// Every known board, for the Boards page
pub fn all() -> &'static [Device] {
    &list().device
}

/// Every known USB id, for the Boards page
pub fn all_usb() -> &'static [Usb] {
    &list().usb
}

/// The board a node is, from the model it reports ("Heltec V4 OLED" -> Heltec WiFi LoRa 32 V4):
/// the longest entry it starts with, ignoring case, ending at a word boundary
pub fn by_model(reported: &str) -> Option<&'static Device> {
    let lower = reported.trim().to_lowercase();
    list()
        .device
        .iter()
        .filter(|d| {
            let m = d.model.trim().to_lowercase();
            !m.is_empty()
                && lower.starts_with(&m)
                && lower[m.len()..].chars().next().is_none_or(|c| !c.is_alphanumeric())
        })
        .max_by_key(|d| d.model.trim().len())
}

/// What a USB port is, from its vendor and product ids
pub fn by_usb(vendor: u16, product: u16) -> Option<&'static Usb> {
    let id = format!("{vendor:04x}:{product:04x}");
    list().usb.iter().find(|u| u.id.trim().eq_ignore_ascii_case(&id))
}

/// "Heltec WiFi LoRa 32 V4 (Heltec, ESP32-S3)", or the reported model as it is when unknown
pub fn describe(reported: &str) -> String {
    match by_model(reported) {
        Some(d) => {
            let extra: Vec<&str> =
                [d.maker.as_str(), d.chip.as_str()].into_iter().filter(|s| !s.is_empty() && !d.name.contains(*s)).collect();
            if extra.is_empty() {
                d.name.clone()
            } else {
                format!("{} ({})", d.name, extra.join(", "))
            }
        }
        None => reported.trim().to_string(),
    }
}
