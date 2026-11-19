//! Edit mode of a node's view: the changes being prepared (a `Draft`, src/node/edit.rs), the text
//! being typed, and the commit question. Nothing reaches the node before the commit is answered
//! yes: then the worker sends each change once and reads the node again.

use ratatui::crossterm::event::{KeyCode, KeyEvent};

use crate::app::{Action, Confirm, Popup};
use crate::help::{self, Help};
use crate::i18n::{t, tf};
use crate::node::edit::{self, plan, ChannelDraft, ContactDraft, Draft};
use crate::node::{acl, Kind, Node};
use crate::presets::{Preset, Radio};
use crate::proto as p;
use crate::regions::{self, Region};
use crate::widgets::{self, clean};
use crate::worker::Cmd;

/// What the text being typed goes to
#[derive(Clone, PartialEq)]
pub enum Target {
    /// A setting, by its id
    Setting(String),
    /// A contact's name, by its public key
    Contact(String),
    /// A channel's name, by its slot
    Channel(u8),
}

#[derive(Default)]
pub struct Editor {
    pub draft: Draft,
    /// Typing into this, with the text so far
    pub input: Option<(Target, String)>,
    /// The last value that could not be taken, and why (shown until the next key)
    pub problem: Option<String>,
    /// The values the setting being changed can take, when they are few: picked with the
    /// arrows instead of typed (typing a letter still switches to free text). Empty = typing.
    pub choices: Vec<String>,
    /// The rules of the setting being changed (what can be typed): None = any text
    pub rules: Option<&'static Help>,
    /// Editing several nodes at once (pages/group.rs): a value equal to the first node's is
    /// still a change (the others may differ)
    pub group: bool,
    /// The draft before each change, the latest last: `u` goes back one change at a time,
    /// whatever it was (a setting, a contact, a channel, a region, a preset)
    back: Vec<Draft>,
    /// What `u` took back, for `U` (cleared by a new change)
    forward: Vec<Draft>,
}

/// How many changes `u` can take back
const UNDO_DEPTH: usize = 200;

/// The values a setting can take, from its hint, when they are a short list:
/// "<off|minimal|moderate|strict>", "on|off", "<0|1>", or a whole-number range of up to 11
/// values ("0-2") when the value now is a whole number too (so "txdelay <0-2>" = 1.4 stays typed)
pub fn choices(hint: &str, current: &str) -> Vec<String> {
    let hint = hint.trim().trim_start_matches('<').trim_end_matches('>');
    if hint.contains('|') && !hint.contains([',', '<', ' ']) {
        return hint.split('|').map(str::to_string).collect();
    }
    let range = hint.split_once('-').and_then(|(a, b)| Some((a.parse::<i32>().ok()?, b.trim().parse::<i32>().ok()?)));
    match range {
        Some((low, high)) if high > low && high - low <= 10 && current.trim().parse::<i32>().is_ok() => {
            (low..=high).map(|n| n.to_string()).collect()
        }
        _ => Vec::new(),
    }
}

impl Editor {
    /// An editor for several nodes at once (pages/group.rs)
    pub fn for_group() -> Editor {
        Editor { group: true, ..Editor::default() }
    }

    /// After a key: when the draft is not what it was (`before`), that was a change, and `u` can
    /// take it back
    pub fn track(&mut self, before: Draft) {
        if before != self.draft {
            self.back.push(before);
            if self.back.len() > UNDO_DEPTH {
                self.back.remove(0);
            }
            self.forward.clear();
        }
    }

    /// While typing a value with a length limit: (used, at most), shown under the field so the
    /// limit is known before it is reached. Names: the node's NAME_MAX.
    pub fn room(&self) -> Option<(usize, usize)> {
        let (_, text) = self.input.as_ref()?;
        if !self.choices.is_empty() {
            return None;
        }
        match self.rules {
            Some(rules) => rules.room(text),
            None => Some((text.len(), p::NAME_MAX)),
        }
    }

    pub fn can_undo(&self) -> bool {
        !self.back.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.forward.is_empty()
    }

    /// `u`: the last change taken back
    pub fn undo_last(&mut self) {
        if let Some(before) = self.back.pop() {
            self.forward.push(std::mem::replace(&mut self.draft, before));
        }
    }

    /// `U`: the change `u` took back, made again
    pub fn redo_last(&mut self) {
        if let Some(after) = self.forward.pop() {
            self.back.push(std::mem::replace(&mut self.draft, after));
        }
    }

    /// Changing `target`, starting from what it will be after the commit: a picker when its
    /// values are a short list, else typing
    pub fn start(&mut self, target: Target, node: &Node) {
        let mut text = self.value(&target, node);
        self.rules = None;
        self.choices = Vec::new();
        if let Target::Setting(id) = &target {
            self.rules = help::find(id, node.kind == Kind::Companion);
            match self.rules {
                // the firmware's rules: its choices, the value as it is typed ("10.0%" -> "10.0")
                Some(rules) => {
                    text = rules.editable(&text);
                    self.choices = rules.choices();
                }
                // no rules known: guess the choices from the command list's hint
                None => {
                    let hint = node.facts.iter().find_map(|f| f.edit.as_ref().filter(|e| &e.id == id)).map(|e| e.hint.as_str());
                    self.choices = choices(hint.unwrap_or(""), &text);
                }
            }
        }
        self.input = Some((target, text));
    }

    /// What `target` will be after the commit: as changed here, else as the node has it
    fn value(&self, target: &Target, node: &Node) -> String {
        match target {
            Target::Setting(id) => self.draft.values.get(id).cloned().unwrap_or_else(|| current_setting(node, id)),
            Target::Contact(key) => {
                self.draft.contacts.get(key).and_then(|d| d.name.clone()).unwrap_or_else(|| current_contact(node, key))
            }
            Target::Channel(slot) => {
                self.draft.channels.get(slot).and_then(|d| d.name.clone()).unwrap_or_else(|| current_channel(node, *slot))
            }
        }
    }

    /// A key while changing a value: arrows (picker), letters, Backspace, Enter (take it), Esc
    /// (drop it)
    pub fn typing_key(&mut self, key: KeyEvent, node: &Node) {
        let Some((target, text)) = &mut self.input else { return };
        let picking = !self.choices.is_empty();
        match key.code {
            KeyCode::Esc => {
                self.input = None;
                self.choices.clear();
                self.rules = None;
                self.problem = None;
            }
            KeyCode::Enter => {
                let (target, text) = (target.clone(), text.clone());
                self.set(target, text, node);
            }
            KeyCode::Left | KeyCode::Up if picking => {
                let len = self.choices.len();
                let at = self.choices.iter().position(|c| c.eq_ignore_ascii_case(text)).map_or(0, |p| (p + len - 1) % len);
                *text = self.choices[at].clone();
            }
            KeyCode::Right | KeyCode::Down if picking => {
                let len = self.choices.len();
                let at = self.choices.iter().position(|c| c.eq_ignore_ascii_case(text)).map_or(0, |p| (p + 1) % len);
                *text = self.choices[at].clone();
            }
            // a setting that only takes its choices: nothing to erase
            KeyCode::Backspace if picking && self.rules.is_some_and(|r| !r.choices().is_empty()) => {}
            // any typing leaves the picker: free text from here
            KeyCode::Backspace => {
                self.choices.clear();
                text.pop();
            }
            // a setting that only takes its choices: a letter jumps to the next one starting with it
            KeyCode::Char(c) if picking && self.rules.is_some_and(|r| !r.choices().is_empty()) => {
                let len = self.choices.len();
                let at = self.choices.iter().position(|v| v.eq_ignore_ascii_case(text)).unwrap_or(len - 1);
                let starts = |v: &String| v.to_lowercase().starts_with(c.to_ascii_lowercase());
                match (1..=len).map(|step| (at + step) % len).find(|i| starts(&self.choices[*i])) {
                    Some(i) => *text = self.choices[i].clone(),
                    None => self.problem = self.rules.and_then(|r| r.refuse("", c)),
                }
            }
            // any character, AltGr ones too (Windows reports AltGr as Ctrl+Alt; a paste too)
            _ if widgets::typed(&key).is_some() => {
                let c = widgets::typed(&key).unwrap_or_default();
                let mut next = if picking { String::new() } else { text.clone() };
                // a key that cannot be part of a valid value is refused (names: also the
                // node's own length)
                let refused = match self.rules {
                    Some(rules) => rules.refuse(&next, c),
                    None if c.is_control() => Some(help::bad_char(&t("set-name"), c)),
                    None if next.len() + c.len_utf8() > p::NAME_MAX => Some(help::too_long(&t("set-name"), p::NAME_MAX)),
                    None => None,
                };
                match refused {
                    None => {
                        next.push(c);
                        *text = next;
                        self.choices.clear();
                        self.problem = None;
                    }
                    Some(why) => self.problem = Some(why),
                }
            }
            _ => {}
        }
    }

    /// Takes a typed value. One that could not be sent is refused right away (the typing stays
    /// open, the reason is shown); one equal to what the node has is no change.
    fn set(&mut self, target: Target, text: String, node: &Node) {
        let text = text.trim().to_string();
        let before = self.draft.clone();
        match &target {
            Target::Setting(id) => {
                if !self.group && text == current_setting(node, id) {
                    self.draft.values.remove(id);
                } else {
                    self.draft.values.insert(id.clone(), text);
                }
            }
            Target::Contact(key) => {
                let same = text == current_contact(node, key);
                self.draft.contacts.entry(key.clone()).or_default().name = (!same).then_some(text);
            }
            Target::Channel(slot) => {
                let same = text == current_channel(node, *slot);
                self.draft.channels.entry(*slot).or_default().name = (!same).then_some(text);
            }
        }
        match plan(node, &self.draft) {
            Ok(_) => {
                self.input = None;
                self.choices.clear();
                self.problem = None;
                self.tidy();
            }
            Err(problem) => {
                self.draft = before;
                self.problem = Some(problem);
            }
        }
    }

    /// Puts a radio preset in as changes: frequency, bandwidth, SF, CR (a repeater: its one
    /// "radio" setting), and the path hash mode when the preset asks for one and the node has it.
    /// A value the node already has is no change. Checked like a typed value.
    pub fn apply_preset(&mut self, node: &Node, preset: &Preset) {
        let before = self.draft.clone();
        // several nodes: the preset's values for all (each node then leaves what it has already),
        // never "no change" because the first of them has it
        let group = self.group;
        let mut put = |id: &str, text: String| {
            let exists = node.facts.iter().any(|f| f.edit.as_ref().is_some_and(|e| e.id == id));
            if !exists {
                return;
            }
            if !group && text == current_setting(node, id) {
                self.draft.values.remove(id);
            } else {
                self.draft.values.insert(id.to_string(), text);
            }
        };
        if node.kind == Kind::Companion {
            put(edit::FREQ, format!("{:.3}", preset.freq));
            put(edit::BW, format!("{}", preset.bw));
            put(edit::SF, preset.sf.to_string());
            put(edit::CR, preset.cr.to_string());
        } else {
            // "869.6179809,62.5,8,8" is the same radio as "869.618,62.5,8,8": compare the numbers
            let now = current_setting(node, edit::CONSOLE_RADIO);
            let same = !group && Radio::parse(&now).is_some_and(|r| preset.same_radio(r));
            // the same radio: put what it has (= no change); else the preset's
            let text = if same { now } else { format!("{:.3},{},{},{}", preset.freq, preset.bw, preset.sf, preset.cr) };
            put(edit::CONSOLE_RADIO, text);
        }
        if let Some(mode) = preset.path_hash_mode() {
            put(edit::PATH_HASH, mode.to_string());
        }
        match plan(node, &self.draft) {
            Ok(_) => self.problem = None,
            Err(problem) => {
                self.draft = before;
                self.problem = Some(problem);
            }
        }
    }

    pub fn toggle_favourite(&mut self, node: &Node, key: &str) {
        let Some(contact) = node.contacts.iter().find(|c| c.public_key == key) else { return };
        let d = self.draft.contacts.entry(key.to_string()).or_default();
        let now = !d.favourite.unwrap_or(contact.favourite);
        d.favourite = (now != contact.favourite).then_some(now);
        self.tidy();
    }

    /// `x` with contacts checked: all of them removed; when all of them were removed already,
    /// all kept again
    pub fn remove_contacts(&mut self, keys: &[String]) {
        let all = keys.iter().all(|k| self.draft.contacts.get(k).is_some_and(|d| d.remove));
        for key in keys {
            self.draft.contacts.entry(key.clone()).or_default().remove = !all;
        }
        self.tidy();
    }

    pub fn toggle_remove_contact(&mut self, key: &str) {
        let d = self.draft.contacts.entry(key.to_string()).or_default();
        d.remove = !d.remove;
        self.tidy();
    }

    pub fn toggle_remove_channel(&mut self, slot: u8) {
        let d = self.draft.channels.entry(slot).or_default();
        d.remove = !d.remove;
        self.tidy();
    }

    /// `←` `→` on an access list entry: the role before / after (read only, read / write, admin).
    /// Back to what the node has = no change. A removed entry gets its role back first.
    pub fn acl_role(&mut self, node: &Node, key: &str, back: bool) {
        let had = node.acl.iter().flatten().find(|e| e.key == key).map(|e| e.perms);
        let shown = match self.draft.acl.get(key) {
            Some(0) | None => had.unwrap_or(acl::ROLES[0]),
            Some(perms) => *perms,
        };
        let removed = self.draft.acl.get(key) == Some(&0);
        let next = if removed { shown } else { acl::with_role(shown, acl::next_role(acl::role_of(shown), back)) };
        if had == Some(next) {
            self.draft.acl.remove(key);
        } else {
            self.draft.acl.insert(key.to_string(), next);
        }
    }

    /// Keys picked from the address book, each with its role: added to the list (not this node's
    /// own key, not one already there). Refused as a whole when the list would be over the
    /// node's room (`plan` says why).
    pub fn add_acl(&mut self, node: &Node, keys: &[(String, u8)]) {
        let before = self.draft.clone();
        for (key, role) in keys {
            let key = key.to_ascii_uppercase();
            let known = node.acl.iter().flatten().any(|e| e.key == key) || self.draft.acl.contains_key(&key);
            if !acl::valid_key(&key) || key.eq_ignore_ascii_case(&node.public_key) || known {
                continue;
            }
            self.draft.acl.insert(key, acl::with_role(0, *role));
        }
        match plan(node, &self.draft) {
            Ok(_) => {
                self.problem = None;
                self.tidy();
            }
            Err(problem) => {
                self.draft = before;
                self.problem = Some(problem);
            }
        }
    }

    /// `x` on an access list entry: out of the list, or kept again; one only added here is
    /// just dropped
    pub fn toggle_remove_acl(&mut self, node: &Node, key: &str) {
        let had = node.acl.iter().flatten().any(|e| e.key == key);
        match self.draft.acl.get(key) {
            Some(0) => {
                self.draft.acl.remove(key);
            }
            _ if had => {
                self.draft.acl.insert(key.to_string(), 0);
            }
            _ => {
                self.draft.acl.remove(key);
            }
        }
    }

    /// Adds region `name` with the regions above it in the user's list (`library`), flood
    /// allowed, leaving out the ones in `have` (the node has them). A name not in the list goes
    /// at the top level.
    pub fn add_region(&mut self, library: &[Region], name: &str, have: &[String]) {
        let mut chain = regions::with_parents(library, name);
        if chain.is_empty() {
            chain.push(Region::new(name.to_string(), None));
        }
        let edits = &mut self.draft.regions;
        for region in chain {
            edits.remove.retain(|n| *n != region.name);
            if !have.contains(&region.name) && !edits.add.iter().any(|a| a.name == region.name) {
                edits.add.push(Region { flood: true, from: Vec::new(), ..region });
            }
        }
    }

    /// Flood allowed / not on region `name` (`now`: what the node has)
    pub fn toggle_flood(&mut self, name: &str, now: Option<bool>) {
        let edits = &mut self.draft.regions;
        if let Some(added) = edits.add.iter_mut().find(|r| r.name == name) {
            added.flood = !added.flood;
            return;
        }
        let Some(now) = now else { return };
        let after = !edits.flood.get(name).copied().unwrap_or(now);
        if after == now {
            edits.flood.remove(name);
        } else {
            edits.flood.insert(name.to_string(), after);
        }
    }

    /// Removes region `name` (and the ones under it, `under`), or keeps it again; a region only
    /// added here is just dropped from the additions
    pub fn toggle_remove_region(&mut self, name: &str, under: &[String]) {
        let edits = &mut self.draft.regions;
        if edits.add.iter().any(|r| r.name == name) {
            edits.add.retain(|r| r.name != name && !under.contains(&r.name));
        } else if edits.remove.iter().any(|n| n == name) {
            edits.remove.retain(|n| n != name);
        } else {
            edits.remove.push(name.to_string());
        }
    }

    /// `h`: region `name` becomes the node's home; pressed on the home: no home any more. A toggle,
    /// like `d`. No home is `*` (the firmware's root, id 0: `region home *` clears it, CommonCLI.cpp).
    pub fn set_home(&mut self, name: &str, now: Option<&String>) {
        let edits = &mut self.draft.regions;
        let now = now.map_or(regions::ROOT, String::as_str);
        let shown = edits.home.as_deref().unwrap_or(now);
        let after = if shown == name { regions::ROOT } else { name };
        edits.home = (after != now).then(|| after.to_string());
    }

    /// Region `name` as the default scope; pressed on the default scope: none
    pub fn toggle_default(&mut self, name: &str, now: Option<&String>) {
        let edits = &mut self.draft.regions;
        let current = edits.default.clone().unwrap_or_else(|| now.cloned());
        let after = if current.as_deref() == Some(name) { None } else { Some(name.to_string()) };
        edits.default = (after.as_ref() != now).then_some(after);
    }

    /// Drops every change to region `name` (group config: the region picked again)
    pub fn undo_region(&mut self, name: &str) {
        let edits = &mut self.draft.regions;
        edits.add.retain(|r| r.name != name);
        edits.remove.retain(|n| n != name);
        edits.flood.remove(name);
        if edits.home.as_deref() == Some(name) {
            edits.home = None;
        }
        if edits.default.as_ref().is_some_and(|d| d.as_deref() == Some(name)) {
            edits.default = None;
        }
    }

    /// Entries that went back to "no change" are forgotten
    fn tidy(&mut self) {
        self.draft.contacts.retain(|_, d| *d != ContactDraft::default());
        self.draft.channels.retain(|_, d| *d != ChannelDraft::default());
    }

    /// The commit question: every change, as it will be sent. "Back up, then write" (the first
    /// button) or "write now" = `Cmd::Apply`; "keep editing" closes it.
    pub fn commit(&mut self, node: &Node) -> Action {
        let steps = match plan(node, &self.draft) {
            Ok(steps) => steps,
            Err(problem) => {
                self.problem = Some(problem.clone());
                return Action::Ask(Popup::note(t("commit-title"), vec![problem]));
            }
        };
        if steps.is_empty() {
            return Action::None;
        }
        let mut lines = vec![tf("commit-ask", &[("name", clean(&node.name).into()), ("count", steps.len().into())]), String::new()];
        for step in &steps {
            lines.push(format!("  {} {}", if step.danger { "!" } else { "·" }, step.line));
        }
        lines.push(String::new());
        if steps.iter().any(|s| s.danger) {
            lines.push(t("commit-danger"));
        }
        lines.push(t("commit-once"));
        Action::Ask(commit_question(lines, vec![(node.clone(), steps)]))
    }

    /// Leaving edit mode: nothing to ask when nothing changed (None); else the question, with
    /// every change that would be lost. Yes = `Page::confirmed`, which drops them.
    pub fn leave(&self, node: &Node) -> Option<Action> {
        let count = self.draft.count();
        if count == 0 {
            return None;
        }
        let mut lines = vec![tf("leave-ask", &[("count", count.into())]), String::new()];
        if let Ok(steps) = plan(node, &self.draft) {
            lines.extend(steps.iter().map(|s| format!("  · {}", s.line)));
            lines.push(String::new());
        }
        lines.push(t("leave-not-sent"));
        let yes = tf("leave-yes", &[("count", count.into())]);
        // the focus on "keep editing": a reflex Enter must not throw the changes away
        Some(Action::Ask(Popup::ask(t("leave-title"), lines, Confirm::Page, yes, t("leave-no")).safe()))
    }
}

/// The buttons of a commit, for one node or several: "back up, then write" (first, the focus),
/// "write now", "keep editing". Each node is written by its own `Cmd::Apply`, in order.
pub fn commit_question(mut lines: Vec<String>, writes: Vec<(Node, Vec<edit::Step>)>) -> Popup {
    let count: usize = writes.iter().map(|(_, steps)| steps.len()).sum();
    // what each button does about backups, plainly: only the nodes written are backed up (a
    // node where nothing changes is not touched at all), and "write now" backs up none
    let names: Vec<String> = writes.iter().map(|(node, _)| clean(&node.name)).collect();
    let yes = tf("commit-yes", &[("count", count.into())]);
    lines.push(tf(
        "commit-backup-who",
        &[
            ("nodes", names.join(", ").into()),
            ("count", names.len().into()),
            ("first", t("commit-backup-first").into()),
            ("now", yes.as_str().into()),
        ],
    ));
    let send = |backup_first: bool| {
        let cmds = writes
            .iter()
            .map(|(node, steps)| Cmd::Apply { node: node.clone(), steps: steps.clone(), backup_first })
            .collect();
        Confirm::Send(cmds)
    };
    Popup::ask(t("commit-title"), lines, send(false), yes, t("commit-no")).first(t("commit-backup-first"), send(true))
}

fn current_setting(node: &Node, id: &str) -> String {
    node.facts.iter().find(|f| f.edit.as_ref().is_some_and(|e| e.id == id)).map(|f| f.value.clone()).unwrap_or_default()
}

fn current_contact(node: &Node, key: &str) -> String {
    node.contacts.iter().find(|c| c.public_key == key).map(|c| c.name.clone()).unwrap_or_default()
}

fn current_channel(node: &Node, slot: u8) -> String {
    node.channels.iter().find(|c| c.slot == slot).map(|c| c.name.clone()).unwrap_or_default()
}
