//! Bluetooth on Linux: BlueZ through bluer (D-Bus), made
//! blocking for the worker's threads: each connection and the watcher run their own small tokio
//! runtime on a thread of their own, and talk to the rest through channels.
//! Needs bluetoothd running (every desktop Linux and Raspberry Pi OS have it).

use std::sync::mpsc as std_mpsc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Context, Result};
use bluer::agent::{Agent, AgentHandle};
use bluer::{Adapter, AdapterEvent, Address, DeviceEvent, DeviceProperty, Session, Uuid};
use futures::{pin_mut, StreamExt};
use tokio::sync::{mpsc, oneshot};
use tokio::time::{sleep, timeout};

use super::{next, BondLost, Event, Heard, CONNECT_WAIT, NUS_RX, NUS_SERVICE, NUS_TX, RETRY, WRITE_WAIT};
use crate::i18n::{t, tf};

fn runtime() -> std::io::Result<tokio::runtime::Runtime> {
    tokio::runtime::Builder::new_current_thread().enable_all().build()
}

/// Unpairs this computer from the companion at `address` (BlueZ forgets the device and its
/// keys); the companion is found again by the watcher and paired again on its next use
pub fn forget(address: &str) -> Result<()> {
    let address: Address = address.parse().map_err(|_| anyhow!(tf("ble-bad-address", &[("address", address.into())])))?;
    let rt = runtime()?;
    rt.block_on(async move {
        let session = Session::new().await?;
        adapter(&session).await?.remove_device(address).await?;
        crate::debug_log!("INFO", "bluetooth: unpaired");
        Ok::<(), anyhow::Error>(())
    })
}

/// The default adapter, switched on when it is off (rfkill aside)
async fn adapter(session: &Session) -> Result<Adapter> {
    let adapter = session.default_adapter().await.map_err(|_| anyhow!(t("ble-no-adapter")))?;
    if !adapter.is_powered().await.unwrap_or(false) {
        adapter.set_powered(true).await.map_err(|_| anyhow!(t("ble-off")))?;
    }
    Ok(adapter)
}

/// Starts the watcher thread: discovery on the default adapter, every device heard (and heard
/// again: `discover_devices_with_changes`) handed to `heard`. Starts over after any problem
/// (bluetoothd restarted, adapter unplugged, Bluetooth switched off), every RETRY.
pub(super) fn watch(heard: Heard) {
    std::thread::Builder::new()
        .name("mcmanager-ble-watch".into())
        .spawn(move || {
            let Ok(rt) = runtime() else { return };
            rt.block_on(async move {
                loop {
                    if let Err(e) = listen(&heard).await {
                        crate::debug_log!("WARN", "bluetooth watch: {e:#}");
                        heard.set_problem(Some(format!("{e:#}")));
                    }
                    sleep(RETRY).await;
                }
            });
        })
        .ok();
}

async fn listen(heard: &Heard) -> Result<()> {
    let session = Session::new().await.map_err(|_| anyhow!(t("ble-no-bluez")))?;
    let adapter = adapter(&session).await?;
    let events = adapter.discover_devices_with_changes().await.context(t("ble-scan-failed"))?;
    crate::debug_log!("INFO", "bluetooth watch: started on {}", adapter.name());
    heard.set_problem(None);
    pin_mut!(events);
    while let Some(event) = events.next().await {
        let AdapterEvent::DeviceAdded(address) = event else { continue };
        let Ok(device) = adapter.device(address) else { continue };
        let name = device.name().await.ok().flatten();
        let nus = device.uuids().await.ok().flatten().is_some_and(|u| u.contains(&Uuid::from_u128(NUS_SERVICE)));
        let rssi = device.rssi().await.ok().flatten();
        heard.hear(address.to_string(), name, rssi, nus);
    }
    bail!(t("ble-scan-stopped"))
}

/// Answers BlueZ's PIN questions with `pin` while pairing (passkey or PIN code), and confirms
/// the companion's own number (a companion with a screen)
async fn agent(session: &Session, pin: &str) -> Result<AgentHandle> {
    let passkey: u32 = pin.parse().map_err(|_| anyhow!(t("ble-bad-pin")))?;
    let code = pin.to_string();
    let agent = Agent {
        request_default: true,
        request_passkey: Some(Box::new(move |_| Box::pin(async move { Ok(passkey) }))),
        request_pin_code: Some(Box::new(move |_| {
            let code = code.clone();
            Box::pin(async move { Ok(code) })
        })),
        request_confirmation: Some(Box::new(|_| Box::pin(async { Ok(()) }))),
        ..Default::default()
    };
    Ok(session.register_agent(agent).await?)
}

/// What the job asks the connection thread
enum Ask {
    Write(Vec<u8>, oneshot::Sender<Result<()>>),
}

/// One connection to a companion, on a thread of its own. Dropped: disconnected.
pub struct Conn {
    asks: Option<mpsc::UnboundedSender<Ask>>,
    events: std_mpsc::Receiver<Event>,
    thread: Option<JoinHandle<()>>,
}

impl Conn {
    /// Connects (pairing first when needed) and subscribes to the companion's frames. Waits until
    /// that is done or failed (CONNECT_WAIT at most).
    pub fn open(address: &str, pin: &str) -> Result<Conn> {
        let address: Address = address.parse().map_err(|_| anyhow!(tf("ble-bad-address", &[("address", address.into())])))?;
        let pin = super::pin(pin).ok_or_else(|| anyhow!(t("ble-bad-pin")))?;
        let (asks, asked) = mpsc::unbounded_channel();
        let (tell, events) = std_mpsc::channel();
        let (ready_tx, ready) = std_mpsc::channel();
        let thread = std::thread::Builder::new().name("mcmanager-ble-link".into()).spawn(move || {
            let rt = match runtime() {
                Ok(rt) => rt,
                Err(e) => {
                    let _ = ready_tx.send(Err(anyhow!(e)));
                    return;
                }
            };
            rt.block_on(run(address, pin, asked, tell, ready_tx));
        })?;
        let conn = Conn { asks: Some(asks), events, thread: Some(thread) };
        match ready.recv_timeout(CONNECT_WAIT + Duration::from_secs(5)) {
            Ok(Ok(())) => Ok(conn),
            Ok(Err(e)) => Err(e),
            Err(_) => Err(anyhow!(t("ble-connect-timeout"))),
        }
    }

    pub fn write(&mut self, frame: &[u8]) -> Result<()> {
        let (reply_tx, reply) = oneshot::channel();
        let asks = self.asks.as_ref().ok_or_else(|| anyhow!(t("ble-closed")))?;
        asks.send(Ask::Write(frame.to_vec(), reply_tx)).map_err(|_| anyhow!(t("ble-closed")))?;
        // the connection thread answers within WRITE_WAIT (its own time limit)
        reply.blocking_recv().map_err(|_| anyhow!(t("ble-closed")))?
    }

    pub fn recv(&mut self, deadline: Instant) -> Result<Option<Vec<u8>>> {
        next(&self.events, deadline)
    }
}

impl Drop for Conn {
    /// Closing the channel ends the connection thread, which disconnects
    fn drop(&mut self) {
        self.asks = None;
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// The connection thread: connect, then pass frames both ways until the job is done or the
/// link is lost; always disconnects at the end (a companion stays connected otherwise, and then
/// does not advertise: the phone could not find it)
async fn run(
    address: Address,
    pin: String,
    mut asked: mpsc::UnboundedReceiver<Ask>,
    tell: std_mpsc::Sender<Event>,
    ready: std_mpsc::Sender<Result<()>>,
) {
    let session = match Session::new().await {
        Ok(s) => s,
        Err(_) => {
            let _ = ready.send(Err(anyhow!(t("ble-no-bluez"))));
            return;
        }
    };
    let link = match timeout(CONNECT_WAIT, connect_or_pair_again(&session, address, &pin)).await {
        Ok(Ok(link)) => link,
        failed => {
            // failed after the connect (no service, notify refused) or timed out: disconnected,
            // so the companion advertises again and the next attempt starts clean
            if let Ok(a) = adapter(&session).await {
                if let Ok(d) = a.device(address) {
                    if d.is_connected().await.unwrap_or(false) {
                        let _ = timeout(Duration::from_secs(5), d.disconnect()).await;
                    }
                }
            }
            let error = match failed {
                Ok(Err(e)) => e,
                _ => anyhow!(t("ble-connect-timeout")),
            };
            let _ = ready.send(Err(error));
            return;
        }
    };
    // `_agent` kept to the end: BlueZ may ask for the PIN again
    let Link { device, rx, mut notify, mut events, _agent } = link;
    let _ = ready.send(Ok(()));
    loop {
        tokio::select! {
            frame = notify.next() => match frame {
                Some(frame) => {
                    if tell.send(Event::Frame(frame)).is_err() {
                        break;
                    }
                }
                None => {
                    let _ = tell.send(Event::Lost(t("ble-notify-stopped")));
                    break;
                }
            },
            event = events.next() => match event {
                Some(DeviceEvent::PropertyChanged(DeviceProperty::Connected(false))) | None => {
                    let _ = tell.send(Event::Lost(t("ble-disconnected")));
                    break;
                }
                Some(_) => {}
            },
            ask = asked.recv() => match ask {
                // the job is done (Conn dropped)
                None => break,
                Some(Ask::Write(frame, reply)) => {
                    let result = match timeout(WRITE_WAIT, rx.write(&frame)).await {
                        Ok(Ok(())) => Ok(()),
                        Ok(Err(e)) => Err(anyhow!(tf("ble-write-failed", &[("error", e.to_string().into())]))),
                        Err(_) => Err(anyhow!(t("ble-write-timeout"))),
                    };
                    let failed = result.is_err();
                    let _ = reply.send(result);
                    if failed {
                        let _ = tell.send(Event::Lost(t("ble-write-timeout")));
                        break;
                    }
                }
            },
        }
    }
    let _ = timeout(Duration::from_secs(5), device.disconnect()).await;
}

struct Link {
    device: bluer::Device,
    rx: bluer::gatt::remote::Characteristic,
    notify: std::pin::Pin<Box<dyn futures::Stream<Item = Vec<u8>> + Send>>,
    events: std::pin::Pin<Box<dyn futures::Stream<Item = DeviceEvent> + Send>>,
    /// Kept for the whole connection: BlueZ may ask for the PIN again
    _agent: Option<AgentHandle>,
}

/// `connect`; a pairing BlueZ holds but the companion lost (erased, re-flashed): removed, then
/// paired again, once
async fn connect_or_pair_again(session: &Session, address: Address, pin: &str) -> Result<Link> {
    match connect(session, address, pin).await {
        Err(e) if e.is::<BondLost>() => {
            crate::debug_log!("INFO", "bluetooth: pairing lost on the companion, paired again");
            adapter(session).await?.remove_device(address).await?;
            connect(session, address, pin).await
        }
        other => other,
    }
}

async fn connect(session: &Session, address: Address, pin: &str) -> Result<Link> {
    let agent = agent(session, pin).await.ok();
    let adapter = adapter(session).await?;
    // not known yet (the watcher is off, or heard it long ago): looked for, 20 s at most
    if !adapter.device_addresses().await?.contains(&address) {
        let events = adapter.discover_devices().await?;
        pin_mut!(events);
        let found = timeout(Duration::from_secs(20), async {
            while let Some(event) = events.next().await {
                if matches!(event, AdapterEvent::DeviceAdded(a) if a == address) {
                    return true;
                }
            }
            false
        })
        .await
        .unwrap_or(false);
        if !found {
            bail!(tf("ble-not-found", &[("address", address.to_string().into())]));
        }
    }
    let device = adapter.device(address)?;
    let was_paired = device.is_paired().await.unwrap_or(false);
    crate::debug_log!("DEBUG", "bluetooth {address}: paired before: {was_paired}");
    if !was_paired {
        crate::debug_log!("INFO", "bluetooth {address}: pairing");
        // an error while the pairing went through after all (checked again): taken as paired
        let failed = match device.pair().await {
            Err(e) if device.is_paired().await.unwrap_or(false) => {
                crate::debug_log!("INFO", "bluetooth {address}: pairing said {:?} but paired after all", e.kind);
                None
            }
            Err(e) => Some(e),
            Ok(()) => None,
        };
        if let Some(e) = failed {
            crate::debug_log!("WARN", "bluetooth {address}: pairing failed: {:?}", e.kind);
            use bluer::ErrorKind::{AuthenticationCanceled, AuthenticationFailed, AuthenticationRejected, AuthenticationTimeout};
            // the companion refused the PIN (or did not get one in time): the user is asked
            if matches!(e.kind, AuthenticationFailed | AuthenticationRejected | AuthenticationCanceled | AuthenticationTimeout) {
                super::refused(&address.to_string());
                bail!(t("ble-pin-refused"));
            }
            bail!(tf("ble-pair-failed", &[("error", e.to_string().into())]));
        }
    }
    // trusted every time: BlueZ then lets it reconnect without asking
    let _ = device.set_trusted(true).await;
    if !device.is_connected().await.unwrap_or(false) {
        device.connect().await.map_err(|e| anyhow!(tf("ble-connect-failed", &[("error", e.to_string().into())])))?;
    }
    let deadline = Instant::now() + Duration::from_secs(15);
    while !device.is_services_resolved().await.unwrap_or(false) {
        if Instant::now() > deadline {
            bail!(t("ble-no-service"));
        }
        sleep(Duration::from_millis(200)).await;
    }
    let (mut rx, mut tx) = (None, None);
    for service in device.services().await? {
        if service.uuid().await? != Uuid::from_u128(NUS_SERVICE) {
            continue;
        }
        for ch in service.characteristics().await? {
            let uuid = ch.uuid().await?;
            if uuid == Uuid::from_u128(NUS_RX) {
                rx = Some(ch);
            } else if uuid == Uuid::from_u128(NUS_TX) {
                tx = Some(ch);
            }
        }
    }
    let (Some(rx), Some(tx)) = (rx, tx) else { bail!(t("ble-no-service")) };
    // refused although paired: the companion no longer has this pairing
    let notify = tx.notify().await.map_err(|_| if was_paired { anyhow::Error::new(BondLost) } else { anyhow!(t("ble-not-paired")) })?;
    let events = device.events().await?;
    Ok(Link { device, rx, notify: Box::pin(notify), events: Box::pin(events), _agent: agent })
}
