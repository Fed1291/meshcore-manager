//! USB serial ports on macOS (and any system that is neither Windows nor Linux): the serialport
//! crate's own list (IOKit on macOS), with the vendor / product ids, serial number and maker it
//! gives. macOS shows each port twice, `/dev/cu.*` and `/dev/tty.*`: only the `cu.` one is kept
//! (the one programs open: it does not wait for a modem's carrier line).

use super::Port;

/// "cu.usbmodem1101" for /dev/cu.usbmodem1101
pub fn short(path: &str) -> String {
    path.rsplit('/').next().unwrap_or(path).to_string()
}

/// The USB serial ports
pub fn ports() -> Vec<Port> {
    let Ok(found) = serialport::available_ports() else { return Vec::new() };
    let mut list: Vec<Port> = found
        .into_iter()
        .filter(|p| !p.port_name.starts_with("/dev/tty."))
        .filter_map(|p| match p.port_type {
            serialport::SerialPortType::UsbPort(usb) => Some(Port {
                path: p.port_name.clone(),
                dev: p.port_name,
                product: usb.product.unwrap_or_default(),
                maker: usb.manufacturer.unwrap_or_default(),
                ids: Some((usb.vid, usb.pid)),
                serial_number: usb.serial_number.unwrap_or_default(),
            }),
            // only USB: Bluetooth serial ports, the debug console... are no node
            _ => None,
        })
        .collect();
    list.sort_by(|a, b| a.dev.cmp(&b.dev));
    list
}
