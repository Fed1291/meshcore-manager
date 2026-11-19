# Le compiler vous-même

*[English version](BUILD.md)*

Des programmes tout prêts sont sur la page [Releases](../../releases). Pour partir des sources,
deux scripts font tout :
- **`build.bat`** sous Windows ;
- **`build.sh`** sous Linux.

## 1. Windows

### Une fois : installer Rust
1. Allez sur [rustup.rs](https://rustup.rs), téléchargez `rustup-init.exe` et lancez-le.
2. S'il propose d'installer les **Visual Studio C++ Build Tools**, acceptez. Rust en a besoin
   sous Windows.
3. Gardez les choix par défaut. À la fin, fermez et rouvrez votre terminal.

### Compiler
Double-cliquez sur `build.bat` (ou lancez-le dans un terminal). Un petit menu s'affiche :

```
    1  for Windows
    2  for Linux: PC and 64-bit ARM, any distribution
    A  both
```

Tapez `1` puis Entrée. La première compilation prend quelques minutes (tout est téléchargé et
compilé une fois), ensuite c'est bien plus rapide.

Votre programme est dans **`dist\manager\mcmanager.exe`**. Copiez-le où vous voulez et lancez-le.

Vous pouvez sauter le menu :

| Commande | Produit |
|---|---|
| `build.bat manager` | le programme Windows, `dist\manager\mcmanager.exe` |
| `build.bat manager-linux` | les programmes Linux, `dist\manager-linux\x86_64\` (PC) et `dist\manager-linux\aarch64\` (ARM 64 bits) |
| `build.bat all` | les deux |

### En option : les programmes Linux, faits sous Windows
`build.bat manager-linux` a besoin de quelques outils en plus, une fois :
1. `rustup target add x86_64-unknown-linux-musl aarch64-unknown-linux-musl`
2. `cargo install --locked cargo-zigbuild`
3. [Zig](https://ziglang.org/download/) : dézippez-le et ajoutez son dossier au `PATH`.

Les programmes Linux sont « statiques » : un seul fichier qui tourne sur toute distribution,
rien à installer.

## 2. Linux

### Une fois : installer Rust et un compilateur C
```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```
Puis fermez et rouvrez votre terminal. Et un compilateur C :

| Système | Commande |
|---|---|
| Debian, Ubuntu, Raspberry Pi OS | `sudo apt install build-essential` |
| Fedora | `sudo dnf install gcc` |
| Arch | `sudo pacman -S base-devel` |

Rien d'autre : la bibliothèque Bluetooth est compilée dedans.

### Compiler
Dans le dossier des sources :
```sh
sh build.sh
```
Votre programme est dans **`dist/manager-linux/<arch>/mcmanager`** (`x86_64` sur un PC,
`aarch64` sur un Raspberry Pi). S'il manque quelque chose, le script vous dit quoi installer.

| Commande | Produit |
|---|---|
| `sh build.sh` | le programme pour cet ordinateur |
| `sh build.sh windows` | le programme Windows, `dist/manager/mcmanager.exe` |
| `sh build.sh all` | les deux |

### En option : le programme Windows, fait sous Linux
Une fois :
```sh
rustup target add x86_64-pc-windows-gnu
sudo apt install mingw-w64        # Fedora : mingw64-gcc, Arch : mingw-w64-gcc
```

### Avant de le lancer
- **Noeuds en USB :** votre utilisateur doit avoir le droit d'ouvrir les ports série. Une fois,
  puis déconnectez-vous et reconnectez-vous :
  ```sh
  sudo usermod -aG dialout "$USER"      # Arch : uucp au lieu de dialout
  ```
- **Compagnons Bluetooth :** le service Bluetooth (`bluetoothd`) doit tourner. C'est déjà le cas
  sur les bureaux et sur Raspberry Pi OS.

## 3. macOS (pas testé)
Personne ne l'a encore essayé sur Mac. Le code a une partie Mac, donc il devrait compiler, mais
aucune garantie.

### Une fois : ce qu'il faut
1. Les outils en ligne de commande d'Apple, dans l'appli Terminal : `xcode-select --install`
2. Rust, depuis [rustup.rs](https://rustup.rs) (la même ligne `curl ... | sh` que pour Linux),
   puis fermez et rouvrez le Terminal.

### Compiler
Dans le dossier des sources :
```sh
cargo build --release -p meshcore-manager
```
Votre programme est `target/release/mcmanager`. Lancez-le avec `./target/release/mcmanager`.

### À vérifier
- Il démarre, et un noeud branché en USB apparaît dans Flotte (port `cu.usbmodem…` ou
  `cu.usbserial…`).
- Lire un noeud, une sauvegarde, une restauration.
- Réglages › dossier des sauvegardes : pas de fenêtre de choix sur Mac, tapez ou collez le chemin.
- Pas de Bluetooth sur Mac : l'appli le dit.

En cas d'erreur de compilation, [rust-lang.org](https://www.rust-lang.org/fr/learn) et le texte
de l'erreur dans un moteur de recherche sont les meilleurs points de départ.

## 4. Tout système, la méthode simple
`cargo build --release` marche aussi. Le programme arrive dans `target/release/`.

## Un problème ?
| Message | Que faire |
|---|---|
| `cargo is missing` / `'cargo' n'est pas reconnu` | Rust n'est pas installé, ou le terminal était ouvert avant l'installation : ouvrez-en un nouveau |
| `Could not replace ... is it running?` | Fermez Meshcore Manager, puis relancez le script |
| `linker 'link.exe' not found` (Windows) | Les Visual Studio C++ Build Tools manquent : relancez `rustup-init.exe`, ou installez-les depuis le site de Microsoft |
| `a C compiler is missing` (Linux) | Installez-le (tableau ci-dessus) |
