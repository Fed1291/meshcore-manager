//! What changed between two backups of one node, the older one first: settings, identity and
//! regions (the restore's own comparison, `diff.rs`, so both always agree on what "differs"),
//! then channels by slot and contacts by key, in words a person reads ("added", "renamed").
//! Secrets are never shown: a changed key or password is "replaced (not shown)". Pure: no I/O.

use super::diff::{self, Mode};
use super::{names, read, State};
use crate::i18n::{t, tf};
use crate::node::{acl, Kind};
use crate::proto::{Channel, Contact};

/// How a line reads (its colour and mark on screen)
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    /// A part's title (Settings, Channels, Contacts)
    Heading,
    Added,
    Removed,
    Changed,
    /// Said, not a change (a part only one backup has)
    Note,
}

pub struct Comparison {
    pub lines: Vec<(Tone, String)>,
    /// How many changes (0: the two hold the same)
    pub changes: usize,
}

/// A comparison with what was compared, for the screen
pub struct Report {
    /// The node, as the newer backup names it
    pub name: String,
    /// The two backups' folder names (date and time)
    pub older: String,
    pub newer: String,
    pub comparison: Comparison,
}

/// The parts compared one by one here, not by `diff` (which writes them as commands)
const OWN_PARTS: [&str; 4] = ["channels", "contacts", read::ACL, names::ID];

pub fn compare(kind: Kind, older: &State, newer: &State) -> Comparison {
    let mut out = Comparison { lines: Vec::new(), changes: 0 };

    // settings, identity, regions: what restoring the newer onto a node holding the older changes
    let without = |s: &State| State {
        parts: s.parts.iter().filter(|p| !OWN_PARTS.contains(&p.id.as_str())).cloned().collect(),
        missing: s.missing.clone(),
    };
    let d = diff::diff(kind, &without(newer), &without(older), Mode::Same);
    out.part("compare-settings", d.summary.into_iter().map(|l| (Tone::Changed, l)).collect());

    if kind == Kind::Companion {
        out.part("compare-channels", channels(older, newer));
        out.part("compare-contacts", contacts(older, newer));
    } else {
        out.part("compare-acl", access(older, newer));
    }
    // yours about its keys (custom names, roles): from the address book, kept in each backup
    out.part("compare-book", names::compare(older, newer).into_iter().map(|l| (Tone::Changed, l)).collect());

    // what only one of them holds (another firmware, a setting of the backup, a failed read).
    // Not the address book's part: a backup has it only when something of yours was there
    let only = |a: &State, b: &State| -> Vec<String> {
        a.parts.iter().filter(|p| p.id != names::ID && !b.parts.iter().any(|q| q.id == p.id)).map(|p| p.id.clone()).collect()
    };
    let mut notes = Vec::new();
    let (older_only, newer_only) = (only(older, newer), only(newer, older));
    if !older_only.is_empty() {
        notes.push((Tone::Note, tf("compare-only-older", &[("parts", older_only.join(", ").into())])));
    }
    if !newer_only.is_empty() {
        notes.push((Tone::Note, tf("compare-only-newer", &[("parts", newer_only.join(", ").into())])));
    }
    notes.extend(d.cannot.into_iter().map(|c| (Tone::Note, tf("compare-not-in-older", &[("what", c.into())]))));
    if !notes.is_empty() {
        out.lines.push((Tone::Heading, t("compare-notes")));
        out.lines.extend(notes);
    }
    out
}

impl Comparison {
    /// A part with its title, only when something changed in it
    fn part(&mut self, title: &str, lines: Vec<(Tone, String)>) {
        if lines.is_empty() {
            return;
        }
        self.changes += lines.len();
        self.lines.push((Tone::Heading, t(title)));
        self.lines.extend(lines);
    }
}

/// By slot: added, removed, renamed, key replaced
/// The access list by public key: added, removed, role changed. A backup without it (older, or
/// a firmware without `get acl`) is in the "only one holds" note, not here.
fn access(older: &State, newer: &State) -> Vec<(Tone, String)> {
    let list = |s: &State| s.lines(read::ACL).as_deref().map(acl::parse);
    let (Some(old), Some(new)) = (list(older), list(newer)) else { return Vec::new() };
    let mut lines = Vec::new();
    for e in &new {
        let key = acl::short(&e.key);
        match old.iter().find(|o| o.key == e.key) {
            None => lines.push((Tone::Added, tf("compare-acl-added", &[("key", key.into()), ("role", acl::role_name(e.perms).into())]))),
            Some(o) if o.perms != e.perms => lines.push((
                Tone::Changed,
                tf("compare-acl-changed", &[("key", key.into()), ("old", acl::role_name(o.perms).into()), ("new", acl::role_name(e.perms).into())]),
            )),
            Some(_) => {}
        }
    }
    for o in old.iter().filter(|o| !new.iter().any(|e| e.key == o.key)) {
        lines.push((Tone::Removed, tf("compare-acl-removed", &[("key", acl::short(&o.key).into()), ("role", acl::role_name(o.perms).into())])));
    }
    lines
}

fn channels(older: &State, newer: &State) -> Vec<(Tone, String)> {
    let parse = |s: &State| -> Vec<Channel> {
        s.frames("channels").iter().filter_map(|f| Channel::parse(f)).filter(|c| !c.name.is_empty()).collect()
    };
    let (old, new) = (parse(older), parse(newer));
    let mut slots: Vec<u8> = old.iter().chain(&new).map(|c| c.slot).collect();
    slots.sort_unstable();
    slots.dedup();
    let mut lines = Vec::new();
    for slot in slots {
        let was = old.iter().find(|c| c.slot == slot);
        let is = new.iter().find(|c| c.slot == slot);
        let line = match (was, is) {
            (None, Some(n)) => (Tone::Added, tf("compare-channel-added", &[("slot", slot.into()), ("name", n.name.as_str().into())])),
            (Some(o), None) => (Tone::Removed, tf("compare-channel-removed", &[("slot", slot.into()), ("name", o.name.as_str().into())])),
            (Some(o), Some(n)) if o.name != n.name => (
                Tone::Changed,
                tf("compare-channel-renamed", &[("slot", slot.into()), ("old", o.name.as_str().into()), ("new", n.name.as_str().into())]),
            ),
            // the key is a secret: only that it changed
            (Some(o), Some(n)) if o.secret != n.secret => {
                (Tone::Changed, tf("compare-channel-key", &[("slot", slot.into()), ("name", n.name.as_str().into())]))
            }
            _ => continue,
        };
        lines.push(line);
    }
    lines
}

/// By public key: added, removed, renamed, favourite or type changed (not the route or the last
/// advert: the mesh changes those by itself)
fn contacts(older: &State, newer: &State) -> Vec<(Tone, String)> {
    let parse = |s: &State| -> Vec<Contact> { s.frames("contacts").iter().filter_map(|f| Contact::parse(f)).collect() };
    let (old, new) = (parse(older), parse(newer));
    // "name (1a2b3c4d)": two contacts can share a name
    let who = |c: &Contact| format!("{} ({})", c.name, c.public_key.chars().take(8).collect::<String>());
    let mut lines = Vec::new();
    for n in &new {
        match old.iter().find(|o| o.public_key == n.public_key) {
            None => lines.push((Tone::Added, tf("compare-contact-added", &[("name", who(n).into())]))),
            Some(o) => {
                if o.name != n.name {
                    let line = tf("compare-contact-renamed", &[("old", who(o).into()), ("new", n.name.as_str().into())]);
                    lines.push((Tone::Changed, line));
                }
                if o.favourite != n.favourite {
                    let id = if n.favourite { "compare-contact-favourite" } else { "compare-contact-not-favourite" };
                    lines.push((Tone::Changed, tf(id, &[("name", who(n).into())])));
                }
                if o.kind != n.kind {
                    lines.push((Tone::Changed, tf("compare-contact-type", &[("name", who(n).into())])));
                }
            }
        }
    }
    for o in old.iter().filter(|o| !new.iter().any(|n| n.public_key == o.public_key)) {
        lines.push((Tone::Removed, tf("compare-contact-removed", &[("name", who(o).into())])));
    }
    lines
}
