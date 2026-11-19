//! A node on a USB port: what kind it is and what it says about itself.
//! Two ways to talk to MeshCore firmware over USB, one module each:
//! - companion.rs: the companion protocol (binary frames), "companion USB" firmware;
//! - cli.rs: the text console of repeaters, room servers and sensors.
//! A new kind of node (KISS modem, Meshtastic...) = a new module here and a case in `read`.

/// The access list of a console node (who may log in as what)
pub mod acl;
pub mod cli;
pub mod companion;
pub mod edit;
pub mod telemetry;

use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::devices;
use crate::i18n::{t, tf};
use crate::proto::{Channel, Contact, SelfInfo};
use crate::regions::NodeRegions;
use crate::serial::{Link, Port};

/// Saved as its name ("room_server"), read through a plain text (`from` / `into`): a kind a newer
/// version adds is read here as `Unknown`, so its backups and journals still open (serde's
/// `#[serde(other)]` is only for tagged enums, serde.rs "Variant attributes")
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(from = "String", into = "String")]
pub enum Kind {
    Companion,
    Repeater,
    RoomServer,
    Sensor,
    /// Waiting to be flashed: no firmware running
    Bootloader,
    /// Nothing answered: another firmware, a Bluetooth-only companion, or not a node at all
    Unknown,
}

/// The names on disk: never change one (old files would read as `Unknown`)
const KIND_NAMES: [(Kind, &str); 6] = [
    (Kind::Companion, "companion"),
    (Kind::Repeater, "repeater"),
    (Kind::RoomServer, "room_server"),
    (Kind::Sensor, "sensor"),
    (Kind::Bootloader, "bootloader"),
    (Kind::Unknown, "unknown"),
];

impl From<String> for Kind {
    fn from(name: String) -> Kind {
        KIND_NAMES.iter().find(|(_, n)| *n == name).map_or(Kind::Unknown, |(k, _)| *k)
    }
}

impl From<Kind> for String {
    fn from(kind: Kind) -> String {
        KIND_NAMES.iter().find(|(k, _)| *k == kind).map_or("unknown", |(_, n)| n).to_string()
    }
}

impl Kind {
    /// From the answer to `get role` (FIRMWARE_ROLE in the firmware's examples)
    fn from_role(role: &str) -> Option<Kind> {
        match role.trim() {
            "repeater" => Some(Kind::Repeater),
            "room_server" => Some(Kind::RoomServer),
            "sensor" => Some(Kind::Sensor),
            _ => None,
        }
    }

    pub fn label(self) -> String {
        t(match self {
            Kind::Companion => "kind-companion",
            Kind::Repeater => "kind-repeater",
            Kind::RoomServer => "kind-room-server",
            Kind::Sensor => "kind-sensor",
            Kind::Bootloader => "kind-bootloader",
            Kind::Unknown => "kind-unknown",
        })
    }

    /// Runs MeshCore firmware we can talk to (so it can be backed up)
    pub fn is_node(self) -> bool {
        matches!(self, Kind::Companion | Kind::Repeater | Kind::RoomServer | Kind::Sensor)
    }
}

/// A node clock this far behind the computer's (seconds) is set when the node is read; a
/// console shows only minutes, so less is not worth a write
pub const CLOCK_SLACK: i64 = 120;

/// The clock line after it was set: "2026-11-18 12:06  ·  synced (was 2024-05-15 20:50)"
pub fn clock_synced(now: &str, was: &str) -> String {
    crate::i18n::tf("fact-clock-synced", &[("now", now.into()), ("was", was.into())])
}

/// One more thing known about a node, for its own view: "battery" - "4.07 V".
/// With `edit`: a setting the node lets change (edit mode, node/edit.rs).
#[derive(Clone, Debug)]
pub struct Fact {
    pub label: String,
    pub value: String,
    pub edit: Option<Editable>,
    /// A password or key: hidden on screen unless asked for (`s`), never logged
    pub secret: bool,
    /// This program just set it on the node (the clock, synced when read): shown in its own colour
    pub updated: bool,
}

/// How a setting is changed
#[derive(Clone, Debug)]
pub struct Editable {
    /// The setting's id: the console's own name ("tx", "af"), or one of node/edit.rs for a companion
    pub id: String,
    /// What the value looks like ("5-12", "on|off", "<dbm>")
    pub hint: String,
    /// It restarts the node or changes how it transmits: said so before a commit
    pub danger: bool,
    /// What it does, for edit mode ("" when not known)
    pub about: String,
}

impl Fact {
    pub fn new(label: String, value: String) -> Fact {
        Fact { label, value, edit: None, secret: false, updated: false }
    }

    pub fn setting(label: String, value: String, id: &str, hint: &str, danger: bool) -> Fact {
        let edit = Editable { id: id.to_string(), hint: hint.to_string(), danger, about: String::new() };
        Fact { label, value, edit: Some(edit), secret: false, updated: false }
    }

    /// Just set on the node by this program (`Fact::updated`)
    pub fn updated(mut self) -> Fact {
        self.updated = true;
        self
    }

    /// A secret (`Fact::secret`); an empty value = the node cannot tell it (the admin password)
    pub fn secret(mut self) -> Fact {
        self.secret = true;
        self
    }

    /// A setting with what it does
    pub fn with_about(mut self, about: String) -> Fact {
        if let Some(edit) = &mut self.edit {
            edit.about = about;
        }
        self
    }
}

/// What a node answered to a change
pub enum Answer {
    Done,
    /// It said no (the reason, as it gave it)
    Refused(String),
    /// Nothing came back: it may or may not have done it. Never sent again.
    Silent,
}

#[derive(Clone, Debug)]
pub struct Node {
    pub port: Port,
    pub kind: Kind,
    /// Seen on the port, not asked yet
    pub probing: bool,
    /// When this was made: for a node being identified, since when (the Fleet page shows it)
    pub seen: Instant,
    /// A companion's settings as it sent them (name, position, radio): what an edit starts from
    pub me: Option<SelfInfo>,
    /// A companion's client repeat setting, kept as it is when the radio is changed
    pub repeat: Option<bool>,
    pub name: String,
    /// The board as the firmware names it ("Heltec V3"); "" when it did not say
    pub model: String,
    /// "v1.18.0 (15 Oct 2026)"
    pub firmware: String,
    /// 64 hex characters, "" when unknown
    pub public_key: String,
    pub radio: String,
    /// What else it told: a few at identification, more once its view was opened (`inspect`)
    pub facts: Vec<Fact>,
    /// A companion's contact list and its channels in use (empty until `inspect`)
    pub contacts: Vec<Contact>,
    pub channels: Vec<Channel>,
    /// A repeater's / room server's / sensor's regions (None: not read yet, or its firmware has
    /// none)
    pub regions: Option<NodeRegions>,
    /// A repeater's / room server's / sensor's access list (None: not read yet, or its firmware
    /// cannot print it)
    pub acl: Option<Vec<acl::Entry>>,
    /// Bluetooth signal strength as last heard, dBm (None on USB)
    pub signal: Option<i16>,
    /// `inspect` read it: `contacts` and `channels` are what the node holds, not "not read yet"
    pub inspected: bool,
    /// The whole state that reading got, as a backup keeps it (a manual backup writes it as it
    /// is, at once); None until inspected, or when that part of the reading failed
    pub reading: Option<std::sync::Arc<crate::backup::Reading>>,
    /// Why it is not identified, or a hint (one line)
    pub note: String,
}

impl Node {
    pub fn new(port: Port) -> Node {
        Node {
            port,
            kind: Kind::Unknown,
            probing: false,
            seen: Instant::now(),
            me: None,
            repeat: None,
            name: String::new(),
            model: String::new(),
            firmware: String::new(),
            public_key: String::new(),
            radio: String::new(),
            facts: Vec::new(),
            contacts: Vec::new(),
            channels: Vec::new(),
            regions: None,
            acl: None,
            signal: None,
            inspected: false,
            reading: None,
            note: String::new(),
        }
    }

    /// The board: from the firmware's model when it answered, else what the USB id usually is
    pub fn board(&self) -> String {
        if !self.model.is_empty() {
            return devices::describe(&self.model);
        }
        // Bluetooth: unknown until the companion is read (its advertised name is its own name)
        if self.port.is_ble() {
            return String::new();
        }
        match self.port.board() {
            Some(usb) => usb.chip.clone(),
            None => self.port.label(),
        }
    }
}

/// Opens the port and asks who is there: the quick look, for the list.
/// Never fails: what went wrong is in `note`. A silent port costs about `wait`.
pub fn identify(port: &Port, wait: Duration) -> Node {
    read(port, wait, false)
}

/// `identify`, then everything else the node can tell (battery, storage, clock, more settings):
/// the long look, for the node's own view. Only reads.
pub fn inspect(port: &Port, wait: Duration) -> Node {
    read(port, wait, true)
}

fn read(port: &Port, wait: Duration, deep: bool) -> Node {
    let mut node = Node::new(port.clone());
    // a board in its bootloader runs no firmware, and nothing should be written into it
    if port.board().is_some_and(|b| b.flash_mode) {
        node.kind = Kind::Bootloader;
        node.note = t("node-bootloader");
        return node;
    }
    // a Bluetooth companion that could not be connected (pairing, out of range) is still the
    // companion it advertises: kind and name kept, the reason in the note
    if port.is_ble() {
        node.kind = Kind::Companion;
        node.name = port.product.clone();
    }
    let link = match Link::open(&port.path) {
        Ok(link) => link,
        Err(e) => {
            node.note = format!("{e:#}");
            return node;
        }
    };

    // companion protocol first
    let mut companion = companion::Companion::new(link);
    match companion.hello(wait) {
        Ok(Some(hello)) => {
            node.kind = Kind::Companion;
            if let Some(me) = hello.me {
                node.radio = me.radio();
                node.facts.extend(edit::companion_settings(&me));
                node.facts.push(Fact::new(t("fact-max-power"), format!("{} dBm", me.max_tx_power_dbm)));
                node.name = me.name.clone();
                node.public_key = me.public_key.clone();
                node.me = Some(me);
            }
            if let Some(device) = hello.device {
                node.repeat = device.repeat;
                node.facts.extend(device.path_hash_mode.map(edit::path_hash_setting));
                node.facts.extend(device.ble_pin.map(edit::ble_pin_setting));
                node.firmware = format!("{} ({})", device.version, device.build_date);
                node.model = device.model;
                node.facts.push(Fact::new(
                    t("fact-capacity"),
                    tf(
                        "fact-capacity-value",
                        &[("contacts", device.max_contacts.into()), ("channels", device.max_channels.into())],
                    ),
                ));
            }
            if deep {
                // what only the view shows (battery, clock, stats...), then the whole state as a
                // backup keeps it: the contacts and channels shown come from it, and it stays
                // with the node (a manual backup writes it at once)
                let read = companion.inside().and_then(|facts| {
                    let state = crate::backup::read::companion(&mut companion, true, &mut |_, _, _| {})?;
                    Ok((facts, state))
                });
                match read {
                    Ok((facts, state)) => {
                        node.facts.extend(facts);
                        node.channels = state.frames("channels").iter().filter_map(|f| Channel::parse(f)).filter(|c| !c.name.is_empty()).collect();
                        node.contacts = state.frames("contacts").iter().filter_map(|f| Contact::parse(f)).collect();
                        // for the view: last heard first
                        node.contacts.sort_by(|a, b| b.last_advert.cmp(&a.last_advert));
                        node.reading = Some(keep(&node, state));
                        node.inspected = true;
                    }
                    Err(e) => node.note = format!("{e:#}"),
                }
            }
            return node;
        }
        // Bluetooth carries only the companion protocol (no text console there)
        Ok(None) if port.is_ble() => {
            node.note = t("node-ble-silent");
            return node;
        }
        Ok(None) => {}
        // nothing on the other end reads the port: the console would not either
        Err(e) if not_listening(&e) => {
            node.note = silent(port);
            return node;
        }
        Err(e) => {
            node.note = format!("{e:#}");
            return node;
        }
    }

    // then the text console
    let mut console = cli::Cli::new(companion.into_link());
    match console.hello() {
        Ok(Some(hello)) => {
            node.kind = hello.kind;
            node.name = hello.name;
            node.model = hello.board;
            node.firmware = hello.version;
            node.public_key = hello.public_key;
            node.radio = hello.radio;
            if deep {
                let kind = node.kind;
                let everything = |console: &mut cli::Cli| -> anyhow::Result<_> {
                    Ok((console.settings(kind)?, console.regions()?, console.acl()?))
                };
                match everything(&mut console) {
                    Ok((more, regions, acl)) => {
                        node.facts.extend(more);
                        node.regions = regions;
                        node.acl = acl.map(|text| acl::parse(&text));
                        node.inspected = true;
                        // the whole state as a backup keeps it, kept with the node (a manual
                        // backup writes it at once); failed: a backup reads the node itself
                        match crate::backup::read::console(&mut console, kind, true, &mut |_, _, _| {}) {
                            Ok(state) => node.reading = Some(keep(&node, state)),
                            Err(e) => crate::debug_log!("WARN", "inspect: the backup part of the reading failed: {e:#}"),
                        }
                    }
                    Err(e) => node.note = format!("{e:#}"),
                }
            }
        }
        Ok(None) => node.note = silent(port),
        Err(e) if not_listening(&e) => node.note = silent(port),
        Err(e) => node.note = format!("{e:#}"),
    }
    node
}

/// The reading kept with the node: its state, who it was (as it said), when
fn keep(node: &Node, state: crate::backup::State) -> std::sync::Arc<crate::backup::Reading> {
    let who = crate::backup::capture::Who {
        name: node.name.clone(),
        public_key: node.public_key.clone(),
        model: node.model.clone(),
        firmware: node.firmware.clone(),
    };
    std::sync::Arc::new(crate::backup::Reading { state, who, at: chrono::Local::now() })
}

/// Why nothing answered. A known LoRa board that answers neither protocol runs a firmware that
/// does not use USB: a Bluetooth or Wi-Fi companion build (USB then only flashes it, through the
/// bootloader, and carries its debug text).
fn silent(port: &Port) -> String {
    t(if port.board().is_some() { "node-silent-wireless" } else { "node-silent" })
}

/// The port takes no data: a write timed out because the firmware never reads USB (Windows says
/// "semaphore timeout", os error 121). Not a lost connection.
fn not_listening(e: &anyhow::Error) -> bool {
    e.chain().any(|cause| {
        cause.downcast_ref::<std::io::Error>().is_some_and(|io| io.kind() == std::io::ErrorKind::TimedOut || io.raw_os_error() == Some(121))
    })
}
