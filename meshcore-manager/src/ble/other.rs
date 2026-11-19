//! Bluetooth on macOS (and any system that is neither Windows nor Linux): not available yet.
//! The Fleet says so where Bluetooth is shown; USB nodes work as everywhere.

use std::time::Instant;

use anyhow::{bail, Result};

use super::Heard;
use crate::i18n::t;

/// Nothing to unpair: this program never paired anything here
pub fn forget(_address: &str) -> Result<()> {
    bail!(t("ble-not-on-system"))
}

/// No watcher: the reason is shown once, nothing is ever heard
pub(super) fn watch(heard: Heard) {
    heard.set_problem(Some(t("ble-not-on-system")));
}

/// Never opened (no companion is ever heard here)
pub struct Conn;

impl Conn {
    pub fn open(_address: &str, _pin: &str) -> Result<Conn> {
        bail!(t("ble-not-on-system"))
    }

    pub fn write(&mut self, _frame: &[u8]) -> Result<()> {
        bail!(t("ble-not-on-system"))
    }

    pub fn recv(&mut self, _deadline: Instant) -> Result<Option<Vec<u8>>> {
        bail!(t("ble-not-on-system"))
    }
}
