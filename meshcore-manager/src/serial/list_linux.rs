//! USB serial ports on Linux: found in /sys (no udev library needed).
//! Each USB serial port with its vendor / product ids, serial number and maker, read from the
//! files the kernel keeps for it.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use super::Port;

/// "ttyACM0" for /dev/ttyACM0 or for its /dev/serial/by-id/... name
pub fn short(path: &str) -> String {
    let real = std::fs::canonicalize(path).unwrap_or_else(|_| PathBuf::from(path));
    real.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| path.to_string())
}

/// The USB serial ports
pub fn ports() -> Vec<Port> {
    let mut by_id: HashMap<PathBuf, String> = HashMap::new();
    if let Ok(dir) = std::fs::read_dir("/dev/serial/by-id") {
        for e in dir.flatten() {
            if let Ok(target) = std::fs::canonicalize(e.path()) {
                by_id.insert(target, e.path().to_string_lossy().into_owned());
            }
        }
    }
    let Ok(dir) = std::fs::read_dir("/sys/class/tty") else { return Vec::new() };
    let mut names: Vec<String> = dir
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.starts_with("ttyACM") || n.starts_with("ttyUSB"))
        .collect();
    names.sort();
    names
        .into_iter()
        .map(|name| {
            let usb = usb_dir(&name);
            let read = |file: &str| {
                usb.as_ref()
                    .and_then(|d| std::fs::read_to_string(d.join(file)).ok())
                    .map(|s| s.trim().to_string())
                    .unwrap_or_default()
            };
            let dev = format!("/dev/{name}");
            let hex = |file: &str| u16::from_str_radix(&read(file), 16).ok();
            Port {
                path: by_id.get(Path::new(&dev)).cloned().unwrap_or_else(|| dev.clone()),
                product: read("product"),
                maker: read("manufacturer"),
                ids: hex("idVendor").zip(hex("idProduct")),
                serial_number: read("serial"),
                dev,
            }
        })
        .collect()
}

/// The USB device folder above a tty in /sys (the one with idVendor, product, manufacturer)
fn usb_dir(tty: &str) -> Option<PathBuf> {
    let mut d = std::fs::canonicalize(format!("/sys/class/tty/{tty}/device")).ok()?;
    for _ in 0..6 {
        if d.join("idVendor").exists() {
            return Some(d);
        }
        d = d.parent()?.to_path_buf();
    }
    None
}
