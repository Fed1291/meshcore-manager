//! Bluetooth companions: found by themselves, used like a USB one.
//!
//! - `Watcher`: listens, in the background, for MeshCore companions advertising nearby (the
//!   Nordic UART service MeshCore uses, or a name starting with "MeshCore"). Nothing connects:
//!   a companion connected to a phone does not advertise, so it is never taken from the phone.
//!   A device not heard for `KEEP` drops out of the list.
//! - `Conn`: one connection, opened when a job needs the node (reading it, a backup, a commit,
//!   a restore) and closed when the job ends. Pairs first when needed, with the PIN given for
//!   that companion this session (`give_pin`), else `[bluetooth] pin` (MeshCore's 123456). A
//!   refused pairing is noted (`take_refused`): the worker then asks the user for the PIN and
//!   runs the job again. A pairing the companion lost is removed and made again, once.
//!   Each GATT write is one companion frame, each notification one frame (no USB framing).
//!
//! Per system, the same API: `linux.rs` (BlueZ, through bluer, made blocking
//! for the worker's threads), `windows.rs` (Windows' own Bluetooth LE API, WinRT) and `other.rs`
//! (macOS and the rest: not available yet, said in the Fleet).
//! A node on Bluetooth is a `serial::Port` whose path is `ble:<address>`: claims, jobs, backups
//! and restores work the same for both links.

// macOS and the rest: other.rs uses none of the link's pieces below (no warning for them there)
#![cfg_attr(not(any(windows, target_os = "linux")), allow(dead_code, unused_imports))]

use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[cfg_attr(windows, path = "windows.rs")]
#[cfg_attr(target_os = "linux", path = "linux.rs")]
#[cfg_attr(not(any(windows, target_os = "linux")), path = "other.rs")]
mod os;

pub use os::{forget, Conn};

/// The path of a Bluetooth node: `ble:AA:BB:CC:DD:EE:FF`
pub const PREFIX: &str = "ble:";
/// MeshCore's default pairing PIN (companion firmware `BLE_PIN_CODE`)
pub const DEFAULT_PIN: &str = "123456";

/// The Nordic UART service: the app writes frames to RX, the companion notifies them on TX
const NUS_SERVICE: u128 = 0x6e400001_b5a3_f393_e0a9_e50e24dcca9e;
const NUS_RX: u128 = 0x6e400002_b5a3_f393_e0a9_e50e24dcca9e;
const NUS_TX: u128 = 0x6e400003_b5a3_f393_e0a9_e50e24dcca9e;

/// A device not heard this long drops out of the list (a companion advertises every second or so;
/// a scan can miss some)
const KEEP: Duration = Duration::from_secs(45);
/// How long a connection may take (found, paired, connected, services read)
const CONNECT_WAIT: Duration = Duration::from_secs(40);
/// How long one frame write may take
const WRITE_WAIT: Duration = Duration::from_secs(10);
/// Watching again after Bluetooth went away (turned off, dongle pulled out)
const RETRY: Duration = Duration::from_secs(5);

/// A companion heard advertising
#[derive(Clone, Debug)]
pub struct Seen {
    /// "AA:BB:CC:DD:EE:FF"
    pub address: String,
    /// As it advertises itself, "MeshCore-" taken off ("" until a name was heard)
    pub name: String,
    /// Signal strength, dBm
    pub rssi: Option<i16>,
    heard: Instant,
}

/// What the watcher heard, shared between its thread and the worker
#[derive(Clone, Default)]
struct Heard {
    devices: Arc<Mutex<HashMap<String, Seen>>>,
    /// Why nothing can be heard (no Bluetooth, turned off); None when it listens
    problem: Arc<Mutex<Option<String>>>,
}

impl Heard {
    /// A device advertised: kept when it is a MeshCore companion. The name and the service can
    /// come in separate packets (advertisement, scan response): a device once known as MeshCore
    /// stays so.
    fn hear(&self, address: String, name: Option<String>, rssi: Option<i16>, nus: bool) {
        let mut devices = self.devices.lock().unwrap_or_else(|p| p.into_inner());
        let named_meshcore = name.as_deref().is_some_and(|n| n.starts_with("MeshCore"));
        let known = devices.contains_key(&address);
        if !(nus || named_meshcore || known) {
            return;
        }
        let entry = devices.entry(address.clone()).or_insert_with(|| Seen { address, name: String::new(), rssi: None, heard: Instant::now() });
        if let Some(name) = name.filter(|n| !n.trim().is_empty()) {
            entry.name = shown_name(&name);
        }
        if rssi.is_some() {
            entry.rssi = rssi;
        }
        entry.heard = Instant::now();
    }

    fn set_problem(&self, problem: Option<String>) {
        *self.problem.lock().unwrap_or_else(|p| p.into_inner()) = problem;
    }
}

/// "MeshCore-Node A" -> "Node A" (the prefix says nothing the list does not)
fn shown_name(name: &str) -> String {
    let name = name.trim();
    name.strip_prefix("MeshCore-").or_else(|| name.strip_prefix("MeshCore")).map_or(name, str::trim).to_string()
}

/// Listens for companions in the background for as long as it lives
pub struct Watcher {
    heard: Heard,
}

impl Watcher {
    pub fn start() -> Watcher {
        let heard = Heard::default();
        os::watch(heard.clone());
        Watcher { heard }
    }

    /// The companions heard lately, strongest first
    pub fn seen(&self) -> Vec<Seen> {
        let mut devices = self.heard.devices.lock().unwrap_or_else(|p| p.into_inner());
        devices.retain(|_, s| s.heard.elapsed() < KEEP);
        let mut list: Vec<Seen> = devices.values().cloned().collect();
        list.sort_by_key(|s| (std::cmp::Reverse(s.rssi.unwrap_or(i16::MIN)), s.address.clone()));
        list
    }

    /// Why no companion can be heard (no adapter, Bluetooth off), if so
    pub fn problem(&self) -> Option<String> {
        self.heard.problem.lock().unwrap_or_else(|p| p.into_inner()).clone()
    }
}

/// A pairing PIN is 6 digits: a Bluetooth LE passkey (000000-999999), and what MeshCore draws or
/// takes (MyMesh.cpp CMD_SET_DEVICE_PIN: 100000-999999)
pub const PIN_DIGITS: usize = 6;

/// The PIN as pairing takes it: 6 digits
pub fn pin(text: &str) -> Option<String> {
    let pin = text.trim();
    (pin.len() == PIN_DIGITS && pin.chars().all(|c| c.is_ascii_digit())).then(|| pin.to_string())
}

/// The PINs the user gave for one companion, this session, by address (typed with `p`). A
/// companion with a screen draws a new random PIN at every start unless one was set (firmware
/// `companion_radio/main.cpp`), and any companion can have its own (MeshCore app).
static GIVEN: Mutex<Vec<(String, String)>> = Mutex::new(Vec::new());

/// The PIN each companion last connected with, kept in `bluetooth.toml` in the data folder
/// (`use_pin_file` at start): tried first the next time, then the one from Settings, and only
/// then is the user asked. Never in a log.
static SAVED: Mutex<Option<Saved>> = Mutex::new(None);

struct Saved {
    path: PathBuf,
    pins: BTreeMap<String, String>,
}

#[derive(Default, serde::Serialize, serde::Deserialize)]
struct PinFile {
    /// address -> PIN
    #[serde(default)]
    pins: BTreeMap<String, String>,
}

/// Reads the saved PINs (a missing or unreadable file: none; it is written again at the next
/// pairing)
pub fn use_pin_file(path: PathBuf) {
    let pins = std::fs::read_to_string(&path)
        .ok()
        .and_then(|text| toml::from_str::<PinFile>(&text).ok())
        .map(|f| f.pins.into_iter().filter(|(_, p)| pin(p).is_some()).collect())
        .unwrap_or_default();
    *SAVED.lock().unwrap_or_else(|p| p.into_inner()) = Some(Saved { path, pins });
}

/// The PIN `address` last connected with
pub fn saved_pin(address: &str) -> Option<String> {
    SAVED.lock().unwrap_or_else(|p| p.into_inner()).as_ref().and_then(|s| s.pins.get(address).cloned())
}

/// A PIN is known for `address` (saved, or given this session): it connects by itself
pub fn known_pin(address: &str) -> bool {
    given_pin(address).is_some() || saved_pin(address).is_some()
}

/// `address` connected with `pin`: kept for the next time (written only when it changed:
/// a new file, then in place of the old one)
pub fn save_pin(address: &str, pin: &str) {
    let mut saved = SAVED.lock().unwrap_or_else(|p| p.into_inner());
    let Some(saved) = saved.as_mut() else { return };
    if saved.pins.get(address).is_some_and(|p| p == pin) {
        return;
    }
    saved.pins.insert(address.to_string(), pin.to_string());
    let file = PinFile { pins: saved.pins.clone() };
    let mut text = String::from("# The PIN each Bluetooth companion last paired with (Meshcore Manager). Keep it to yourself.\n\n");
    text.push_str(&toml::to_string_pretty(&file).unwrap_or_default());
    let new = saved.path.with_extension("toml.new");
    let written = std::fs::write(&new, text).and_then(|_| std::fs::rename(&new, &saved.path));
    match written {
        Ok(()) => crate::debug_log!("INFO", "bluetooth: pin saved for a companion"),
        Err(e) => crate::debug_log!("WARN", "bluetooth: pin not saved ({e})"),
    }
}

/// `address` is unpaired: its saved PIN goes too (the next pairing starts from Settings)
pub fn forget_pin(address: &str) {
    let mut saved = SAVED.lock().unwrap_or_else(|p| p.into_inner());
    let Some(saved) = saved.as_mut() else { return };
    if saved.pins.remove(address).is_some() {
        let file = PinFile { pins: saved.pins.clone() };
        let text = toml::to_string_pretty(&file).unwrap_or_default();
        if let Err(e) = std::fs::write(&saved.path, text) {
            crate::debug_log!("WARN", "bluetooth: pin file not written ({e})");
        }
    }
    GIVEN.lock().unwrap_or_else(|p| p.into_inner()).retain(|(a, _)| a != address);
}
/// The companions whose pairing was refused (wrong PIN), until the worker asks the user
static REFUSED: Mutex<Vec<String>> = Mutex::new(Vec::new());

/// The PIN given for `address` this session (None: the default one from Settings is used)
pub fn given_pin(address: &str) -> Option<String> {
    let given = GIVEN.lock().unwrap_or_else(|p| p.into_inner());
    given.iter().find(|(a, _)| a == address).map(|(_, pin)| pin.clone())
}

/// Use `pin` for `address` from now on (None: back to the default one)
pub fn give_pin(address: &str, pin: Option<String>) {
    let mut given = GIVEN.lock().unwrap_or_else(|p| p.into_inner());
    given.retain(|(a, _)| a != address);
    if let Some(pin) = pin {
        given.push((address.to_string(), pin));
    }
}

/// Pairing with `address` was refused: the PIN tried is not the companion's
fn refused(address: &str) {
    let mut list = REFUSED.lock().unwrap_or_else(|p| p.into_inner());
    if !list.iter().any(|a| a == address) {
        list.push(address.to_string());
    }
}

/// Whether pairing with `address` was refused since the last ask (and forgets it)
pub fn take_refused(address: &str) -> bool {
    let mut list = REFUSED.lock().unwrap_or_else(|p| p.into_inner());
    let was = list.iter().any(|a| a == address);
    list.retain(|a| a != address);
    was
}

/// The system holds a pairing the companion no longer has (it was erased, re-flashed, or paired
/// elsewhere): the connection removes it and pairs again, once
#[derive(Debug)]
struct BondLost;

impl std::fmt::Display for BondLost {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&crate::i18n::t("ble-not-paired"))
    }
}

impl std::error::Error for BondLost {}

/// What a connection tells the job using it
enum Event {
    /// One companion frame
    Frame(Vec<u8>),
    /// The connection is gone (the reason)
    Lost(String),
}

/// The next frame of a connection, or None when none came before `deadline`. Lost = an error.
fn next(events: &std::sync::mpsc::Receiver<Event>, deadline: Instant) -> anyhow::Result<Option<Vec<u8>>> {
    use std::sync::mpsc::RecvTimeoutError;
    let wait = deadline.saturating_duration_since(Instant::now());
    match events.recv_timeout(wait) {
        Ok(Event::Frame(frame)) => Ok(Some(frame)),
        Ok(Event::Lost(why)) => Err(anyhow::anyhow!(crate::i18n::tf("ble-lost", &[("why", why.into())]))),
        Err(RecvTimeoutError::Timeout) => Ok(None),
        Err(RecvTimeoutError::Disconnected) => Err(anyhow::anyhow!(crate::i18n::tf("ble-lost", &[("why", "-".into())]))),
    }
}
