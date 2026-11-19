# Meshcore Manager : aide

*[English version](README.md)*

Une appli en terminal pour vos noeuds [MeshCore](https://github.com/meshcore-dev/MeshCore) :
compagnons, répéteurs, room servers, capteurs.

- Trouve vos noeuds tout seul : en USB (plusieurs à la fois) et les compagnons Bluetooth proches.
- Les sauvegarde : réglages, identité (clé privée), canaux, contacts, liste d'accès, régions.
- Les restaure : sur le même noeud, ou sur une autre carte.
- Change leurs réglages : un noeud, ou plusieurs d'un coup. Vous voyez chaque changement avant
  l'envoi.
- Un seul fichier, rien à installer. Windows et Linux. Anglais et français.

## Sommaire
- [Démarrer](#démarrer)
- [Les bases](#les-bases)
- [Pages](#pages) : [Flotte](#flotte) · [Un noeud](#un-noeud) · [Mode édition](#mode-édition) ·
  [Sauvegardes](#sauvegardes) · [Contacts](#contacts) · [Régions](#page-régions) ·
  [Réglages](#réglages) · [Appareils, Logs, Info](#appareils-logs-info)
- [Comment faire](#comment-faire)
- [Touches](#touches)
- [Fichiers](#fichiers) · [config.toml](#configtoml)
- [Langues](#langues)
- [Problèmes](#problèmes)

## Démarrer
- **Windows :** dézippez, lancez `mcmanager.exe`. Le rendu est meilleur dans Windows Terminal.
- **Linux :** décompressez (`tar xzf mcmanager-*.tar.gz`), lancez `./mcmanager`.
  - Pour l'USB, une fois : `sudo usermod -aG dialout "$USER"` (Arch : `uucp`), puis
    déconnectez-vous et reconnectez-vous.
  - Pour le Bluetooth : `bluetoothd` doit tourner (c'est le cas sur les bureaux et Raspberry Pi OS).

Branchez vos noeuds : ils apparaissent tout seuls.

```
mcmanager                 l'appli
mcmanager scan            liste les ports USB et ce qui répond, en texte simple
mcmanager --data <dossier> garde réglages et sauvegardes dans un autre dossier
```

Ce qu'il faut à vos noeuds :
- **Compagnon en USB :** le firmware compagnon **USB**. Un firmware Bluetooth ou Wi-Fi ne parle
  pas en USB (l'USB sert seulement à le flasher) : la carte apparaît comme ne répondant pas.
- **Compagnon en Bluetooth :** il doit émettre des annonces. Connecté à un téléphone, il n'en
  émet pas, donc il n'est pas vu : déconnectez-le du téléphone d'abord.
- **Répéteur, room server, capteur :** en USB (leur console texte).

## Les bases
- **Lire est libre, écrire demande.** Ouvrir, lire, comparer ne change jamais un noeud. Tout ce
  qui écrit (un réglage, une restauration, un favori) montre d'abord exactement ce qui va
  changer et demande. Un changement est envoyé une fois, jamais deux.
- **La barre du bas** montre les touches qui marchent là où vous êtes, et seulement celles-là.
- **`Échap` revient d'un pas** : ferme une fenêtre, arrête la saisie, quitte l'édition, quitte un
  noeud.
- **`x` retire ou supprime**, toujours en rouge, demande toujours avant (deux fois pour
  plusieurs).
- **Les questions** ont des boutons qui disent ce qu'ils font. `←` `→` choisissent, `Entrée`
  valide, `Échap` = non.
- **Horloge :** quand l'horloge d'un noeud a plus de 2 minutes de retard sur cet ordinateur,
  l'appli la règle en lisant le noeud, comme les applis MeshCore (un noeud sans pile d'horloge
  redémarre en 2024). C'est affiché en vert, « horloge mise à jour ». Une horloge en avance est
  montrée, jamais reculée.
- **Les secrets** (mots de passe, clé privée) sont masqués par des points jusqu'à `s`. Jamais
  dans les logs.

## Pages
`Tab` / `Maj+Tab` ou `1`-`8`.

| # | Page | Pour |
|---|---|---|
| 1 | [Flotte](#flotte) | Tous vos noeuds. En ouvrir un, sauvegarder, en modifier plusieurs d'un coup |
| 2 | [Sauvegardes](#sauvegardes) | Toutes les sauvegardes, par carte. Comparer, restaurer, supprimer |
| 3 | [Contacts](#contacts) | Votre carnet d'adresses : chaque contact et clé d'accès que vos noeuds connaissent |
| 4 | [Régions](#page-régions) | Votre liste de noms de régions, prête à donner à vos noeuds |
| 5 | [Appareils](#appareils-logs-info) | Les cartes que l'appli connaît |
| 6 | [Logs](#appareils-logs-info) | Ce qui s'est passé depuis le lancement |
| 7 | [Réglages](#réglages) | Langue, dossier des sauvegardes, PIN Bluetooth, log debug |
| 8 | [Info](#appareils-logs-info) | Version, licence |

### Flotte
Une ligne par noeud :
- **Liaison :** `USB` et son port, ou `BLE` et son signal (dBm) ;
- **Clé :** les 4 premiers caractères de sa clé publique, dans sa propre couleur, pour
  distinguer les noeuds d'un coup d'oeil ;
- type, nom, carte, firmware, dernière sauvegarde (une animation pendant une sauvegarde).

Les détails du noeud sélectionné sont sous la liste. Le titre dit si le Bluetooth écoute, n'est
pas disponible (et pourquoi), ou est coupé dans les Réglages.

Un compagnon Bluetooth est lu une fois, tout seul, dès qu'il est entendu. Le résultat s'affiche
sur sa ligne et dans la page Logs.

| Touche | Fait |
|---|---|
| `Entrée` | ouvrir le noeud |
| `b` | sauvegarder (les noeuds cochés, s'il y en a) |
| `r` / `R` | relire ce noeud / tous |
| `o` | trier : par liaison, type, nom |
| `f` | montrer un seul type : compagnons, répéteurs, room servers, capteurs |
| `s` | mode sélection (cases à cocher). `Espace` en coche un, `a` tous ou aucun, `Échap` sort |
| `e` | avec des noeuds cochés (même type) : [les modifier ensemble](#changer-plusieurs-noeuds-dun-coup) |
| `p` | un compagnon Bluetooth dont le PIN a été refusé : taper son PIN |
| `u` | désappairer un compagnon Bluetooth de cet ordinateur (les cochés, s'il y en a) |

### Un noeud
`Entrée` dessus dans la Flotte. `Tab` passe d'une section à l'autre, `Échap` revient à la
Flotte. Tant qu'un noeud est ouvert, les touches de page ne font rien : une touche de travers ne
le quitte jamais.

| Section | Montre | Touches |
|---|---|---|
| **Résumé** | Identité, radio, réglages. Un compagnon montre aussi batterie, température, capteurs et compteurs (temps de marche, bruit de fond, temps d'antenne, paquets) | `e` modifier, `s` montrer / cacher les secrets, `r` relire, `b` sauvegarder, `←` puis `↑` `↓` défiler le panneau d'infos, `p` / `u` appairer / désappairer (Bluetooth) |
| **Contacts** | Les contacts d'un compagnon, de A à Z : ★, type, clé, le nom qu'il annonce, votre nom perso | `/` chercher, `o` trier (type, dernier entendu), `n` ou `Entrée` nom perso (gardé tout de suite dans votre carnet), `*` favori (passe par le mode édition) |
| **Canaux** | Les canaux d'un compagnon (clés masquées) | `e` puis `Entrée` renommer, `x` retirer |
| **Régions** | Les régions d'un répéteur ou room server (firmware 1.10+). `*` en premier (paquets sans région), puis l'arbre. Sous une ligne, les régions de votre liste que le noeud n'a pas | `+` copier la région sélectionnée dans votre liste |
| **Accès** | Qui peut se connecter à un répéteur, room server ou capteur : chaque clé et son rôle (lecture seule, lecture / écriture, admin) | `e` puis `a` ajouter des gens, `←` `→` rôle, `x` retirer |
| **Sauvegardes** | Les sauvegardes de cette carte, la plus récente en haut | `Entrée` comparer avec le noeud maintenant, `R` restaurer, `b` sauvegarder, `Espace` cocher, `x` supprimer les cochées |

### Mode édition
`e` dans une section d'un noeud. Vous restez dans cette section jusqu'à en sortir (`Échap`, qui
demande si vous avez des changements).

- `↑` `↓` choisir un réglage, `Entrée` le changer.
  - Un réglage à quelques valeurs : choix avec `←` `→` (ou tapez une lettre pour saisir
    librement).
  - Les autres se tapent. Les limites s'affichent pendant la saisie (« 3 / 15 caractères ») et
    une valeur fausse est refusée sur place, avec la raison.
- `?` tout sur le réglage sélectionné : ce qu'il fait, ses valeurs, sa valeur par défaut, depuis
  quel firmware. Écrit à partir du firmware MeshCore.
- `p` préréglages radio, ceux des applis MeshCore officielles, par pays (tapez pour chercher).
- `u` annuler le dernier changement (encore : celui d'avant), `U` refaire.
- `c` valider : chaque changement listé (ceux de la radio marqués `!`), puis :
  - **sauver, puis écrire** : une nouvelle sauvegarde d'abord ; si elle échoue, rien n'est
    écrit ;
  - **l'écrire**.
  Chaque changement est envoyé une fois, puis le noeud est relu.

Ce que vous pouvez changer, c'est ce que le firmware permet :
- **Compagnon :** nom, position, radio, puissance, « ajouter chaque noeud entendu » et l'ajout
  auto (quels types, remplacer le plus ancien quand c'est plein, nombre de sauts max),
  télémétrie, position dans les annonces, ACK multiples, portée flood par défaut, délai rx,
  rapport cyclique, mode path hash, PIN Bluetooth, clé privée ; contacts (favori, retirer),
  canaux (renommer, retirer).
- **Répéteur, room server, capteur :** chaque réglage de la console qui a une commande `set`,
  les mots de passe admin et invité, la clé privée, les régions, la liste d'accès.

Section Régions en mode édition : `Entrée` ajoute la région sous la ligne, `a` en cherche une ou
ajoute un nouveau nom (au noeud et à votre liste), `Espace` flood permis / bloqué, `h` home, `d`
portée par défaut, `x` la retire (avec celles en dessous).

### Sauvegardes
À gauche, chaque carte déjà sauvegardée, branchée ou non (`●` = branchée). À droite, ses
sauvegardes : date, type (*manuelle*, *auto (édition)*, *auto (restauration)*...), contenu.

| Touche | Fait |
|---|---|
| `←` `→` | cartes / sauvegardes |
| `Entrée` | (gauche) voir ses sauvegardes ; (droite) comparer la sauvegarde au noeud maintenant (rien n'est écrit) |
| `c` | comparer deux sauvegardes : les 2 cochées, ou celle sélectionnée avec la précédente |
| `R` | restaurer la sauvegarde : sur sa propre carte, ou [sur une autre](#restaurer-sur-une-autre-carte) |
| `Espace` / `a` | cocher une sauvegarde / toutes ou aucune |
| `x` | (gauche) supprimer toutes les sauvegardes de la carte ; (droite) supprimer les cochées. Demande, deux fois pour plusieurs |
| `f` | montrer un seul type |

### Contacts
Votre carnet d'adresses, rempli tout seul à chaque lecture d'un noeud : chaque contact de vos
compagnons, chaque clé d'une liste d'accès. Pour chaque clé : les compagnons qui l'ont, le rôle
que chaque noeud lui donne.

Colonnes : ★, type, clé, nom, nom perso, rôle, où il est (« 3 compagnons », « accès sur 2 »).

| Touche | Fait |
|---|---|
| `/` | chercher |
| `n` / `Entrée` | nom perso : gardé dans l'appli seulement (un compagnon reprend toujours le nom annoncé par le contact) |
| `*` | favori : écrit aussi dans chaque compagnon qui l'a (demande avant) |
| `r` | rôle : lecture seule, lecture / écriture, admin. Gardé ici, utilisé pour donner accès à un répéteur |
| `o` | trier |
| `f` | montrer : tous, un type, les favoris, un rôle |
| `Espace` / `x` | cocher / retirer les cochés de la liste (jamais d'un noeud) |

### Page Régions
Votre propre liste de noms de régions, au format du firmware. La sortie de la commande `region`
d'un noeud peut être collée telle quelle.

`a` nouveau nom sous celui sélectionné, `A` en haut, `x` retirer (avec ceux en dessous, demande
avant ; seulement de la liste, jamais d'un noeud). Les noms sont en minuscules.

### Réglages
`↑` `↓` choisir, `Entrée` changer. Appliqué tout de suite, gardé dans [config.toml](#configtoml).

| Réglage | Quoi |
|---|---|
| **Langue** | `←` `→`. « Automatique » suit votre système |
| **Log debug** | oui / non. Un journal détaillé dans `logs/`, pour trouver un problème. Noms, clés et positions sont masqués |
| **Dossier des sauvegardes** | tapez ou collez un dossier (`Ctrl+U` vide la ligne), ou `o` pour la fenêtre de choix du système. Il est testé d'abord, puis vous choisissez : copier, déplacer ou laisser les sauvegardes déjà faites. Tout ou rien : chaque copie est vérifiée et le nouveau dossier enregistré avant que rien ne bascule ; **Déplacer** ne retire les anciennes qu'à ce moment-là. Si quoi que ce soit échoue, rien ne change et l'appli dit pourquoi |
| **PIN d'appairage** | le PIN essayé d'abord avec un compagnon Bluetooth (par défaut MeshCore : `123456`) |
| **Fichiers d'origine** | remet les fichiers anglais et français d'origine dans `lang/` (un fichier modifié est gardé en `.bak`) |

### Appareils, Logs, Info
- **Appareils :** les cartes que l'appli reconnaît (nom, fabricant, puce, le modèle annoncé par
  le firmware) et leurs ids USB. Une carte en mode flash est montrée comme telle et jamais
  écrite.
- **Logs :** tout depuis le lancement, le plus récent en bas.
- **Info :** version et licence.

## Comment faire

### Sauvegarder
Flotte, sélectionnez le noeud, `b`. C'est fait : écrit tout de suite à partir de la dernière
lecture du noeud (un noeud pas encore lu est lu d'abord). Plusieurs : `Espace` sur chacun (ou
`a`), puis `b`.

Deux fois la même lecture, c'est refusé : `r` relit le noeud d'abord.

Ce qu'il y a dedans :
- **compagnon :** réglages, radio, identité (clé privée), canaux, contacts, ajout auto, PIN
  Bluetooth ;
- **répéteur, room server, capteur :** chaque réglage, mots de passe, clé privée, régions, liste
  d'accès ;
- vos noms perso, rôles et favoris pour les clés du noeud.

⚠️ Une sauvegarde contient la clé privée et les clés des canaux. Gardez vos sauvegardes pour
vous.

### Voir ce qui a changé
- `Entrée` sur une sauvegarde : la compare au noeud tel qu'il est maintenant, réglage par
  réglage.
- `c` sur la page Sauvegardes : compare deux sauvegardes (réglages, canaux, contacts : ajoutés,
  retirés, changés).

Les secrets ne sont jamais montrés, seulement « remplacés ».

### Restaurer
Section **Sauvegardes** d'un noeud, sélectionnez la sauvegarde, `R`. Vous voyez chaque
changement en clair (« ajout auto des répéteurs : non → oui »), puis **restaurer**.

Seul ce qui diffère est envoyé et vérifié. Si ça s'arrête en route (câble débranché, noeud
muet), c'est dit : relancez et le reste est envoyé.

Contacts d'un compagnon : ceux qui manquent reviennent tant qu'il y a de la place, ceux ajoutés
depuis sont gardés, noms et routes sont laissés au réseau.

### Restaurer sur une autre carte
Carte cassée ou remplacée ? Page **Sauvegardes** : sélectionnez la sauvegarde, `R`, choisissez
le noeud cible (même type), puis :
- **Déplacer** (un clone) : la nouvelle carte devient l'ancienne (même clé, même nom). L'appli
  demande deux fois. ⚠️ Jamais les deux allumées en même temps : le réseau verrait deux fois le
  même noeud.
- **Copier** : radio, réglages, canaux, contacts. La nouvelle carte garde son nom, sa clé, sa
  position et son PIN Bluetooth.

Ce que l'autre carte ne peut pas prendre (autre puce radio, firmware plus ancien...) reste tel
quel et est listé à la fin.

### Changer des réglages
Ouvrez le noeud, `e`, changez, `c`. Voir [Mode édition](#mode-édition).

### Changer plusieurs noeuds d'un coup
Flotte : `s`, `Espace` sur chaque noeud (même type), `e`.
- Vous avez les réglages qu'ils ont tous (sans nom, clé ni position).
- Un réglage qui diffère affiche « n valeurs différentes » ; celui sélectionné montre un
  tableau : chaque noeud, sa valeur maintenant, après.
- `p` un préréglage radio pour tous, `a` une région pour tous.
- `c` liste les changements noeud par noeud, puis écrit chacun.

### Donner accès à un répéteur
Ouvrez-le, **Accès**, `e`, `a` : choisissez des gens dans vos Contacts (`←` `→` un rôle,
`Espace` choisir, `+` tout le rôle, tapez pour chercher). Ils reçoivent le rôle mis sur la page
Contacts, sinon lecture seule. `c` pour valider.

Astuce : donnez d'abord un rôle à vos gens sur la page Contacts (`r`) : `a` s'ouvre alors sur
vos admins.

### Appairage Bluetooth
En général rien à faire : un compagnon est lu dès qu'il est entendu, avec le PIN de son dernier
appairage, sinon celui des Réglages.

PIN refusé ? Sa ligne montre `p` : appuyez, tapez le PIN à 6 chiffres (sur son écran, ou réglé
dans l'appli MeshCore). L'appli le retient. `u` le désappaire de cet ordinateur (le compagnon
garde ses réglages).

Le PIN d'un compagnon est un réglage du mode édition : vous pouvez donner le même à plusieurs
compagnons d'un coup.

## Touches
| Touche | Fait |
|---|---|
| `Tab` `Maj+Tab`, `1`-`8` | changer de page (dans un noeud : ses sections) |
| `↑` `↓` `PgUp` `PgDn` `Home` `End` | se déplacer |
| `Entrée` | ouvrir / changer / comparer |
| `Échap` | revenir d'un pas |
| `r` / `R` | relire / tous (Flotte) |
| `o` / `f` / `/` | trier / montrer un type / chercher |
| `s` | Flotte : mode sélection. Dans un noeud : montrer / cacher les secrets |
| `Espace` / `a` | cocher un / tous ou aucun |
| `b` | sauvegarder |
| `e` / `c` | modifier / valider |
| `u` / `U` | annuler / refaire |
| `?` / `p` | aide sur un réglage / préréglages radio (mode édition) |
| `p` / `u` | appairer / désappairer un compagnon Bluetooth (Flotte, Résumé) |
| `n` / `*` / `r` | nom perso / favori / rôle (Contacts) |
| `R` | restaurer (Sauvegardes) |
| `x` | retirer ou supprimer (en rouge, demande toujours) |
| `q` | quitter (demande si quelque chose tourne) |

## Fichiers
Tout est dans `mcmanager-data`, à côté du programme. Ailleurs : `--data <dossier>`, ou la
variable `MCMANAGER_DATA`. Rien n'est écrit ailleurs.

| Fichier | Quoi |
|---|---|
| `config.toml` | [Réglages](#configtoml). Le précédent est gardé en `.bak` |
| `backups/<nom>_<clé>/<date_heure>/` | Les sauvegardes (ou le dossier choisi), un dossier par carte, un par sauvegarde |
| `contacts.toml` | Le carnet d'adresses. Le précédent est gardé en `.bak` |
| `bluetooth.toml` | Le PIN du dernier appairage de chaque compagnon. ⚠️ Avec ce fichier, n'importe qui peut s'appairer à vos compagnons |
| `regions.txt` | Vos noms de régions |
| `lang/` | Les fichiers de langue |
| `logs/` | Le journal de débogage, seulement s'il est activé. Gardé jusqu'à ce que vous le supprimiez |
| `devices.toml` | Optionnel : vos propres cartes (format : [data/devices.toml](data/devices.toml)) |
| `radio_presets.toml` | Optionnel : vos propres préréglages radio (format : [data/radio_presets.toml](data/radio_presets.toml)) |

Les sauvegardes suivent la carte (son numéro de série USB, sinon sa clé) : un noeud renommé dans
une appli MeshCore garde ses sauvegardes ensemble. L'appli ne supprime jamais une sauvegarde
d'elle-même ; vous seul le faites, page Sauvegardes.

Quand le dossier des sauvegardes choisi a disparu, l'appli le dit : supprimé, le dossier par
défaut prend le relais ; sur un disque pas branché, le dossier par défaut sert en attendant et
le réglage est gardé.

## config.toml
Créé au premier lancement, chaque ligne expliquée. Changez-le dans la page Réglages ou à la main
(supprimez une ligne pour retrouver sa valeur par défaut). Une version plus récente ajoute ses
nouveaux réglages et n'en renomme jamais.

| Réglage | Par défaut | Quoi |
|---|---|---|
| `[ui] language` | `"auto"` | `auto`, ou un code de langue (`en`, `fr`...) |
| `[scan] every_secs` | `2` | tous les combien les ports USB sont regardés (secondes) |
| `[scan] answer_ms` | `4000` | le temps qu'a un noeud pour répondre quand il est branché. Un ESP32 redémarre quand son port s'ouvre : en dessous de 3000 il peut être raté |
| `[backup] private_key` | `true` | garder la clé privée dans les sauvegardes (nécessaire pour restaurer la même identité) |
| `[backup] hardware_id` | `true` | garder le numéro de série USB de la carte, pour reconnaître « la même carte » |
| `[backup] folder` | `""` | où vont les sauvegardes ; vide = `backups` dans le dossier de données |
| `[bluetooth] enabled` | `true` | chercher les compagnons Bluetooth |
| `[bluetooth] pin` | `"123456"` | le PIN d'appairage essayé d'abord |
| `[debug] log` | `false` | le journal de débogage |

## Langues
Anglais et français intégrés. **Automatique** suit les langues de votre système.

Les fichiers sont copiés dans `lang/en/` et `lang/fr/` :
- **Corriger un texte :** modifiez-le là. Votre fichier n'est jamais écrasé ; un fichier non
  modifié suit les mises à jour.
- **Nouvelle langue :** copiez `lang/en/` vers `lang/<code>/` (`de`, `es`, `pt-BR`...),
  traduisez, choisissez-la dans Réglages. Gardez les ids et les `{ $variables }`, et à peu près
  la longueur de l'anglais. Ce qui n'est pas traduit s'affiche en anglais.
- **Revenir aux originaux :** Réglages > **Fichiers d'origine** (le vôtre est gardé en `.bak`).

## Problèmes
| Ce que vous voyez | Que faire |
|---|---|
| Un compagnon en USB « ne répond pas » | Il a un firmware Bluetooth ou Wi-Fi : flashez le firmware compagnon USB, ou utilisez-le en Bluetooth |
| Une carte branchée est ratée | Augmentez `answer_ms` dans config.toml (un ESP32 redémarre quand son port s'ouvre) |
| Linux : un port ne s'ouvre pas | Votre utilisateur n'est pas dans `dialout` (Arch : `uucp`) : voir [Démarrer](#démarrer) |
| Un compagnon Bluetooth n'est pas listé | Il est connecté à un téléphone (déconnectez-le), éteint, ou trop loin |
| « Bluetooth : pas disponible » | Pas d'adaptateur Bluetooth, ou il est coupé. Linux : `bluetoothd` ne tourne pas |
| Le PIN est refusé | `p` sur sa ligne, tapez le PIN affiché sur son écran ou réglé dans l'appli MeshCore |
| Une restauration s'est arrêtée | Relancez-la : elle envoie le reste |
| Caractères bizarres ou carrés à l'écran | Utilisez Windows Terminal (Windows) ou un terminal UTF-8 |

## Licence
Fed1291 · [CC BY-NC-SA 4.0](../LICENSE)
