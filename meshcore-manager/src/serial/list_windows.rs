//! USB serial ports on Windows: the COM ports of USB devices, with their USB ids, names and
//! serial numbers (serialport crate).

use serialport::SerialPortType;

use super::Port;

/// "COM3" (a `\\.\COM3` name too)
pub fn short(path: &str) -> String {
    path.trim().trim_start_matches(r"\\.\").to_string()
}

/// The COM number, for a natural order (COM3 before COM10)
fn number(name: &str) -> u32 {
    name.trim_start_matches(|c: char| !c.is_ascii_digit()).parse().unwrap_or(u32::MAX)
}

/// Windows names a port after its driver: "USB Serial Device (COM13)" by "Microsoft". The port is
/// shown apart, and the generic driver says nothing about the board (its USB id does): left out.
fn device_name(product: Option<String>, maker: Option<String>, port: &str) -> (String, String) {
    let product = product.unwrap_or_default();
    let product = product.trim().trim_end_matches(&format!("({port})")).trim().to_string();
    let maker = maker.unwrap_or_default();
    if maker.eq_ignore_ascii_case("microsoft") {
        (String::new(), String::new())
    } else {
        (product, maker)
    }
}

/// The serial ports of USB devices
pub fn ports() -> Vec<Port> {
    let mut list: Vec<Port> = serialport::available_ports()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|p| match p.port_type {
            SerialPortType::UsbPort(u) => {
                let (product, maker) = device_name(u.product, u.manufacturer, &p.port_name);
                Some(Port {
                    path: p.port_name.clone(),
                    dev: p.port_name,
                    product,
                    maker,
                    ids: Some((u.vid, u.pid)),
                    serial_number: u.serial_number.unwrap_or_default().trim().to_string(),
                })
            }
            _ => None,
        })
        .collect();
    list.sort_by_key(|p| number(&p.dev));
    list
}
