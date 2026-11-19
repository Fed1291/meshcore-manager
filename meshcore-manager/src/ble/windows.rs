//! Bluetooth on Windows: the system's own Bluetooth LE API (WinRT, `windows` crate 0.62), with no
//! driver or program to install. Windows 10 or newer with a Bluetooth 4.0+ adapter.
//! The calls are asynchronous in WinRT: each is waited for on a thread of its own with a time
//! limit (`wait`), so a link that went away never hangs a job.

use std::sync::mpsc as std_mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Result};
use windows::core::{GUID, HSTRING};
use windows::Devices::Bluetooth::Advertisement::{
    BluetoothLEAdvertisementReceivedEventArgs, BluetoothLEAdvertisementWatcher, BluetoothLEAdvertisementWatcherStatus,
    BluetoothLEAdvertisementWatcherStoppedEventArgs, BluetoothLEScanningMode,
};
use windows::Devices::Bluetooth::GenericAttributeProfile::{
    GattCharacteristic, GattClientCharacteristicConfigurationDescriptorValue, GattCommunicationStatus, GattDeviceService,
    GattValueChangedEventArgs, GattWriteOption,
};
use windows::Devices::Bluetooth::{
    BluetoothAdapter, BluetoothAddressType, BluetoothCacheMode, BluetoothConnectionStatus, BluetoothError, BluetoothLEDevice,
};
use windows::Devices::Radios::RadioState;
use windows::Devices::Enumeration::{
    DeviceInformationCustomPairing, DevicePairingKinds, DevicePairingRequestedEventArgs, DevicePairingResultStatus,
};
use windows::Foundation::TypedEventHandler;
use windows::Storage::Streams::{DataReader, DataWriter};

use super::{next, BondLost, Event, Heard, CONNECT_WAIT, NUS_RX, NUS_SERVICE, NUS_TX, RETRY, WRITE_WAIT};
use crate::i18n::{t, tf};

/// 0xAABBCCDDEEFF -> "AA:BB:CC:DD:EE:FF"
fn format_address(address: u64) -> String {
    (0..6).rev().map(|i| format!("{:02X}", (address >> (i * 8)) & 0xff)).collect::<Vec<_>>().join(":")
}

/// "AA:BB:CC:DD:EE:FF" -> 0xAABBCCDDEEFF
fn parse_address(text: &str) -> Option<u64> {
    let parts: Vec<&str> = text.trim().split(':').collect();
    if parts.len() != 6 {
        return None;
    }
    parts.iter().try_fold(0u64, |acc, p| Some((acc << 8) | u8::from_str_radix(p, 16).ok()? as u64))
}

/// Runs one WinRT call (started and waited for in `call`) on a thread of its own, `limit` at
/// most: a call that does not come back (the device went away) never blocks the job
fn wait<T: Send + 'static>(limit: Duration, call: impl FnOnce() -> windows::core::Result<T> + Send + 'static) -> Result<T> {
    let (done, result) = std_mpsc::channel();
    std::thread::Builder::new().name("mcmanager-ble-call".into()).spawn(move || {
        let _ = done.send(call());
    })?;
    match result.recv_timeout(limit) {
        Ok(Ok(value)) => Ok(value),
        Ok(Err(e)) => Err(anyhow!(e.message())),
        Err(_) => Err(anyhow!(t("ble-call-timeout"))),
    }
}

/// How long a scan may take to start (the radio waking up)
const START_WAIT: Duration = Duration::from_secs(5);
/// How often a running scan is checked
const CHECK: Duration = Duration::from_secs(1);

/// Why Windows stopped a scan, said for people (the `Stopped` event's error)
fn why(error: Option<BluetoothError>) -> String {
    match error {
        Some(e) if e == BluetoothError::RadioNotAvailable => t("ble-off"),
        Some(e) if e == BluetoothError::DisabledByUser || e == BluetoothError::ConsentRequired => t("ble-blocked"),
        Some(e) if e == BluetoothError::DisabledByPolicy => t("ble-policy"),
        Some(e) if e == BluetoothError::ResourceInUse => t("ble-busy"),
        Some(e) if e == BluetoothError::NotSupported || e == BluetoothError::TransportNotSupported => t("ble-no-le"),
        _ => t("ble-scan-stopped"),
    }
}

/// Starts the watcher thread: an active scan (names come in scan responses), each companion
/// heard handed to `heard`. Started again when Windows stops it (Bluetooth switched off, the
/// adapter removed), every RETRY; why it stopped is kept for the Fleet page and the log.
pub(super) fn watch(heard: Heard) {
    std::thread::Builder::new()
        .name("mcmanager-ble-watch".into())
        .spawn(move || loop {
            // the adapter first: none, no Bluetooth LE, switched off. Asked again every RETRY,
            // so a dongle plugged in or Bluetooth switched on is found by itself
            if let Err(problem) = adapter_ready() {
                heard.set_problem(Some(problem));
                std::thread::sleep(RETRY);
                continue;
            }
            let stopped: Arc<Mutex<Option<BluetoothError>>> = Arc::default();
            match start(&heard, &stopped) {
                Ok(watcher) => {
                    crate::debug_log!("INFO", "bluetooth watch: started (active scan)");
                    heard.set_problem(None);
                    while watcher.Status().is_ok_and(|s| s == BluetoothLEAdvertisementWatcherStatus::Started) {
                        std::thread::sleep(CHECK);
                    }
                    // read before our own Stop, which reports "no error"
                    let error = *stopped.lock().unwrap_or_else(|p| p.into_inner());
                    let status = watcher.Status().map(|s| s.0).unwrap_or(-1);
                    crate::debug_log!("WARN", "bluetooth watch: stopped, status {status}, error {}", error.map_or(-1, |e| e.0));
                    let _ = watcher.Stop();
                    heard.set_problem(Some(why(error)));
                }
                Err(problem) => {
                    crate::debug_log!("WARN", "bluetooth watch: not started: {problem}");
                    heard.set_problem(Some(problem));
                }
            }
            std::thread::sleep(RETRY);
        })
        .ok();
}

/// Whether the computer can scan: Windows' default Bluetooth adapter (built in or a dongle),
/// with Bluetooth LE, its radio on. Err: why not, said for people (and in the debug log).
fn adapter_ready() -> Result<(), String> {
    let adapter = match wait(CALL_WAIT, || BluetoothAdapter::GetDefaultAsync()?.join()) {
        Ok(adapter) => adapter,
        // none (Windows answers "nothing"), or the Bluetooth service not there
        Err(e) => {
            crate::debug_log!("INFO", "bluetooth: no adapter ({e})");
            return Err(t("ble-no-adapter"));
        }
    };
    if !adapter.IsLowEnergySupported().unwrap_or(false) {
        crate::debug_log!("INFO", "bluetooth: the adapter has no Bluetooth LE");
        return Err(t("ble-no-le"));
    }
    // its radio: a radio Windows cannot tell about is tried anyway (the scan then says)
    let state = wait(CALL_WAIT, move || adapter.GetRadioAsync()?.join()).ok().and_then(|radio| radio.State().ok());
    crate::debug_log!("DEBUG", "bluetooth: adapter found, radio state {}", state.map_or(-1, |s| s.0));
    match state {
        Some(s) if s == RadioState::Off => Err(t("ble-radio-off")),
        Some(s) if s == RadioState::Disabled => Err(t("ble-radio-disabled")),
        _ => Ok(()),
    }
}

/// How long an adapter question may take
const CALL_WAIT: Duration = Duration::from_secs(10);

/// A started scan, or why there is none. The status stays "created" for a moment after
/// `Start` (the radio starting; there is no event for it): waited for, as the Bleak library
/// does, then "aborted" = Windows refused (the reason from `Stopped`).
fn start(heard: &Heard, stopped: &Arc<Mutex<Option<BluetoothError>>>) -> Result<BluetoothLEAdvertisementWatcher, String> {
    let no_adapter = |e: windows::core::Error| {
        crate::debug_log!("WARN", "bluetooth watch: {}", e.code().0);
        t("ble-no-adapter")
    };
    let watcher = make(heard, stopped).map_err(no_adapter)?;
    watcher.Start().map_err(no_adapter)?;
    let began = Instant::now();
    while watcher.Status().is_ok_and(|s| s == BluetoothLEAdvertisementWatcherStatus::Created) && began.elapsed() < START_WAIT {
        std::thread::sleep(Duration::from_millis(10));
    }
    let status = watcher.Status();
    crate::debug_log!(
        "DEBUG",
        "bluetooth watch: status {} after {} ms",
        status.as_ref().map_or(-1, |s| s.0),
        began.elapsed().as_millis()
    );
    match status {
        Ok(s) if s == BluetoothLEAdvertisementWatcherStatus::Started => Ok(watcher),
        _ => {
            let error = *stopped.lock().unwrap_or_else(|p| p.into_inner());
            crate::debug_log!("WARN", "bluetooth watch: refused, error {}", error.map_or(-1, |e| e.0));
            let _ = watcher.Stop();
            Err(match error {
                None => t("ble-scan-failed"),
                some => why(some),
            })
        }
    }
}

/// The watcher with its two handlers: each advert heard, and why it stopped
fn make(heard: &Heard, stopped: &Arc<Mutex<Option<BluetoothError>>>) -> windows::core::Result<BluetoothLEAdvertisementWatcher> {
    let watcher = BluetoothLEAdvertisementWatcher::new()?;
    watcher.SetScanningMode(BluetoothLEScanningMode::Active)?;
    let slot = stopped.clone();
    let on_stop = TypedEventHandler::<BluetoothLEAdvertisementWatcher, BluetoothLEAdvertisementWatcherStoppedEventArgs>::new(
        move |_, args| {
            if let Ok(args) = args.ok() {
                *slot.lock().unwrap_or_else(|p| p.into_inner()) = args.Error().ok();
            }
            Ok(())
        },
    );
    watcher.Stopped(&on_stop)?;
    let heard = heard.clone();
    let handler = TypedEventHandler::<BluetoothLEAdvertisementWatcher, BluetoothLEAdvertisementReceivedEventArgs>::new(
        move |_, args| {
            let Ok(args) = args.ok() else { return Ok(()) };
            let number = args.BluetoothAddress()?;
            // public or random (MeshCore's nRF52 boards use a random one): needed to connect
            if let Ok(kind) = args.BluetoothAddressType() {
                remember_type(number, kind);
            }
            let address = format_address(number);
            let rssi = args.RawSignalStrengthInDBm().ok();
            let advert = args.Advertisement()?;
            let name = advert.LocalName().ok().map(|n| n.to_string());
            let nus = advert.ServiceUuids().is_ok_and(|list| {
                (0..list.Size().unwrap_or(0)).any(|i| list.GetAt(i).is_ok_and(|g| g == GUID::from_u128(NUS_SERVICE)))
            });
            heard.hear(address, name, rssi, nus);
            Ok(())
        },
    );
    watcher.Received(&handler)?;
    Ok(watcher)
}

/// The address type each companion advertised with (public / random), by address
static TYPES: Mutex<Vec<(u64, BluetoothAddressType)>> = Mutex::new(Vec::new());

fn remember_type(number: u64, kind: BluetoothAddressType) {
    let mut types = TYPES.lock().unwrap_or_else(|p| p.into_inner());
    if !types.iter().any(|(n, k)| *n == number && *k == kind) {
        types.retain(|(n, _)| *n != number);
        types.push((number, kind));
    }
}

/// The companion at `number`, opened with the address type it advertised with when known (a
/// random address opened as public is another device to Windows), as the bluest crate does
fn device(number: u64) -> windows::core::Result<BluetoothLEDevice> {
    let kind = TYPES.lock().unwrap_or_else(|p| p.into_inner()).iter().find(|(n, _)| *n == number).map(|(_, k)| *k);
    crate::debug_log!("DEBUG", "bluetooth: open device, address type {}", kind.map_or(-1, |k| k.0));
    match kind {
        Some(kind) => BluetoothLEDevice::FromBluetoothAddressWithBluetoothAddressTypeAsync(number, kind)?.join(),
        None => BluetoothLEDevice::FromBluetoothAddressAsync(number)?.join(),
    }
}

/// Pairs with `pin` when Windows has not paired with it yet (the companion asks for its PIN).
/// Ok(true): it was paired already. A wrong PIN is noted for the worker (`refused`).
fn pair(device: &BluetoothLEDevice, pin: &str, address: &str) -> Result<bool> {
    let pairing = device.DeviceInformation()?.Pairing()?;
    if pairing.IsPaired()? {
        return Ok(true);
    }
    let custom = pairing.Custom()?;
    let code = HSTRING::from(pin);
    // answered under a deferral, completed after the answer (as the bluest crate does): Windows
    // waits for it instead of taking the handler's return as "no answer"
    let handler = TypedEventHandler::<DeviceInformationCustomPairing, DevicePairingRequestedEventArgs>::new(move |_, args| {
        let Ok(args) = args.ok() else { return Ok(()) };
        let deferral = args.GetDeferral()?;
        let kind = args.PairingKind()?;
        crate::debug_log!("DEBUG", "bluetooth pairing: the companion asks kind {}", kind.0);
        let answered = if kind == DevicePairingKinds::ProvidePin {
            args.AcceptWithPin(&code)
        } else {
            // "confirm only", "do the numbers match", "show this PIN": yes
            args.Accept()
        };
        deferral.Complete()?;
        answered
    });
    let token = custom.PairingRequested(&handler)?;
    // every kind a keyboard-and-screen computer can do (bluest's KeyboardDisplay)
    let kinds = DevicePairingKinds::ConfirmOnly
        | DevicePairingKinds::DisplayPin
        | DevicePairingKinds::ProvidePin
        | DevicePairingKinds::ConfirmPinMatch;
    let asked = custom.clone();
    let result = wait(CONNECT_WAIT, move || asked.PairAsync(kinds)?.join());
    let _ = custom.RemovePairingRequested(token);
    let status = result?.Status()?;
    crate::debug_log!("INFO", "bluetooth pairing: result {}", status.0);
    if status == DevicePairingResultStatus::Paired || status == DevicePairingResultStatus::AlreadyPaired {
        return Ok(false);
    }
    // Windows can answer "failed" while the pairing went through (seen: the companion paired, the
    // job said "PIN refused"): asked again after a moment, and taken when it is paired
    for _ in 0..4 {
        std::thread::sleep(Duration::from_millis(500));
        if pairing.IsPaired().unwrap_or(false) {
            crate::debug_log!("INFO", "bluetooth pairing: result {} but paired after all", status.0);
            return Ok(false);
        }
    }
    // the companion refused the PIN (or did not get one in time). Windows also answers a plain
    // "failed" (19) to a wrong PIN (seen with a MeshCore companion): asked as a PIN too
    if status == DevicePairingResultStatus::AuthenticationFailure
        || status == DevicePairingResultStatus::AuthenticationTimeout
        || status == DevicePairingResultStatus::Failed
    {
        super::refused(address);
        bail!(t("ble-pin-refused"));
    }
    bail!(tf("ble-pair-failed", &[("error", format!("{}", status.0).into())]))
}

/// Unpairs this computer from the companion at `address` (Windows forgets it and its keys);
/// it is paired again on its next use
pub fn forget(address: &str) -> Result<()> {
    let number = parse_address(address).ok_or_else(|| anyhow!(tf("ble-bad-address", &[("address", address.into())])))?;
    unpair(number)?;
    crate::debug_log!("INFO", "bluetooth: unpaired");
    Ok(())
}

/// Removes Windows' pairing with the companion at `number` (it no longer has its side)
fn unpair(number: u64) -> Result<()> {
    let device = wait(CONNECT_WAIT, move || device(number))?;
    let pairing = device.DeviceInformation()?.Pairing()?;
    wait(CONNECT_WAIT, move || pairing.UnpairAsync()?.join())?;
    let _ = device.Close();
    Ok(())
}

/// The characteristic `uuid` of the companion's service, read from the device (not a cache)
fn characteristic(service: &GattDeviceService, uuid: u128) -> Result<GattCharacteristic> {
    let service = service.clone();
    let result = wait(CONNECT_WAIT, move || {
        service.GetCharacteristicsForUuidWithCacheModeAsync(GUID::from_u128(uuid), BluetoothCacheMode::Uncached)?.join()
    })?;
    if result.Status()? != GattCommunicationStatus::Success {
        bail!(t("ble-no-service"));
    }
    let list = result.Characteristics()?;
    if list.Size()? == 0 {
        bail!(t("ble-no-service"));
    }
    Ok(list.GetAt(0)?)
}

/// One connection to a companion. Dropped: its handlers removed and the device let go (Windows
/// then disconnects, and the companion advertises again for the phone).
pub struct Conn {
    device: BluetoothLEDevice,
    service: GattDeviceService,
    rx: GattCharacteristic,
    tx: GattCharacteristic,
    value_token: i64,
    status_token: i64,
    events: std_mpsc::Receiver<Event>,
}

impl Conn {
    /// Finds the companion, pairs when needed, reads its service and subscribes to its frames.
    /// Windows' pairing refused by a companion that lost its side: removed, paired again, once.
    pub fn open(address: &str, pin: &str) -> Result<Conn> {
        let number = parse_address(address).ok_or_else(|| anyhow!(tf("ble-bad-address", &[("address", address.into())])))?;
        let pin = super::pin(pin).ok_or_else(|| anyhow!(t("ble-bad-pin")))?;
        match Conn::open_once(address, number, &pin) {
            Err(e) if e.is::<BondLost>() => {
                crate::debug_log!("INFO", "bluetooth: pairing lost on the companion, paired again");
                unpair(number)?;
                Conn::open_once(address, number, &pin)
            }
            other => other,
        }
    }

    fn open_once(address: &str, number: u64, pin: &str) -> Result<Conn> {
        let device = wait(CONNECT_WAIT, move || device(number)).map_err(|_| anyhow!(tf("ble-not-found", &[("address", address.into())])))?;
        let was_paired = pair(&device, pin, address)?;
        // refused although paired: the companion no longer has this pairing
        let lost = || if was_paired { anyhow::Error::new(BondLost) } else { anyhow!(t("ble-not-paired")) };
        let asked = device.clone();
        let services = wait(CONNECT_WAIT, move || {
            asked.GetGattServicesForUuidWithCacheModeAsync(GUID::from_u128(NUS_SERVICE), BluetoothCacheMode::Uncached)?.join()
        })?;
        match services.Status()? {
            s if s == GattCommunicationStatus::Success => {}
            // out of range, off, or connected to something else
            s if s == GattCommunicationStatus::Unreachable => bail!(tf("ble-not-found", &[("address", address.into())])),
            s if s == GattCommunicationStatus::AccessDenied => return Err(lost()),
            s => bail!(tf("ble-connect-failed", &[("error", format!("{}", s.0).into())])),
        }
        let list = services.Services()?;
        if list.Size()? == 0 {
            bail!(t("ble-no-service"));
        }
        let service = list.GetAt(0)?;
        let rx = characteristic(&service, NUS_RX)?;
        let tx = characteristic(&service, NUS_TX)?;

        let (tell, events) = std_mpsc::channel();
        let frames = tell.clone();
        let on_value = TypedEventHandler::<GattCharacteristic, GattValueChangedEventArgs>::new(move |_, args| {
            let Ok(args) = args.ok() else { return Ok(()) };
            let reader = DataReader::FromBuffer(&args.CharacteristicValue()?)?;
            let mut frame = vec![0u8; reader.UnconsumedBufferLength()? as usize];
            reader.ReadBytes(&mut frame)?;
            let _ = frames.send(Event::Frame(frame));
            Ok(())
        });
        let value_token = tx.ValueChanged(&on_value)?;
        let on_status = TypedEventHandler::<BluetoothLEDevice, windows::core::IInspectable>::new(move |device, _| {
            if let Ok(device) = device.ok() {
                if device.ConnectionStatus()? == BluetoothConnectionStatus::Disconnected {
                    let _ = tell.send(Event::Lost(t("ble-disconnected")));
                }
            }
            Ok(())
        });
        let status_token = device.ConnectionStatusChanged(&on_status)?;
        let conn = Conn { device, service, rx, tx, value_token, status_token, events };
        // notifications on: refused when the pairing is missing or was removed on one side
        let subscribe = conn.tx.clone();
        let written = wait(CONNECT_WAIT, move || {
            subscribe.WriteClientCharacteristicConfigurationDescriptorAsync(GattClientCharacteristicConfigurationDescriptorValue::Notify)?.join()
        })?;
        if written != GattCommunicationStatus::Success {
            return Err(lost());
        }
        Ok(conn)
    }

    pub fn write(&mut self, frame: &[u8]) -> Result<()> {
        let rx = self.rx.clone();
        let frame = frame.to_vec();
        let status = wait(WRITE_WAIT, move || {
            let writer = DataWriter::new()?;
            writer.WriteBytes(&frame)?;
            let buffer = writer.DetachBuffer()?;
            rx.WriteValueWithOptionAsync(&buffer, GattWriteOption::WriteWithResponse)?.join()
        })?;
        if status != GattCommunicationStatus::Success {
            bail!(tf("ble-write-failed", &[("error", format!("{}", status.0).into())]));
        }
        Ok(())
    }

    pub fn recv(&mut self, deadline: Instant) -> Result<Option<Vec<u8>>> {
        next(&self.events, deadline)
    }
}

impl Drop for Conn {
    fn drop(&mut self) {
        let _ = self.tx.RemoveValueChanged(self.value_token);
        let _ = self.device.RemoveConnectionStatusChanged(self.status_token);
        let _ = self.service.Close();
        let _ = self.device.Close();
    }
}
