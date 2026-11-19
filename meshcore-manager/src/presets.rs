//! Radio presets: the radio settings a mesh uses in a country or region, as the official MeshCore
//! apps offer them (data/radio_presets.toml, copied from the MeshCore app API; radio_presets.toml in
//! the data folder replaces the list). Edit mode puts a preset's values in as changes (`p`); the
//! node's own radio is named after the presets that match it.

use std::path::Path;
use std::sync::OnceLock;

use serde::Deserialize;

use crate::i18n::tf;
use crate::node::{Kind, Node};

const BUILT_IN: &str = include_str!("../data/radio_presets.toml");

#[derive(Deserialize, Clone)]
pub struct Preset {
    pub title: String,
    /// MHz
    pub freq: f64,
    /// kHz
    pub bw: f64,
    pub sf: u8,
    pub cr: u8,
    /// Bytes per hop id in paths (1-3), when the mesh agreed on one
    #[serde(default)]
    pub path_hash_size: Option<u8>,
}

#[derive(Deserialize, Default)]
struct File {
    #[serde(default)]
    preset: Vec<Preset>,
}

static LIST: OnceLock<Vec<Preset>> = OnceLock::new();

/// Reads the list: the user's own file when there is one, else the built-in one. Once, at start.
/// Returns a line to show when the user's file could not be used.
pub fn init(own: &Path) -> Option<String> {
    let built_in = || toml::from_str::<File>(BUILT_IN).map(|f| f.preset).unwrap_or_default();
    let (list, note) = if own.exists() {
        let read = std::fs::read_to_string(own)
            .map_err(|e| e.to_string())
            .and_then(|s| toml::from_str::<File>(&s).map_err(|e| e.to_string()));
        match read {
            Ok(file) if !file.preset.is_empty() => (file.preset, None),
            Ok(_) => (built_in(), None),
            Err(error) => (
                built_in(),
                Some(tf("presets-unreadable", &[("file", own.display().to_string().into()), ("error", error.into())])),
            ),
        }
    } else {
        (built_in(), None)
    };
    let _ = LIST.set(list);
    note
}

pub fn list() -> &'static [Preset] {
    LIST.get().map_or(&[], Vec::as_slice)
}

impl Preset {
    /// The same radio settings (to the kHz / the 10 Hz of bandwidth: the node stores floats)
    pub fn same_radio(&self, radio: Radio) -> bool {
        (self.freq - radio.freq).abs() < 0.001
            && (self.bw - radio.bw).abs() < 0.01
            && self.sf == radio.sf
            && self.cr == radio.cr
    }

    /// The node's path hash mode this preset asks for (size - 1), if it asks for one
    pub fn path_hash_mode(&self) -> Option<u8> {
        self.path_hash_size.filter(|s| (1..=3).contains(s)).map(|s| s - 1)
    }

    /// "869.618 MHz · BW 62.5 · SF8 · CR8"
    pub fn describe(&self) -> String {
        format!("{:.3} MHz · BW {} · SF{} · CR{}", self.freq, self.bw, self.sf, self.cr)
    }
}

/// A node's radio: frequency (MHz), bandwidth (kHz), spreading factor, coding rate
#[derive(Clone, Copy)]
pub struct Radio {
    pub freq: f64,
    pub bw: f64,
    pub sf: u8,
    pub cr: u8,
}

impl Radio {
    /// "freq,bw,sf,cr", as a repeater's `get radio` answers and `set radio` takes it
    pub fn parse(text: &str) -> Option<Radio> {
        let mut parts = text.split(',').map(str::trim);
        let radio = Radio {
            freq: parts.next()?.parse().ok()?,
            bw: parts.next()?.parse().ok()?,
            sf: parts.next()?.parse().ok()?,
            cr: parts.next()?.parse().ok()?,
        };
        parts.next().is_none().then_some(radio)
    }
}

/// The radio the node has now: a companion's from its settings, a console node's from `get radio`
pub fn radio_of(node: &Node) -> Option<Radio> {
    if node.kind == Kind::Companion {
        let me = node.me.as_ref()?;
        return Some(Radio { freq: me.freq_mhz, bw: me.bandwidth_khz, sf: me.spreading_factor, cr: me.coding_rate });
    }
    Radio::parse(&node.radio)
}

/// The presets with the node's radio settings ("EU/UK (Narrow)" and "Switzerland" share theirs)
pub fn matching(node: &Node) -> Vec<&'static Preset> {
    match radio_of(node) {
        Some(radio) => list().iter().filter(|p| p.same_radio(radio)).collect(),
        None => Vec::new(),
    }
}

/// "EU/UK (Narrow) / Switzerland", or "" when no preset has the node's settings
pub fn names(node: &Node) -> String {
    matching(node).iter().map(|p| p.title.as_str()).collect::<Vec<_>>().join(" / ")
}
