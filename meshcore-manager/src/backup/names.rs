//! The address book's part of a backup (section "address-book"): what is yours about the keys
//! this node holds (a companion's contacts, a console node's access list): the custom name, the
//! role, the favourite set here. The node cannot keep them (a custom name: the next advert writes
//! over it), so the backup keeps them next to the node's own sections, and a restore puts them
//! back into the address book (contacts.toml), never on the node.
//!
//! Kept as replies (the layout every version reads): one per key, the command is the key (64
//! hex digits), the reply what is yours, as JSON. A key with nothing of yours is not kept.

use std::path::Path;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use super::{read, Data, Part, Reply, State};
use crate::book::{self, Book};
use crate::i18n::tf;
use crate::node::acl;
use crate::proto::Contact;
use crate::widgets::clean;

pub const ID: &str = "address-book";

/// What is yours about one key
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
struct Mine {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    own_name: String,
    #[serde(default, skip_serializing_if = "is_zero")]
    role: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    favourite: Option<bool>,
}

fn is_zero(n: &u8) -> bool {
    *n == 0
}

/// The keys the node holds: a companion's contacts, a console node's access list
fn keys(state: &State) -> Vec<String> {
    let mut keys: Vec<String> = state.frames("contacts").iter().filter_map(|f| Contact::parse(f)).map(|c| c.public_key).collect();
    if let Some(text) = state.lines(read::ACL) {
        keys.extend(acl::parse(&text).into_iter().map(|e| e.key));
    }
    keys
}

/// The section for a backup of `state`: what the book has of yours about its keys (None:
/// nothing of yours about any of them)
pub fn part(book: &Book, state: &State) -> Option<Part> {
    let replies: Vec<Reply> = keys(state)
        .iter()
        .filter_map(|key| book.find(key))
        .map(|p| (p, Mine { own_name: p.own_name.clone(), role: p.role, favourite: p.favourite }))
        .filter(|(_, mine)| *mine != Mine::default())
        .filter_map(|(p, mine)| Some(Reply { command: p.key.clone(), reply: serde_json::to_string(&mine).ok()? }))
        .collect();
    (!replies.is_empty()).then(|| Part { id: ID.to_string(), secret: false, data: Data::Replies(replies) })
}

/// What a backup keeps of yours, by key (a line it cannot read is left out)
fn kept(state: &State) -> Vec<(String, Mine)> {
    state
        .replies(ID)
        .iter()
        .filter(|r| acl::valid_key(&r.command))
        .filter_map(|r| Some((r.command.to_ascii_uppercase(), serde_json::from_str::<Mine>(&r.reply).ok()?)))
        .collect()
}

/// What putting the backup back changes in the book, as lines for the question ("custom name
/// for X: Bob → Alice"). Only what the backup has: an empty name or no role there takes nothing
/// away from the book.
pub fn changes(book: &Book, state: &State) -> Vec<String> {
    let mut lines = Vec::new();
    for (key, mine) in kept(state) {
        let person = book.find(&key);
        let who = person.map_or_else(|| acl::short(&key), |p| clean(&p.shown_name()));
        let now_name = person.map(|p| p.own_name.clone()).unwrap_or_default();
        if !mine.own_name.is_empty() && mine.own_name != now_name {
            lines.push(tf("restore-book-name", &[("who", who.as_str().into()), ("old", now_name.into()), ("new", mine.own_name.as_str().into())]));
        }
        let now_role = person.map_or(0, |p| p.role);
        if mine.role != 0 && mine.role != now_role {
            let old = if now_role == 0 { "-".to_string() } else { acl::role_name(now_role) };
            lines.push(tf("restore-book-role", &[("who", who.as_str().into()), ("old", old.into()), ("new", acl::role_name(mine.role).into())]));
        }
    }
    lines
}

/// What changed of yours from `older` to `newer` (two backups, or a backup and the node now
/// with the book as it is now: `part`), key by key: custom names and roles
pub fn compare(older: &State, newer: &State) -> Vec<String> {
    let (old, new) = (kept(older), kept(newer));
    let mut keys: Vec<&String> = old.iter().chain(&new).map(|(k, _)| k).collect();
    keys.sort();
    keys.dedup();
    let mut lines = Vec::new();
    for key in keys {
        let was = old.iter().find(|(k, _)| k == key).map(|(_, m)| m.clone()).unwrap_or_default();
        let is = new.iter().find(|(k, _)| k == key).map(|(_, m)| m.clone()).unwrap_or_default();
        let who = acl::short(key);
        let dash = |s: &str| if s.is_empty() { "-".to_string() } else { s.to_string() };
        if was.own_name != is.own_name {
            let (old, new) = (dash(&was.own_name), dash(&is.own_name));
            lines.push(tf("restore-book-name", &[("who", who.as_str().into()), ("old", old.into()), ("new", new.into())]));
        }
        if was.role != is.role {
            let role = |r: u8| if r == 0 { "-".to_string() } else { acl::role_name(r) };
            lines.push(tf("restore-book-role", &[("who", who.as_str().into()), ("old", role(was.role).into()), ("new", role(is.role).into())]));
        }
    }
    lines
}

/// Puts what the backup keeps of yours back into the book (contacts.toml): custom names,
/// roles, favourites set here. Nothing is taken away. Returns how many keys changed.
pub fn put_back(file: &Path, state: &State) -> Result<usize> {
    let kept = kept(state);
    if kept.is_empty() {
        return Ok(0);
    }
    let mut changed = 0;
    book::update(file, |book| {
        for (key, mine) in &kept {
            let before = book.find(key).cloned();
            book.restore(key, &mine.own_name, mine.role, mine.favourite);
            if book.find(key).cloned() != before {
                changed += 1;
            }
        }
    })?;
    Ok(changed)
}
