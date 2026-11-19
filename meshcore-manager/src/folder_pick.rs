//! The system's own "select folder" window, for the backups folder (Settings page, `o`).
//! An extra only: typing or pasting the path always works, and is the way on a console-only
//! machine (a Pi over SSH has no window to show).
//! - Windows: the Explorer folder window (rfd crate, IFileOpenDialog).
//! - Linux: zenity (GNOME and most desktops) or kdialog (KDE), when a desktop is there
//!   (DISPLAY / WAYLAND_DISPLAY set). Run as programs: nothing for the build to link.
//!
//! The window blocks until it closes: it runs on a thread of its own, one at a time.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

/// A window is open: a second `o` does not open another
static OPEN: AtomicBool = AtomicBool::new(false);

/// What came of it
pub enum Picked {
    /// This folder was chosen
    Folder(PathBuf),
    /// Closed without a choice
    Cancelled,
    /// No window could be shown here (the reason): type or paste the path instead. Linux only
    /// (no desktop, no zenity / kdialog); Windows always has its folder window
    #[cfg_attr(windows, allow(dead_code))]
    NoWindow(String),
    /// One is open already
    Busy,
}

/// Shows the window, starting in `start`, and waits for it
pub fn pick(start: &Path, title: &str) -> Picked {
    if OPEN.swap(true, Ordering::SeqCst) {
        return Picked::Busy;
    }
    let picked = show(start, title);
    OPEN.store(false, Ordering::SeqCst);
    picked
}

#[cfg(windows)]
fn show(start: &Path, title: &str) -> Picked {
    let mut dialog = rfd::FileDialog::new().set_title(title);
    if start.is_dir() {
        dialog = dialog.set_directory(start);
    }
    match dialog.pick_folder() {
        Some(folder) => Picked::Folder(folder),
        None => Picked::Cancelled,
    }
}

#[cfg(not(windows))]
fn show(start: &Path, title: &str) -> Picked {
    use std::os::unix::ffi::OsStringExt;
    use std::process::{Command, Stdio};

    use crate::i18n::t;

    let desktop = ["DISPLAY", "WAYLAND_DISPLAY"].iter().any(|v| std::env::var_os(v).is_some_and(|s| !s.is_empty()));
    if !desktop {
        return Picked::NoWindow(t("settings-folder-no-desktop"));
    }
    // zenity wants a folder name ending in "/" to start inside it
    let mut inside = start.as_os_str().to_os_string();
    inside.push("/");
    let tries: [(&str, Vec<std::ffi::OsString>); 2] = [
        (
            "zenity",
            vec!["--file-selection".into(), "--directory".into(), format!("--title={title}").into(), {
                let mut a = std::ffi::OsString::from("--filename=");
                a.push(&inside);
                a
            }],
        ),
        ("kdialog", vec!["--title".into(), title.into(), "--getexistingdirectory".into(), start.as_os_str().to_os_string()]),
    ];
    for (program, args) in tries {
        // not installed: the next one
        let Ok(out) = Command::new(program).args(&args).stdin(Stdio::null()).stderr(Stdio::null()).output() else {
            continue;
        };
        if !out.status.success() {
            // both say "closed without a choice" with exit code 1
            return Picked::Cancelled;
        }
        // the path as bytes: a Linux folder name need not be UTF-8
        let mut bytes = out.stdout;
        while bytes.last().is_some_and(|b| *b == b'\n' || *b == b'\r') {
            bytes.pop();
        }
        return if bytes.is_empty() {
            Picked::Cancelled
        } else {
            Picked::Folder(PathBuf::from(std::ffi::OsString::from_vec(bytes)))
        };
    }
    Picked::NoWindow(t("settings-folder-no-picker"))
}
