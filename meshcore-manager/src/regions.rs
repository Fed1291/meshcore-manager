//! Regions (flood scopes, firmware 1.10+): the user's own list of region names, and a node's
//! regions as it tells them.
//!
//! The list is a text file in the firmware's own format, the one `region` prints and
//! `region load` reads: `*` (the root) first, then one region per line, one more space of indent
//! for each level under its parent. What a node adds to a line (`F` = flood allowed, `^` = its
//! home region) is read when a node's list is pasted in, and not written to the file: flood and
//! home belong to each node. So a node's `region` output can be pasted into the file as it is.
//! This program adds one thing of its own after a name: `; from <node>, <node>` (the nodes the
//! region was copied from); everything after `;` is a note, not part of the tree.
//!
//! Names: up to 29 bytes (30 on the node, a `#` is put in front inside); lowercase letters,
//! digits, `-`, and `$` first for a private region. The flood key comes from the name, so every
//! node of an area must use exactly the same spelling: names are kept lowercase. A leading `#` is
//! dropped (the firmware ignores it when comparing names).
//! Firmware: src/helpers/RegionMap.cpp, src/helpers/CommonCLI.cpp (`region ...`).

use std::path::Path;

use anyhow::{Context, Result};

use crate::i18n::tf;

/// The longest name, in bytes
pub const NAME_MAX: usize = 29;

/// A console reply is cut at 160 characters: a region tree this long may be missing regions
const REPLY_CUT: usize = 155;

#[derive(Clone, Debug, PartialEq)]
pub struct Region {
    pub name: String,
    /// None: directly under `*`
    pub parent: Option<String>,
    /// Flood allowed (on a node; always false in the user's list)
    pub flood: bool,
    /// In the user's list: the nodes it was copied from (their names then), so the list says
    /// where each name came from
    pub from: Vec<String>,
}

impl Region {
    pub fn new(name: String, parent: Option<String>) -> Region {
        Region { name, parent, flood: false, from: Vec::new() }
    }
}

/// A node's regions as it told them
#[derive(Clone, Debug, Default, PartialEq)]
pub struct NodeRegions {
    pub list: Vec<Region>,
    pub home: Option<String>,
    /// The scope it floods its own adverts with (firmware 1.15+)
    pub default: Option<String>,
    /// Its console reply was cut (160 characters): some regions may be missing here
    pub cut: bool,
    /// `*`, the root: packets with no region (or one the node does not know). Its flood can be
    /// allowed or blocked like any region's (`region allowf *` / `denyf *`); allowed by default
    pub root_flood: bool,
}

/// The name of the root region, every packet without a region
pub const ROOT: &str = "*";

/// `c` can be in a name (RegionMap::is_name_char, kept to lowercase by convention)
pub fn name_char(c: char) -> bool {
    c == '-' || c == '$' || c.is_ascii_digit() || c.is_ascii_lowercase() || (!c.is_ascii() && c.is_alphanumeric())
}

/// A name as typed or as a node prints it: lowercase, without a leading `#`, None if it cannot be
/// a region name
pub fn clean_name(text: &str) -> Option<String> {
    let name = text.trim().trim_start_matches('#').to_lowercase();
    let ok = !name.is_empty() && name.len() <= NAME_MAX && name.chars().all(name_char);
    ok.then_some(name)
}

/// Reads a tree in the firmware's format; also returns the region marked `^` (home)
pub fn parse(text: &str) -> (Vec<Region>, Option<String>) {
    let mut list: Vec<Region> = Vec::new();
    let mut home = None;
    // the regions above this line: (indent, name)
    let mut above: Vec<(usize, String)> = Vec::new();
    for line in text.lines() {
        // "  geneva  ; from Node A, Node B": the note after `;` is where it came from
        let (line, note) = line.split_once(';').unwrap_or((line, ""));
        let from: Vec<String> = note
            .trim()
            .strip_prefix("from")
            .map(|names| names.split(',').map(|n| n.trim().to_string()).filter(|n| !n.is_empty()).collect())
            .unwrap_or_default();
        let indent = line.len() - line.trim_start().len();
        let mut words = line.split_whitespace();
        let Some(first) = words.next() else { continue };
        let is_home = first.ends_with('^');
        let first = first.trim_end_matches('^');
        if first == "*" {
            above.clear();
            continue;
        }
        let Some(name) = clean_name(first) else { continue };
        let flood = words.any(|w| w == "F");
        while above.last().is_some_and(|(i, _)| *i >= indent) {
            above.pop();
        }
        if is_home {
            home = Some(name.clone());
        }
        if !list.iter().any(|r| r.name == name) {
            list.push(Region { name: name.clone(), parent: above.last().map(|(_, n)| n.clone()), flood, from });
        }
        above.push((indent, name));
    }
    (list, home)
}

/// The tree in the firmware's format (names only: no flood, no home), each with where it came
/// from as a note
pub fn to_text(list: &[Region]) -> String {
    fn under(list: &[Region], parent: Option<&str>, depth: usize, out: &mut String) {
        for r in list.iter().filter(|r| r.parent.as_deref() == parent) {
            out.push_str(&" ".repeat(depth));
            out.push_str(&r.name);
            if !r.from.is_empty() {
                // a name cannot hold ',' or ';' in the note
                let names: Vec<String> = r.from.iter().map(|n| n.replace([',', ';', '\n'], " ")).collect();
                out.push_str(&format!("  ; from {}", names.join(", ")));
            }
            out.push('\n');
            under(list, Some(&r.name), depth + 1, out);
        }
    }
    let mut out = String::from("*\n");
    under(list, None, 1, &mut out);
    out
}

/// The tree in display order (parents first, each followed by its own), with each one's depth
pub fn ordered(list: &[Region]) -> Vec<(usize, &Region)> {
    fn under<'a>(list: &'a [Region], parent: Option<&str>, depth: usize, out: &mut Vec<(usize, &'a Region)>) {
        for r in list.iter().filter(|r| r.parent.as_deref() == parent) {
            out.push((depth, r));
            under(list, Some(&r.name), depth + 1, out);
        }
    }
    let mut out = Vec::new();
    under(list, None, 0, &mut out);
    // a region whose parent is missing from the list: at the end, at the top level
    for r in list {
        if !out.iter().any(|(_, o)| o.name == r.name) {
            out.push((0, r));
        }
    }
    out
}

/// `name` and the regions above it, the top one first
pub fn with_parents(list: &[Region], name: &str) -> Vec<Region> {
    let mut chain = Vec::new();
    let mut at = list.iter().find(|r| r.name == name);
    while let Some(region) = at {
        if chain.iter().any(|r: &Region| r.name == region.name) {
            break;
        }
        chain.push(region.clone());
        at = region.parent.as_ref().and_then(|p| list.iter().find(|r| &r.name == p));
    }
    chain.reverse();
    chain
}

/// `name` and every region under it, the deepest first (the order the firmware can remove them)
pub fn with_children(list: &[Region], name: &str) -> Vec<String> {
    let mut out = Vec::new();
    for r in list.iter().filter(|r| r.parent.as_deref() == Some(name)) {
        out.extend(with_children(list, &r.name));
    }
    out.push(name.to_string());
    out
}

/// A node's answers: the tree (`region`), `region home`, `region default`
pub fn from_node(tree: &str, home: Option<&str>, default: Option<&str>) -> NodeRegions {
    let (list, marked) = parse(tree);
    // "home is geneva" / "home is *"; "default scope is geneva" / "default scope is <null>"
    let last = |reply: Option<&str>| reply.and_then(|r| r.split_whitespace().last()).and_then(clean_name);
    // the root's line: "* F" (flood allowed) or "*" (blocked); missing: the firmware's default
    let root = tree.lines().map(str::split_whitespace).find_map(|mut w| {
        w.next().filter(|first| first.trim_end_matches('^') == ROOT).map(|_| w.any(|x| x == "F"))
    });
    NodeRegions {
        list,
        home: last(home).or(marked),
        default: last(default),
        cut: tree.len() >= REPLY_CUT,
        root_flood: root.unwrap_or(true),
    }
}

/// The user's list (a missing file = an empty list); a line to show when it could not be read
pub fn load(path: &Path) -> (Vec<Region>, Option<String>) {
    if !path.exists() {
        return (Vec::new(), None);
    }
    match std::fs::read_to_string(path) {
        Ok(text) => (parse(&text).0.into_iter().map(|r| Region { flood: false, ..r }).collect(), None),
        Err(e) => (Vec::new(), Some(tf("regions-unreadable", &[("file", path.display().to_string().into()), ("error", e.to_string().into())]))),
    }
}

/// Writes the user's list
pub fn save(path: &Path, list: &[Region]) -> Result<()> {
    std::fs::write(path, to_text(list)).with_context(|| tf("regions-unwritable", &[("file", path.display().to_string().into())]))
}
