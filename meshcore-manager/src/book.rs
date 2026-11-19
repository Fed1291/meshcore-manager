//! The address book: the manager's own list of people and nodes, by public key, in
//! `contacts.toml` in the data folder. Filled by itself, never typed in:
//! - each companion read adds its contacts (the name each one advertises, its type, when it
//!   was last heard, which companions hold it and under which name);
//! - each access list read (repeaters, room servers, sensors) adds its keys and the role each
//!   node gives them.
//!
//! What only the book has: a custom name for a key (`own_name`), the access role you give it
//! (`role`: the firmware's ACL roles, read only / read-write / admin, ClientACL.h) and its
//! favourite mark (`favourite`: written to the companions that hold it, flags bit 0). A companion
//! cannot keep a name of yours: the contact's next advert writes its own name over it
//! (firmware `BaseChatMesh.cpp`, onAdvertRecv). So the advertised name stays as it is, and
//! yours is shown next to it. A key is never forgotten by itself: a companion that no longer
//! holds it is only taken off its "held by" list (the book keeps what it saw).
//!
//! Several jobs can read nodes at once: every change goes through `update` (one at a time,
//! read, change, write). A file that cannot be read is never written over.

use std::path::Path;
use std::sync::Mutex;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::i18n::tf;
use crate::node::acl;
use crate::proto::Contact;

/// One change at a time: two nodes read at once must not write the file over each other
static LOCK: Mutex<()> = Mutex::new(());

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Book {
    #[serde(default, rename = "person")]
    pub people: Vec<Person>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Person {
    /// 64 hex digits, upper case
    pub key: String,
    /// What it advertises as (ADV_TYPE_*): 1 chat, 2 repeater, 3 room, 4 sensor; 0 not known
    /// (a key only seen in an access list)
    #[serde(default)]
    pub kind: u8,
    /// The name it advertises, as last read from a companion
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub name: String,
    /// Your own name for it: only here, never sent to a node
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub own_name: String,
    /// The access role you give it on your nodes (`acl::ROLES`: 1 read only, 2 read-write,
    /// 3 admin; 0 none): to find your admins and give them access in one go
    #[serde(default, skip_serializing_if = "is_zero")]
    pub role: u8,
    /// Favourite as you set it here (None: never set here, then it is a favourite when a
    /// companion holding it marks it so)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub favourite: Option<bool>,
    /// When its last advert was heard (seconds since 1970, the companions' clocks; 0 never)
    #[serde(default)]
    pub last_heard: u32,
    /// The companions that hold it as a contact
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub held_by: Vec<Holder>,
    /// The nodes whose access list gives it a role
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub access: Vec<Access>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Holder {
    /// The companion's public key
    pub node: String,
    /// Its name when it was read
    pub node_name: String,
    /// The contact's name in that companion
    #[serde(default)]
    pub name: String,
    /// A favourite in that companion
    #[serde(default)]
    pub favourite: bool,
}

fn is_zero(n: &u8) -> bool {
    *n == 0
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Access {
    /// The node's public key
    pub node: String,
    /// Its name when it was read
    pub node_name: String,
    /// The permissions byte (`acl::role_of` gives the role)
    pub perms: u8,
}

impl Person {
    /// Your name for it, else the name it advertises, else the start of its key
    pub fn shown_name(&self) -> String {
        if !self.own_name.is_empty() {
            self.own_name.clone()
        } else if !self.name.is_empty() {
            self.name.clone()
        } else {
            acl::short(&self.key)
        }
    }

    /// A favourite: as set here, else as a companion holding it has it
    pub fn is_favourite(&self) -> bool {
        self.favourite.unwrap_or_else(|| self.held_by.iter().any(|h| h.favourite))
    }

    /// Has this access role: the one you gave it here, or one a node's list gives it
    pub fn has_role(&self, role: u8) -> bool {
        self.role == role || self.access.iter().any(|a| acl::role_of(a.perms) == role)
    }

    /// The names the companions hold for it, when they are not all the same
    pub fn other_names(&self) -> Vec<String> {
        let mut names: Vec<String> = Vec::new();
        for h in &self.held_by {
            if !h.name.is_empty() && !names.contains(&h.name) {
                names.push(h.name.clone());
            }
        }
        if names.len() > 1 {
            names
        } else {
            Vec::new()
        }
    }
}

impl Book {
    pub fn find(&self, key: &str) -> Option<&Person> {
        self.people.iter().find(|p| p.key.eq_ignore_ascii_case(key))
    }

    /// The entry of this key, made when missing
    fn entry(&mut self, key: &str) -> &mut Person {
        let key = key.to_ascii_uppercase();
        match self.people.iter().position(|p| p.key == key) {
            Some(at) => &mut self.people[at],
            None => {
                self.people.push(Person { key, ..Person::default() });
                let last = self.people.len() - 1;
                &mut self.people[last]
            }
        }
    }

    /// A companion was read: each of its contacts in the book (new ones added, the advertised
    /// name and type as the companion has them now), and this companion off the "held by" list
    /// of the keys it no longer holds
    pub fn add_contacts(&mut self, node_key: &str, node_name: &str, contacts: &[Contact]) {
        for c in contacts {
            let person = self.entry(&c.public_key);
            if !c.name.is_empty() {
                person.name = c.name.clone();
            }
            if c.kind != 0 {
                person.kind = c.kind;
            }
            person.last_heard = person.last_heard.max(c.last_advert);
            let holder = Holder {
                node: node_key.to_ascii_uppercase(),
                node_name: node_name.to_string(),
                name: c.name.clone(),
                favourite: c.favourite,
            };
            match person.held_by.iter_mut().find(|h| h.node.eq_ignore_ascii_case(node_key)) {
                Some(known) => *known = holder,
                None => person.held_by.push(holder),
            }
            // a favourite set here and now on every companion holding it: nothing left to carry,
            // the companions say it from now on (a change made in a MeshCore app then shows here)
            if let Some(on) = person.favourite {
                if !person.held_by.is_empty() && person.held_by.iter().all(|h| h.favourite == on) {
                    person.favourite = None;
                }
            }
        }
        for person in &mut self.people {
            if !contacts.iter().any(|c| c.public_key.eq_ignore_ascii_case(&person.key)) {
                person.held_by.retain(|h| !h.node.eq_ignore_ascii_case(node_key));
            }
        }
    }

    /// What a backup kept of yours about `key`, back in (a restore): a custom name, a role, a
    /// favourite; what the backup does not have is left as it is (nothing taken away)
    pub fn restore(&mut self, key: &str, own_name: &str, role: u8, favourite: Option<bool>) {
        let person = self.entry(key);
        if !own_name.trim().is_empty() {
            person.own_name = own_name.trim().to_string();
        }
        if role != 0 {
            person.role = role;
        }
        if favourite.is_some() {
            person.favourite = favourite;
        }
    }

    /// A node's access list was read: each key in the book with the role this node gives it,
    /// and this node off the keys it no longer lists
    pub fn add_access(&mut self, node_key: &str, node_name: &str, entries: &[acl::Entry]) {
        for e in entries {
            let access = Access { node: node_key.to_ascii_uppercase(), node_name: node_name.to_string(), perms: e.perms };
            let person = self.entry(&e.key);
            match person.access.iter_mut().find(|a| a.node.eq_ignore_ascii_case(node_key)) {
                Some(known) => *known = access,
                None => person.access.push(access),
            }
        }
        for person in &mut self.people {
            if !entries.iter().any(|e| e.key.eq_ignore_ascii_case(&person.key)) {
                person.access.retain(|a| !a.node.eq_ignore_ascii_case(node_key));
            }
        }
    }
}

/// What the user changes on the Contacts page or in a node's view
#[derive(Clone, Debug)]
pub enum Change {
    /// Your name for a key (empty: none, the advertised one is shown)
    OwnName { key: String, name: String },
    /// The access role you give it (0: none)
    Role { key: String, role: u8 },
    /// Favourite or not (the companions holding it are written by their own commit)
    Favourite { key: String, on: bool },
    /// Out of the book (it comes back when a node read again still has it)
    Remove { key: String },
}

impl Change {
    fn apply(self, book: &mut Book) {
        match self {
            Change::OwnName { key, name } => book.entry(&key).own_name = name.trim().to_string(),
            Change::Role { key, role } => book.entry(&key).role = role,
            Change::Favourite { key, on } => book.entry(&key).favourite = Some(on),
            Change::Remove { key } => book.people.retain(|p| !p.key.eq_ignore_ascii_case(&key)),
        }
    }
}

/// Reads the book. A missing file is an empty book; one that cannot be read is an error (and is
/// then never written over: `update` refuses).
pub fn load(path: &Path) -> Result<Book> {
    if !path.exists() {
        return Ok(Book::default());
    }
    let text = std::fs::read_to_string(path).with_context(|| tf("book-unreadable", &[("file", path.display().to_string().into())]))?;
    toml::from_str(&text).with_context(|| tf("book-unreadable", &[("file", path.display().to_string().into())]))
}

/// Writes the book: to a new file first, then in place (a cut-off write never leaves half a
/// file); the one before is kept as `contacts.toml.bak`
fn save(path: &Path, book: &Book) -> Result<()> {
    let unwritable = || tf("book-unwritable", &[("file", path.display().to_string().into())]);
    let mut text = String::from(
        "# Meshcore Manager address book: every contact and access-list key seen on your nodes.\n\
         # Filled by the program; own_name, role and favourite are yours (Contacts page).\n\n",
    );
    text.push_str(&toml::to_string_pretty(book).with_context(unwritable)?);
    let new = path.with_extension("toml.new");
    std::fs::write(&new, text).with_context(unwritable)?;
    if path.exists() {
        std::fs::copy(path, path.with_extension("toml.bak")).with_context(unwritable)?;
    }
    std::fs::rename(&new, path).with_context(unwritable)
}

/// Reads the book, changes it, and writes it when something changed (one change at a time).
/// Returns the book as it is now.
pub fn update(path: &Path, change: impl FnOnce(&mut Book)) -> Result<Book> {
    let _one = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut book = load(path)?;
    let before = book.clone();
    change(&mut book);
    if book != before {
        save(path, &book)?;
    }
    Ok(book)
}

/// A change from the user
pub fn change(path: &Path, change: Change) -> Result<Book> {
    update(path, |book| change.apply(book))
}
