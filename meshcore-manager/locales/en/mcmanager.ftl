# Meshcore Manager - English (the reference: every message exists here).
# Format: Project Fluent (https://projectfluent.org). Ids are kebab-case, grouped by screen.
# This file is also in lang/en/ in the data folder: the program reads it from there, so a text
# corrected there is used. A translation, with no new program: copy lang/en/ to lang/<code>/
# (lang/de/...), translate the texts, keep the ids and the { $variables }. It shows up in
# Settings > Language. A message it lacks is taken from English. Keep each text about as long as
# the English one: the screens are laid out for that length. Settings > "Original files" puts
# the English and French files back as they came (a changed one is kept as .bak).

# How this language names itself, in the Settings page
language-name = English

## The app frame
app-name = Meshcore Manager
app-by = (by Fed1291)
tab-fleet = Fleet
tab-backups = Backups
tab-contacts = Contacts
tab-regions = Regions
people-about = Every contact read from your companions and every key of an access list, kept here by itself (contacts.toml). A custom name and a role stay in this program only: a companion always shows the name each node advertises (it replaces it at every advert). A favourite (*) is written to the companions that hold the contact.
people-title = { $count ->
    [one] 1 key
   *[other] { $count } keys
}  ·  { $sort }
people-empty = Nothing yet: open a companion (its contacts) or a repeater (its access list) in the Fleet, and what it holds comes here.
people-held = { $count ->
    [one] on 1 companion
   *[other] on { $count } companions
}
people-access = { $count ->
    [one] in 1 access list
   *[other] in { $count } access lists
}
people-remove-ask = Take { $name } out of the address book?
people-remove-ask-n = Take these { $count } contacts out of the address book?
people-remove-back = Only from this list, never from a node. It comes back when a node read again still holds it.
people-remove-yes = take it out
key-role = role
col-role = role
people-favourites = favourites
people-favourite-ask = { $name }: { $what } on { $count ->
    [one] 1 companion
   *[other] { $count } companions
}?
people-favourite-away = Not read now, so not written: { $nodes }. Read it in the Fleet, then * again.
col-on = on
field-advertised = advertises as
field-held-by = held by
field-access = access
field-role = your role for it
tab-boards = Boards
tab-log = Log
tab-settings = Settings
tab-about = Info

## Settings page
settings-about-page = The settings of this program, saved in config.toml (each change is used at once). The nodes' own settings are in their view (Fleet, Enter).
settings-selected = Selected
settings-language = Language
settings-language-auto = automatic ({ $name })
settings-language-about = The language of every text. Automatic: the system's, else English. A text a language lacks is shown in English.
settings-language-folder = Language files: { $folder }, one folder per language code (en, fr...) with its mcmanager.ftl. Copy one to start another language.
settings-debug = Debug log
settings-on = on
settings-off = off
settings-debug-about = Writes every action and error, with the part of the program it comes from, one JSON line each: to find a problem or send with a bug report. Never a key, a password, a node's name, position or messages (written <node>, <key>, <hex>...). Files are kept until you delete them.
settings-debug-file = Today's file: { $file }
settings-backups-folder = Backups folder
settings-backups-folder-about = Where backups are saved. Another drive or a synced folder works too. The folder is tested before anything is done; then you choose: copy the backups already saved, move them, or leave them where they are.
settings-folder-change = Enter: type or paste another folder (quotes, spaces, ~ for your home folder are fine). o: choose it in the system's folder window.
settings-folder-typing = Typing or pasting replaces the folder shown; Backspace edits it. Enter: test this folder, Esc: cancel. Empty: backups in the data folder.
settings-key-type = type or paste
settings-key-browse = browse
settings-key-clear = clear
settings-folder-window = Backups folder
settings-folder-no-desktop = No folder window here: this computer shows no desktop (a console, or SSH).
settings-folder-no-picker = No folder window here: neither zenity nor kdialog is installed.
settings-folder-type-instead = Type or paste the folder instead: Enter on the Backups folder setting.
settings-debug-writing = writing
settings-group-display = Display
settings-group-diagnostics = Diagnostics
settings-group-backups = Backups
settings-group-translation = Translation
settings-group-bluetooth = Bluetooth
settings-ble-pin = Pairing PIN
settings-ble-pin-about = The PIN tried first when a Bluetooth companion pairs with this computer (the first time only). MeshCore's default is 123456. A companion with a screen shows its own, and any can have its own: when it refuses this one, the program asks for its PIN (Fleet, `p` too).
settings-key-digits = digits
settings-folder-files-moved = Other files moved: { $count }.
settings-folder-files-copied = Other files copied: { $count }.
settings-folder-left = { $count } files are still in { $folder } (a restore not finished needs them, or they could not be removed): their checked copies are in the new folder, nothing was lost.
settings-folder-not-text = This folder's name is not valid text (it holds bytes of another encoding): it cannot be saved in the settings. Choose or make a folder with a plain name.
settings-folder-not-carried = Not everything could be brought to the new folder: nothing was changed. No backup was moved or removed, the copies made there were removed again.
settings-folder-not-saved = The new folder could not be saved in the settings: nothing was changed. No backup was moved or removed, the copies made there were removed again.
settings-folder-kept = Backups still go to { $folder }.
log-copy-exists = a different { $file } is already there (never written over)
log-copy-differs = the copy { $file } does not match its original

## Bluetooth
ble-no-adapter = No Bluetooth adapter found (built in, or a USB dongle).
ble-not-on-system = Bluetooth companions are not available on this system yet: use USB.
ble-off = Bluetooth is switched off, or its adapter is unplugged.
ble-radio-off = Bluetooth is switched off on this computer (Windows: Settings > Devices > Bluetooth).
ble-radio-disabled = Bluetooth is disabled on this computer (airplane mode, a switch, or the adapter turned off).
ble-blocked = The system does not let programs use Bluetooth (Windows: Settings > Privacy, Bluetooth / Radios).
ble-policy = Bluetooth is turned off by a policy of this computer.
ble-busy = The Bluetooth adapter is in use by another program.
ble-no-le = This Bluetooth adapter has no Bluetooth Low Energy (4.0 or newer needed).
ble-no-bluez = Bluetooth service not reachable (bluetoothd: sudo systemctl start bluetooth).
ble-scan-failed = The Bluetooth scan could not start.
ble-scan-stopped = The Bluetooth scan stopped.
ble-bad-pin = A PIN is 6 digits.
ble-pin-refused = Pairing refused: the PIN tried is not this companion's.
ble-bad-address = { $address } is not a Bluetooth address.
ble-not-found = { $address } not found: out of range, switched off, or connected to a phone (disconnect it there).
ble-pair-failed = Pairing failed ({ $error }).
ble-connect-failed = Could not connect ({ $error }).
ble-connect-timeout = The companion did not answer in time (out of range, or busy with a phone).
ble-no-service = No MeshCore service on this device: not a companion firmware?
ble-not-paired = The companion refused: the pairing is missing on one side. Remove it in the system's Bluetooth settings and try again.
ble-closed = The Bluetooth connection is closed.
ble-notify-stopped = the companion stopped sending
ble-disconnected = the companion disconnected
ble-write-failed = Bluetooth write failed ({ $error }).
ble-write-timeout = Bluetooth write timed out.
ble-call-timeout = Bluetooth did not answer in time.
ble-lost = Bluetooth connection lost: { $why }.
log-ble-problem = Bluetooth: { $why } (USB still works).
node-ble-not-read = Heard over Bluetooth. It connects and is read by itself, with the PIN it has (the one you gave, its saved one, else the default); a PIN is asked only if that one is refused.
node-ble-silent = Connected over Bluetooth, but no companion answered.
node-link-ble = Bluetooth { $name }
node-link-usb = USB { $name }
field-bluetooth = Bluetooth
col-link = link
sort-link = link
sort-kind = type
sort-name = name
sort-heard = last heard
key-sort = sort: { $now }
node-contacts-sorted = { $count ->
    [one] 1 contact
   *[other] { $count } contacts
}  ·  { $sort }
node-contacts-found = { $title }  ·  { $count } found
node-contacts-none-found = No contact holds what you searched. Esc clears the search.
key-search = search
key-find-clear = clear the search
key-own-name = custom name
key-save = save
col-own-name = custom name
field-also-on = also on
set-autoadd-chat = auto-add companions
set-autoadd-repeater = auto-add repeaters
set-autoadd-room = auto-add rooms
set-autoadd-sensor = auto-add sensors
set-autoadd-overwrite = full: replace oldest
set-autoadd-hops = auto-add max hops
group-no-radio = These nodes have no radio settings in common.
settings-folder-bad-char = A folder cannot hold control characters.
settings-key-test = test this folder
settings-originals = Original files
settings-originals-button = put the English and French files back
settings-originals-about = The English and French files are in the lang folder and are read from there: a text corrected there is used. Unchanged copies follow each new version by themselves; a changed one is never touched. This puts the originals back.
settings-originals-ask = Write the original English and French files into the lang folder again?
settings-originals-kept = A file you changed is kept next to it, renamed .bak (with the date).
settings-originals-yes = put them back
settings-key-originals = put back
settings-file = Settings file: { $file }
settings-not-saved = The setting is used, but could not be saved: it is lost when the program closes.
settings-folder-busy = Jobs are running: change the backups folder once they are done.
settings-folder-unusable = This folder cannot hold the backups: nothing was changed.
settings-folder-nested = One folder is inside the other: choose a folder outside the current backups folder (and not one that holds it).
settings-folder-ok = { $folder } can hold the backups (tested: a file written and read back).
settings-folder-nothing = There are no backups to bring over.
settings-folder-use = use this folder
settings-folder-has = { $count ->
    [one] 1 backup is saved in the current folder ({ $size } MB).
   *[other] { $count } backups are saved in the current folder ({ $size } MB).
}
settings-folder-how = Copy: each one copied and checked, the old ones stay. Move: all copied and checked, the new folder saved, only then the old ones removed. Leave: only new backups go there. If anything fails, nothing changes.
settings-folder-copy = copy them there
settings-folder-move = move them there
settings-folder-leave = leave them
settings-folder-move-sure = { $count ->
    [one] Move 1 backup? It is removed from the old folder only once its copy is checked and the new folder saved. A backup a restore still needs is kept there.
   *[other] Move { $count } backups? They are removed from the old folder only once every copy is checked and the new folder saved. A backup a restore still needs is kept there.
}
settings-folder-now = Backups now go to { $folder }.
settings-folder-copied = Backups copied and checked: { $done }, already there: { $there }.
settings-folder-moved = Backups moved (each copy checked first): { $done }, already there: { $there }.
settings-originals-done = Written again: { $files }
settings-originals-same = The language files are already the originals: nothing written.
settings-originals-failed = The language files could not be written.
lang-not-written = The language file { $file } could not be written ({ $error }): the built-in texts are used.
lang-no-folder = There is no lang folder.
log-copy-failed = { $what } could not be brought over
folder-unusable = Cannot use the folder { $folder } (it cannot be made, or a file cannot be written and read back there)
backups-folder-fallback = The backups folder in the settings cannot be used ({ $error }): backups go to { $folder } until it is back.
backups-folder-gone = The backups folder { $folder } is no longer there (deleted, moved or renamed).
backups-folder-gone-now = Backups go back to the default folder, { $default }, and the setting too. Choose another folder in Settings if you want.
backups-folder-away = The backups folder { $folder } cannot be reached: its drive or network share is not there.
backups-folder-away-now = Backups go to { $default } meanwhile. The setting is kept: plug the drive in, then start the program again.
config-by-hand = { $file }: { $key } is written in a way this program does not change (an inline table): change it there by hand.
file-too-big = the file is bigger than { $max } KB: not read

## Info page
about-name = Meshcore Manager { $version }
about-share-title = You changed it? Share it.
about-share = If you change this program and pass your version on, the license asks you to share it under the same license, with credit. Please also publish your changes where others can find them, even small fixes: that way every MeshCore user gets them, and the tools of the whole MeshCore community get better together.
about-license-title = License: CC BY-NC-SA 4.0
about-license = Free to use, share and change, as long as you give credit (BY), do not use it commercially (NC), and share what you build from it under the same license (SA). The full text:
about-license-link = creativecommons.org/licenses/by-nc-sa/4.0
about-meshcore-title = MeshCore
about-meshcore = Made for MeshCore nodes (github.com/meshcore-dev/MeshCore). The settings help is written from the MeshCore firmware and its documentation (MIT License).
key-switch = switch
key-quit = quit
key-select = select
key-scroll = scroll
key-panel = nodes / backups
key-its-backups = its backups
key-backup = back up
key-rescan-one = scan it again
key-rescan-all = scan all again
key-restore = restore
key-open = open
key-back = back
key-back-to-list = back to the list
key-close = close
key-section = section
key-other-panel = other panel
key-read-again = read again
key-only = show: { $now }
only-all = all
only-title = { $title }  [{ $kind }]
only-none = None of this kind. Press f to show another kind.
key-edit = edit
key-ok = done
key-cancel = cancel
key-leave-edit = leave edit
key-change = change
key-favourite = favourite
key-remove = remove
# undo / redo: keep them in English in every language (short, and known on every keyboard)
key-undo = undo
key-redo = redo
key-commit = commit ({ $count })
popup-ok = OK
popup-keys-question = ← →  choose    Enter  OK    Esc  cancel
popup-keys-note = Enter / Esc  close
answer-cancel = cancel

## Kinds of node
kind-companion = companion
kind-repeater = repeater
kind-room-server = room server
kind-sensor = sensor
kind-bootloader = flash mode
kind-unknown = unknown

## Table columns and "label  value" lines
col-port = port
col-kind = kind
col-name = name
col-board = board
col-firmware = firmware
col-date = date
col-contents = contents
col-state = state
col-reason = type
col-maker = maker
col-chip = chip
col-reported = reports itself as
col-usb-id = USB id
col-boards = usually
col-last-backup = last backup
col-type = type
col-route = route
col-heard = heard
col-key = key
col-slot = slot
field-port = port
field-usb = USB
field-hardware-id = hardware id
field-board = board
field-firmware = firmware
field-public-key = public key
field-radio = radio
field-node = node
field-created = made
field-reason = type
field-contents = contents
field-last-backup = last backup
field-heard = last heard
field-position = position
field-route = route

## How long ago
age-never = never
age-now = now
age-minutes = { $n } min
age-hours = { $n } h
age-days = { $n } d

## What a node tells about itself (its own view)
fact-max-power = max power
fact-capacity = room for
fact-capacity-value = { $contacts } contacts, { $channels } channels
fact-battery = battery
fact-storage = storage used
more-above = ▲ more
more-below = ▼ more
fact-clock = clock
fact-clock-synced = { $now }  ·  clock updated from this computer (was { $was })
fact-private-key = private key
fact-unreadable = the node cannot tell it
key-secrets-show = show secrets
key-secrets-hide = hide secrets

## A companion's settings (firmware: CMD_SET_* in companion_radio/MyMesh.cpp)
set-name = name
set-lat = latitude
set-lon = longitude
set-freq = frequency MHz
set-bw = bandwidth kHz
set-sf = spreading factor
set-cr = coding rate
set-tx = power dBm
set-manual-add = add every node heard
set-advert-loc = position in adverts
set-multi-acks = multi ACKs
set-telemetry-base = telemetry: base
set-telemetry-loc = telemetry: position
set-telemetry-env = telemetry: sensors
set-rx-delay = rx delay
set-duty-cycle = duty cycle %
set-path-hash-mode = path hash mode
set-position = position
set-radio = radio (frequency, bandwidth, spreading factor, coding rate)
fact-ble-pin = Bluetooth PIN

## Config mode (e in a node's view) and the commit
edit-title = Config
edit-badge = CONFIG
edit-pending = { $count ->
    [0] nothing changed yet
    [one] 1 change not sent
   *[other] { $count } changes not sent
}
node-own-name-for = Custom name for { $name }:
node-contacts-checked = { $title }  ·  { $count } checked (x: take them out)
leave-title = Leave edit
leave-ask = { $count ->
    [one] You have 1 change that is not written to the node:
   *[other] You have { $count } changes that are not written to the node:
}
leave-not-sent = Nothing has been sent to the node yet. To keep them: Esc, then c to commit.
leave-yes = { $count ->
    [one] drop it and leave
   *[other] drop them and leave
}
leave-no = keep configuring
commit-yes = { $count ->
    [one] write it now
   *[other] write all { $count } now
}
commit-no = keep configuring
commit-backup-first = back up, then write
commit-backup-who = { $count ->
    [one] "{ $first }": a new backup of { $nodes } first (if it fails, nothing is written). "{ $now }": no backup.
   *[other] "{ $first }": a new backup of each node written first ({ $nodes }; one that fails is not written). "{ $now }": no backup.
}
backup-yes = { $count ->
    [one] back it up
   *[other] back up all { $count }
}
edit-read-first = Read the node first (r): its settings come from that reading.
edit-selected = Selected setting
edit-values = values
edit-now = now
edit-new = new
edit-danger-long = ! Changes how the node transmits: every node it talks with must use the same radio settings.
edit-select-hint = Enter: change it   ?: all about it   u: undo it   c: commit everything
edit-typing-hint = Type the new value. Enter: done (sent with c)   Esc: cancel
edit-pick-hint = ← →: choose   Enter: done (sent with c)   Esc: cancel   a letter: type another value
key-pick = choose
key-type-instead = type instead
key-help = help
key-presets = radio presets
key-use-preset = use this preset
key-find = find
edit-preset = preset
edit-radio-hint = Enter: change it   p: choose a country preset   ?: all about it   c: commit
presets-title = Radio presets
presets-find = Find
presets-ids = { $size }-byte ids
presets-no-match = No preset has that in its name.
presets-none = none of the presets (own settings)
presets-note = Suggested by the MeshCore community, as in the official apps. ● = what the node uses now. Choosing one only prepares the changes: you see them, and they go with the commit.
presets-unreadable = Your radio presets { $file } could not be read; the built-in list is used: { $error }
edit-out-of-range-of = { $what } must be { $range }
edit-not-a-whole-number = { $what } must be a whole number
edit-not-a-choice = { $what } must be one of: { $values }
edit-bad-radio = The radio must be freq,bw,sf,cr: 150-2500 MHz, 7-500 kHz, SF 5-12, CR 5-8 (for example 869.618,62.5,8,5)
edit-bad-char = { $what } cannot hold "{ $char }"
edit-range = range
edit-default = default
edit-since = since firmware
edit-or-off = or 0 = off
edit-key-too-long = { $what } is full: at most { $max } characters (the node's limit)
edit-key-hex = { $what } takes hex digits only: 0-9 and A-F
edit-key-whole = { $what } takes a whole number: { $range }
edit-key-number = { $what } takes a number: { $range }
edit-up-to = text, up to { $max } characters
edit-room = { $used } / { $max } characters · { $left } left
help-unreadable = Your settings help { $file } could not be read and is not used: { $error }
help-values = Values
help-source = From the MeshCore firmware and its documentation (MIT License, Scott Powell / rippleradios.com)
help-close = Esc / ? / Enter: close    ↑↓: scroll
help-none = No help for this setting yet. Its own words: { $about }
edit-removed = remove
edit-secret-line = { $what }: replaced (not shown)
edit-bad-hex = { $what } must be { $digits } hex digits (0-9, A-F)
edit-bad-key = The private key must be 128 hex digits
edit-disabled = this firmware was built without that feature
edit-refused-secret =the node did not take it (its answer is not shown: it can hold the password)
edit-line = { $what }: { $old } → { $new }
edit-contact = contact { $name }: { $what }
edit-contact-remove = remove contact { $name }
edit-favourite-on = favourite
edit-favourite-off = not favourite
edit-channel-rename = channel { $slot }: { $old } → { $new }
edit-channel-remove = remove channel { $slot } ({ $name })
edit-not-a-number = { $what }: "{ $value }" is not a number
edit-out-of-range = { $what } must be between { $low } and { $high }
edit-not-on-off = { $what }: "{ $value }" must be on or off
edit-bad-text = { $what } cannot be empty or hold control characters
edit-error-code = error { $code }
edit-other-node = Another node is on this port now: nothing was sent.
edit-wrong-kind = not for this kind of node
edit-region-remove = remove region { $name }
edit-region-add = add region { $name }
edit-region-flood-on = region { $name }: flood allowed
edit-region-flood-off = region { $name }: no flood
edit-region-home = home region → { $name }
edit-region-no-home = no home region any more
edit-region-default = default scope → { $name }
edit-region-no-default = no default scope
edit-region-save = save the regions on the node
commit-title = Commit
commit-ask = { $count ->
    [one] Write 1 change to { $name }?
   *[other] Write { $count } changes to { $name }?
}
commit-danger = ! changes how the node transmits: every node it talks with must use the same radio settings.
commit-once = Each change is sent once. The node is read again afterwards: the view then shows what it really holds.

## Start, files
data-folder-failed = Cannot use the data folder { $folder }
config-unwritable = Cannot write the settings file { $file }
config-unreadable = Cannot read the settings file { $file }
config-empty = The settings file { $file } is empty. Delete it to get a new one with the defaults.
devices-unreadable = Your board list { $file } could not be read and is not used: { $error }

## Serial ports and nodes
serial-open-failed = { $port } cannot be opened (unplugged, or another program is using it): { $error }
serial-lost = The connection to the node was lost
node-bootloader = The board is in flash mode: no firmware is running. Reset it to use it.
node-silent = Nothing answered. A companion must run the USB firmware (not the Bluetooth or Wi-Fi one); other firmware is not recognised yet.
node-silent-wireless = A MeshCore board, but its firmware does not answer on USB: most likely a companion with the Bluetooth (or Wi-Fi) firmware, which talks only over the air. USB still flashes it (through the bootloader). To use it here: flash the companion USB firmware, or wait for the Bluetooth link (planned).

## Fleet page
fleet-title = { $count ->
    [0] Nodes
    [one] 1 node
   *[other] { $count } nodes
}
fleet-looking = Looking at the USB ports...
fleet-empty = No USB serial device found. Plug a node in: it shows up here by itself.
fleet-probing = identifying  { $secs } s
fleet-details = Selected node
fleet-open-hint = Enter: open the node (contacts, channels, backups...)
fleet-never = never
fleet-backing-up = backing up
fleet-checked = { $title }  ·  { $count } checked
key-check = check
key-check-all = select all
key-uncheck-all = unselect all
key-unpair-n = unpair { $count }
key-uncheck = uncheck all
key-select-mode = select
key-select-leave = leave select
key-pin = pair (PIN)
pin-hint = type its PIN and pair (a companion with a screen shows it)
stat-uptime = running for
stat-queue = packets waiting
stat-noise = noise floor
stat-last-packet = last packet heard
stat-air-time = air time
stat-air-time-value = sending { $tx }, receiving { $rx }
stat-packets = packets
stat-packets-value = { $recv } received, { $sent } sent
stat-sent-by = sent
stat-recv-by = received
stat-by-value = { $flood } flood, { $direct } direct
stat-recv-errors = receive errors
span-seconds = { $s } s
span-minutes = { $m } min
span-hours = { $h } h { $m } min
span-days = { $d } d { $h } h
tel-digital-in = digital input
tel-digital-out = digital output
tel-analog-in = analog input
tel-analog-out = analog output
tel-generic = sensor value
tel-illuminance = light
tel-presence = presence
tel-temperature = temperature
tel-humidity = humidity
tel-pressure = pressure
tel-voltage = voltage
tel-current = current
tel-frequency = frequency
tel-percentage = percentage
tel-altitude = altitude
tel-concentration = concentration
tel-power = power
tel-distance = distance
tel-energy = energy
tel-direction = direction
tel-unixtime = sensor time
tel-colour = colour
tel-switch = switch
tel-latitude = GPS latitude
tel-longitude = GPS longitude
tel-gps-altitude = GPS altitude
tel-accel-x = acceleration X
tel-accel-y = acceleration Y
tel-accel-z = acceleration Z
tel-gyro-x = rotation X
tel-gyro-y = rotation Y
tel-gyro-z = rotation Z
fleet-ble-connecting = connecting { $with }…
fleet-ble-will-connect = connects by itself soon, { $with }
fleet-pin-yours = with the PIN you gave
fleet-pin-saved-one = with its saved PIN
fleet-pin-default = with the default PIN
fleet-ble-failed = could not connect
fleet-ble-try-again = try again
node-ble-not-connected = Not connected yet: why is on the left. p: type its PIN and pair.
fleet-ble-listening = Bluetooth: listening
fleet-ble-problem = Bluetooth: not available
fleet-ble-disabled = Bluetooth: off in settings
fleet-ble-why = Bluetooth: { $why }
fleet-pin-given = PIN given this session
fleet-pin-settings = PIN from Settings
fleet-pin-saved = paired before: its PIN is saved
pin-title = Bluetooth PIN
pin-for = Pairing PIN for { $name }.
pin-refused = { $name } refused the PIN tried. Type its PIN: it is then tried again.
pin-where = A companion with a screen shows its PIN; otherwise 123456, or the one set in the MeshCore app.
pin-label = PIN
pin-empty-default = Empty: the PIN from Settings. Kept until the program closes, never saved.
key-unpair = unpair
unpair-title = Unpair
unpair-ask = { $count ->
    [one] Unpair this computer from { $names }?
   *[other] Unpair this computer from these { $count } companions: { $names }?
}
unpair-after = This computer forgets the pairing (the companion keeps its settings). To use it again: p, its PIN, and it pairs again.
unpair-yes = unpair
job-unpair = Unpairing { $name }
log-unpaired = { $name } unpaired from this computer.
log-unpair-failed = { $name } could not be unpaired: { $error }
pin-key-pair = pair
key-backup-n = back up { $count }
key-edit-n = edit { $count } together

## Group edit (several checked nodes of one kind, e in Fleet)
group-title = { $count } × { $kind }
group-mixed = { $count } different values
group-no-change = no change
group-on-each = { $what } on each node
group-commit-ask = { $nodes ->
    [one] Write to 1 node ({ $count } changes)?
   *[other] Write to { $nodes } nodes ({ $count } changes)?
}
group-node-unchanged = { $name }: nothing changes
group-gone = None of the checked nodes is plugged in any more. Esc: back to the list.
group-nothing-common = These nodes have no setting in common that can be changed together (a name, a private key and a position are each node's own).
group-need-checks = Check the nodes to configure together with Space first (or open one node with Enter).
group-not-nodes = Only nodes that answer can be configured together: uncheck the others (Space).
group-choose = Nodes of one kind are configured together. Which ones?
group-silent = { $count ->
    [one] 1 checked node does not answer: it is left aside.
   *[other] { $count } checked nodes do not answer: they are left aside.
}
group-edit-these = configure the { $count } { $kind }
group-regions-adding = regions to add on each node: { $names }
group-no-regions = None of these nodes has regions (firmware 1.10 or later).
col-node = node
col-now = now
col-after = after the commit

## Regions page (your list of region names, regions.txt)
regions-title = { $count ->
    [0] Your regions
    [one] Your regions: 1
   *[other] Your regions: { $count }
}
regions-empty = Empty for now. a adds a name (lowercase letters, digits, -; $ first for a private one), or + copies one a node already has, from its Regions section.
regions-new = New region
regions-name = A region name
regions-top = at the top:
regions-under = under { $name }:
regions-about = Your own list of region names, kept on this computer (regions.txt), not on any node. It is what you pick from when you add a region to a node, or to several at once: a region works only when every node spells it exactly the same, and this list keeps the spelling. Flood and home are set per node, in its view. "from": the node a name was copied from (+ in a node's Regions).
regions-exists = { $name } is already in the list.
regions-remove-ask = { $count ->
    [0] Remove { $name } from your list?
    [one] Remove { $name } and the region under it from your list?
   *[other] Remove { $name } and the { $count } regions under it from your list?
}
regions-remove-under = Also removed: { $names }
regions-remove-nodes = Only your list changes: the nodes keep their regions.
regions-remove-yes = remove from the list
regions-pick-title = Add a region
regions-pick-new = { $name }  (new: added to the node and to your list)
regions-pick-empty = Your list is empty: type a name, Enter adds it to the node and to your list.
regions-pick-note = Enter: the selected region, with the ones above it.  ● = already there (in a group: picking it again takes it back).
regions-unreadable = Your region list { $file } could not be read and is not used: { $error }
regions-unwritable = Cannot write your region list { $file }
key-region-add-under = new under it
key-region-add-top = new at the top

## A node's own view (Enter in Fleet)
node-title = Node
node-gone = The node is not on USB any more. Esc: back to the list.
node-reading-hint = Asking the node for each of its settings: a few seconds.
node-settings = Settings
node-read-only = Read only
node-edit-settings = Settings: edit
node-not-read = Not read yet: press r.
node-nothing-to-read = No MeshCore node answers on this port: nothing to read.
node-overview = Overview
node-contacts = Contacts
node-channels = Channels
node-backups = Backups
node-identity = Info
node-told = What it says
node-last-backup = { $stamp }  ({ $count ->
    [one] 1 backup
   *[other] { $count } backups
})
node-never-backed-up = never: press b
node-no-contacts = The node has no contacts.
node-channels-title = { $count ->
    [one] 1 channel
   *[other] { $count } channels
}
node-no-channels = No channel is set on the node.
node-channel-keys = Channel keys are not shown: they are only kept in backups.
node-regions = Regions

## Access list of a repeater, room server or sensor (who may log in as what)
acl-title = Access
acl-count = Access list: { $count } / { $max }
acl-role = role
acl-guest = guest
acl-read-only = read only
acl-read-write = read / write
acl-admin = admin
acl-added = new
acl-about = Who may log in as what. Someone who logs in with the admin password is added by the node itself; guests are not kept. At most { $max }.
acl-none = This firmware cannot list its access list (no "get acl").
acl-empty = Nobody in the list: only the passwords let anyone in.
acl-empty-edit = Nobody in the list. a: add people from your contacts.
acl-not-read = The access list was not read: read the node again (r).
acl-bad-key = { $key } is not a whole public key (64 hex digits).
acl-full = The access list would hold { $count } keys; the node keeps { $max } and would drop some by itself. Remove some first.
acl-set-line = access: { $key } as { $role }
acl-remove-line = access: { $key } removed
key-acl-role = role
key-acl-add = add from contacts
key-acl-give = give access
acl-pick-title = Give access: your contacts  ·  { $count } checked
acl-pick-empty = Your address book is empty: read a companion first (its contacts come there), then give them a role on the Contacts page (r).
acl-pick-pasted = Enter: add the key { $key } (in none of your contacts).
acl-pick-note = Each one gets the role you gave it on the Contacts page (r); one without a role gets the role shown, else read only. ← → one role · Space check · + every one shown · type to search a name (or paste a whole key).
node-regions-title = { $count ->
    [0] Regions
    [one] 1 region
   *[other] { $count } regions
}
node-no-regions = The node has no region yet. In edit (e), a adds one from your list.
col-flood = flood
regions-flood = allowed
regions-no-flood = blocked
regions-home = home
regions-default = default scope
regions-not-in-list = not in your list
regions-in-list = in your list
regions-root = every packet with no region
regions-root-flood-only = * stands for every packet with no region: only its flood can change
regions-offer-line = in your list, not on this node
regions-offer-add = Enter: add
col-from = from
regions-cut = The node's answer was cut (160 characters): some regions may be missing here.
key-region-take = add it
key-region-offer = add the one below the line
key-region-to-list = into my list
key-region-add = add a region
key-region-flood = flood on / off
key-region-home = home on / off
key-region-default = default scope on / off
node-backups-title = { $count ->
    [one] 1 backup of this board
   *[other] { $count } backups of this board
}
node-no-backups = No backup of this board yet: press b. Backups follow the board (its USB serial number, else its key), so renaming the node keeps them here.
contact-chat = chat
contact-repeater = repeater
contact-room = room
contact-sensor = sensor
contact-other = other
route-flood = flood
route-direct = direct
route-hops = { $n ->
    [one] 1 hop
   *[other] { $n } hops
}

## Backing up
job-backup = Backup of { $name }
backup-title = Backup
backup-ask = Back up { $name } ({ $board }) now?
backup-ask-many = Back up these { $count } nodes now, side by side?
backup-left-out = Left out (no node answers there): { $ports }
backup-ask-secrets =The backup holds the node's private key and channel keys: keep the backups folder to yourself.
backup-not-a-node = This port has no MeshCore node that answers: nothing to back up.
backup-no-answer = The node did not answer
backup-already = This reading of the node is backed up already: r reads it again, then b
backup-unwritable = Cannot write { $file }
backup-complete = complete
backup-partial = partial
backup-verified = verified
backup-newer = newer version
backup-unverified = { $file } did not read back as it was written: the backup is not kept
backup-newer-format = This backup was made by a newer version ({ $tool }, format { $format }): update this program to use it. It is kept as it is.
backup-damaged ={ $file } is not as it was saved (changed or damaged since): this backup cannot be used
backup-stopped-answering = the node stopped answering ({ $step }): nothing was kept, try again
backup-board-changed = another board answers on the port now (or none): nothing was kept
reason-unknown = -
reason-manual = manual
section-self = identity and radio
section-device = device info
section-tuning = radio tuning
section-autoadd = contact auto-add
section-flood-scope = flood scope
section-custom-vars = custom variables
section-private-key = private key
section-channels = channels
section-contacts = contacts
section-console = settings
section-acl = access list
section-regions = regions
section-address-book = your names and roles
restore-book = { $count ->
    [one] 1 change of yours goes back in the address book (never on the node):
   *[other] { $count } changes of yours go back in the address book (never on the node):
}
restore-book-name = custom name for { $who }: { $old } → { $new }
restore-book-role = role for { $who }: { $old } → { $new }
restore-book-done = { $count ->
    [one] 1 contact's custom name / role put back in the address book.
   *[other] { $count } contacts' custom names / roles put back in the address book.
}
compare-book = Your names and roles
restore-left = { $count ->
    [one] This board did not take 1 setting: it keeps its own.
   *[other] This board did not take { $count } settings: it keeps its own.
}
restore-left-line = { $what } ({ $why })
key-restore-here = restore on this node
restore-other-note = Onto another board: what it cannot take (another radio chip, less power, a setting its firmware lacks) is left as it has it, and listed at the end.
book-unreadable = The address book { $file } cannot be read: it is left as it is (nothing written to it) until it is fixed or moved away.
book-unwritable = The address book { $file } cannot be written.
reason-before-config = auto (edit)
reason-before-restore = auto (restore)
reason-before-firmware = auto (firmware)
reason-before-reset = auto (reset)
backup-unreadable = unreadable
backup-contents = { $sections } parts, { $items } items
backup-secret-mark = * holds a private key, a channel key or a password
backup-missing = Not in this backup: { $missing }

## Backups page
backups-nodes = Nodes  ·  ● here now
backups-of = Backups of { $name }
backups-of-gone = Backups of { $name } (not here now: R restores one on another device)
backups-details = Selected backup
backups-empty = No backup yet. Open Fleet, select a node and press b.
key-compare = compare with the one before
key-compare-checked = compare the 2 checked
compare-title = Compare
key-compare-node = compare with the node now
compare-node-now = the node now
compare-node-absent = This backup's node is not here (plugged in, or heard over Bluetooth): it cannot be compared with it now. c compares it with the backup before.
job-compare-node = Comparing with the node on { $port }
job-connecting = connecting…
compare-close = ↑↓ PgUp PgDn: scroll    Esc / Enter: close
compare-head = { $name }: from { $older } to { $newer }
compare-count = { $count ->
    [0] No difference: both hold the same.
    [one] 1 difference
   *[other] { $count } differences
}
compare-settings = Settings
compare-channels = Channels
compare-acl = Access list
compare-acl-added = { $key } added ({ $role })
compare-acl-removed = { $key } removed (was { $role })
compare-acl-changed = { $key }: { $old } → { $new }
compare-contacts = Contacts
compare-notes = Notes
compare-only-older = Only in the older one: { $parts }
compare-only-newer = Only in the newer one: { $parts }
compare-not-in-older = Not in the older one: { $what }
compare-channel-added = slot { $slot }: { $name }
compare-channel-removed = slot { $slot }: { $name }
compare-channel-renamed = slot { $slot }: { $old } → { $new }
compare-channel-key = slot { $slot } ({ $name }): key replaced (not shown)
compare-contact-added = { $name }
compare-contact-removed = { $name }
compare-contact-renamed = { $old } → { $new }
compare-contact-favourite = { $name }: now a favourite
compare-contact-not-favourite = { $name }: no longer a favourite
compare-contact-type = { $name }: another type
compare-kinds-differ = These two backups are of different kinds of node: they cannot be compared.
compare-no-older = This is the oldest backup of the node: nothing before it to compare with. Check two backups (Space) to compare those.
compare-pick-two = Check exactly two backups (Space) to compare them, or none to compare the selected one with the one before it.
key-delete-node-backups = delete all its backups
key-delete-n = delete { $count }
delete-title = Delete backups
delete-ask = { $count ->
    [one] Delete this backup of { $name }?
   *[other] Delete these { $count } backups of { $name }?
}
delete-none-left = No backup of { $name } will be left.
delete-no-verified-left = No verified backup of this node will be left.
delete-way-back = Some were made before a change (edit, restore, firmware, reset): they are the way back to how the node was.
delete-cannot-undo = The files are deleted from the disk: this cannot be undone.
delete-sure = Really delete { $count } backups of { $name }, for good?
delete-yes = { $count ->
    [one] delete it
   *[other] delete { $count }
}
log-delete-done = Backup { $stamp } deleted
log-delete-failed = Backup { $stamp } could not be deleted: { $error }
log-delete-refused = Backup { $stamp } not deleted: it is not in the backups folder
log-delete-needed = Backup { $stamp } not deleted: a cut-short restore still needs it

## Restoring
restore-title = Restore
job-restore-check = Restore: reading the node on { $port } to show what would change (nothing is written yet)
job-restore = Restoring onto the node on { $port }
restore-onto = Onto { $name } ({ $board })
restore-kind-differs = The backup is of a { $backup }, this node is a { $target }: they do not match.
restore-no-target = Plug in the node to restore onto first (a node of the same kind, that answers).
restore-choose-target = Restore this backup on which device?
restore-target-own = { $name } (the board it came from)
restore-choose-mode = This backup comes from another board. What should this one take from it?
restore-move-means = Clone: this board becomes that node (same identity: private key, name, position), to replace a broken or lost one. Never have both powered on at the same time: two nodes with one identity break routes, messages and logins on the mesh.
restore-copy-means = Settings only: radio, settings, channels and contacts. This board keeps its own name, key, position and Bluetooth PIN.
restore-choose-move = clone it
restore-choose-copy = settings only
restore-mode-same = Its own backup: everything goes back as it was.
restore-mode-move = Clone: this board becomes the backed-up node (same identity).
restore-mode-copy = Settings only: this board keeps its own name, key, position and Bluetooth PIN.
restore-firmware = The backup was made with firmware { $backup }, the node runs { $node }: settings that firmware lacks are left out.
restore-nothing = The node already holds this backup: nothing to change.
restore-changes = { $count ->
    [one] 1 change:
   *[other] { $count } changes:
}
restore-more = … and { $count } more
restore-cannot = Cannot be restored: { $what }
restore-how = Only what differs is sent, then read back and checked.
restore-yes = restore
restore-stopped = The restore did not finish:
restore-stopped-how = The node holds part of the backup. Restore it again: only what is left is sent.
restore-move-title = Make a clone?
restore-move-sure = This board will take the identity of the backed-up node (its private key): its own identity is lost unless it has a backup. Never power the old board on next to it. Sure?
restore-move-yes = yes, take that identity
restore-no-key = the backup holds no private key (backups keep it only when set in config.toml)
restore-no-slot = channel { $name } (no free slot)
restore-channel = channel slot { $slot }
restore-channels = { $count ->
    [one] 1 channel
   *[other] { $count } channels
}
restore-contact = contact { $name }
restore-contacts-back = { $count ->
    [one] contact put back: { $names }
   *[other] { $count } contacts put back: { $names }
}
restore-contacts-fav = { $count ->
    [one] favourite as in the backup: { $names }
   *[other] favourites as in the backup ({ $count }): { $names }
}
restore-contacts-full = { $count ->
    [one] left as it is: { $names } is no longer on the node (its contact list is full, a newer contact took its place)
   *[other] left as they are: { $count } contacts are no longer on the node ({ $names }): its contact list is full, newer contacts took their place
}
restore-contacts-kept = { $count ->
    [one] kept: 1 contact the node added since the backup
   *[other] kept: { $count } contacts the node added since the backup
}
restore-and-more = { $names } and { $count } more
set-flood-scope = default flood scope
restore-regions-cut = removing regions (a region list was cut short when read)
restore-waiting = Waiting for the node to come back ({ $secs } s)…
restore-gone = The node did not come back.
restore-restarting = Restarting the node to use the new settings…
restore-refused = The node refused { $what }: { $why }
restore-not-settled = The node does not keep the values sent.
restore-not-started = Nothing was written: the node is as it was.
restore-done = Restored and checked: the node holds the backup.
restore-done-move = This board is now a clone of the old one: never power both on at the same time (same identity: routes, messages and logins break). Erase or keep the old one off.
restore-clone-warning = ! A clone: this board will have the same identity as { $name }. Never have both powered on at the same time; if the old one comes back, erase it or keep it off.
restore-put-back = The restore did not finish. The node was put back as it was, and checked. Why:
restore-stuck = The restore did not finish, and the node could not be put back as it was either. Why:
restore-stuck-how = Plug the node in again: at the next start the app offers to finish putting it back. The backup made before is in its backups ("before a restore").
restore-interrupted-title = A restore was cut short
restore-interrupted = A restore onto { $name } (started { $started }) did not finish: the program or the computer stopped.
restore-was-putting-back = It was putting the node back as it was.
restore-interrupted-how = Plug the node in, then put it back as it was (safest) or finish the restore. Later: asked again at the next start.
restore-put-back-button = put it back as it was
restore-finish-button = finish the restore
restore-later = later
restore-journal-open = A restore of this board was cut short and is not finished: finish it or put the node back as it was first (asked at the start of the program).

## Boards page
boards-title = { $count } boards known
boards-usb-title = { $count } USB ids known

## Log
log-empty = Nothing happened yet.
log-found = { $port }: { $kind } { $name }
log-backup-done = Backup of { $name } saved: { $stamp }
log-backup-failed = Backup of { $name } failed: { $error }
job-inspect = Reading the node on { $port }
log-job-waiting = { $port }: another job is working on this board; this one starts when it ends
log-job-crashed = { $port }: the job stopped on an unexpected error (a bug: please report it). The board is free again; nothing more was sent.
log-job-not-started = A job could not be started (the system has no room for another thread): nothing was done.
jobs-more = (+{ $count } more)
quit-title = Jobs are running
quit-ask = { $count ->
    [one] A job is still working on a node.
   *[other] { $count } jobs are still working on nodes.
}
quit-why = Quitting now cuts them off: a backup is not kept, a commit may be half sent, a restore is offered again at the next start (to finish, or to put the node back as it was).
quit-yes = quit anyway
quit-wait = wait for them
log-inspect-companion = { $name } read: { $contacts } contacts, { $channels } channels
log-inspect-console = { $name } read: { $settings } settings
job-apply = Writing to { $name }
log-edit-done = { $what }: done
log-edit-refused = { $what }: refused ({ $why })
log-edit-silent = { $what }: no answer. It may or may not have been done; it is not sent again.
log-edit-not-sent = { $count ->
    [one] 1 change was not sent (the node stopped answering)
   *[other] { $count } changes were not sent (the node stopped answering)
}
log-edit-summary = { $name }: { $done } of { $total } changes done
log-edit-failed = Nothing written to { $name }: { $error }
log-edit-no-backup = Nothing written to { $name }: the backup before it failed
log-inspect-failed = { $name } could not be read: { $error }
