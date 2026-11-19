# Changelog

## Meshcore Manager 0.1.1
- Changing the backups folder (Settings) is all or nothing. Every copy is checked and the new
  folder saved before the switch, and **Move** removes the old backups only then. A file already
  in the new folder is never written over. If anything fails, the copies are removed again,
  nothing changes, and the app says why.

## Meshcore Manager 0.1.0
First public version.
- Finds nodes on USB and Bluetooth companions nearby, and names their boards.
- Backs up companions, repeaters, room servers and sensors: settings, identity, channels,
  contacts, access list.
- Restores on the same node or on another board (clone or settings only), sending only what
  differs.
- Edit mode for one node or several at once, with radio presets, undo / redo and a full list of
  changes before writing.
- Address book with custom names, roles and favourites.
- Windows and Linux (PC and 64-bit ARM). English and French.
