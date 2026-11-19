//! Changing a node, the way its firmware does it:
//! - what can change: the node's facts that have `edit` (its settings), a companion's contacts
//!   and channels;
//! - what the user prepares in edit mode: a `Draft`. Nothing is sent while editing;
//! - `plan` turns a draft into the node's own commands, checked against the firmware's limits,
//!   each with the line that shows it before the commit;
//! - `apply` (worker thread) sends them, each once. A command that got no answer is never sent
//!   again, and nothing after it is sent: the node's state is then unknown. The node is read again
//!   afterwards, so the view shows what it really holds.
//!
//! Firmware: companion `examples/companion_radio/MyMesh.cpp` (CMD_SET_*), console
//! `src/helpers/CommonCLI.cpp` (`set <name> <value>`, listed in data/cli_commands.toml).

use std::collections::BTreeMap;
use std::str::FromStr;
use std::time::Duration;

use anyhow::Result;

use super::acl;
use super::cli::Cli;
use super::companion::Companion;
use super::{Answer, Fact, Kind, Node};
use crate::help;
use crate::i18n::{t, tf};
use crate::proto::{self as p, SelfInfo};
use crate::regions::{self, NodeRegions, Region};
use crate::serial::Link;

// The ids of a companion's settings (a console node's are its own setting names)
/// The node's name (the same id on both kinds)
pub const NAME: &str = "name";
const LAT: &str = "lat";
const LON: &str = "lon";
pub const FREQ: &str = "freq";
pub const BW: &str = "bw";
pub const SF: &str = "sf";
pub const CR: &str = "cr";
/// A console node's radio: one setting, "freq,bw,sf,cr"
pub const CONSOLE_RADIO: &str = "radio";
/// The same id on both kinds of node
const TX: &str = "tx";
const MANUAL_ADD: &str = "manual.add";
const ADVERT_LOC: &str = "advert.loc";
const MULTI_ACKS: &str = "multi.acks";
const TELEMETRY_BASE: &str = "telemetry.base";
const TELEMETRY_LOC: &str = "telemetry.loc";
const TELEMETRY_ENV: &str = "telemetry.env";
const RX_DELAY: &str = "rxdelay";
/// A companion's airtime factor, shown and typed as a duty cycle in % like a repeater's
/// `dutycycle` (CommonRadioPrefs.cpp): % = 100 / (af + 1), af = 100 / % - 1
const DUTY: &str = "dutycycle";
pub const PATH_HASH: &str = "path.hash.mode";
/// A companion's Bluetooth pairing PIN (CMD_SET_DEVICE_PIN: 0 or 6 digits, used from its next start)
const BLE_PIN: &str = "ble.pin";
/// A companion's auto-add (CMD_SET_AUTOADD_CONFIG, firmware 1.12+): which types of node it adds
/// by itself when "add contacts by hand" is on, whether a new contact replaces the oldest one when
/// the list is full, and how far away a node may be to be added
const AUTOADD_CHAT: &str = "autoadd.chat";
const AUTOADD_REPEATER: &str = "autoadd.repeater";
const AUTOADD_ROOM: &str = "autoadd.room";
const AUTOADD_SENSOR: &str = "autoadd.sensor";
const AUTOADD_OVERWRITE: &str = "autoadd.overwrite";
const AUTOADD_HOPS: &str = "autoadd.hops";
/// Its bits, in the firmware's order (MyMesh.cpp AUTO_ADD_*)
const AUTOADD_BITS: [(&str, u8); 5] =
    [(AUTOADD_OVERWRITE, 0x01), (AUTOADD_CHAT, 0x02), (AUTOADD_REPEATER, 0x04), (AUTOADD_ROOM, 0x08), (AUTOADD_SENSOR, 0x10)];
/// The node's private key (64 bytes, hex): the console's own name for it, used for both kinds
pub const PRIVATE_KEY: &str = "prv.key";

/// What makes a node itself: never set on several nodes at once (group edit leaves them out).
/// Two nodes with one name, one key or one position make no sense.
pub const OWN: [&str; 4] = [NAME, PRIVATE_KEY, LAT, LON];

fn on_off(on: bool) -> String {
    (if on { "on" } else { "off" }).to_string()
}

/// A companion's settings from its RESP_SELF_INFO, as editable facts (what each one is and
/// accepts: src/help.rs)
pub fn companion_settings(me: &SelfInfo) -> Vec<Fact> {
    let max = format!("-9..{} dBm", me.max_tx_power_dbm);
    vec![
        Fact::setting(t("set-name"), me.name.clone(), NAME, "", false),
        Fact::setting(t("set-lat"), format!("{:.6}", me.lat), LAT, "-90..90", false),
        Fact::setting(t("set-lon"), format!("{:.6}", me.lon), LON, "-180..180", false),
        Fact::setting(t("set-freq"), format!("{:.3}", me.freq_mhz), FREQ, "150-2500 MHz", true),
        Fact::setting(t("set-bw"), format!("{}", me.bandwidth_khz), BW, "7-500 kHz", true),
        Fact::setting(t("set-sf"), me.spreading_factor.to_string(), SF, "5-12", true),
        Fact::setting(t("set-cr"), me.coding_rate.to_string(), CR, "5-8", true),
        Fact::setting(t("set-tx"), me.tx_power_dbm.to_string(), TX, &max, true),
        // shown the positive way: "on" = every node heard is added (the firmware's bit 0 is
        // "manual add", and only that bit counts: MyMesh::isAutoAddEnabled)
        Fact::setting(t("set-manual-add"), on_off((me.manual_add_contacts & 1) == 0), MANUAL_ADD, "on|off", false),
        Fact::setting(t("set-advert-loc"), on_off(me.advert_loc_policy != 0), ADVERT_LOC, "on|off", false),
        Fact::setting(t("set-multi-acks"), me.multi_acks.to_string(), MULTI_ACKS, "0|1", false),
        Fact::setting(t("set-telemetry-base"), (me.telemetry & 3).to_string(), TELEMETRY_BASE, "0-2", false),
        Fact::setting(t("set-telemetry-loc"), ((me.telemetry >> 2) & 3).to_string(), TELEMETRY_LOC, "0-2", false),
        Fact::setting(t("set-telemetry-env"), ((me.telemetry >> 4) & 3).to_string(), TELEMETRY_ENV, "0-2", false),
    ]
}

/// From RESP_DEVICE_INFO (firmware v10+)
pub fn path_hash_setting(mode: u8) -> Fact {
    Fact::setting(t("set-path-hash-mode"), mode.to_string(), PATH_HASH, "0-2", false)
}

/// From RESP_DEVICE_INFO: the Bluetooth PIN (0 = the firmware's choice: random on a companion
/// with a screen, else 123456). Hidden like a secret: it lets anyone nearby pair.
pub fn ble_pin_setting(pin: u32) -> Fact {
    Fact::setting(t("fact-ble-pin"), pin.to_string(), BLE_PIN, "0 | 100000-999999", false).secret()
}

/// From RESP_TUNING_PARAMS (read with the node's own view). The airtime factor as a duty cycle
/// in %, one decimal, as the repeater's `get dutycycle` prints it
pub fn tuning_settings(rx_delay: f64, airtime_factor: f64) -> Vec<Fact> {
    vec![
        Fact::setting(t("set-rx-delay"), format!("{rx_delay}"), RX_DELAY, "0-20", false),
        Fact::setting(t("set-duty-cycle"), duty_text(airtime_factor), DUTY, "1-100 %", false),
    ]
}

/// "50", "33.3", "10": the duty cycle of an airtime factor
fn duty_text(airtime_factor: f64) -> String {
    let duty = (100.0 / (airtime_factor.max(0.0) + 1.0) * 10.0).round() / 10.0;
    format!("{duty}")
}

/// From RESP_AUTOADD_CONFIG: the bits (`AUTOADD_BITS`) and the farthest node added (0 = any,
/// 1 = only heard directly, N = up to N-1 hops; the firmware keeps it at 64 at most)
pub fn autoadd_settings(config: u8, max_hops: u8) -> Vec<Fact> {
    let labels = [
        (AUTOADD_CHAT, "set-autoadd-chat"),
        (AUTOADD_REPEATER, "set-autoadd-repeater"),
        (AUTOADD_ROOM, "set-autoadd-room"),
        (AUTOADD_SENSOR, "set-autoadd-sensor"),
        (AUTOADD_OVERWRITE, "set-autoadd-overwrite"),
    ];
    let mut facts: Vec<Fact> = labels
        .iter()
        .map(|(id, label)| {
            let bit = AUTOADD_BITS.iter().find(|(b, _)| b == id).map_or(0, |(_, bit)| *bit);
            Fact::setting(t(label), on_off(config & bit != 0), id, "on|off", false)
        })
        .collect();
    facts.push(Fact::setting(t("set-autoadd-hops"), max_hops.to_string(), AUTOADD_HOPS, "0-64", false));
    facts
}

/// What the user changed in edit mode, not sent yet
#[derive(Default, Clone, PartialEq)]
pub struct Draft {
    /// Setting id -> the new value, as typed
    pub values: BTreeMap<String, String>,
    /// Contact public key -> what changes
    pub contacts: BTreeMap<String, ContactDraft>,
    /// Channel slot -> what changes
    pub channels: BTreeMap<u8, ChannelDraft>,
    /// What changes in the node's regions (also for several nodes at once: each node gets
    /// what it does not have yet)
    pub regions: RegionEdits,
    /// The access list: public key -> its new permissions (0: out of the list). A key the node
    /// does not have yet is an addition.
    pub acl: BTreeMap<String, u8>,
}

/// Changes to a node's regions, as intentions: each node turns them into its own commands
/// (`region_steps`), from what it has now
#[derive(Default, Clone, PartialEq)]
pub struct RegionEdits {
    /// To add (with flood allowed), each after the region above it
    pub add: Vec<Region>,
    /// To remove, with the regions under them
    pub remove: Vec<String>,
    /// Flood allowed or not
    pub flood: BTreeMap<String, bool>,
    pub home: Option<String>,
    /// Some(None): no default scope any more
    pub default: Option<Option<String>>,
}

impl RegionEdits {
    pub fn count(&self) -> usize {
        self.add.len() + self.remove.len() + self.flood.len() + self.home.is_some() as usize + self.default.is_some() as usize
    }
}

#[derive(Default, Clone, PartialEq)]
pub struct ContactDraft {
    pub name: Option<String>,
    pub favourite: Option<bool>,
    pub remove: bool,
}

#[derive(Default, Clone, PartialEq)]
pub struct ChannelDraft {
    pub name: Option<String>,
    pub remove: bool,
}

impl Draft {
    /// How many things change
    pub fn count(&self) -> usize {
        self.values.len()
            + self.contacts.values().filter(|c| **c != ContactDraft::default()).count()
            + self.channels.values().filter(|c| **c != ChannelDraft::default()).count()
            + self.regions.count()
            + self.acl.len()
    }
}

/// One command of a commit
#[derive(Clone)]
pub enum Change {
    /// A companion command frame
    Frame(Vec<u8>),
    /// A console `set <name> <value>` (`secret`: a password, its replies never passed on)
    Set { name: String, value: String, secret: bool },
    /// Any other console command ("region put geneva switzerland")
    Console(String),
}

/// One change, as it will be shown before the commit and sent after it
#[derive(Clone)]
pub struct Step {
    pub line: String,
    /// Restarts the node or changes how it transmits
    pub danger: bool,
    pub change: Change,
}

/// The region commands that make `edits` true on `node`, from what it has now (CommonCLI.cpp
/// `region ...`): removals first, deepest first (the firmware keeps a region that still has
/// some under it); then additions, each after the one above it (`region put` = flood allowed;
/// `denyf` after it when not); flood changes; home; default scope; and one `region save`, else
/// the changes are gone at the next restart (`region default` saves by itself).
pub fn region_steps(now: &NodeRegions, edits: &RegionEdits, steps: &mut Vec<Step>) {
    let has = |name: &str| now.list.iter().any(|r| r.name == name);
    let start = steps.len();
    let mut add = |line: String, command: String| steps.push(Step { line, danger: false, change: Change::Console(command) });
    let mut removed: Vec<String> = Vec::new();
    for name in edits.remove.iter().filter(|n| has(n)) {
        for each in regions::with_children(&now.list, name) {
            if !removed.contains(&each) {
                removed.push(each);
            }
        }
    }
    for name in &removed {
        add(tf("edit-region-remove", &[("name", name.as_str().into())]), format!("region remove {name}"));
    }
    for r in edits.add.iter().filter(|r| !has(&r.name)) {
        let command = match &r.parent {
            Some(parent) => format!("region put {} {parent}", r.name),
            None => format!("region put {}", r.name),
        };
        add(tf("edit-region-add", &[("name", r.name.as_str().into())]), command);
        if !r.flood {
            add(tf("edit-region-flood-off", &[("name", r.name.as_str().into())]), format!("region denyf {}", r.name));
        }
    }
    for (name, flood) in &edits.flood {
        // `*`, the root, has a flood flag too (packets with no region)
        let was = if name == regions::ROOT { Some(now.root_flood) } else { now.list.iter().find(|r| &r.name == name).map(|r| r.flood) };
        if was.is_some_and(|w| w != *flood) && !removed.contains(name) {
            let (id, verb) = if *flood { ("edit-region-flood-on", "allowf") } else { ("edit-region-flood-off", "denyf") };
            add(tf(id, &[("name", name.as_str().into())]), format!("region {verb} {name}"));
        }
    }
    if let Some(home) = edits.home.as_ref().filter(|h| now.home.as_ref() != Some(*h)) {
        // `*`: no home (the firmware's root, id 0)
        let line = if home == regions::ROOT {
            t("edit-region-no-home")
        } else {
            tf("edit-region-home", &[("name", home.as_str().into())])
        };
        add(line, format!("region home {home}"));
    }
    let changed_tree = steps.len() > start;
    if let Some(default) = edits.default.as_ref().filter(|d| **d != now.default) {
        let (line, name) = match default {
            Some(name) => (tf("edit-region-default", &[("name", name.as_str().into())]), name.clone()),
            None => (t("edit-region-no-default"), "<null>".to_string()),
        };
        steps.push(Step { line, danger: false, change: Change::Console(format!("region default {name}")) });
    }
    if changed_tree {
        steps.push(Step { line: t("edit-region-save"), danger: false, change: Change::Console("region save".into()) });
    }
}

/// The `setperm` commands of the access list's changes (node::acl): removals first (room for
/// the additions), then changes and additions. Refused when the list would hold more than the
/// node keeps (it would drop an entry by itself), or for a key that is not a whole public key.
fn acl_steps(node: &Node, changes: &BTreeMap<String, u8>, steps: &mut Vec<Step>) -> std::result::Result<(), String> {
    if changes.is_empty() {
        return Ok(());
    }
    let Some(now) = &node.acl else { return Err(t("acl-not-read")) };
    let has = |key: &str| now.iter().any(|e| e.key == key);
    if let Some(bad) = changes.keys().find(|k| !acl::valid_key(k)) {
        return Err(tf("acl-bad-key", &[("key", acl::short(bad).into())]));
    }
    let after = now.len() + changes.iter().filter(|(k, p)| **p != 0 && !has(k.as_str())).count()
        - changes.iter().filter(|(k, p)| **p == 0 && has(k.as_str())).count();
    if after > acl::MAX {
        return Err(tf("acl-full", &[("count", after.into()), ("max", acl::MAX.into())]));
    }
    let mut push = |line: String, key: &str, perms: u8| {
        steps.push(Step { line, danger: false, change: Change::Console(acl::command(key, perms)) });
    };
    for (key, _) in changes.iter().filter(|(k, p)| **p == 0 && has(k.as_str())) {
        push(tf("acl-remove-line", &[("key", acl::short(key).into())]), key, 0);
    }
    for (key, perms) in changes.iter().filter(|(_, p)| **p != 0) {
        push(tf("acl-set-line", &[("key", acl::short(key).into()), ("role", acl::role_name(*perms).into())]), key, *perms);
    }
    Ok(())
}

fn fact<'a>(node: &'a Node, id: &str) -> Option<&'a Fact> {
    node.facts.iter().find(|f| f.edit.as_ref().is_some_and(|e| e.id == id))
}

/// The commands that make `draft` true on `node`, in the order they go. An error is a value that
/// cannot be sent (the message says which and why); nothing is sent then.
pub fn plan(node: &Node, draft: &Draft) -> std::result::Result<Vec<Step>, String> {
    let mut steps = Vec::new();
    let changed = |ids: &[&str]| ids.iter().any(|id| draft.values.contains_key(*id));
    // the value it will have: as typed, else as the node has it
    let value = |id: &str| -> String {
        draft.values.get(id).cloned().or_else(|| fact(node, id).map(|f| f.value.clone())).unwrap_or_default()
    };
    let label = |id: &str| fact(node, id).map_or_else(|| id.to_string(), |f| f.label.clone());
    // "name: Old -> New; ..." for the ids that changed; a secret only "replaced", never shown
    let line = |ids: &[&str]| -> String {
        ids.iter()
            .filter_map(|id| {
                let new = draft.values.get(*id)?;
                let fact = fact(node, id);
                if fact.is_some_and(|f| f.secret) {
                    return Some(tf("edit-secret-line", &[("what", label(id).into())]));
                }
                let old = fact.map(|f| f.value.as_str()).unwrap_or("");
                Some(tf("edit-line", &[("what", label(id).into()), ("old", old.into()), ("new", new.as_str().into())]))
            })
            .collect::<Vec<_>>()
            .join("; ")
    };
    let push = |steps: &mut Vec<Step>, ids: &[&str], danger: bool, frame: Vec<u8>| {
        steps.push(Step { line: line(ids), danger, change: Change::Frame(frame) });
    };
    // every value against the firmware's rules first (locales/<code>/settings_help.toml)
    let companion = node.kind == Kind::Companion;
    for (id, new) in &draft.values {
        if let Some(rules) = help::find(id, companion) {
            rules.check(new)?;
        }
    }

    if companion {
        if changed(&[NAME]) {
            let name = value(NAME);
            check_name(&label(NAME), &name)?;
            push(&mut steps, &[NAME], false, p::set_advert_name(&name));
        }
        if changed(&[LAT, LON]) {
            let lat: f64 = number(&label(LAT), &value(LAT))?;
            let lon: f64 = number(&label(LON), &value(LON))?;
            within(&label(LAT), lat, -90.0, 90.0)?;
            within(&label(LON), lon, -180.0, 180.0)?;
            push(&mut steps, &[LAT, LON], false, p::set_advert_latlon(lat, lon));
        }
        if changed(&[FREQ, BW, SF, CR]) {
            let freq: f64 = number(&label(FREQ), &value(FREQ))?;
            let bw: f64 = number(&label(BW), &value(BW))?;
            let sf: u8 = number(&label(SF), &value(SF))?;
            let cr: u8 = number(&label(CR), &value(CR))?;
            within(&label(FREQ), freq, 150.0, 2500.0)?;
            within(&label(BW), bw, 7.0, 500.0)?;
            within(&label(SF), sf as f64, 5.0, 12.0)?;
            within(&label(CR), cr as f64, 5.0, 8.0)?;
            let frame = p::set_radio_params((freq * 1000.0).round() as u32, (bw * 1000.0).round() as u32, sf, cr, node.repeat);
            push(&mut steps, &[FREQ, BW, SF, CR], true, frame);
        }
        if changed(&[TX]) {
            let max = node.me.as_ref().map_or(22, |m| m.max_tx_power_dbm) as f64;
            let dbm: i8 = number(&label(TX), &value(TX))?;
            within(&label(TX), dbm as f64, -9.0, max)?;
            push(&mut steps, &[TX], true, p::set_radio_tx_power(dbm));
        }
        let others = [MANUAL_ADD, TELEMETRY_BASE, TELEMETRY_LOC, TELEMETRY_ENV, ADVERT_LOC, MULTI_ACKS];
        if changed(&others) {
            // the firmware takes the four together: the ones not changed go as they are
            // "add every node heard" on = the firmware's manual bit off; its other bits kept
            let every = switch(&label(MANUAL_ADD), &value(MANUAL_ADD))?;
            let kept = node.me.as_ref().map_or(0, |m| m.manual_add_contacts & !1);
            let manual = kept | (1 - every);
            let loc = switch(&label(ADVERT_LOC), &value(ADVERT_LOC))?;
            let acks: u8 = number(&label(MULTI_ACKS), &value(MULTI_ACKS))?;
            within(&label(MULTI_ACKS), acks as f64, 0.0, 1.0)?;
            let mut telemetry = 0u8;
            for (shift, id) in [(0, TELEMETRY_BASE), (2, TELEMETRY_LOC), (4, TELEMETRY_ENV)] {
                let mode: u8 = number(&label(id), &value(id))?;
                within(&label(id), mode as f64, 0.0, 2.0)?;
                telemetry |= mode << shift;
            }
            push(&mut steps, &others, false, p::set_other_params(manual, telemetry, loc, acks));
        }
        if changed(&[RX_DELAY, DUTY]) {
            let rx: f64 = number(&label(RX_DELAY), &value(RX_DELAY))?;
            let duty: f64 = number(&label(DUTY), value(DUTY).trim_end_matches('%').trim())?;
            within(&label(RX_DELAY), rx, 0.0, 20.0)?;
            within(&label(DUTY), duty, 1.0, 100.0)?;
            // what the companion takes: the airtime factor (as the repeater's set dutycycle does)
            let af = 100.0 / duty - 1.0;
            push(&mut steps, &[RX_DELAY, DUTY], false, p::set_tuning_params(rx, af));
        }
        // the auto-add bits and the hops go together: the ones not changed go as they are
        let autoadd = [AUTOADD_CHAT, AUTOADD_REPEATER, AUTOADD_ROOM, AUTOADD_SENSOR, AUTOADD_OVERWRITE, AUTOADD_HOPS];
        if changed(&autoadd) {
            let mut bits = 0u8;
            for (id, bit) in AUTOADD_BITS {
                if switch(&label(id), &value(id))? != 0 {
                    bits |= bit;
                }
            }
            let hops: u8 = number(&label(AUTOADD_HOPS), &value(AUTOADD_HOPS))?;
            within(&label(AUTOADD_HOPS), hops as f64, 0.0, 64.0)?;
            push(&mut steps, &autoadd, false, vec![p::CMD_SET_AUTOADD_CONFIG, bits, hops]);
        }
        if changed(&[PATH_HASH]) {
            let mode: u8 = number(&label(PATH_HASH), &value(PATH_HASH))?;
            within(&label(PATH_HASH), mode as f64, 0.0, 2.0)?;
            push(&mut steps, &[PATH_HASH], false, p::set_path_hash_mode(mode));
        }
        // checked by its rules above (0, or 100000-999999: MyMesh.cpp CMD_SET_DEVICE_PIN)
        if changed(&[BLE_PIN]) {
            let pin: u32 = number(&label(BLE_PIN), &value(BLE_PIN))?;
            push(&mut steps, &[BLE_PIN], false, p::set_device_pin(pin));
        }
        // a new identity: CMD_IMPORT_PRIVATE_KEY (64 bytes, checked as hex by its rules above;
        // the firmware also refuses a key that is not a valid one)
        if changed(&[PRIVATE_KEY]) {
            let key: Vec<u8> = (0..value(PRIVATE_KEY).len() / 2)
                .filter_map(|i| u8::from_str_radix(value(PRIVATE_KEY).get(i * 2..i * 2 + 2)?, 16).ok())
                .collect();
            let key: [u8; 64] = key.try_into().map_err(|_| tf("edit-bad-key", &[]))?;
            push(&mut steps, &[PRIVATE_KEY], true, p::import_private_key(&key));
        }
    } else {
        // a console node: one `set` per changed setting, in the order of its list
        for f in &node.facts {
            let Some(edit) = &f.edit else { continue };
            let Some(new) = draft.values.get(&edit.id) else { continue };
            // checked by its rules above; a setting without rules: plain text at least
            if help::find(&edit.id, false).is_none() {
                check_text(&f.label, new)?;
            }
            // a password: sent exactly as typed (spaces too); the line says only "replaced"
            let value = if f.secret { new.clone() } else { new.trim().to_string() };
            steps.push(Step {
                line: line(&[edit.id.as_str()]),
                danger: edit.danger,
                change: Change::Set { name: edit.id.clone(), value, secret: f.secret },
            });
        }
        if let Some(now) = &node.regions {
            region_steps(now, &draft.regions, &mut steps);
        }
        acl_steps(node, &draft.acl, &mut steps)?;
    }

    for (key, d) in &draft.contacts {
        let Some(c) = node.contacts.iter().find(|c| &c.public_key == key) else { continue };
        if d.remove {
            let line = tf("edit-contact-remove", &[("name", c.name.as_str().into())]);
            steps.push(Step { line, danger: false, change: Change::Frame(p::remove_contact(c)) });
            continue;
        }
        if d.name.is_none() && d.favourite.is_none() {
            continue;
        }
        let name = d.name.clone().unwrap_or_else(|| c.name.clone());
        check_name(&t("col-name"), &name)?;
        let favourite = d.favourite.unwrap_or(c.favourite);
        let mut parts = Vec::new();
        if d.name.is_some() {
            parts.push(tf("edit-line", &[("what", t("col-name").into()), ("old", c.name.as_str().into()), ("new", name.as_str().into())]));
        }
        if d.favourite.is_some() {
            parts.push(t(if favourite { "edit-favourite-on" } else { "edit-favourite-off" }));
        }
        let line = tf("edit-contact", &[("name", c.name.as_str().into()), ("what", parts.join("; ").into())]);
        steps.push(Step { line, danger: false, change: Change::Frame(p::update_contact(c, &name, favourite)) });
    }

    for (slot, d) in &draft.channels {
        let Some(c) = node.channels.iter().find(|c| c.slot == *slot) else { continue };
        let (line, frame) = if d.remove {
            (tf("edit-channel-remove", &[("slot", (*slot).into()), ("name", c.name.as_str().into())]), p::set_channel(*slot, None))
        } else if let Some(name) = &d.name {
            check_name(&t("col-name"), name)?;
            let line = tf(
                "edit-channel-rename",
                &[("slot", (*slot).into()), ("old", c.name.as_str().into()), ("new", name.as_str().into())],
            );
            (line, p::set_channel(*slot, Some((name, &c.secret))))
        } else {
            continue;
        };
        steps.push(Step { line, danger: false, change: Change::Frame(frame) });
    }
    Ok(steps)
}

fn number<T: FromStr>(label: &str, value: &str) -> std::result::Result<T, String> {
    value.trim().parse().map_err(|_| tf("edit-not-a-number", &[("what", label.into()), ("value", value.into())]))
}

fn within(label: &str, value: f64, low: f64, high: f64) -> std::result::Result<(), String> {
    if value.is_finite() && value >= low && value <= high {
        return Ok(());
    }
    Err(tf("edit-out-of-range", &[("what", label.into()), ("low", low.into()), ("high", high.into())]))
}

/// "on" / "off" (also 1 / 0, yes / no)
fn switch(label: &str, value: &str) -> std::result::Result<u8, String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "on" | "1" | "yes" => Ok(1),
        "off" | "0" | "no" => Ok(0),
        _ => Err(tf("edit-not-on-off", &[("what", label.into()), ("value", value.into())])),
    }
}

/// Not empty, no control characters
fn check_text(label: &str, value: &str) -> std::result::Result<(), String> {
    if value.trim().is_empty() || value.chars().any(char::is_control) {
        return Err(tf("edit-bad-text", &[("what", label.into())]));
    }
    Ok(())
}

/// A name as the node keeps it: up to 31 bytes
fn check_name(label: &str, value: &str) -> std::result::Result<(), String> {
    check_text(label, value)?;
    if value.len() > p::NAME_MAX {
        return Err(help::too_long(label, p::NAME_MAX));
    }
    Ok(())
}

/// Tries to open the node for a commit before giving up (the link may be slow to settle: a
/// Bluetooth companion just connected, a board that restarted when its port opened)
const OPEN_TRIES: usize = 3;
/// Between two tries
const OPEN_PAUSE: Duration = Duration::from_millis(1500);
/// A step whose answer was lost with the link: the link opened again and the step sent again,
/// this many times at most
const RESEND_TRIES: usize = 2;

/// Another node answers on the port: never retried, nothing is written
#[derive(Debug)]
struct OtherNode;

impl std::fmt::Display for OtherNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&t("edit-other-node"))
    }
}

impl std::error::Error for OtherNode {}

/// The node open for a commit, checked to be the one edited
enum Session {
    Companion(Companion),
    Console(Cli),
}

impl Session {
    /// Opens the port and checks who answers. The hello is asked twice on one link (the first
    /// one can be lost while the link settles), then the link is opened again, `OPEN_TRIES`
    /// times. Another node on the port ends it at once.
    fn open(node: &Node, wait: Duration) -> Result<Session> {
        let mut last = anyhow::anyhow!(t("backup-no-answer"));
        for attempt in 0..OPEN_TRIES {
            if attempt > 0 {
                std::thread::sleep(OPEN_PAUSE);
            }
            match Session::open_once(node, wait) {
                Ok(session) => return Ok(session),
                Err(e) if e.is::<OtherNode>() => return Err(e),
                Err(e) => {
                    crate::debug_log!("WARN", "commit: open try {} failed: {e:#}", attempt + 1);
                    last = e;
                }
            }
        }
        Err(last)
    }

    fn open_once(node: &Node, wait: Duration) -> Result<Session> {
        let link = Link::open(&node.port.path)?;
        if node.kind == Kind::Companion {
            let mut companion = Companion::new(link);
            let hello = match companion.hello(wait)? {
                Some(hello) => hello,
                None => companion.hello(wait)?.ok_or_else(|| anyhow::anyhow!(t("backup-no-answer")))?,
            };
            if hello.me.map(|m| m.public_key) != Some(node.public_key.clone()) {
                return Err(OtherNode.into());
            }
            Ok(Session::Companion(companion))
        } else {
            let mut console = Cli::new(link);
            let hello = match console.hello()? {
                Some(hello) => hello,
                None => console.hello()?.ok_or_else(|| anyhow::anyhow!(t("backup-no-answer")))?,
            };
            if hello.public_key != node.public_key {
                return Err(OtherNode.into());
            }
            Ok(Session::Console(console))
        }
    }

    fn send(&mut self, change: &Change) -> Result<Answer> {
        match (self, change) {
            (Session::Companion(companion), Change::Frame(frame)) => companion.command(frame),
            (Session::Console(console), Change::Set { name, value, secret }) => console.set(name, value, *secret),
            (Session::Console(console), Change::Console(command)) => console.command(command),
            _ => Ok(Answer::Refused(t("edit-wrong-kind"))),
        }
    }
}

/// Sends the steps of a commit to the node, in order. The node is opened surely first (tried
/// again while it does not answer; never written when another node is there: Err, nothing
/// sent). A step whose answer is lost with the link (an error, or silence) is not given up: the
/// link is opened again, the node checked again, and that step sent again (every step sets a
/// value, so a second time leaves the same result), `RESEND_TRIES` times. Only then the commit
/// stops there. Returns an answer per step sent; the node is read again afterwards (the worker),
/// which shows what it holds in the end.
pub fn apply(node: &Node, steps: &[Step], wait: Duration, progress: &mut dyn FnMut(&str, usize, usize)) -> Result<Vec<Answer>> {
    let mut session = Some(Session::open(node, wait)?);
    let mut answers = Vec::new();
    let total = steps.len();
    for (i, step) in steps.iter().enumerate() {
        progress(&step.line, i, total);
        let mut answer = match session.as_mut() {
            Some(s) => s.send(&step.change),
            None => Ok(Answer::Silent),
        };
        let mut resent = 0;
        while resent < RESEND_TRIES && matches!(answer, Err(_) | Ok(Answer::Silent)) {
            resent += 1;
            crate::debug_log!("WARN", "commit: step {} got no answer, opening the node again", i + 1);
            // the old link first: a Bluetooth companion takes one connection at a time
            drop(session.take());
            match Session::open(node, wait) {
                Ok(mut s) => {
                    answer = s.send(&step.change);
                    session = Some(s);
                }
                Err(e) => {
                    answer = Err(e);
                    break;
                }
            }
        }
        match answer {
            Ok(answer) => {
                let silent = matches!(answer, Answer::Silent);
                answers.push(answer);
                if silent {
                    break;
                }
            }
            // the link could not be had again: this step's fate is unknown, the rest not sent
            Err(e) => {
                crate::debug_log!("WARN", "commit: stopped at step {}: {e:#}", i + 1);
                answers.push(Answer::Silent);
                break;
            }
        }
    }
    Ok(answers)
}
