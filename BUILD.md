# Compile it yourself

*[Version française](BUILD.fr.md)*

Ready-made programs are on the [Releases](../../releases) page. To build from the source instead:
the two scripts do everything for you:
- **`build.bat`** on Windows;
- **`build.sh`** on Linux.

## 1. Windows

### Once: install Rust
1. Go to [rustup.rs](https://rustup.rs) and download `rustup-init.exe`, then run it.
2. If it offers to install the **Visual Studio C++ Build Tools**, say yes. Rust needs them on
   Windows.
3. Keep the default choices. When it's done, close and reopen your terminal.

### Build
Double-click `build.bat` (or run it in a terminal). You get a small menu:

```
    1  for Windows
    2  for Linux: PC and 64-bit ARM, any distribution
    A  both
```

Type `1` and Enter. The first build takes a few minutes (it downloads and compiles everything
once), then it's much faster.

Your program is in **`dist\manager\mcmanager.exe`**. Copy it anywhere and run it.

You can skip the menu:

| Command | Makes |
|---|---|
| `build.bat manager` | the Windows program, `dist\manager\mcmanager.exe` |
| `build.bat manager-linux` | the Linux programs, `dist\manager-linux\x86_64\` (PC) and `dist\manager-linux\aarch64\` (64-bit ARM) |
| `build.bat all` | both |

### Optional: Linux programs, made on Windows
`build.bat manager-linux` needs a few more things, once:
1. `rustup target add x86_64-unknown-linux-musl aarch64-unknown-linux-musl`
2. `cargo install --locked cargo-zigbuild`
3. [Zig](https://ziglang.org/download/): unzip it and add its folder to your `PATH`.

The Linux programs are "static": one file that runs on any distribution, nothing to install.

## 2. Linux

### Once: install Rust and a C compiler
```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```
Then close and reopen your terminal. And a C compiler:

| System | Command |
|---|---|
| Debian, Ubuntu, Raspberry Pi OS | `sudo apt install build-essential` |
| Fedora | `sudo dnf install gcc` |
| Arch | `sudo pacman -S base-devel` |

Nothing else: the Bluetooth library is compiled in.

### Build
In the source folder:
```sh
sh build.sh
```
Your program is in **`dist/manager-linux/<arch>/mcmanager`** (`x86_64` on a PC, `aarch64` on a
Raspberry Pi). If something is missing, the script tells you what to install.

| Command | Makes |
|---|---|
| `sh build.sh` | the program for this computer |
| `sh build.sh windows` | the Windows program, `dist/manager/mcmanager.exe` |
| `sh build.sh all` | both |

### Optional: the Windows program, made on Linux
Once:
```sh
rustup target add x86_64-pc-windows-gnu
sudo apt install mingw-w64        # Fedora: mingw64-gcc, Arch: mingw-w64-gcc
```

### Before you run it
- **USB nodes:** your user must be allowed to open serial ports. Once, then log out and back in:
  ```sh
  sudo usermod -aG dialout "$USER"      # Arch: uucp instead of dialout
  ```
- **Bluetooth companions:** the Bluetooth service (`bluetoothd`) must be running. It already is
  on desktops and on Raspberry Pi OS.

## 3. macOS (not tested)
Nobody has tried it on a Mac yet. The code has a Mac part, so it should build, but no promises.

### Once: what you need
1. Apple's command-line tools, in the Terminal app: `xcode-select --install`
2. Rust, from [rustup.rs](https://rustup.rs) (the same `curl ... | sh` line as for Linux), then
   close and reopen the Terminal.

### Build
In the source folder:
```sh
cargo build --release -p meshcore-manager
```
Your program is `target/release/mcmanager`. Run it with `./target/release/mcmanager`.

### What to check
- It starts, and a node plugged in by USB shows up on Fleet (as a `cu.usbmodem…` or
  `cu.usbserial…` port).
- Reading a node, a backup, a restore.
- Settings › backups folder: there's no folder window on a Mac, type or paste the path instead.
- Bluetooth isn't there on a Mac: the app says so.

If a build error shows up, [rust-lang.org](https://www.rust-lang.org/learn) and the error text
in a search engine are the best places to start.

## 4. Any system, the plain way
`cargo build --release` works too. The program lands in `target/release/`.

## Something went wrong?
| Message | What to do |
|---|---|
| `cargo is missing` / `'cargo' is not recognized` | Rust isn't installed, or the terminal was open before you installed it: open a new one |
| `Could not replace ... is it running?` | Close Meshcore Manager, then run the script again |
| `linker 'link.exe' not found` (Windows) | The Visual Studio C++ Build Tools are missing: run `rustup-init.exe` again, or install them from Microsoft |
| `a C compiler is missing` (Linux) | Install it (table above) |
