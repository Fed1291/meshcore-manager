#!/bin/sh
# Build Meshcore Manager on Linux (PC or Raspberry Pi).
#   sh build.sh           for this computer: dist/manager-linux/<arch>/mcmanager
#   sh build.sh linux     the same (also: sh build.sh manager)
#   sh build.sh windows   for Windows, made here: dist/manager/mcmanager.exe
#   sh build.sh all       both
# (or chmod +x build.sh once, then ./build.sh)
#
# Needs, once:
#   Rust:           curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
#   a C compiler:   sudo apt install build-essential   (Debian, Ubuntu, Raspberry Pi OS)
#                   sudo dnf install gcc               (Fedora)
#                   sudo pacman -S base-devel          (Arch)
#   (libdbus, for Bluetooth, is compiled in: nothing else to install)
# For the Windows version, also once:
#   rustup target add x86_64-pc-windows-gnu
#   the MinGW-w64 cross compiler (it also puts the icon into the .exe):
#                   sudo apt install mingw-w64         (Debian, Ubuntu, Raspberry Pi OS)
#                   sudo dnf install mingw64-gcc       (Fedora)
#                   sudo pacman -S mingw-w64-gcc       (Arch)
#
# To use it:
#   USB:        your user in the "dialout" group (Arch: "uucp"), then log out and in again:
#               sudo usermod -aG dialout "$USER"
#   Bluetooth:  bluetoothd running (it is on every desktop and on Raspberry Pi OS)
set -eu
cd "$(dirname "$0")"

what="${1:-linux}"
case "$what" in
  manager|linux) do_linux=1; do_windows= ;;
  windows) do_linux=; do_windows=1 ;;
  all) do_linux=1; do_windows=1 ;;
  *) echo "Usage: sh build.sh [linux|windows|all]   (no word = linux, for this computer)"; exit 1 ;;
esac

command -v cargo >/dev/null 2>&1 || {
  echo "cargo is missing: install Rust (https://rustup.rs), then open a new terminal"
  exit 1
}
command -v cc >/dev/null 2>&1 || {
  echo "a C compiler is missing (cc): sudo apt install build-essential (or your system's equivalent)"
  exit 1
}

# copy_to <program built> <where it goes>
copy_to() {
  mkdir -p "$(dirname "$2")"
  cp "$1" "$2"
  echo "Built $2"
}

if [ -n "$do_linux" ]; then
  echo "=== Meshcore Manager for Linux (this computer)"
  cargo build --release -p meshcore-manager
  arch="$(uname -m)"
  copy_to target/release/mcmanager "dist/manager-linux/$arch/mcmanager"
  cp meshcore-manager/README.md dist/manager-linux/README.md
  echo "Run it: dist/manager-linux/$arch/mcmanager"
fi

if [ -n "$do_windows" ]; then
  echo "=== Meshcore Manager for Windows"
  target=x86_64-pc-windows-gnu
  rustup target list --installed 2>/dev/null | grep -q "^$target\$" || {
    echo "The Windows target is missing: rustup target add $target"
    exit 1
  }
  command -v x86_64-w64-mingw32-gcc >/dev/null 2>&1 || {
    echo "The MinGW-w64 cross compiler is missing: sudo apt install mingw-w64 (Fedora: mingw64-gcc, Arch: mingw-w64-gcc)"
    exit 1
  }
  # its own C compiler and linker for Windows (the one of this computer makes Linux programs)
  CC_x86_64_pc_windows_gnu=x86_64-w64-mingw32-gcc \
  CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER=x86_64-w64-mingw32-gcc \
    cargo build --release -p meshcore-manager --target "$target"
  copy_to "target/$target/release/mcmanager.exe" dist/manager/mcmanager.exe
  cp meshcore-manager/README.md dist/manager/README.md
  echo "Copy dist/manager/mcmanager.exe to the Windows computer and run it there"
fi
