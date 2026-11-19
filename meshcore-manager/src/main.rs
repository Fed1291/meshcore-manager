//! Meshcore Manager: a terminal manager for MeshCore nodes (companions, repeaters, room servers,
//! sensors) on USB or Bluetooth. Finds them, names their boards, edits, backs them up and
//! restores them. Without a command it opens the full-screen view (src/app.rs): the pages are in
//! src/pages/, the work on the nodes runs in src/worker.rs, the node protocols in src/node/.

mod app;
mod backup;
mod ble;
mod book;
mod config;
mod debug;
mod devices;
mod event;
mod folder_pick;
mod help;
mod host;
mod i18n;
mod jobs;
mod node;
mod pages;
mod paths;
mod presets;
mod proto;
mod regions;
mod serial;
mod theme;
mod widgets;
mod worker;

use std::path::PathBuf;
use std::time::Duration;

use anyhow::Result;
use clap::{Parser, Subcommand};

use crate::i18n::{t, tf};

#[derive(Parser)]
#[command(name = "mcmanager", version, about = "Meshcore Manager: MeshCore nodes on USB - detect, back up, restore")]
struct Cli {
    /// Folder for the settings and the backups (default: mcmanager-data next to the program)
    #[arg(long, global = true, value_name = "FOLDER")]
    data: Option<PathBuf>,
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// List the USB serial ports and what answers on each (plain text)
    Scan,
}

fn main() -> Result<()> {
    // the Windows console in UTF-8 while the program runs, whatever the system's language
    // (put back when `_console` goes, at the end)
    let _console = host::Console::utf8();
    let cli = Cli::parse();
    let mut paths = paths::resolve(cli.data)?;
    let (mut config, config_note) = config::load(&paths.config)?;
    // the languages' files in lang/<code>/ (the built-in ones written there when missing: the
    // program reads them from there), then the chosen language
    i18n::set_folder(paths.root.join("lang"));
    let lang_notes = i18n::mirror();
    i18n::init(&config.ui.language);
    debug::start(paths.logs.clone(), config.debug.log);
    // the PIN Bluetooth pairing uses (a PIN is never written to the debug log)
    serial::set_pin(&config.bluetooth.pin);
    debug::private(&paths.root.to_string_lossy(), debug::FOLDER_TAG);
    // the backups folder chosen in the settings. Never made again empty when it is gone (that would
    // hide where the backups went): deleted -> the default folder, setting too; its drive not
    // there -> the default folder meanwhile, setting kept. Any other problem is said, and backups
    // go to the data folder meanwhile (never nowhere)
    let mut notes: Vec<String> = config_note.into_iter().chain(lang_notes).collect();
    let chosen = config.backup.folder_in(&paths.root);
    let fallback = paths.root.join("backups");
    let (shown, default) = (chosen.display().to_string(), fallback.display().to_string());
    match paths::missing(&chosen) {
        _ if chosen == fallback => paths.use_backups(fallback)?,
        paths::Missing::Deleted => {
            notes.push(tf("backups-folder-gone", &[("folder", shown.as_str().into())]));
            notes.push(tf("backups-folder-gone-now", &[("default", default.as_str().into())]));
            config.backup.folder.clear();
            if let Err(e) = config::set(&paths.config, "backup", "folder", toml::Value::String(String::new())) {
                notes.push(format!("{} {e:#}", t("settings-not-saved")));
            }
            paths.use_backups(fallback)?;
        }
        paths::Missing::Unplugged => {
            notes.push(tf("backups-folder-away", &[("folder", shown.as_str().into())]));
            notes.push(tf("backups-folder-away-now", &[("default", default.as_str().into())]));
            paths.use_backups(fallback)?;
        }
        paths::Missing::No => {
            if let Err(e) = paths.use_backups(chosen.clone()) {
                notes.push(tf("backups-folder-fallback", &[("error", format!("{e:#}").into()), ("folder", default.as_str().into())]));
                paths.use_backups(fallback)?;
            }
        }
    }
    debug::private(&paths.backups.to_string_lossy(), debug::FOLDER_TAG);
    debug::private(&chosen.to_string_lossy(), debug::FOLDER_TAG);
    // shown in the Log page (or printed, for a plain command)
    notes.extend(devices::init(&paths.devices).into_iter().chain(help::init(&paths.help)).chain(presets::init(&paths.presets)));
    match cli.command {
        None => app::run(paths, config, notes),
        Some(Command::Scan) => {
            for n in &notes {
                eprintln!("{n}");
            }
            scan(&config);
            Ok(())
        }
    }
}

/// `mcmanager scan`: one line per port. Each silent port costs the answer time.
fn scan(config: &config::Config) {
    let ports = serial::ports();
    if ports.is_empty() {
        println!("{}", t("fleet-empty"));
        return;
    }
    for port in ports {
        let node = node::identify(&port, Duration::from_millis(config.scan.answer_ms));
        println!(
            "{:<10} {:<10} {:<12} {}  |  {}  {}  {}",
            serial::short(&port.dev),
            port.usb_id(),
            node.kind.label(),
            widgets::clean(&node.name),
            node.board(),
            node.firmware,
            node.note,
        );
    }
}
