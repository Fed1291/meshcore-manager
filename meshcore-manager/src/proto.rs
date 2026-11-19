//! The companion protocol over USB: frames, command / reply codes, and the two replies decoded so
//! far. The firmware is the reference (examples/companion_radio/MyMesh.cpp, docs/companion_protocol.md):
//! read it before adding a command, and add only the codes that are used.
//!
//! Framing on a serial port: to the node '<' + length (2 bytes, low first) + frame; from the node
//! '>' + length + frame.

/// The protocol version we say we understand (second byte of CMD_DEVICE_QUERY)
pub const APP_VERSION: u8 = 3;
/// How we name ourselves to the node (it only logs it)
pub const APP_NAME: &[u8] = b"mcmanager";

pub const CMD_APP_START: u8 = 1;
pub const CMD_GET_CONTACTS: u8 = 4;
pub const CMD_GET_DEVICE_TIME: u8 = 5;
/// [6, seconds since 1970 u32 LE]: taken only if not before the node's clock (else ERR)
pub const CMD_SET_DEVICE_TIME: u8 = 6;
/// [8, name (up to 31 bytes)]
pub const CMD_SET_ADVERT_NAME: u8 = 8;
/// A RESP_CONTACT frame with this code instead (updateContactFromFrame): adds or changes a contact
pub const CMD_ADD_UPDATE_CONTACT: u8 = 9;
/// [11, freq kHz (4), bandwidth Hz (4), sf, cr, client repeat (v9+, 0 when left out)]
pub const CMD_SET_RADIO_PARAMS: u8 = 11;
/// [12, dBm (signed)]
pub const CMD_SET_RADIO_TX_POWER: u8 = 12;
/// [14, lat * 1e6 (4), lon * 1e6 (4)]
pub const CMD_SET_ADVERT_LATLON: u8 = 14;
/// [15, public key (32)]
pub const CMD_REMOVE_CONTACT: u8 = 15;
pub const CMD_GET_BATT_AND_STORAGE: u8 = 20;
/// [21, rx delay base * 1000 (4), airtime factor * 1000 (4)]
pub const CMD_SET_TUNING_PARAMS: u8 = 21;
/// [38, manual add contacts, telemetry modes, advert location policy, multi acks]
pub const CMD_SET_OTHER_PARAMS: u8 = 38;
/// [61, 0, mode 0-2]
pub const CMD_SET_PATH_HASH_MODE: u8 = 61;
pub const CMD_DEVICE_QUERY: u8 = 22;
pub const CMD_EXPORT_PRIVATE_KEY: u8 = 23;
/// [24, private key (64)]: OK, ERR (not a valid key) or DISABLED (built without key import)
pub const CMD_IMPORT_PRIVATE_KEY: u8 = 24;
/// [19, "reboot"]
pub const CMD_REBOOT: u8 = 19;
pub const CMD_GET_CHANNEL: u8 = 31;
/// [37, pin (u32): 0 or 100000-999999]
pub const CMD_SET_DEVICE_PIN: u8 = 37;
/// [41, "name:value"]
pub const CMD_SET_CUSTOM_VAR: u8 = 41;
/// [58, auto-add flags, max hops]: the same bytes RESP_AUTOADD_CONFIG carries
pub const CMD_SET_AUTOADD_CONFIG: u8 = 58;
/// [63, name (31), key (16)], or [63] alone for none: the same bytes RESP_DEFAULT_FLOOD_SCOPE carries
pub const CMD_SET_DEFAULT_FLOOD_SCOPE: u8 = 63;
/// [32, slot, name (32), secret (16)]; an empty name and secret free the slot
pub const CMD_SET_CHANNEL: u8 = 32;
pub const CMD_GET_CUSTOM_VARS: u8 = 40;
pub const CMD_GET_TUNING_PARAMS: u8 = 43;
pub const CMD_GET_AUTOADD_CONFIG: u8 = 59;
pub const CMD_GET_DEFAULT_FLOOD_SCOPE: u8 = 64;
/// [39, 0, 0, 0] (exactly 4 bytes): the companion's own telemetry, answered by
/// PUSH_TELEMETRY_RESPONSE (with a public key after them it would ask another node)
pub const CMD_SEND_TELEMETRY_REQ: u8 = 39;
/// [56, type]: 0 core, 1 radio, 2 packets (firmware v8+), answered by RESP_STATS
pub const CMD_GET_STATS: u8 = 56;
/// [24, type, data...] (little-endian)
pub const RESP_STATS: u8 = 24;
/// [0x8B, reserved, public key prefix (6), LPP records]
pub const PUSH_TELEMETRY_RESPONSE: u8 = 0x8B;

/// The command was done (every SET answers OK or ERR)
pub const RESP_OK: u8 = 0;
pub const RESP_ERR: u8 = 1;
pub const RESP_CONTACT: u8 = 3;
/// [2, how many contacts follow (u32)]: the contact list starts
pub const RESP_CONTACTS_START: u8 = 2;
pub const RESP_END_OF_CONTACTS: u8 = 4;
pub const RESP_SELF_INFO: u8 = 5;
/// [9, seconds since 1970 (4 bytes)]
pub const RESP_CURR_TIME: u8 = 9;
/// [12, battery millivolts (2 bytes), storage used kB (4), storage total kB (4)]
pub const RESP_BATT_AND_STORAGE: u8 = 12;
pub const RESP_DEVICE_INFO: u8 = 13;
pub const RESP_PRIVATE_KEY: u8 = 14;
/// The firmware was built without that feature (private key export)
pub const RESP_DISABLED: u8 = 15;
pub const RESP_CHANNEL_INFO: u8 = 18;
pub const RESP_CUSTOM_VARS: u8 = 21;
pub const RESP_TUNING_PARAMS: u8 = 23;
pub const RESP_AUTOADD_CONFIG: u8 = 25;
pub const RESP_DEFAULT_FLOOD_SCOPE: u8 = 28;

// Codes from 0x80 up are pushed by the node at any time (adverts, messages waiting...)

/// The firmware's largest frame is 176 bytes (MAX_FRAME_SIZE); anything far above is noise
const MAX_FRAME: usize = 512;

/// A frame as it goes to the node
pub fn encode(frame: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(frame.len() + 3);
    out.push(b'<');
    out.extend_from_slice(&(frame.len() as u16).to_le_bytes());
    out.extend_from_slice(frame);
    out
}

/// CMD_APP_START: 7 reserved bytes, then the app's name
pub fn app_start() -> Vec<u8> {
    let mut f = vec![CMD_APP_START, 0, 0, 0, 0, 0, 0, 0];
    f.extend_from_slice(APP_NAME);
    f
}

/// Cuts what the node sends into frames. Bytes that are no frame (boot text, a half frame) are
/// skipped one at a time, so the frames around them are kept.
#[derive(Default)]
pub struct FrameParser {
    buf: Vec<u8>,
}

impl FrameParser {
    pub fn push(&mut self, data: &[u8]) -> Vec<Vec<u8>> {
        self.buf.extend_from_slice(data);
        let mut frames = Vec::new();
        loop {
            match self.buf.iter().position(|&b| b == b'>') {
                Some(start) => {
                    self.buf.drain(..start);
                }
                None => {
                    self.buf.clear();
                    break;
                }
            }
            if self.buf.len() < 3 {
                break;
            }
            let len = u16::from_le_bytes([self.buf[1], self.buf[2]]) as usize;
            if len == 0 || len > MAX_FRAME {
                self.buf.remove(0);
                continue;
            }
            if self.buf.len() < 3 + len {
                break;
            }
            frames.push(self.buf[3..3 + len].to_vec());
            self.buf.drain(..3 + len);
        }
        frames
    }
}

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Text in a fixed-size field: up to the first NUL
fn text(bytes: &[u8]) -> String {
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..end]).trim().to_string()
}

pub fn u32_at(f: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(f.get(at..at + 4)?.try_into().ok()?))
}

pub fn u16_at(f: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(f.get(at..at + 2)?.try_into().ok()?))
}

/// RESP_SELF_INFO, the reply to CMD_APP_START: who the node is and its radio settings
#[derive(Clone, Debug)]
pub struct SelfInfo {
    pub public_key: String,
    pub name: String,
    pub tx_power_dbm: i8,
    /// The most this board's radio can send with
    pub max_tx_power_dbm: u8,
    /// The position the node advertises, in degrees (0, 0 = not set)
    pub lat: f64,
    pub lon: f64,
    pub freq_mhz: f64,
    pub bandwidth_khz: f64,
    pub spreading_factor: u8,
    pub coding_rate: u8,
    /// The four "other params" (CMD_SET_OTHER_PARAMS sends them together)
    pub multi_acks: u8,
    /// 0 = adverts carry no position, 1 = they do
    pub advert_loc_policy: u8,
    /// base (bits 0-1), location (2-3), environment (4-5): 0 deny, 1 by contact flags, 2 everyone
    pub telemetry: u8,
    /// 1 = new contacts are added only by hand
    pub manual_add_contacts: u8,
}

impl SelfInfo {
    pub fn parse(f: &[u8]) -> Option<SelfInfo> {
        if f.first() != Some(&RESP_SELF_INFO) || f.len() < 58 {
            return None;
        }
        Some(SelfInfo {
            tx_power_dbm: f[2] as i8,
            max_tx_power_dbm: f[3],
            public_key: hex(&f[4..36]),
            lat: u32_at(f, 36)? as i32 as f64 / 1_000_000.0,
            lon: u32_at(f, 40)? as i32 as f64 / 1_000_000.0,
            freq_mhz: u32_at(f, 48)? as f64 / 1000.0,
            bandwidth_khz: u32_at(f, 52)? as f64 / 1000.0,
            spreading_factor: f[56],
            coding_rate: f[57],
            multi_acks: f[44],
            advert_loc_policy: f[45],
            telemetry: f[46],
            manual_add_contacts: f[47],
            name: String::from_utf8_lossy(&f[58..]).trim().to_string(),
        })
    }

    /// "869.618 MHz  BW 62.5  SF8  CR8  22 dBm"
    pub fn radio(&self) -> String {
        format!(
            "{:.3} MHz  BW {}  SF{}  CR{}  {} dBm",
            self.freq_mhz, self.bandwidth_khz, self.spreading_factor, self.coding_rate, self.tx_power_dbm
        )
    }
}

/// RESP_CONTACT, one entry of the node's contact list:
/// [3, key (32), type, flags, path length, path (64), name (32), last advert (4), lat (4), lon (4), last change (4)]
#[derive(Clone, Debug)]
pub struct Contact {
    pub public_key: String,
    /// What the contact advertises as: 1 chat, 2 repeater, 3 room, 4 sensor
    pub kind: u8,
    pub name: String,
    /// Marked as a favourite on the node (flags bit 0, ContactInfo::isFav)
    pub favourite: bool,
    /// Hops of the known route to it; None = no route known (messages to it are flooded)
    pub hops: Option<u8>,
    /// When its last advert was heard, seconds since 1970 (0 = never)
    pub last_advert: u32,
    pub lat: f64,
    pub lon: f64,
    /// The frame as the node sent it, without its last 4 bytes (last change): an edit sends it
    /// back changed, and the node then stamps the change with its own clock
    pub frame: Vec<u8>,
}

impl Contact {
    pub fn parse(f: &[u8]) -> Option<Contact> {
        if f.first() != Some(&RESP_CONTACT) || f.len() < 144 {
            return None;
        }
        Some(Contact {
            frame: f[..144].to_vec(),
            public_key: hex(&f[1..33]),
            kind: f[33],
            favourite: f[34] & 0x01 != 0,
            // 0xFF = no route known (OUT_PATH_UNKNOWN); else the low 6 bits are the hop count and
            // the top 2 the size of each hop's hash (Packet::getPathHashCount)
            hops: (f[35] != 0xFF).then_some(f[35] & 63),
            name: text(&f[100..132]),
            last_advert: u32_at(f, 132)?,
            lat: u32_at(f, 136)? as i32 as f64 / 1_000_000.0,
            lon: u32_at(f, 140)? as i32 as f64 / 1_000_000.0,
        })
    }
}

/// RESP_CHANNEL_INFO, one channel slot: [18, slot, name (32), key (16)]
#[derive(Clone)]
pub struct Channel {
    pub slot: u8,
    /// "" = the slot is free
    pub name: String,
    /// Kept only to send it back unchanged when the channel is renamed: never shown, never logged
    /// (no Debug, on purpose)
    pub secret: [u8; 16],
}

impl Channel {
    pub fn parse(f: &[u8]) -> Option<Channel> {
        if f.first() != Some(&RESP_CHANNEL_INFO) || f.len() < 3 {
            return None;
        }
        let mut secret = [0u8; 16];
        if let Some(key) = f.get(34..50) {
            secret.copy_from_slice(key);
        }
        Some(Channel { slot: f[1], name: text(&f[2..f.len().min(34)]), secret })
    }
}

impl std::fmt::Debug for Channel {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "Channel {{ slot: {}, name: {:?} }}", self.slot, self.name)
    }
}

/// A text in a fixed field of `size` bytes: cut at a character boundary to leave room for the NUL
fn fixed(text: &str, size: usize) -> Vec<u8> {
    let mut end = text.len().min(size - 1);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    let mut out = text.as_bytes()[..end].to_vec();
    out.resize(size, 0);
    out
}

/// The longest name a node keeps (32 bytes with its NUL)
pub const NAME_MAX: usize = 31;

pub fn set_advert_name(name: &str) -> Vec<u8> {
    let mut f = vec![CMD_SET_ADVERT_NAME];
    let mut bytes = fixed(name, NAME_MAX + 1);
    bytes.retain(|&b| b != 0);
    f.extend_from_slice(&bytes);
    f
}

pub fn set_advert_latlon(lat: f64, lon: f64) -> Vec<u8> {
    let mut f = vec![CMD_SET_ADVERT_LATLON];
    f.extend_from_slice(&((lat * 1_000_000.0).round() as i32).to_le_bytes());
    f.extend_from_slice(&((lon * 1_000_000.0).round() as i32).to_le_bytes());
    f
}

/// `repeat`: the client repeat setting to keep (None on a firmware that does not report it: the
/// byte is then left out, as that firmware expects)
pub fn set_radio_params(freq_khz: u32, bandwidth_hz: u32, sf: u8, cr: u8, repeat: Option<bool>) -> Vec<u8> {
    let mut f = vec![CMD_SET_RADIO_PARAMS];
    f.extend_from_slice(&freq_khz.to_le_bytes());
    f.extend_from_slice(&bandwidth_hz.to_le_bytes());
    f.push(sf);
    f.push(cr);
    if let Some(repeat) = repeat {
        f.push(repeat as u8);
    }
    f
}

pub fn set_tuning_params(rx_delay: f64, airtime_factor: f64) -> Vec<u8> {
    let mut f = vec![CMD_SET_TUNING_PARAMS];
    f.extend_from_slice(&((rx_delay * 1000.0).round() as u32).to_le_bytes());
    f.extend_from_slice(&((airtime_factor * 1000.0).round() as u32).to_le_bytes());
    f
}

pub fn set_other_params(manual_add_contacts: u8, telemetry: u8, advert_loc_policy: u8, multi_acks: u8) -> Vec<u8> {
    vec![CMD_SET_OTHER_PARAMS, manual_add_contacts, telemetry, advert_loc_policy, multi_acks]
}

/// A new identity for the companion (it takes it at once, and reloads its contacts)
pub fn import_private_key(key: &[u8; 64]) -> Vec<u8> {
    let mut f = vec![CMD_IMPORT_PRIVATE_KEY];
    f.extend_from_slice(key);
    f
}

pub fn set_path_hash_mode(mode: u8) -> Vec<u8> {
    vec![CMD_SET_PATH_HASH_MODE, 0, mode]
}

pub fn set_radio_tx_power(dbm: i8) -> Vec<u8> {
    vec![CMD_SET_RADIO_TX_POWER, dbm as u8]
}

/// The contact's own frame, with a new name and favourite mark
pub fn update_contact(contact: &Contact, name: &str, favourite: bool) -> Vec<u8> {
    let mut f = contact.frame.clone();
    f[0] = CMD_ADD_UPDATE_CONTACT;
    f[34] = (f[34] & !0x01) | favourite as u8;
    f[100..132].copy_from_slice(&fixed(name, 32));
    f
}

pub fn remove_contact(contact: &Contact) -> Vec<u8> {
    let mut f = vec![CMD_REMOVE_CONTACT];
    f.extend_from_slice(&contact.frame[1..33]);
    f
}

/// A frame the node sent, as the command that sets the same thing again: the firmware's GET and
/// SET of a contact, the auto-add config and the default flood scope carry the same bytes, only
/// the code differs
pub fn as_command(code: u8, reply: &[u8]) -> Vec<u8> {
    let mut f = reply.to_vec();
    if let Some(first) = f.first_mut() {
        *first = code;
    }
    f
}

pub fn set_device_pin(pin: u32) -> Vec<u8> {
    let mut f = vec![CMD_SET_DEVICE_PIN];
    f.extend_from_slice(&pin.to_le_bytes());
    f
}

/// One custom variable, "name:value"
pub fn set_custom_var(pair: &str) -> Vec<u8> {
    let mut f = vec![CMD_SET_CUSTOM_VAR];
    f.extend_from_slice(pair.as_bytes());
    f
}

pub fn reboot() -> Vec<u8> {
    let mut f = vec![CMD_REBOOT];
    f.extend_from_slice(b"reboot");
    f
}

/// `None` frees the slot (empty name, empty secret)
pub fn set_channel(slot: u8, name_and_secret: Option<(&str, &[u8; 16])>) -> Vec<u8> {
    let mut f = vec![CMD_SET_CHANNEL, slot];
    match name_and_secret {
        Some((name, secret)) => {
            f.extend_from_slice(&fixed(name, 32));
            f.extend_from_slice(secret);
        }
        None => f.resize(2 + 32 + 16, 0),
    }
    f
}

/// RESP_DEVICE_INFO, the reply to CMD_DEVICE_QUERY: the board and its firmware.
/// Older firmware sends a shorter frame: what is missing stays empty / 0.
pub struct DeviceInfo {
    /// How many contacts the firmware can hold
    pub max_contacts: u16,
    /// How many channel slots the firmware has
    pub max_channels: u8,
    /// The Bluetooth PIN (0: none set, the firmware's own)
    pub ble_pin: Option<u32>,
    pub build_date: String,
    /// The board, as the firmware names it ("Heltec V3")
    pub model: String,
    pub version: String,
    /// Client repeat on (firmware v9+; None when older)
    pub repeat: Option<bool>,
    /// Size of each hop's id in paths, 0-2 (firmware v10+)
    pub path_hash_mode: Option<u8>,
}

impl DeviceInfo {
    pub fn parse(f: &[u8]) -> Option<DeviceInfo> {
        if f.first() != Some(&RESP_DEVICE_INFO) || f.len() < 2 {
            return None;
        }
        let field = |from: usize, len: usize| f.get(from..(from + len).min(f.len())).map(text).unwrap_or_default();
        Some(DeviceInfo {
            // sent halved, to fit one byte
            max_contacts: f.get(2).copied().unwrap_or(0) as u16 * 2,
            max_channels: f.get(3).copied().unwrap_or(0),
            ble_pin: u32_at(f, 4),
            build_date: field(8, 12),
            model: field(20, 40),
            version: field(60, 20),
            repeat: f.get(80).map(|&b| b != 0),
            path_hash_mode: f.get(81).copied(),
        })
    }
}
