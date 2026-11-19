//! Where the program keeps its files. One folder holds everything (settings, own board list,
//! backups, logs), so it can be moved or copied as a whole and nothing is left in the user profile:
//! `--data <folder>`, else the MCMANAGER_DATA variable, else `mcmanager-data` next to the program.
//! Backups can go to another folder of the user's choice (`[backup] folder`, Settings page).

use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};

use crate::i18n::tf;

const FOLDER: &str = "mcmanager-data";

#[derive(Clone)]
pub struct Paths {
    /// The data folder itself
    pub root: PathBuf,
    /// config.toml (written with its comments on first start)
    pub config: PathBuf,
    /// The user's own boards, added to the built-in list (optional file)
    pub devices: PathBuf,
    /// <node>/<date_time>/ (the layout: src/backup/mod.rs): `backups` in the data folder, or the folder
    /// chosen in the settings (`use_backups`)
    pub backups: PathBuf,
    /// The debug log, one file a day (src/debug.rs), when it is on
    pub logs: PathBuf,
    /// A newer or corrected settings help, over the built-in one (optional file)
    pub help: PathBuf,
    /// The user's own list of radio presets, instead of the built-in one (optional file)
    pub presets: PathBuf,
    /// The user's list of region names, in the firmware's `region` format (src/regions.rs)
    pub regions: PathBuf,
    /// The address book: every contact and access-list key seen on the nodes, with your own
    /// names for them (src/book.rs)
    pub book: PathBuf,
    /// The PIN each Bluetooth companion last connected with (ble/mod.rs)
    pub bluetooth: PathBuf,
    /// The restores that have not ended yet, one file per board (backup/journal.rs): there only
    /// while one runs, or when one was cut short (the program closed, the computer stopped)
    pub journals: PathBuf,
}

pub fn resolve(given: Option<PathBuf>) -> Result<Paths> {
    let root = match given.or_else(|| std::env::var_os("MCMANAGER_DATA").map(PathBuf::from)) {
        Some(folder) => folder,
        None => std::env::current_exe()
            .ok()
            .and_then(|exe| exe.parent().map(|d| d.join(FOLDER)))
            .ok_or_else(|| anyhow!(tf("data-folder-failed", &[("folder", FOLDER.into())])))?,
    };
    std::fs::create_dir_all(&root)
        .with_context(|| tf("data-folder-failed", &[("folder", root.display().to_string().into())]))?;
    Ok(Paths {
        config: root.join("config.toml"),
        devices: root.join("devices.toml"),
        help: root.join("settings_help.toml"),
        presets: root.join("radio_presets.toml"),
        regions: root.join("regions.txt"),
        book: root.join("contacts.toml"),
        bluetooth: root.join("bluetooth.toml"),
        journals: root.join("restore-journals"),
        backups: root.join("backups"),
        logs: root.join("logs"),
        root,
    })
}

impl Paths {
    /// Backups go to `folder` from now on (made when missing, and checked by writing a file in
    /// it). Err: it cannot be used, nothing changed. Nothing is moved: what was saved stays.
    pub fn use_backups(&mut self, folder: PathBuf) -> Result<()> {
        writable(&folder)?;
        self.backups = folder;
        Ok(())
    }
}

/// What became of a chosen backups folder that is not there
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Missing {
    /// It is there (or cannot be told: no rights to look)
    No,
    /// Its drive is there, the folder is not: deleted, moved or renamed. Back to the default
    /// folder for good (made again empty, it would hide where the backups went)
    Deleted,
    /// Its drive, network share or removable disk is not there (Windows `Z:\` or `\\server\share`
    /// not reachable; Linux nothing mounted at /media/<user>/<disk>, /run/media/<user>/<disk> or
    /// /mnt/<disk>): it may come back, the setting is kept
    Unplugged,
}

/// Whether the chosen backups `folder` is still there (`Missing`). Only "surely not there" counts
/// (`try_exists` = Ok(false)): a folder that cannot be looked at is left to fail with its reason.
pub fn missing(folder: &Path) -> Missing {
    if !matches!(folder.try_exists(), Ok(false)) {
        return Missing::No;
    }
    // the nearest folder above it that is there
    let there = folder.ancestors().skip(1).find(|a| !a.as_os_str().is_empty() && a.is_dir());
    let mount_area =
        |a: &Path| ["/media", "/run/media", "/mnt"].into_iter().map(Path::new).any(|base| a == base || a.parent() == Some(base));
    match there {
        None => Missing::Unplugged,
        Some(a) if mount_area(a) => Missing::Unplugged,
        Some(_) => Missing::Deleted,
    }
}

/// The folder exists (made when missing) and takes a file: a test file is written, synced, read
/// back and removed (it is ours: nothing of the user's)
pub fn writable(folder: &Path) -> Result<()> {
    let fail = || tf("folder-unusable", &[("folder", folder.display().to_string().into())]);
    std::fs::create_dir_all(folder).with_context(fail)?;
    let probe = folder.join(".mcmanager-write-test");
    std::fs::write(&probe, b"ok").with_context(fail)?;
    let back = std::fs::read(&probe).with_context(fail);
    let _ = std::fs::remove_file(&probe);
    if back? != b"ok" {
        anyhow::bail!(fail());
    }
    Ok(())
}
