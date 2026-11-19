//! What depends on the computer the program runs on, kept in one place. The program must work the
//! same on any system, in any language:
//! - the Windows console: its own code page follows the system's language (437 in the US, 850 in
//!   western Europe, 866 Cyrillic, 932 Japanese...) and garbles anything outside it. While the
//!   program runs it is UTF-8 (65001), then put back as it was (`Console`).
//! - paths are the system's own (`PathBuf`, never split by hand); a path that is not valid text
//!   (possible on Linux) is refused by name rather than changed (`path_text`);
//! - file names the program makes are plain ASCII and never a name Windows reserves
//!   (store.rs `node_folder`);
//! - nothing reads a text the system wrote in its own language: errors by their kind or code,
//!   never by their words; numbers and dates in one fixed format, whatever the locale.

use std::path::Path;

/// UTF-8 for the console while it lives (Windows), its code pages put back when it is dropped.
/// Nothing to do elsewhere: terminals there take UTF-8.
pub struct Console {
    #[cfg(windows)]
    before: (u32, u32),
}

/// The UTF-8 code page
#[cfg(windows)]
const UTF8: u32 = 65001;

impl Console {
    #[cfg(windows)]
    pub fn utf8() -> Console {
        use windows::Win32::System::Console::{GetConsoleCP, GetConsoleOutputCP, SetConsoleCP, SetConsoleOutputCP};
        // SAFETY: plain calls on the program's own console, no pointers
        unsafe {
            let before = (GetConsoleOutputCP(), GetConsoleCP());
            let _ = SetConsoleOutputCP(UTF8);
            let _ = SetConsoleCP(UTF8);
            Console { before }
        }
    }

    #[cfg(not(windows))]
    pub fn utf8() -> Console {
        Console {}
    }
}

#[cfg(windows)]
impl Drop for Console {
    fn drop(&mut self) {
        use windows::Win32::System::Console::{SetConsoleCP, SetConsoleOutputCP};
        // 0: there was no console to ask (the program was not started from one)
        if self.before.0 != 0 {
            // SAFETY: as above
            unsafe {
                let _ = SetConsoleOutputCP(self.before.0);
                let _ = SetConsoleCP(self.before.1);
            }
        }
    }
}

/// A path as text (for config.toml, which is UTF-8): None when it is not valid text, which only
/// Linux allows. Such a path is refused by name, never saved changed (it would point elsewhere).
pub fn path_text(path: &Path) -> Option<String> {
    path.to_str().map(str::to_string)
}
