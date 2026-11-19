# Meshcore Manager: help

*[Version française](README.fr.md)*

A terminal app for your [MeshCore](https://github.com/meshcore-dev/MeshCore) nodes: companions,
repeaters, room servers, sensors.

- Finds your nodes by itself: USB (several at once) and Bluetooth companions nearby.
- Backs them up: settings, identity (private key), channels, contacts, access list, regions.
- Restores them: on the same node, or on another board.
- Changes their settings: one node, or several at once. You see every change before it's sent.
- One file, nothing to install. Windows and Linux. English and French.

## Contents
- [Start](#start)
- [The basics](#the-basics)
- [Pages](#pages): [Fleet](#fleet) · [A node](#a-node) · [Edit mode](#edit-mode) ·
  [Backups](#backups) · [Contacts](#contacts) · [Regions](#regions-page) ·
  [Settings](#settings) · [Boards, Log, Info](#boards-log-info)
- [How to](#how-to)
- [Keys](#keys)
- [Files](#files) · [config.toml](#configtoml)
- [Languages](#languages)
- [Problems](#problems)

## Start
- **Windows:** unzip, run `mcmanager.exe`. Windows Terminal looks best.
- **Linux:** unpack (`tar xzf mcmanager-*.tar.gz`), run `./mcmanager`.
  - For USB, once: `sudo usermod -aG dialout "$USER"` (Arch: `uucp`), then log out and back in.
  - For Bluetooth: `bluetoothd` must be running (it is on desktops and Raspberry Pi OS).

Plug in your nodes: they show up on their own.

```
mcmanager                 the app
mcmanager scan            list the USB ports and what answers, as plain text
mcmanager --data <folder> keep settings and backups in another folder
```

What your nodes need:
- **Companion on USB:** the **USB** companion firmware. A Bluetooth or Wi-Fi firmware doesn't talk
  over USB (USB only flashes it): the board shows as one that doesn't answer.
- **Companion on Bluetooth:** it must advertise. One connected to a phone doesn't, so it isn't
  seen: disconnect it on the phone first.
- **Repeater, room server, sensor:** USB (their text console).

## The basics
- **Reading is free, writing asks.** Opening, reading, comparing never change a node. Anything
  that writes (a setting, a restore, a favourite) first shows exactly what will change and asks.
  A change is sent once, never twice.
- **The bar at the bottom** shows the keys that work right now, and only those.
- **`Esc` goes back one step**: closes a box, stops typing, leaves edit mode, leaves a node.
- **`x` removes or deletes**, always in red, always asks first (twice for several things).
- **Questions** have buttons that say what they do. `←` `→` choose, `Enter` presses, `Esc` =
  no.
- **Clock:** when a node's clock is more than 2 minutes behind this computer's, the app sets it
  when it reads the node, as the MeshCore apps do (a node without a clock battery restarts in
  2024). It's shown in green, "clock updated". A clock ahead is shown, never set back.
- **Secrets** (passwords, private key) show as dots until you press `s`. Never in the log.

## Pages
`Tab` / `Shift+Tab` or `1`-`8`.

| # | Page | For |
|---|---|---|
| 1 | [Fleet](#fleet) | All your nodes. Open one, back up, edit several at once |
| 2 | [Backups](#backups) | Every backup, by board. Compare, restore, delete |
| 3 | [Contacts](#contacts) | Your address book: every contact and access key your nodes hold |
| 4 | [Regions](#regions-page) | Your list of region names, ready to give to your nodes |
| 5 | [Boards](#boards-log-info) | The boards the app knows |
| 6 | [Log](#boards-log-info) | What happened since the start |
| 7 | [Settings](#settings) | Language, backups folder, Bluetooth PIN, debug log |
| 8 | [Info](#boards-log-info) | Version, license |

### Fleet
One line per node:
- **Link:** `USB` and its port, or `BLE` and its signal (dBm);
- **Key:** the first 4 characters of its public key, in a colour of its own, to tell nodes apart
  at a glance;
- kind, name, board, firmware, last backup (a spinner while a backup runs).

The details of the selected node are under the list. The title says whether Bluetooth is
listening, not available (and why), or off in Settings.

A Bluetooth companion is read once, by itself, as soon as it's heard. What happens shows in its
line and on the Log page.

| Key | Does |
|---|---|
| `Enter` | open the node |
| `b` | back up (the checked nodes, if any) |
| `r` / `R` | read this node again / all of them |
| `o` | sort: by link, kind, name |
| `f` | show one kind only: companions, repeaters, room servers, sensors |
| `s` | select mode (check boxes). `Space` checks one, `a` all or none, `Esc` leaves |
| `e` | with nodes checked (same kind): [edit them together](#change-several-nodes-at-once) |
| `p` | a Bluetooth companion whose PIN was refused: type its PIN |
| `u` | unpair a Bluetooth companion from this computer (the checked ones, if any) |

### A node
`Enter` on it in the Fleet. `Tab` goes through its sections, `Esc` back to the Fleet. While a
node is open, the page keys do nothing: a stray key never leaves it.

| Section | Shows | Keys |
|---|---|---|
| **Overview** | Identity, radio, settings. A companion also shows its battery, temperature, sensors and counters (running time, noise floor, air time, packets) | `e` edit, `s` show / hide secrets, `r` read again, `b` back up, `←` then `↑` `↓` scroll the info panel, `p` / `u` pair / unpair (Bluetooth) |
| **Contacts** | A companion's contacts, A to Z: ★, type, key, the name it advertises, your custom name | `/` search, `o` sort (type, last heard), `n` or `Enter` custom name (saved at once in your address book), `*` favourite (goes through edit mode) |
| **Channels** | A companion's channels (keys hidden) | `e` then `Enter` rename, `x` remove |
| **Regions** | A repeater's or room server's regions (firmware 1.10+). `*` first (packets with no region), then the tree. Below a line, the regions of your list the node doesn't have | `+` copy the selected region into your list |
| **Access** | Who may log in to a repeater, room server or sensor: each key and its role (read only, read / write, admin) | `e` then `a` add people, `←` `→` role, `x` remove |
| **Backups** | This board's backups, newest first | `Enter` compare with the node now, `R` restore, `b` back up, `Space` check, `x` delete the checked ones |

### Edit mode
`e` in a node's section. You stay in that section until you leave (`Esc`, which asks if you have
changes).

- `↑` `↓` choose a setting, `Enter` change it.
  - A setting with a few values: pick with `←` `→` (or type a letter to type freely).
  - Others are typed. Limits show while you type ("3 / 15 characters") and a wrong value is
    refused right there, with the reason.
- `?` everything about the selected setting: what it does, its values, its default, since which
  firmware. Written from the MeshCore firmware.
- `p` radio presets, the ones the official MeshCore apps offer, by country (type to find one).
- `u` undo the last change (again: the one before), `U` redo.
- `c` commit: every change listed (radio ones marked `!`), then:
  - **back up, then write**: a fresh backup first; if it fails, nothing is written;
  - **write it now**.
  Each change is sent once, then the node is read again.

What you can change is what the firmware lets you change:
- **Companion:** name, position, radio, power, "add every node heard" and auto-add (which types,
  replace the oldest when full, farthest hops), telemetry, position in adverts, multi ACKs,
  default flood scope, rx delay, duty cycle, path hash mode, Bluetooth PIN, private key;
  contacts (favourite, remove), channels (rename, remove).
- **Repeater, room server, sensor:** every console setting that has a `set` command, the admin
  and guest passwords, the private key, regions, the access list.

Regions section in edit mode: `Enter` adds the region under the line, `a` finds one or adds a
new name (to the node and your list), `Space` flood allowed / blocked, `h` home, `d` default
scope, `x` removes it (and the ones under it).

### Backups
Left, every board ever backed up, plugged in or not (`●` = here now). Right, its backups: date,
type (*manual*, *auto (edit)*, *auto (restore)*...), what's in it.

| Key | Does |
|---|---|
| `←` `→` | boards / backups |
| `Enter` | (left) show its backups; (right) compare the backup with the node now (nothing written) |
| `c` | compare two backups: the 2 checked, or the selected one with the one before |
| `R` | restore the backup: onto its own board, or [onto another one](#restore-onto-another-board) |
| `Space` / `a` | check a backup / all or none |
| `x` | (left) delete all of the board's backups; (right) delete the checked ones. Asks, twice for several |
| `f` | show one kind only |

### Contacts
Your address book, filled by itself every time a node is read: every contact of your
companions, every key of an access list. For each key: the companions that hold it, the role
each node gives it.

Columns: ★, type, key, name, custom name, role, where it is ("3 companions", "access on 2").

| Key | Does |
|---|---|
| `/` | search |
| `n` / `Enter` | custom name: kept in the app only (a companion always takes the name a contact advertises) |
| `*` | favourite: also written to every companion that holds it (asks first) |
| `r` | role: read only, read / write, admin. Kept here, used when you give access to a repeater |
| `o` | sort |
| `f` | show: all, one type, the favourites, one role |
| `Space` / `x` | check / take the checked ones out of the list (never off a node) |

### Regions page
Your own list of region names, in the firmware's format. A node's `region` output can be pasted
as is.

`a` new name under the selected one, `A` at the top, `x` remove (with the ones under it, asks
first; only from the list, never from a node). Names are lowercase.

### Settings
`↑` `↓` choose, `Enter` change. Used at once, saved in [config.toml](#configtoml).

| Setting | What |
|---|---|
| **Language** | `←` `→`. "Automatic" follows your system |
| **Debug log** | on / off. A detailed log in `logs/`, to find a problem. Names, keys and positions are masked |
| **Backups folder** | type or paste a folder (`Ctrl+U` empties the line), or `o` for the system's folder window. It's tested first, then you choose: copy, move or leave the backups already saved. All or nothing: every copy is checked and the new folder saved before anything switches; **Move** removes the old backups only then. If anything fails, nothing changes and the app says why |
| **Pairing PIN** | the PIN tried first with a Bluetooth companion (MeshCore default `123456`) |
| **Original files** | puts the original English and French files back in `lang/` (a file you changed is kept as `.bak`) |

### Boards, Log, Info
- **Boards:** the boards the app recognises (name, maker, chip, the model the firmware reports)
  and their USB ids. A board in flash mode is shown as such and never written to.
- **Log:** everything since the start, newest at the bottom.
- **Info:** version and license.

## How to

### Back up
Fleet, select the node, `b`. Done: it's written at once from the node's last reading (a node not
read yet is read first). Several: `Space` on each (or `a`), then `b`.

The same reading twice is refused: `r` reads the node again first.

What's in it:
- **companion:** settings, radio, identity (private key), channels, contacts, auto-add,
  Bluetooth PIN;
- **repeater, room server, sensor:** every setting, passwords, private key, regions, access list;
- your custom names, roles and favourites for the node's keys.

⚠️ A backup holds the private key and channel keys. Keep your backups to yourself.

### Check what changed
- `Enter` on a backup: compares it with the node as it is now, setting by setting.
- `c` on the Backups page: compares two backups (settings, channels, contacts: added, removed,
  changed).

Secrets are never shown, only "replaced".

### Restore
A node's **Backups** section, select the backup, `R`. You see every change by name ("auto-add
repeaters: off → on"), then **restore**.

Only what differs is sent and checked. If it stops halfway (cable pulled, node silent), it says
so: run it again and it sends the rest.

A companion's contacts: missing ones go back while there's room, ones added since are kept,
names and routes are left to the mesh.

### Restore onto another board
Broken or replaced board? **Backups** page: select the backup, `R`, pick the target node (same
kind), then:
- **clone**: the new board becomes the old one (same key, same name). The app asks twice.
  ⚠️ Never power both at once: the mesh would see the same node twice.
- **settings only**: radio, settings, channels, contacts. The new board keeps its own name, key,
  position and Bluetooth PIN.

What the other board can't take (other radio chip, older firmware...) is left as it has it and
listed at the end.

### Change settings
Open the node, `e`, change, `c`. See [Edit mode](#edit-mode).

### Change several nodes at once
Fleet: `s`, `Space` on each node (same kind), `e`.
- You get the settings they all have (not name, key or position).
- A setting that differs shows "n different values"; the selected one shows a table: each node,
  its value now, after.
- `p` one radio preset for all, `a` one region for all.
- `c` lists the changes node by node, then writes each one.

### Give access to a repeater
Open it, **Access**, `e`, `a`: pick people from your Contacts (`←` `→` one role, `Space` pick,
`+` the whole role, type to search). They get the role you set on the Contacts page, else read
only. `c` to commit.

Tip: give your people a role on the Contacts page first (`r`): then `a` opens on your admins.

### Bluetooth pairing
Usually nothing to do: a companion is read as soon as it's heard, with the PIN it last paired
with, else the one in Settings.

PIN refused? Its line shows `p`: press it, type the 6-digit PIN (on its screen, or set in the
MeshCore app). The app remembers it. `u` unpairs it from this computer (the companion keeps its
settings).

A companion's own PIN is a setting in edit mode, so you can give several companions the same
one with a group edit.

## Keys
| Key | Does |
|---|---|
| `Tab` `Shift+Tab`, `1`-`8` | change page (in a node: its sections) |
| `↑` `↓` `PgUp` `PgDn` `Home` `End` | move |
| `Enter` | open / change / compare |
| `Esc` | back one step |
| `r` / `R` | read again / all (Fleet) |
| `o` / `f` / `/` | sort / show one kind / search |
| `s` | Fleet: select mode. In a node: show / hide secrets |
| `Space` / `a` | check one / all or none |
| `b` | back up |
| `e` / `c` | edit / commit |
| `u` / `U` | undo / redo |
| `?` / `p` | help on a setting / radio presets (edit mode) |
| `p` / `u` | pair / unpair a Bluetooth companion (Fleet, Overview) |
| `n` / `*` / `r` | custom name / favourite / role (Contacts) |
| `R` | restore (Backups) |
| `x` | remove or delete (red, always asks) |
| `q` | quit (asks while something runs) |

## Files
All in `mcmanager-data`, next to the program. Elsewhere: `--data <folder>`, or the
`MCMANAGER_DATA` variable. Nothing is written anywhere else.

| File | What |
|---|---|
| `config.toml` | [Settings](#configtoml). The previous one is kept as `.bak` |
| `backups/<name>_<key>/<date_time>/` | The backups (or the folder you chose), one folder per board, one per backup |
| `contacts.toml` | The address book. The previous one is kept as `.bak` |
| `bluetooth.toml` | The PIN each companion last paired with. ⚠️ Anyone with this file can pair with your companions |
| `regions.txt` | Your region names |
| `lang/` | The language files |
| `logs/` | The debug log, only when on. Kept until you delete it |
| `devices.toml` | Optional: your own boards (format: [data/devices.toml](data/devices.toml)) |
| `radio_presets.toml` | Optional: your own radio presets (format: [data/radio_presets.toml](data/radio_presets.toml)) |

Backups follow the board (its USB serial number, else its key): a node renamed in a MeshCore app
keeps its backups together. The app never deletes a backup on its own; only you do, on the
Backups page.

When the backups folder you chose is gone, the app says so: deleted, the default folder takes
over; on a drive that isn't plugged in, the default folder is used meanwhile and the setting is
kept.

## config.toml
Made at the first start, each line explained. Change it on the Settings page or by hand (delete
a line to get its default back). A newer version adds its new settings and never renames one.

| Setting | Default | What |
|---|---|---|
| `[ui] language` | `"auto"` | `auto`, or a language code (`en`, `fr`...) |
| `[scan] every_secs` | `2` | how often the USB ports are looked at (seconds) |
| `[scan] answer_ms` | `4000` | how long a node gets to answer when plugged in. An ESP32 restarts when its port opens: below 3000 it may be missed |
| `[backup] private_key` | `true` | keep the private key in backups (needed to restore the same identity) |
| `[backup] hardware_id` | `true` | keep the board's USB serial number, to tell "the same board" from another |
| `[backup] folder` | `""` | where backups go; empty = `backups` in the data folder |
| `[bluetooth] enabled` | `true` | look for Bluetooth companions |
| `[bluetooth] pin` | `"123456"` | the pairing PIN tried first |
| `[debug] log` | `false` | the debug log |

## Languages
English and French built in. **Automatic** follows your system's languages.

The files are copied to `lang/en/` and `lang/fr/`:
- **Fix a text:** edit it there. Your file is never overwritten; an untouched one follows updates.
- **New language:** copy `lang/en/` to `lang/<code>/` (`de`, `es`, `pt-BR`...), translate, pick it
  in Settings. Keep the ids and `{ $variables }`, and about the same length as the English. What
  you haven't translated shows in English.
- **Back to the originals:** Settings > **Original files** (yours is kept as `.bak`).

## Problems
| What you see | What to do |
|---|---|
| A companion on USB "doesn't answer" | It runs a Bluetooth or Wi-Fi firmware: flash the USB companion firmware, or use it over Bluetooth |
| A board plugged in is missed | Raise `answer_ms` in config.toml (an ESP32 restarts when its port opens) |
| Linux: a port can't be opened | Your user isn't in `dialout` (Arch: `uucp`): see [Start](#start) |
| A Bluetooth companion isn't listed | It's connected to a phone (disconnect it there), off, or out of range |
| "Bluetooth: not available" | No Bluetooth adapter, or it's off. Linux: `bluetoothd` not running |
| The PIN is refused | `p` on its line, type the PIN shown on its screen or set in the MeshCore app |
| A restore stopped | Run it again: it sends what's left |
| Odd characters or boxes on screen | Use Windows Terminal (Windows) or a UTF-8 terminal |

## License
Fed1291 · [CC BY-NC-SA 4.0](../LICENSE)
