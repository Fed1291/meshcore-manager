//! What differs between a wanted state (a backup) and a node's state now, as the node's own
//! commands (`node::edit::Step`). Pure: no I/O. A restore uses it to plan, then again after each
//! round of writes: nothing left = done and verified (restore.rs).
//!
//! The firmware decides the commands: a companion's contact, auto-add config and default flood
//! scope go back as the very bytes it sent (only the code differs, `proto::as_command`); the rest
//! with the same commands config uses. Firmware: examples/companion_radio/MyMesh.cpp,
//! src/helpers/CommonCLI.cpp.
//!
//! Compared on what the node keeps, not on what the mesh changes by itself: a contact by its key,
//! type, flags and name (its route and last advert follow the traffic), a setting by its value.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::{read, State};
use crate::help;
use crate::node::acl;
use crate::i18n::{t, tf};
use crate::node::cli::{self, ADMIN_PASSWORD};
use crate::node::edit::{self, Change, RegionEdits, Step, OWN};
use crate::node::Kind;
use crate::proto::{self as p, Channel, Contact, DeviceInfo, SelfInfo};
use crate::regions::{self, NodeRegions};

/// Which parts of a backup go onto the target
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Mode {
    /// The board the backup came from: everything, identity included
    Same,
    /// Another board becomes this node: everything, private key included (the old board must
    /// then stay off the air)
    Move,
    /// Another board gets the settings, channels and contacts, and keeps its own identity
    /// (private key, name, position)
    Copy,
}

#[derive(Default)]
pub struct Diff {
    /// In the order they go
    pub steps: Vec<Step>,
    /// One line per change, contacts and channels counted, for the question before writing
    pub summary: Vec<String>,
    /// What the backup holds that this node cannot take (another firmware, less room)
    pub cannot: Vec<String>,
    /// A console setting that is used only after a restart (radio, key) changes
    pub reboot: bool,
    /// Said in the question, not done: what the restore leaves as it is on purpose (the
    /// contacts a companion gained since the backup)
    pub notes: Vec<String>,
}

impl Diff {
    fn step(&mut self, line: String, danger: bool, change: Change) {
        self.summary.push(line.clone());
        self.steps.push(Step { line, danger, change });
    }

    /// A step counted in a summary line of its own (contacts, channels)
    fn quiet(&mut self, line: String, change: Change) {
        self.steps.push(Step { line, danger: false, change });
    }
}

pub fn diff(kind: Kind, want: &State, now: &State, mode: Mode) -> Diff {
    let mut d = Diff::default();
    if kind == Kind::Companion {
        companion(want, now, mode, &mut d);
    } else {
        console(kind, want, now, mode, &mut d);
    }
    d
}

fn changed(what: &str, old: &str, new: &str) -> String {
    tf("edit-line", &[("what", what.into()), ("old", old.into()), ("new", new.into())])
}

fn on_off(on: bool) -> String {
    (if on { "on" } else { "off" }).to_string()
}

/// "label: now → backup" when the two differ
fn each(lines: &mut Vec<String>, label: &str, now: String, want: String) {
    if now != want {
        lines.push(changed(&t(label), &now, &want));
    }
}

/// What a tuning / auto-add / flood-scope frame changes, setting by setting (the frames as the
/// companion sends them: MyMesh.cpp RESP_TUNING_PARAMS, RESP_AUTOADD_CONFIG,
/// RESP_DEFAULT_FLOOD_SCOPE)
fn frame_change(id: &str, now: &[u8], want: &[u8]) -> String {
    let mut lines = Vec::new();
    match id {
        "tuning" => {
            let thousandths = |f: &[u8], at: usize| p::u32_at(f, at).map_or(0.0, |v| v as f64 / 1000.0);
            each(&mut lines, "set-rx-delay", thousandths(now, 1).to_string(), thousandths(want, 1).to_string());
            // the airtime factor as the duty cycle edit mode shows (% = 100 / (af + 1))
            let duty = |f: &[u8]| format!("{}", (100.0 / (thousandths(f, 5) + 1.0) * 10.0).round() / 10.0);
            each(&mut lines, "set-duty-cycle", duty(now), duty(want));
        }
        "autoadd" => {
            let bit = |f: &[u8], b: u8| on_off(f.get(1).is_some_and(|c| c & b != 0));
            for (b, label) in [
                (0x02, "set-autoadd-chat"),
                (0x04, "set-autoadd-repeater"),
                (0x08, "set-autoadd-room"),
                (0x10, "set-autoadd-sensor"),
                (0x01, "set-autoadd-overwrite"),
            ] {
                each(&mut lines, label, bit(now, b), bit(want, b));
            }
            let hops = |f: &[u8]| f.get(2).map_or_else(|| "-".to_string(), |h| h.to_string());
            each(&mut lines, "set-autoadd-hops", hops(now), hops(want));
        }
        _ => {
            // the scope's name (text up to the first NUL)
            let name = |f: &[u8]| {
                // 31 bytes of name, then its key (MyMesh.cpp CMD_GET_DEFAULT_FLOOD_SCOPE)
                let text = f.get(1..).unwrap_or_default();
                let text = &text[..text.len().min(31)];
                let end = text.iter().position(|b| *b == 0).unwrap_or(text.len());
                let name = String::from_utf8_lossy(&text[..end]).into_owned();
                if name.is_empty() { "-".to_string() } else { name }
            };
            each(&mut lines, "set-flood-scope", name(now), name(want));
        }
    }
    if lines.is_empty() {
        // differs in bytes this program does not name (a newer firmware): said as a whole
        t(match id {
            "tuning" => "section-tuning",
            "autoadd" => "section-autoadd",
            _ => "section-flood-scope",
        })
    } else {
        lines.join("; ")
    }
}

fn replaced(what: &str) -> String {
    tf("edit-secret-line", &[("what", what.into())])
}

fn companion(want: &State, now: &State, mode: Mode, d: &mut Diff) {
    let identity = mode != Mode::Copy;
    let w_me = want.frame("self").and_then(SelfInfo::parse);
    let n_me = now.frame("self").and_then(SelfInfo::parse);
    let w_dev = want.frame("device").and_then(DeviceInfo::parse);
    let n_dev = now.frame("device").and_then(DeviceInfo::parse);

    // the identity first: the key (checked by the public key it gives, so a firmware that does
    // not let its key out can still be checked), then the name and position
    if let (Some(w), Some(n)) = (&w_me, &n_me) {
        if identity && !w.public_key.eq_ignore_ascii_case(&n.public_key) {
            match want.frame("private-key").and_then(|f| f.get(1..65)).and_then(|k| <[u8; 64]>::try_from(k).ok()) {
                Some(key) => d.step(replaced(&t("fact-private-key")), true, Change::Frame(p::import_private_key(&key))),
                None => d.cannot.push(t("restore-no-key")),
            }
        }
        if identity && w.name != n.name {
            d.step(changed(&t("set-name"), &n.name, &w.name), false, Change::Frame(p::set_advert_name(&w.name)));
        }
        // the node keeps micro-degrees: 1 off after the round trip is the same place
        let micro = |x: f64| (x * 1_000_000.0).round() as i64;
        if identity && ((micro(w.lat) - micro(n.lat)).abs() > 1 || (micro(w.lon) - micro(n.lon)).abs() > 1) {
            let line = changed(&t("set-position"), &format!("{:.6}, {:.6}", n.lat, n.lon), &format!("{:.6}, {:.6}", w.lat, w.lon));
            d.step(line, false, Change::Frame(p::set_advert_latlon(w.lat, w.lon)));
        }
        if (w.manual_add_contacts, w.telemetry, w.advert_loc_policy, w.multi_acks)
            != (n.manual_add_contacts, n.telemetry, n.advert_loc_policy, n.multi_acks)
        {
            // each setting that differs, by its name: "add every node heard: on → off" (the
            // firmware's manual bit, bit 0, off)
            let mut lines = Vec::new();
            each(&mut lines, "set-manual-add", on_off((n.manual_add_contacts & 1) == 0), on_off((w.manual_add_contacts & 1) == 0));
            each(&mut lines, "set-advert-loc", on_off(n.advert_loc_policy != 0), on_off(w.advert_loc_policy != 0));
            each(&mut lines, "set-multi-acks", n.multi_acks.to_string(), w.multi_acks.to_string());
            for (shift, label) in [(0, "set-telemetry-base"), (2, "set-telemetry-loc"), (4, "set-telemetry-env")] {
                each(&mut lines, label, ((n.telemetry >> shift) & 3).to_string(), ((w.telemetry >> shift) & 3).to_string());
            }
            let frame = p::set_other_params(w.manual_add_contacts, w.telemetry, w.advert_loc_policy, w.multi_acks);
            d.step(lines.join("; "), false, Change::Frame(frame));
        }
    } else if w_me.is_some() {
        d.cannot.push("self".into());
    }

    if let (Some(w), Some(n)) = (&w_dev, &n_dev) {
        match (w.path_hash_mode, n.path_hash_mode) {
            (Some(wm), Some(nm)) if wm != nm => {
                let line = changed(&t("set-path-hash-mode"), &nm.to_string(), &wm.to_string());
                d.step(line, false, Change::Frame(p::set_path_hash_mode(wm)));
            }
            (Some(_), None) => d.cannot.push(edit::PATH_HASH.into()),
            _ => {}
        }
        // the Bluetooth PIN belongs to the board's identity (who may connect to it)
        if let (true, Some(wp), Some(np)) = (identity, w.ble_pin, n.ble_pin) {
            if wp != np {
                d.step(replaced(&t("fact-ble-pin")), false, Change::Frame(p::set_device_pin(wp)));
            }
        }
    }

    // the same bytes back with the SET code (tuning, auto-add, default flood scope); the line
    // says what changes, setting by setting, as edit mode shows them
    for (id, code) in [
        ("tuning", p::CMD_SET_TUNING_PARAMS),
        ("autoadd", p::CMD_SET_AUTOADD_CONFIG),
        ("flood-scope", p::CMD_SET_DEFAULT_FLOOD_SCOPE),
    ] {
        match (want.frame(id), now.frame(id)) {
            (Some(w), Some(n)) if !same_frame(id, w, n) => d.step(frame_change(id, n, w), false, Change::Frame(p::as_command(code, w))),
            (Some(_), None) => d.cannot.push(id.into()),
            _ => {}
        }
    }
    custom_vars(want, now, d);
    channels(want, now, n_dev.as_ref(), d);
    contacts(want, now, n_dev.as_ref(), d);

    // the radio last: the link over USB does not depend on it, the mesh does
    if let (Some(w), Some(n)) = (&w_me, &n_me) {
        let khz = |mhz: f64| (mhz * 1000.0).round() as u32;
        // repeat is compared only when both the backup and the node say it (an older firmware
        // does not: then it is left as the node has it, never a change that cannot converge)
        let now_repeat = n_dev.as_ref().and_then(|x| x.repeat);
        let repeat = w_dev.as_ref().and_then(|x| x.repeat).or(now_repeat);
        let repeat_differs = matches!((w_dev.as_ref().and_then(|x| x.repeat), now_repeat), (Some(a), Some(b)) if a != b);
        if (khz(w.freq_mhz), khz(w.bandwidth_khz), w.spreading_factor, w.coding_rate)
            != (khz(n.freq_mhz), khz(n.bandwidth_khz), n.spreading_factor, n.coding_rate)
            || repeat_differs
        {
            let frame = p::set_radio_params(khz(w.freq_mhz), khz(w.bandwidth_khz), w.spreading_factor, w.coding_rate, repeat);
            d.step(changed(&t("set-radio"), &n.radio(), &w.radio()), true, Change::Frame(frame));
        }
        if w.tx_power_dbm != n.tx_power_dbm {
            let line = changed(&t("set-tx"), &n.tx_power_dbm.to_string(), &w.tx_power_dbm.to_string());
            d.step(line, true, Change::Frame(p::set_radio_tx_power(w.tx_power_dbm)));
        }
    }
}

/// A setting read back the same as in the backup. The flood scope is its name (31 bytes, up to
/// its first NUL: what follows the NUL is not kept by the node) and its 16-byte key; the others
/// byte for byte.
fn same_frame(id: &str, want: &[u8], now: &[u8]) -> bool {
    if id != "flood-scope" {
        return want.get(1..) == now.get(1..);
    }
    let name = |f: &[u8]| {
        let bytes = f.get(1..32).unwrap_or_default();
        bytes[..bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len())].to_vec()
    };
    let key = |f: &[u8]| f.get(32..48).map(|k| k.to_vec());
    name(want) == name(now) && key(want) == key(now)
}

/// "gps:1,gps_interval:900" -> name -> value
fn vars(frame: Option<&[u8]>) -> BTreeMap<String, String> {
    let text = frame.and_then(|f| f.get(1..)).map(|b| String::from_utf8_lossy(b).into_owned()).unwrap_or_default();
    text.split(',')
        .filter_map(|pair| pair.split_once(':'))
        .map(|(name, value)| (name.trim().to_string(), value.trim().to_string()))
        .collect()
}

fn custom_vars(want: &State, now: &State, d: &mut Diff) {
    let now_vars = vars(now.frame("custom-vars"));
    for (name, value) in vars(want.frame("custom-vars")) {
        match now_vars.get(&name) {
            Some(was) if *was != value => {
                let pair = format!("{name}:{value}");
                d.step(changed(&name, was, &value), false, Change::Frame(p::set_custom_var(&pair)));
            }
            Some(_) => {}
            // a sensor or setting this board does not have
            None => d.cannot.push(name),
        }
    }
}

fn channels(want: &State, now: &State, n_dev: Option<&DeviceInfo>, d: &mut Diff) {
    let now_ch: BTreeMap<u8, Channel> = now.frames("channels").iter().filter_map(|f| Channel::parse(f)).map(|c| (c.slot, c)).collect();
    let slots = n_dev.map_or(now_ch.len(), |x| x.max_channels as usize);
    let mut count = 0;
    for w in want.frames("channels").iter().filter_map(|f| Channel::parse(f)) {
        let same = now_ch.get(&w.slot).is_some_and(|n| n.name == w.name && (w.name.is_empty() || n.secret == w.secret));
        if same {
            continue;
        }
        if w.slot as usize >= slots {
            if !w.name.is_empty() {
                d.cannot.push(tf("restore-no-slot", &[("name", w.name.as_str().into())]));
            }
            continue;
        }
        let set = if w.name.is_empty() { None } else { Some((w.name.as_str(), &w.secret)) };
        d.quiet(tf("restore-channel", &[("slot", w.slot.into())]), Change::Frame(p::set_channel(w.slot, set)));
        count += 1;
    }
    if count > 0 {
        d.summary.push(tf("restore-channels", &[("count", count.into())]));
    }
}

/// What of a contact the user decides: key, type, favourite. Not its name (the contact's next
/// advert writes its own name over it, BaseChatMesh.cpp), nor its route or last advert (the
/// mesh changes them by itself): writing those back would only undo fresher news.
fn contact_key(c: &Contact) -> (String, u8, bool) {
    (c.public_key.clone(), c.kind, c.favourite)
}

/// A companion's contacts: the backup's ones that are missing go back, a favourite or type
/// changed since goes back. Contacts the companion gained since the backup are kept: a live
/// companion adds the nodes it hears by itself (auto-add), so removing them would only make it
/// learn them again (and lose what it learnt of them).
fn contacts(want: &State, now: &State, n_dev: Option<&DeviceInfo>, d: &mut Diff) {
    let parse = |frames: &[Vec<u8>]| -> Vec<(Contact, Vec<u8>)> {
        frames.iter().filter_map(|f| Contact::parse(f).map(|c| (c, f.clone()))).collect()
    };
    let want_c = parse(want.frames("contacts"));
    let now_c = parse(now.frames("contacts"));
    let held = |key: &str| now_c.iter().any(|(n, _)| n.public_key == key);
    let gained = now_c.iter().filter(|(n, _)| !want_c.iter().any(|(w, _)| w.public_key == n.public_key)).count();
    // free places on the node: a missing contact goes back only into one (never pushing out
    // another); with the list full, the companion replaced it (auto-add) and it stays out
    let size = n_dev.map_or(usize::MAX, |x| x.max_contacts as usize);
    let mut free = size.saturating_sub(now_c.len());
    let (mut back, mut changed_names, mut left_out) = (Vec::new(), Vec::new(), Vec::new());
    for (w, frame) in &want_c {
        if !held(&w.public_key) {
            if free == 0 {
                left_out.push(w.name.clone());
                continue;
            }
            free -= 1;
            back.push(w.name.clone());
        } else if now_c.iter().any(|(n, _)| contact_key(n) == contact_key(w)) {
            continue;
        } else {
            // held, but its favourite or type changed since the backup
            changed_names.push(w.name.clone());
        }
        let line = tf("restore-contact", &[("name", w.name.as_str().into())]);
        d.quiet(line, Change::Frame(p::as_command(p::CMD_ADD_UPDATE_CONTACT, frame)));
    }
    if !back.is_empty() {
        d.summary.push(tf("restore-contacts-back", &[("count", back.len().into()), ("names", names(&back).into())]));
    }
    if !changed_names.is_empty() {
        d.summary.push(tf("restore-contacts-fav", &[("count", changed_names.len().into()), ("names", names(&changed_names).into())]));
    }
    // said, never steps: the restore leaves them as they are
    if !left_out.is_empty() {
        d.notes.push(tf("restore-contacts-full", &[("count", left_out.len().into()), ("names", names(&left_out).into())]));
    }
    if gained > 0 {
        d.notes.push(tf("restore-contacts-kept", &[("count", gained.into())]));
    }
}

/// "Alice, Bob, Carol and 4 more": a few names, the rest counted
fn names(list: &[String]) -> String {
    const SHOWN: usize = 3;
    let mut text = list.iter().take(SHOWN).map(|n| crate::widgets::clean(n)).collect::<Vec<_>>().join(", ");
    if list.len() > SHOWN {
        text = tf("restore-and-more", &[("names", text.into()), ("count", (list.len() - SHOWN).into())]);
    }
    text
}

fn console(kind: Kind, want: &State, now: &State, mode: Mode, d: &mut Diff) {
    let settable = cli::settable(kind);
    let mut secret_steps = Vec::new();
    for (section, secret) in [("settings", false), ("secrets", true)] {
        for reply in want.replies(section) {
            let Some(name) = reply.command.strip_prefix("get ") else { continue };
            let Some(s) = settable.iter().find(|s| s.name == name) else { continue };
            if cli::not_supported(&reply.reply) || name == ADMIN_PASSWORD {
                continue;
            }
            // a copy keeps the target's own identity
            if mode == Mode::Copy && OWN.contains(&name) {
                continue;
            }
            let was = now.reply(section, &reply.command).filter(|r| !cli::not_supported(r));
            let Some(was) = was else {
                d.cannot.push(name.to_string());
                continue;
            };
            let differs = if secret { was != reply.reply } else { was.trim() != reply.reply.trim() };
            if !differs {
                continue;
            }
            // as `set` takes it ("10.0%" as read -> "10.0")
            let value = help::find(name, false).map_or_else(|| reply.reply.trim().to_string(), |r| r.editable(&reply.reply));
            let line = if secret { replaced(name) } else { changed(name, was, &reply.reply) };
            let step = Step { line, danger: s.danger, change: Change::Set { name: name.to_string(), value, secret } };
            d.reboot |= s.danger;
            if secret {
                secret_steps.push(step);
            } else {
                d.summary.push(step.line.clone());
                d.steps.push(step);
            }
        }
    }
    regions_diff(want, now, d);
    acl_diff(want, now, mode, d);
    // the key last (it takes effect at the restart)
    for step in secret_steps {
        d.summary.push(step.line.clone());
        d.steps.push(step);
    }
}

/// The access list kept in a state (None: not in it: an older backup, a firmware without it)
fn node_acl(state: &State) -> Option<Vec<acl::Entry>> {
    state.lines(read::ACL).as_deref().map(acl::parse)
}

/// The `setperm` commands that make the node's access list the backup's. On the board itself
/// (same / move) the list becomes exactly the backup's (an entry it does not have is removed:
/// permissions 0); a copy only adds and changes. One line per entry ("4A1F09C2…: admin").
/// A list over the node's room is refused: the node would drop entries by itself.
fn acl_diff(want: &State, now: &State, mode: Mode, d: &mut Diff) {
    let Some(w) = node_acl(want) else { return };
    let Some(n) = node_acl(now) else {
        d.cannot.push(t("acl-title"));
        return;
    };
    let exact = mode != Mode::Copy;
    let kept = if exact { w.len() } else { n.len() + w.iter().filter(|e| !n.iter().any(|x| x.key == e.key)).count() };
    if kept > acl::MAX {
        d.cannot.push(tf("acl-full", &[("count", kept.into()), ("max", acl::MAX.into())]));
        return;
    }
    if exact {
        for e in n.iter().filter(|e| !w.iter().any(|x| x.key == e.key)) {
            let line = tf("acl-remove-line", &[("key", acl::short(&e.key).into())]);
            d.step(line, false, Change::Console(acl::command(&e.key, 0)));
        }
    }
    for e in &w {
        if n.iter().any(|x| x.key == e.key && x.perms == e.perms) {
            continue;
        }
        let line = tf("acl-set-line", &[("key", acl::short(&e.key).into()), ("role", acl::role_name(e.perms).into())]);
        d.step(line, false, Change::Console(acl::command(&e.key, e.perms)));
    }
}

fn node_regions(state: &State) -> Option<NodeRegions> {
    let tree = state.reply("regions", "region")?;
    Some(regions::from_node(tree, state.reply("regions", "region home"), state.reply("regions", "region default")))
}

/// The region commands that make the node's regions the backup's: added, removed, flood, home,
/// default scope (node::edit::region_steps, the same as config)
fn regions_diff(want: &State, now: &State, d: &mut Diff) {
    let Some(w) = node_regions(want) else { return };
    let Some(n) = node_regions(now) else {
        d.cannot.push(t("node-regions"));
        return;
    };
    let mut edits = RegionEdits::default();
    for r in w.list.iter().filter(|r| !n.list.iter().any(|x| x.name == r.name)) {
        edits.add.push(r.clone());
    }
    // a list cut by the 160-character reply is not complete: nothing is removed on its word
    if w.cut || n.cut {
        d.cannot.push(t("restore-regions-cut"));
    } else {
        edits.remove = n.list.iter().filter(|r| !w.list.iter().any(|x| x.name == r.name)).map(|r| r.name.clone()).collect();
    }
    for r in &w.list {
        if let Some(x) = n.list.iter().find(|x| x.name == r.name) {
            if x.flood != r.flood {
                edits.flood.insert(r.name.clone(), r.flood);
            }
        }
    }
    if w.root_flood != n.root_flood {
        edits.flood.insert(regions::ROOT.to_string(), w.root_flood);
    }
    if w.home.is_some() && w.home != n.home {
        edits.home = w.home.clone();
    }
    if w.default != n.default {
        edits.default = Some(w.default.clone());
    }
    let start = d.steps.len();
    edit::region_steps(&n, &edits, &mut d.steps);
    let lines: Vec<String> = d.steps[start..].iter().map(|s| s.line.clone()).collect();
    d.summary.extend(lines);
}
