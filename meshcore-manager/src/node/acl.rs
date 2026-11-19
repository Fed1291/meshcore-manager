//! The access list of a repeater, room server or sensor: who may log in as what. Firmware:
//! src/helpers/ClientACL.{h,cpp} and each node's `handleCommand` (examples/simple_repeater/MyMesh.cpp).
//! - `get acl` (serial console only) prints "ACL:", then one line per entry: its permissions in
//!   two hex digits and its public key in 64 ("03 4A1F..."). Entries with permissions 0 (guests)
//!   are not listed and not kept.
//! - `setperm <public key> <permissions>`: the permissions as a decimal number (`atoi`). The role
//!   is the lower two bits: 1 read only, 2 read / write, 3 admin; 0 (guest) removes the entry.
//!   Saved by the node about 5 s later (`LAZY_CONTACTS_WRITE_DELAY`).
//! - At most 32 entries (`MAX_CLIENTS`): one more and the node drops its least active non-admin
//!   entry, without saying so. A change here never does that (`node::edit::plan` refuses it).

use crate::i18n::t;

/// Entries a node keeps (`MAX_CLIENTS` in ClientACL.h, the firmware's default)
pub const MAX: usize = 32;
/// A public key in hex
pub const KEY_HEX: usize = 64;
/// The role bits of the permissions (`PERM_ACL_ROLE_MASK`)
const ROLE_MASK: u8 = 3;
/// The roles an entry can have, in order (0, guest, is "not in the list")
pub const ROLES: [u8; 3] = [1, 2, 3];
pub const ADMIN: u8 = 3;

#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    /// 64 hex digits, upper case (as the node prints it)
    pub key: String,
    /// The whole permissions byte (bits above the role are kept as they are)
    pub perms: u8,
}

/// The role of these permissions (0 guest, 1 read only, 2 read / write, 3 admin)
pub fn role_of(perms: u8) -> u8 {
    perms & ROLE_MASK
}

/// The permissions with another role, the other bits kept
pub fn with_role(perms: u8, role: u8) -> u8 {
    (perms & !ROLE_MASK) | (role & ROLE_MASK)
}

/// The next role (`back`: the one before), round: read only, read / write, admin
pub fn next_role(role: u8, back: bool) -> u8 {
    let at = ROLES.iter().position(|r| *r == role).unwrap_or(0);
    let len = ROLES.len();
    ROLES[if back { (at + len - 1) % len } else { (at + 1) % len }]
}

/// "admin", "read / write"... (by the role bits)
pub fn role_name(perms: u8) -> String {
    t(match perms & ROLE_MASK {
        1 => "acl-read-only",
        2 => "acl-read-write",
        3 => "acl-admin",
        _ => "acl-guest",
    })
}

/// The entries of a `get acl` print (the text after "ACL:"). A line that is not "PP KEY" is
/// left out: it is not an entry (an echo, a log line of the node).
pub fn parse(text: &str) -> Vec<Entry> {
    text.lines()
        .filter_map(|line| {
            let (perms, key) = line.trim().split_once(' ')?;
            let perms = u8::from_str_radix(perms, 16).ok().filter(|_| perms.len() == 2)?;
            let key = key.trim();
            valid_key(key).then(|| Entry { key: key.to_ascii_uppercase(), perms })
        })
        .collect()
}

/// A whole public key: 64 hex digits
pub fn valid_key(key: &str) -> bool {
    key.len() == KEY_HEX && key.chars().all(|c| c.is_ascii_hexdigit())
}

/// The command that gives `key` these permissions (0: out of the list)
pub fn command(key: &str, perms: u8) -> String {
    format!("setperm {key} {perms}")
}

/// "4A1F09C2…" for lists and lines
pub fn short(key: &str) -> String {
    format!("{}…", key.chars().take(8).collect::<String>())
}
