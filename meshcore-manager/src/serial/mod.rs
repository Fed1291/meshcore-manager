//! USB serial ports: which ones are there (per system: list_windows.rs / list_linux.rs /
//! list_other.rs for macOS and the rest) and an
//! open port (`Link`, the same on every system: the serialport crate, blocking, 115200 baud).
//! Ports are opened by the worker's scan and by its jobs, each on a board it claimed first
//! (src/jobs.rs), so two jobs never talk to one node at once. The system refuses a second open of
//! a port anyway (exclusive on Windows and, with serialport's TIOCEXCL, on Linux).

use std::io::{self, Read, Write};
use std::time::{Duration, Instant};

use anyhow::{anyhow, Result};

use crate::ble;
use crate::devices;
use crate::i18n::tf;

#[cfg_attr(windows, path = "list_windows.rs")]
#[cfg_attr(target_os = "linux", path = "list_linux.rs")]
#[cfg_attr(not(any(windows, target_os = "linux")), path = "list_other.rs")]
mod list;

/// The USB serial ports (`ports`), a port's short name (`short`)
pub use list::{ports, short};

/// A read gives up after this long with nothing received (callers loop until their own deadline)
const READ_SLICE: Duration = Duration::from_millis(100);
/// An open is tried this many times: right after another handle closed it, Windows can still
/// refuse a port for a moment ("access denied"); a board that is restarting needs a moment too
const OPEN_TRIES: u32 = 3;
const OPEN_PAUSE: Duration = Duration::from_millis(300);
/// The USB serial numbers shorter than this are not a board's own (some USB-serial chips give
/// "0001" to every board): then the port tells boards apart
const OWN_SERIAL: usize = 8;

/// A USB serial port (Windows: COM, Linux: ttyACM / ttyUSB)
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Port {
    /// What to open: on Linux the /dev/serial/by-id/... name when there is one
    pub path: String,
    /// COM3, /dev/ttyACM0
    pub dev: String,
    /// As the USB device names itself ("" when it does not)
    pub product: String,
    pub maker: String,
    /// USB vendor and product id, when known
    pub ids: Option<(u16, u16)>,
    /// USB serial number ("" when the device has none). On boards with built-in USB it is the chip's
    /// own unique number: the same board always shows the same one. Seen on nRF52840 boards: 16 hex
    /// digits (the chip's device id, not its Bluetooth address), different for each board. On
    /// Windows it belongs to the USB device above the COM port (the port is one interface of it).
    /// ESP32 boards with built-in USB are said to give their MAC address: not checked yet.
    pub serial_number: String,
}

impl Port {
    /// What its USB id usually is (a LoRa board family, a USB-serial chip, a bootloader)
    pub fn board(&self) -> Option<&'static devices::Usb> {
        self.ids.and_then(|(v, p)| devices::by_usb(v, p))
    }

    /// "303a:1001", or "-"
    pub fn usb_id(&self) -> String {
        self.ids.map(|(v, p)| format!("{v:04x}:{p:04x}")).unwrap_or_else(|| "-".into())
    }

    /// How the USB device names itself, else the short port name
    pub fn label(&self) -> String {
        let mut parts = Vec::new();
        if !self.maker.is_empty() && !self.product.contains(&self.maker) {
            parts.push(self.maker.as_str());
        }
        if !self.product.is_empty() {
            parts.push(self.product.as_str());
        }
        if parts.is_empty() {
            short(&self.dev)
        } else {
            parts.join(" ")
        }
    }

    /// The same board: by its USB serial number when both have a real one (a board found again on
    /// another port after a restart), else by the port
    pub fn same_board(&self, other: &Port) -> bool {
        if self.serial_number.len() >= OWN_SERIAL && other.serial_number.len() >= OWN_SERIAL {
            return self.serial_number == other.serial_number;
        }
        self.path == other.path
    }
}

impl Port {
    /// A companion heard over Bluetooth: its path is `ble:<address>` (ble.rs)
    pub fn ble(seen: &ble::Seen) -> Port {
        Port {
            path: format!("{}{}", ble::PREFIX, seen.address),
            dev: seen.address.clone(),
            product: seen.name.clone(),
            maker: String::new(),
            ids: None,
            serial_number: String::new(),
        }
    }

    /// On Bluetooth (else USB)
    pub fn is_ble(&self) -> bool {
        self.path.starts_with(ble::PREFIX)
    }

    /// "COM3", "ttyACM0", or the Bluetooth address
    pub fn name(&self) -> String {
        if self.is_ble() {
            self.dev.clone()
        } else {
            short(&self.dev)
        }
    }
}

/// An open link to a node: a USB serial port (raw bytes at 115200), or a Bluetooth connection
/// (one companion frame per write and per notification)
pub enum Link {
    Serial(Box<dyn serialport::SerialPort>),
    Ble(ble::Conn),
}

/// The PIN Bluetooth pairing uses (`[bluetooth] pin`, set at start and by the Settings page)
static PIN: std::sync::RwLock<String> = std::sync::RwLock::new(String::new());

pub fn set_pin(pin: &str) {
    *PIN.write().unwrap_or_else(|p| p.into_inner()) = pin.to_string();
}

impl Link {
    /// Opens `path`: a serial port, or `ble:<address>` (connects, pairing first when needed)
    pub fn open(path: &str) -> Result<Link> {
        if let Some(address) = path.strip_prefix(ble::PREFIX) {
            // the PINs to try, in order: the one given this session, the one it last connected
            // with, the one from Settings (MeshCore's 123456 by default). A refused one goes on
            // to the next; the last refusal is left for the worker, which then asks the user.
            // The log says which, never the PIN.
            let settings = PIN.read().map(|p| p.clone()).unwrap_or_default();
            let settings = if settings.is_empty() { ble::DEFAULT_PIN.to_string() } else { settings };
            let mut tries: Vec<(&str, String)> = Vec::new();
            for (from, pin) in [("given for it", ble::given_pin(address)), ("saved", ble::saved_pin(address)), ("from settings", Some(settings))] {
                if let Some(pin) = pin.filter(|p| !tries.iter().any(|(_, t)| t == p)) {
                    tries.push((from, pin));
                }
            }
            let last = tries.len() - 1;
            for (i, (from, pin)) in tries.iter().enumerate() {
                crate::debug_log!("DEBUG", "open bluetooth {address}, pin {from}");
                match ble::Conn::open(address, pin) {
                    Ok(conn) => {
                        ble::save_pin(address, pin);
                        return Ok(Link::Ble(conn));
                    }
                    // refused: the next PIN, if there is one
                    Err(_) if i < last && ble::take_refused(address) => continue,
                    Err(e) => {
                        crate::debug_log!("ERROR", "bluetooth {address}: {e:#}");
                        return Err(e);
                    }
                }
            }
        }
        Self::open_serial(path)
    }

    fn open_serial(path: &str) -> Result<Link> {
        let fail = |e: &dyn std::fmt::Display| {
            anyhow!(tf("serial-open-failed", &[("port", short(path).into()), ("error", e.to_string().into())]))
        };
        let mut tries = 0;
        let mut port = loop {
            tries += 1;
            match serialport::new(path, 115_200).timeout(READ_SLICE).open() {
                Ok(port) => break port,
                Err(e) if tries >= OPEN_TRIES => {
                    crate::debug_log!("ERROR", "open {} failed ({tries} tries): {e}", short(path));
                    return Err(fail(&e));
                }
                Err(e) => {
                    crate::debug_log!("WARN", "open {} refused, trying again: {e}", short(path));
                    std::thread::sleep(OPEN_PAUSE)
                }
            }
        };
        // DTR and RTS on, like a terminal program: a board with built-in USB sends only with DTR on
        port.write_data_terminal_ready(true).map_err(|e| fail(&e))?;
        port.write_request_to_send(true).map_err(|e| fail(&e))?;
        // whatever the board printed before
        let _ = port.clear(serialport::ClearBuffer::All);
        crate::debug_log!("DEBUG", "open {}", short(path));
        Ok(Link::Serial(port))
    }

    /// Frames go whole (Bluetooth), not as bytes to frame (USB)
    pub fn carries_frames(&self) -> bool {
        match self {
            Link::Ble(_) => true,
            Link::Serial(_) => false,
        }
    }

    /// Bytes on a serial port. On Bluetooth: one whole frame.
    pub fn write(&mut self, data: &[u8]) -> io::Result<()> {
        match self {
            Link::Serial(port) => {
                port.write_all(data)?;
                port.flush()
            }
            Link::Ble(conn) => conn.write(data).map_err(io::Error::other),
        }
    }

    /// Some bytes, or 0 when nothing came within READ_SLICE. An unplugged port is an error.
    /// Bluetooth carries frames, not bytes (`read_frame`): the text console is not there.
    pub fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let port = match self {
            Link::Serial(port) => port,
            Link::Ble(_) => return Err(io::Error::new(io::ErrorKind::Unsupported, "bluetooth carries frames")),
        };
        match port.read(buf) {
            Ok(n) => Ok(n),
            Err(e) if e.kind() == io::ErrorKind::TimedOut => Ok(0),
            Err(e) => {
                crate::debug_log!("ERROR", "read failed (unplugged?): {e}");
                Err(e)
            }
        }
    }

    /// Bluetooth: the next frame, None when none came before `deadline`; a lost link is an error
    pub fn read_frame(&mut self, deadline: Instant) -> Result<Option<Vec<u8>>> {
        match self {
            Link::Ble(conn) => conn.recv(deadline),
            Link::Serial(_) => Err(anyhow!("a serial port carries bytes")),
        }
    }
}
