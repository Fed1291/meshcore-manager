# Meshcore Manager - Français (traduit de l'anglais, la référence : chaque message y existe).
# Format : Project Fluent (https://projectfluent.org). Les ids sont en kebab-case, groupés par écran.
# Ce fichier est aussi dans lang/fr/ du dossier de données : le programme le lit de là, un texte
# corrigé là est utilisé. Réglages > "Fichiers d'origine" remet les fichiers anglais et français
# tels qu'ils sont venus (un fichier modifié est gardé en .bak). Gardez chaque texte à peu près
# aussi long que l'anglais : les écrans sont prévus pour cette longueur.

# Comment cette langue se nomme, dans la page Réglages
language-name = Français

## Le cadre de l'app
app-name = Meshcore Manager
app-by = (par Fed1291)
tab-fleet = Flotte
tab-backups = Sauvegardes
tab-contacts = Contacts
tab-regions = Régions
people-about = Chaque contact lu sur vos compagnons et chaque clé d'une liste d'accès, gardés ici tout seuls (contacts.toml). Un nom perso et un rôle restent dans ce programme seulement : un compagnon montre toujours le nom que chaque noeud annonce (il le remplace à chaque annonce). Un favori (*) est écrit sur les compagnons qui ont le contact.
people-title = { $count ->
    [one] 1 clé
   *[other] { $count } clés
}  ·  { $sort }
people-empty = Rien encore : ouvrez un compagnon (ses contacts) ou un répéteur (sa liste d'accès) dans la Flotte, et ce qu'il contient arrive ici.
people-held = { $count ->
    [one] sur 1 compagnon
   *[other] sur { $count } compagnons
}
people-access = { $count ->
    [one] dans 1 liste d'accès
   *[other] dans { $count } listes d'accès
}
people-remove-ask = Retirer { $name } du carnet d'adresses ?
people-remove-ask-n = Retirer ces { $count } contacts du carnet d'adresses ?
people-remove-back = De cette liste seulement, jamais d'un noeud. Il revient quand un noeud relu le contient encore.
people-remove-yes = le retirer
key-role = rôle
col-role = rôle
people-favourites = favoris
people-favourite-ask = { $name } : { $what } sur { $count ->
    [one] 1 compagnon
   *[other] { $count } compagnons
} ?
people-favourite-away = Pas lus maintenant, donc pas écrits : { $nodes }. Lisez-les dans la Flotte, puis * à nouveau.
col-on = sur
field-advertised = s'annonce comme
field-held-by = sur
field-access = accès
field-role = votre rôle pour lui
tab-boards = Appareils
tab-log = Logs
tab-settings = Réglages
tab-about = Info

## Page Réglages
settings-about-page = Les réglages de ce programme, dans config.toml (chaque changement s'applique aussitôt). Les réglages des noeuds sont dans leur vue (Flotte, Enter).
settings-selected = Choisi
settings-language = Langue
settings-language-auto = automatique ({ $name })
settings-language-about = La langue de tous les textes. Automatique : celle du système, sinon l'anglais. Un texte absent d'une langue s'affiche en anglais.
settings-language-folder = Fichiers de langue : { $folder }, un dossier par code (en, fr...) avec son mcmanager.ftl. Copiez-en un pour commencer une autre langue.
settings-debug = Log debug
settings-on = oui
settings-off = non
settings-debug-about = Écrit chaque action et erreur, avec la partie du programme d'où elle vient, une ligne JSON chacune : pour trouver un problème ou joindre à un rapport de bug. Jamais une clé, un mot de passe, le nom, la position ou les messages d'un noeud (écrits <node>, <key>, <hex>...). Les fichiers restent jusqu'à ce que vous les supprimiez.
settings-debug-file = Fichier du jour : { $file }
settings-backups-folder = Dossier sauvegardes
settings-backups-folder-about = Où les sauvegardes sont enregistrées. Un autre disque ou un dossier synchronisé convient aussi. Le dossier est testé avant tout ; puis vous choisissez : copier les sauvegardes déjà faites, les déplacer, ou les laisser où elles sont.
settings-folder-change = Enter : taper ou coller un autre dossier (guillemets, espaces, ~ pour votre dossier perso acceptés). o : le choisir dans la fenêtre du système.
settings-folder-typing = Taper ou coller remplace le dossier affiché ; Backspace le modifie. Enter : tester ce dossier, Esc : annuler. Vide : sauvegardes dans le dossier de données.
settings-key-type = taper, coller
settings-key-browse = parcourir
settings-key-clear = vider
settings-folder-window = Dossier des sauvegardes
settings-folder-no-desktop = Pas de fenêtre de dossier ici : cet ordinateur n'a pas de bureau (console ou SSH).
settings-folder-no-picker = Pas de fenêtre de dossier ici : ni zenity ni kdialog n'est installé.
settings-folder-type-instead = Tapez ou collez plutôt le dossier : Enter sur le réglage Dossier sauvegardes.
settings-debug-writing = actif
settings-group-display = Affichage
settings-group-diagnostics = Diagnostic
settings-group-backups = Sauvegardes
settings-group-translation = Traduction
settings-group-bluetooth = Bluetooth
settings-ble-pin = PIN d'appairage
settings-ble-pin-about = Le PIN essayé en premier quand un compagnon Bluetooth s'appaire à cet ordinateur (la première fois seulement). Par défaut 123456. Un compagnon avec écran affiche le sien, et chacun peut avoir le sien : s'il refuse celui-ci, le programme demande son PIN (Flotte, `p` aussi).
settings-key-digits = chiffres
settings-folder-files-moved = Autres fichiers déplacés : { $count }.
settings-folder-files-copied = Autres fichiers copiés : { $count }.
settings-folder-left = { $count } fichiers restent dans { $folder } (une restauration non terminée en a besoin, ou ils n'ont pas pu être retirés) : leurs copies vérifiées sont dans le nouveau dossier, rien n'est perdu.
settings-folder-not-text = Le nom de ce dossier n'est pas un texte valide (des octets d'un autre encodage) : il ne peut pas être enregistré dans les réglages. Choisissez ou créez un dossier au nom simple.
settings-folder-not-carried = Tout n'a pas pu être transféré dans le nouveau dossier : rien n'a changé. Aucune sauvegarde déplacée ni retirée, les copies faites là-bas ont été retirées.
settings-folder-not-saved = Le nouveau dossier n'a pas pu être enregistré dans les réglages : rien n'a changé. Aucune sauvegarde déplacée ni retirée, les copies faites là-bas ont été retirées.
settings-folder-kept = Les sauvegardes vont toujours dans { $folder }.
log-copy-exists = un autre { $file } est déjà là (jamais écrasé)
log-copy-differs = la copie { $file } ne correspond pas à l'original

## Bluetooth
ble-no-adapter = Aucun adaptateur Bluetooth (intégré ou clé USB).
ble-not-on-system = Les compagnons Bluetooth ne sont pas encore pris en charge sur ce système : utilisez l'USB.
ble-off = Le Bluetooth est éteint, ou son adaptateur débranché.
ble-radio-off = Le Bluetooth est éteint sur cet ordinateur (Windows : Paramètres > Périphériques > Bluetooth).
ble-radio-disabled = Le Bluetooth est désactivé sur cet ordinateur (mode avion, un interrupteur, ou l'adaptateur désactivé).
ble-blocked = Le système ne laisse pas les programmes utiliser le Bluetooth (Windows : Paramètres > Confidentialité, Bluetooth / Radios).
ble-policy = Le Bluetooth est désactivé par une stratégie de cet ordinateur.
ble-busy = L'adaptateur Bluetooth est utilisé par un autre programme.
ble-no-le = Cet adaptateur Bluetooth n'a pas le Bluetooth basse consommation (4.0 ou plus récent requis).
ble-no-bluez = Service Bluetooth injoignable (bluetoothd : sudo systemctl start bluetooth).
ble-scan-failed = La recherche Bluetooth n'a pas pu démarrer.
ble-scan-stopped = La recherche Bluetooth s'est arrêtée.
ble-bad-pin = Un PIN a 6 chiffres.
ble-pin-refused = Appairage refusé : le PIN essayé n'est pas celui de ce compagnon.
ble-bad-address = { $address } n'est pas une adresse Bluetooth.
ble-not-found = { $address } introuvable : hors de portée, éteint, ou connecté à un téléphone (déconnectez-le là).
ble-pair-failed = Échec de l'appairage ({ $error }).
ble-connect-failed = Connexion impossible ({ $error }).
ble-connect-timeout = Le compagnon n'a pas répondu à temps (hors de portée, ou occupé avec un téléphone).
ble-no-service = Pas de service MeshCore sur cet appareil : pas un firmware compagnon ?
ble-not-paired = Le compagnon refuse : l'appairage manque d'un côté. Retirez-le dans les réglages Bluetooth du système et réessayez.
ble-closed = La connexion Bluetooth est fermée.
ble-notify-stopped = le compagnon n'envoie plus rien
ble-disconnected = le compagnon s'est déconnecté
ble-write-failed = Écriture Bluetooth échouée ({ $error }).
ble-write-timeout = Écriture Bluetooth trop longue.
ble-call-timeout = Le Bluetooth n'a pas répondu à temps.
ble-lost = Connexion Bluetooth perdue : { $why }.
log-ble-problem = Bluetooth : { $why } (l'USB marche toujours).
node-ble-not-read = Entendu en Bluetooth. Il se connecte et est lu tout seul, avec le PIN qu'il a (celui donné, son PIN gardé, sinon celui par défaut) ; un PIN n'est demandé que s'il est refusé.
node-ble-silent = Connecté en Bluetooth, mais aucun compagnon n'a répondu.
node-link-ble = Bluetooth { $name }
node-link-usb = USB { $name }
field-bluetooth = Bluetooth
col-link = lien
sort-link = lien
sort-kind = type
sort-name = nom
sort-heard = dernier entendu
key-sort = trier : { $now }
node-contacts-sorted = { $count ->
    [one] 1 contact
   *[other] { $count } contacts
}  ·  { $sort }
node-contacts-found = { $title }  ·  { $count } trouvés
node-contacts-none-found = Aucun contact ne contient ce texte. Esc efface la recherche.
key-search = chercher
key-find-clear = effacer la recherche
key-own-name = nom perso
key-save = enregistrer
col-own-name = nom perso
field-also-on = aussi sur
set-autoadd-chat = ajout auto compagnons
set-autoadd-repeater = ajout auto répéteurs
set-autoadd-room = ajout auto salons
set-autoadd-sensor = ajout auto capteurs
set-autoadd-overwrite = plein : remplacer ancien
set-autoadd-hops = ajout auto sauts max
group-no-radio = Ces noeuds n'ont pas de réglages radio en commun.
settings-folder-bad-char = Un dossier ne peut contenir de caractères de contrôle.
settings-key-test = tester ce dossier
settings-originals = Fichiers d'origine
settings-originals-button = remettre les fichiers anglais et français
settings-originals-about = Les fichiers anglais et français sont dans le dossier lang et sont lus de là : un texte corrigé là est utilisé. Une copie non modifiée suit chaque nouvelle version d'elle-même ; une copie modifiée n'est jamais touchée. Ceci remet les originaux.
settings-originals-ask = Réécrire les fichiers anglais et français d'origine dans le dossier lang ?
settings-originals-kept = Un fichier que vous avez modifié est gardé à côté, renommé .bak (avec la date).
settings-originals-yes = les remettre
settings-key-originals = remettre
settings-file = Fichier de réglages : { $file }
settings-not-saved = Le réglage est appliqué mais n'a pu être enregistré : il sera perdu à la fermeture du programme.
settings-folder-busy = Des tâches tournent : changez le dossier des sauvegardes quand elles sont finies.
settings-folder-unusable = Ce dossier ne peut pas recevoir les sauvegardes : rien n'a changé.
settings-folder-nested = Un dossier est dans l'autre : choisissez un dossier hors du dossier actuel des sauvegardes (et qui ne le contient pas).
settings-folder-ok = { $folder } peut recevoir les sauvegardes (testé : un fichier écrit puis relu).
settings-folder-nothing = Aucune sauvegarde à transférer.
settings-folder-use = utiliser celui-ci
settings-folder-has = { $count ->
    [one] { $count } sauvegarde est dans le dossier actuel ({ $size } Mo).
   *[other] { $count } sauvegardes sont dans le dossier actuel ({ $size } Mo).
}
settings-folder-how = Copier : chacune copiée et vérifiée, les anciennes restent. Déplacer : toutes copiées et vérifiées, le nouveau dossier enregistré, alors seulement les anciennes retirées. Laisser : seules les nouvelles y vont. Au moindre échec, rien ne change.
settings-folder-copy = les copier là-bas
settings-folder-move = les déplacer
settings-folder-leave = les laisser
settings-folder-move-sure = { $count ->
    [one] Déplacer { $count } sauvegarde ? Elle n'est retirée de l'ancien dossier qu'une fois sa copie vérifiée et le nouveau dossier enregistré. Une sauvegarde utile à une restauration y reste.
   *[other] Déplacer { $count } sauvegardes ? Elles ne sont retirées de l'ancien dossier qu'une fois toutes les copies vérifiées et le nouveau dossier enregistré. Une sauvegarde utile à une restauration y reste.
}
settings-folder-now = Les sauvegardes vont désormais dans { $folder }.
settings-folder-copied = Sauvegardes copiées et vérifiées : { $done }, déjà là : { $there }.
settings-folder-moved = Sauvegardes déplacées (chaque copie vérifiée d'abord) : { $done }, déjà là : { $there }.
settings-originals-done = Réécrits : { $files }
settings-originals-same = Les fichiers de langue sont déjà les originaux : rien d'écrit.
settings-originals-failed = Les fichiers de langue n'ont pas pu être écrits.
lang-not-written = Le fichier de langue { $file } n'a pas pu être écrit ({ $error }) : les textes intégrés sont utilisés.
lang-no-folder = Il n'y a pas de dossier lang.
log-copy-failed = { $what } n'a pas pu être transféré
folder-unusable = Impossible d'utiliser le dossier { $folder } (impossible de le créer, ou d'y écrire et relire un fichier)
backups-folder-fallback = Le dossier des sauvegardes des réglages est inutilisable ({ $error }) : les sauvegardes vont dans { $folder } en attendant.
backups-folder-gone = Le dossier des sauvegardes { $folder } n'existe plus (supprimé, déplacé ou renommé).
backups-folder-gone-now = Les sauvegardes reviennent au dossier par défaut, { $default }, et le réglage aussi. Choisissez un autre dossier dans Réglages si vous le souhaitez.
backups-folder-away = Le dossier des sauvegardes { $folder } est injoignable : son disque ou son partage réseau n'est pas là.
backups-folder-away-now = Les sauvegardes vont dans { $default } en attendant. Le réglage est gardé : branchez le disque, puis relancez le programme.
config-by-hand = { $file } : { $key } est écrit d'une façon que ce programme ne modifie pas (une table inline) : modifiez-le là à la main.
file-too-big = le fichier dépasse { $max } Ko : non lu

## Page Info
about-name = Meshcore Manager { $version }
about-share-title = Vous l'avez modifié ? Partagez-le.
about-share = Si vous modifiez ce programme et diffusez votre version, la licence demande de la partager sous la même licence, en citant l'auteur. Publiez aussi vos changements là où d'autres les trouvent, même de petits correctifs : ainsi chaque utilisateur MeshCore en profite, et les outils de toute la communauté MeshCore progressent ensemble.
about-license-title = Licence : CC BY-NC-SA 4.0
about-license = Libre d'utiliser, partager et modifier, à condition de citer l'auteur (BY), de ne pas en faire un usage commercial (NC), et de partager ce que vous en tirez sous la même licence (SA). Le texte complet :
about-license-link = creativecommons.org/licenses/by-nc-sa/4.0
about-meshcore-title = MeshCore
about-meshcore = Fait pour les noeuds MeshCore (github.com/meshcore-dev/MeshCore). L'aide des réglages est tirée du firmware MeshCore et de sa documentation (licence MIT).
key-switch = changer
key-quit = sortir
key-select = choisir
key-scroll = défiler
key-panel = noeuds/sauvegardes
key-its-backups = ses sauvegardes
key-backup = sauver
key-rescan-one = le rescanner
key-rescan-all = tout rescanner
key-restore = restaurer
key-open = ouvrir
key-back = retour
key-back-to-list = retour à la liste
key-close = fermer
key-section = section
key-other-panel = autre volet
key-read-again = relire
key-only = afficher : { $now }
only-all = tous
only-title = { $title }  [{ $kind }]
only-none = Aucun de ce type. f pour afficher un autre type.
key-edit = éditer
key-ok = valider
key-cancel = annuler
key-leave-edit = fin édition
key-change = modifier
key-favourite = favori
key-remove = retirer
# undo / redo stay in English in every language: short, and known on every keyboard
key-undo = undo
key-redo = redo
key-commit = écrire ({ $count })
popup-ok = OK
popup-keys-question = ← →  choisir    Enter  valider    Esc  annuler
popup-keys-note = Enter / Esc  fermer
answer-cancel = annuler

## Types de noeud
kind-companion = compagnon
kind-repeater = répéteur
kind-room-server = salon
kind-sensor = capteur
kind-bootloader = mode flash
kind-unknown = inconnu

## Colonnes de tableau et lignes "libellé  valeur"
col-port = port
col-kind = type
col-name = nom
col-board = appareil
col-firmware = firmware
col-date = date
col-contents = contenu
col-state = état
col-reason = type
col-maker = marque
col-chip = puce
col-reported = se présente comme
col-usb-id = id USB
col-boards = souvent
col-last-backup = dern. sauveg.
col-type = type
col-route = route
col-heard = entendu
col-key = clé
col-slot = slot
field-port = port
field-usb = USB
field-hardware-id = id matériel
field-board = appareil
field-firmware = firmware
field-public-key = clé publique
field-radio = radio
field-node = noeud
field-created = créée
field-reason = type
field-contents = contenu
field-last-backup = dern. sauvegarde
field-heard = entendu
field-position = position
field-route = route

## Il y a combien de temps
age-never = jamais
age-now = récent
age-minutes = { $n } min
age-hours = { $n } h
age-days = { $n } j

## Ce qu'un noeud dit de lui-même (sa propre vue)
fact-max-power = puissance max
fact-capacity = place pour
fact-capacity-value = { $contacts } contacts, { $channels } canaux
fact-battery = batterie
fact-storage = stockage utilisé
more-above = ▲ suite
more-below = ▼ suite
fact-clock = horloge
fact-clock-synced = { $now }  ·  horloge mise à l'heure de cet ordinateur (était { $was })
fact-private-key = clé privée
fact-unreadable = le noeud ne peut le dire
key-secrets-show = afficher secrets
key-secrets-hide = cacher secrets

## Les réglages d'un compagnon (firmware : CMD_SET_* dans companion_radio/MyMesh.cpp)
set-name = nom
set-lat = latitude
set-lon = longitude
set-freq = fréquence MHz
set-bw = bande kHz
set-sf = étalement (SF)
set-cr = codage (CR)
set-tx = puissance dBm
set-manual-add = ajouter tout noeud entendu
set-advert-loc = position en annonce
set-multi-acks = ACK multiples
set-telemetry-base = télémétrie : base
set-telemetry-loc = télémétrie : position
set-telemetry-env = télémétrie : capteurs
set-rx-delay = délai rx
set-duty-cycle = rapport cyclique %
set-path-hash-mode = mode hash chemin
set-position = position
set-radio = radio (fréquence, bande, étalement, codage)
fact-ble-pin = PIN Bluetooth

## Mode config (e dans la vue d'un noeud) et l'écriture
edit-title = Config
edit-badge = CONFIG
edit-pending = { $count ->
    [0] rien de modifié
    [one] 1 modif non envoyée
   *[other] { $count } modifs non envoyées
}
node-own-name-for = Nom perso pour { $name } :
node-contacts-checked = { $title }  ·  { $count } cochés (x : les retirer)
leave-title = Fin d'édition
leave-ask = { $count ->
    [one] Vous avez { $count } changement non écrit sur le noeud :
   *[other] Vous avez { $count } changements non écrits sur le noeud :
}
leave-not-sent = Rien n'a encore été envoyé au noeud. Pour les garder : Esc, puis c pour écrire.
leave-yes = { $count ->
    [one] jeter et sortir
   *[other] jeter et sortir
}
leave-no = continuer
commit-yes = { $count ->
    [one] l'écrire
   *[other] tout écrire ({ $count })
}
commit-no = continuer
commit-backup-first = sauver, puis écrire
commit-backup-who = { $count ->
    [one] « { $first } » : une nouvelle sauvegarde de { $nodes } d'abord (si elle échoue, rien n'est écrit). « { $now } » : aucune sauvegarde.
   *[other] « { $first } » : une nouvelle sauvegarde de chaque noeud écrit d'abord ({ $nodes } ; un noeud dont elle échoue n'est pas écrit). « { $now } » : aucune sauvegarde.
}
backup-yes = { $count ->
    [one] le sauver
   *[other] sauver les { $count }
}
edit-read-first = Lisez d'abord le noeud (r) : ses réglages viennent de cette lecture.
edit-selected = Réglage choisi
edit-values = valeurs
edit-now = actuel
edit-new = nouveau
edit-danger-long = ! Change la façon dont le noeud émet : tout noeud avec qui il parle doit avoir les mêmes réglages radio.
edit-select-hint = Enter: modifier   ?: tout savoir   u: annuler   c: tout écrire
edit-typing-hint = Tapez la valeur. Enter: valider (envoyé avec c)   Esc: annuler
edit-pick-hint = ← →: choisir   Enter: valider (envoyé avec c)   Esc: annuler   une lettre: taper une autre valeur
key-pick = choisir
key-type-instead = saisir
key-help = aide
key-presets = préréglages
key-use-preset = prendre celui-ci
key-find = trouver
edit-preset = préréglage
edit-radio-hint = Enter: modifier   p: préréglage pays   ?: tout savoir   c: écrire
presets-title = Préréglages radio
presets-find = Chercher
presets-ids = ids de { $size } octets
presets-no-match = Aucun préréglage n'a cela dans son nom.
presets-none = aucun préréglage (réglages propres)
presets-note = Proposés par la communauté MeshCore, comme dans les apps officielles. ● = ce que le noeud utilise. En choisir un prépare seulement les changements : vous les voyez, ils partent avec l'écriture.
presets-unreadable = Vos préréglages radio { $file } sont illisibles ; la liste intégrée est utilisée : { $error }
edit-out-of-range-of = { $what } doit être { $range }
edit-not-a-whole-number = { $what } doit être un nombre entier
edit-not-a-choice = { $what } doit être l'un de : { $values }
edit-bad-radio = La radio doit être freq,bw,sf,cr : 150-2500 MHz, 7-500 kHz, SF 5-12, CR 5-8 (par exemple 869.618,62.5,8,5)
edit-bad-char = { $what } ne peut contenir "{ $char }"
edit-range = plage
edit-default = défaut
edit-since = depuis firmware
edit-or-off = ou 0 = arrêt
edit-key-too-long = { $what } est plein : { $max } caractères max (limite du noeud)
edit-key-hex = { $what } ne prend que des chiffres hex : 0-9 et A-F
edit-key-whole = { $what } prend un nombre entier : { $range }
edit-key-number = { $what } prend un nombre : { $range }
edit-up-to = texte, { $max } caractères max
edit-room = { $used } / { $max } caractères · reste { $left }
help-unreadable = Votre aide des réglages { $file } est illisible et non utilisée : { $error }
help-values = Valeurs
help-source = Tiré du firmware MeshCore et de sa documentation (licence MIT, Scott Powell / rippleradios.com)
help-close = Esc / ? / Enter: fermer    ↑↓: défiler
help-none = Pas encore d'aide pour ce réglage. Ses propres mots : { $about }
edit-removed = retirer
edit-secret-line = { $what } : remplacé (non affiché)
edit-bad-hex = { $what } doit faire { $digits } chiffres hex (0-9, A-F)
edit-bad-key = La clé privée doit faire 128 chiffres hex
edit-disabled = ce firmware a été compilé sans cette fonction
edit-refused-secret = le noeud ne l'a pas pris (réponse non affichée : elle peut contenir le mot de passe)
edit-line = { $what } : { $old } → { $new }
edit-contact = contact { $name } : { $what }
edit-contact-remove = retirer le contact { $name }
edit-favourite-on = favori
edit-favourite-off = pas favori
edit-channel-rename = canal { $slot } : { $old } → { $new }
edit-channel-remove = retirer le canal { $slot } ({ $name })
edit-not-a-number = { $what } : "{ $value }" n'est pas un nombre
edit-out-of-range = { $what } doit être entre { $low } et { $high }
edit-not-on-off = { $what } : "{ $value }" doit être on ou off
edit-bad-text = { $what } ne peut être vide ni contenir de caractères de contrôle
edit-error-code = erreur { $code }
edit-other-node = Un autre noeud est sur ce port : rien n'a été envoyé.
edit-wrong-kind = pas pour ce type de noeud
edit-region-remove = retirer la région { $name }
edit-region-add = ajouter la région { $name }
edit-region-flood-on = région { $name } : flood permis
edit-region-flood-off = région { $name } : pas de flood
edit-region-home = région locale → { $name }
edit-region-no-home = plus de région locale
edit-region-default = portée par défaut → { $name }
edit-region-no-default = pas de portée par défaut
edit-region-save = enregistrer les régions sur le noeud
commit-title = Écriture
commit-ask = { $count ->
    [one] Écrire { $count } changement sur { $name } ?
   *[other] Écrire { $count } changements sur { $name } ?
}
commit-danger = ! change la façon dont le noeud émet : tout noeud avec qui il parle doit avoir les mêmes réglages radio.
commit-once = Chaque changement est envoyé une fois. Le noeud est ensuite relu : la vue montre alors ce qu'il contient vraiment.

## Démarrage, fichiers
data-folder-failed = Impossible d'utiliser le dossier de données { $folder }
config-unwritable = Impossible d'écrire le fichier de réglages { $file }
config-unreadable = Impossible de lire le fichier de réglages { $file }
config-empty = Le fichier de réglages { $file } est vide. Supprimez-le pour en avoir un neuf avec les défauts.
devices-unreadable = Votre liste d'appareils { $file } est illisible et non utilisée : { $error }

## Ports série et noeuds
serial-open-failed = { $port } ne s'ouvre pas (débranché, ou pris par un autre programme) : { $error }
serial-lost = Connexion au noeud perdue
node-bootloader = L'appareil est en mode flash : aucun firmware ne tourne. Réinitialisez-le pour l'utiliser.
node-silent = Pas de réponse. Un compagnon doit avoir le firmware USB (pas Bluetooth ni Wi-Fi) ; les autres firmwares ne sont pas encore reconnus.
node-silent-wireless = Un appareil MeshCore, mais son firmware ne répond pas en USB : sans doute un compagnon au firmware Bluetooth (ou Wi-Fi), qui ne parle que par radio. L'USB sert encore à le flasher (via le bootloader). Pour l'utiliser ici : flashez le firmware compagnon USB, ou attendez le lien Bluetooth (prévu).

## Page Flotte
fleet-title = { $count ->
    [0] Noeuds
    [one] 1 noeud
   *[other] { $count } noeuds
}
fleet-looking = Lecture des ports USB...
fleet-empty = Aucun périphérique série USB. Branchez un noeud : il apparaît ici tout seul.
fleet-probing = identification  { $secs } s
fleet-details = Noeud choisi
fleet-open-hint = Enter : ouvrir le noeud (contacts, canaux, sauvegardes...)
fleet-never = jamais
fleet-backing-up = sauvegarde
fleet-checked = { $title }  ·  { $count } coché(s)
key-check = cocher
key-check-all = tout cocher
key-uncheck-all = tout décocher
key-unpair-n = dissocier { $count }
key-uncheck = tout décocher
key-select-mode = sélection
key-select-leave = quitter la sélection
key-pin = appairer (PIN)
pin-hint = tapez son PIN et appairez (un compagnon avec écran l'affiche)
stat-uptime = en marche depuis
stat-queue = paquets en attente
stat-noise = bruit de fond
stat-last-packet = dernier paquet reçu
stat-air-time = temps d'antenne
stat-air-time-value = émission { $tx }, réception { $rx }
stat-packets = paquets
stat-packets-value = { $recv } reçus, { $sent } envoyés
stat-sent-by = envoyés
stat-recv-by = reçus
stat-by-value = { $flood } flood, { $direct } direct
stat-recv-errors = erreurs de réception
span-seconds = { $s } s
span-minutes = { $m } min
span-hours = { $h } h { $m } min
span-days = { $d } j { $h } h
tel-digital-in = entrée numérique
tel-digital-out = sortie numérique
tel-analog-in = entrée analogique
tel-analog-out = sortie analogique
tel-generic = valeur de capteur
tel-illuminance = lumière
tel-presence = présence
tel-temperature = température
tel-humidity = humidité
tel-pressure = pression
tel-voltage = tension
tel-current = courant
tel-frequency = fréquence
tel-percentage = pourcentage
tel-altitude = altitude
tel-concentration = concentration
tel-power = puissance
tel-distance = distance
tel-energy = énergie
tel-direction = direction
tel-unixtime = heure du capteur
tel-colour = couleur
tel-switch = interrupteur
tel-latitude = latitude GPS
tel-longitude = longitude GPS
tel-gps-altitude = altitude GPS
tel-accel-x = accélération X
tel-accel-y = accélération Y
tel-accel-z = accélération Z
tel-gyro-x = rotation X
tel-gyro-y = rotation Y
tel-gyro-z = rotation Z
fleet-ble-connecting = connexion { $with }…
fleet-ble-will-connect = se connecte tout seul bientôt, { $with }
fleet-pin-yours = avec le PIN donné
fleet-pin-saved-one = avec son PIN gardé
fleet-pin-default = avec le PIN par défaut
fleet-ble-failed = connexion impossible
fleet-ble-try-again = réessayer
node-ble-not-connected = Pas encore connecté : la raison est à gauche. p : tapez son PIN et appairez.
fleet-ble-listening = Bluetooth : à l'écoute
fleet-ble-problem = Bluetooth : indisponible
fleet-ble-disabled = Bluetooth : coupé dans les réglages
fleet-ble-why = Bluetooth : { $why }
fleet-pin-given = PIN donné pour cette session
fleet-pin-settings = PIN des réglages
fleet-pin-saved = déjà appairé : son PIN est gardé
pin-title = PIN Bluetooth
pin-for = PIN d'appairage pour { $name }.
pin-refused = { $name } a refusé le PIN essayé. Tapez son PIN : il est alors réessayé.
pin-where = Un compagnon avec écran affiche son PIN ; sinon 123456, ou celui réglé dans l'app MeshCore.
pin-label = PIN
pin-empty-default = Vide : le PIN des réglages. Gardé jusqu'à la fermeture du programme, jamais enregistré.
key-unpair = dissocier
unpair-title = Dissocier
unpair-ask = { $count ->
    [one] Dissocier cet ordinateur de { $names } ?
   *[other] Dissocier cet ordinateur de ces { $count } compagnons : { $names } ?
}
unpair-after = Cet ordinateur oublie l'appairage (le compagnon garde ses réglages). Pour l'utiliser à nouveau : p, son PIN, et il s'appaire à nouveau.
unpair-yes = dissocier
job-unpair = Dissociation de { $name }
log-unpaired = { $name } dissocié de cet ordinateur.
log-unpair-failed = { $name } n'a pas pu être dissocié : { $error }
pin-key-pair = appairer
key-backup-n = sauver { $count }
key-edit-n = éditer { $count } ensemble

## Édition groupée (plusieurs noeuds cochés d'un même type, e dans Flotte)
group-title = { $count } × { $kind }
group-mixed = { $count } valeurs différentes
group-no-change = inchangé
group-on-each = { $what } sur chaque noeud
group-commit-ask = { $nodes ->
    [one] Écrire sur { $nodes } noeud ({ $count } changements) ?
   *[other] Écrire sur { $nodes } noeuds ({ $count } changements) ?
}
group-node-unchanged = { $name } : rien ne change
group-gone = Aucun des noeuds cochés n'est encore branché. Esc : retour à la liste.
group-nothing-common = Ces noeuds n'ont aucun réglage commun modifiable ensemble (nom, clé privée et position sont propres à chaque noeud).
group-need-checks = Cochez d'abord les noeuds à configurer ensemble avec Space (ou ouvrez un noeud avec Enter).
group-not-nodes = Seuls les noeuds qui répondent se configurent ensemble : décochez les autres (Space).
group-choose = Les noeuds d'un même type se configurent ensemble. Lesquels ?
group-silent = { $count ->
    [one] { $count } noeud coché ne répond pas : il est mis de côté.
   *[other] { $count } noeuds cochés ne répondent pas : ils sont mis de côté.
}
group-edit-these = configurer les { $count } { $kind }
group-regions-adding = régions à ajouter sur chaque noeud : { $names }
group-no-regions = Aucun de ces noeuds n'a de régions (firmware 1.10 ou plus).
col-node = noeud
col-now = actuel
col-after = après l'écriture

## Page Régions (votre liste de noms de régions, regions.txt)
regions-title = { $count ->
    [0] Vos régions
    [one] Vos régions : 1
   *[other] Vos régions : { $count }
}
regions-empty = Vide pour l'instant. a ajoute un nom (minuscules, chiffres, - ; $ en tête pour une privée), ou + copie celui d'un noeud, depuis sa section Régions.
regions-new = Nouvelle région
regions-name = Un nom de région
regions-top = en haut :
regions-under = sous { $name } :
regions-about = Votre propre liste de noms de régions, gardée sur cet ordinateur (regions.txt), sur aucun noeud. Vous y choisissez quand vous ajoutez une région à un noeud, ou à plusieurs : une région ne marche que si chaque noeud l'écrit exactement pareil, et cette liste garde l'orthographe. Flood et locale se règlent par noeud, dans sa vue. "de" : le noeud d'où un nom a été copié (+ dans les Régions d'un noeud).
regions-exists = { $name } est déjà dans la liste.
regions-remove-ask = { $count ->
    [0] Retirer { $name } de votre liste ?
    [one] Retirer { $name } et la région sous elle de votre liste ?
   *[other] Retirer { $name } et les { $count } régions sous elle de votre liste ?
}
regions-remove-under = Retirées aussi : { $names }
regions-remove-nodes = Seule votre liste change : les noeuds gardent leurs régions.
regions-remove-yes = retirer de la liste
regions-pick-title = Ajouter une région
regions-pick-new = { $name }  (nouvelle : ajoutée au noeud et à votre liste)
regions-pick-empty = Votre liste est vide : tapez un nom, Enter l'ajoute au noeud et à votre liste.
regions-pick-note = Enter : la région choisie, avec celles au-dessus.  ● = déjà là (en groupe : la reprendre l'enlève).
regions-unreadable = Votre liste de régions { $file } est illisible et non utilisée : { $error }
regions-unwritable = Impossible d'écrire votre liste de régions { $file }
key-region-add-under = nouv. dessous
key-region-add-top = nouv. en haut

## La vue d'un noeud (Enter dans Flotte)
node-title = Noeud
node-gone = Le noeud n'est plus en USB. Esc : retour à la liste.
node-reading-hint = Lecture de chaque réglage du noeud : quelques secondes.
node-settings = Réglages
node-read-only = Lecture seule
node-edit-settings = Réglages : édition
node-not-read = Pas encore lu : appuyez sur r.
node-nothing-to-read = Aucun noeud MeshCore ne répond sur ce port : rien à lire.
node-overview = Résumé
node-contacts = Contacts
node-channels = Canaux
node-backups = Sauvegardes
node-identity = Info
node-told = Ce qu'il dit
node-last-backup = { $stamp }  ({ $count ->
    [one] { $count } sauvegarde
   *[other] { $count } sauvegardes
})
node-never-backed-up = jamais : appuyez sur b
node-no-contacts = Le noeud n'a aucun contact.
node-channels-title = { $count ->
    [one] { $count } canal
   *[other] { $count } canaux
}
node-no-channels = Aucun canal réglé sur le noeud.
node-channel-keys = Les clés de canal ne sont pas affichées : elles ne sont gardées que dans les sauvegardes.
node-regions = Régions

## Liste d'accès d'un répéteur, salon ou capteur (qui peut se connecter, et comment)
acl-title = Accès
acl-count = Liste d'accès : { $count } / { $max }
acl-role = rôle
acl-guest = invité
acl-read-only = lecture
acl-read-write = lecture / écriture
acl-admin = admin
acl-added = nouveau
acl-about = Qui peut se connecter, et comment. Qui se connecte avec le mot de passe admin est ajouté par le noeud lui-même ; les invités ne sont pas gardés. { $max } au plus.
acl-none = Ce firmware ne sait pas lister sa liste d'accès (pas de "get acl").
acl-empty = Personne dans la liste : seuls les mots de passe font entrer.
acl-empty-edit = Personne dans la liste. a : ajouter des gens de vos contacts.
acl-not-read = La liste d'accès n'a pas été lue : relisez le noeud (r).
acl-bad-key = { $key } n'est pas une clé publique entière (64 chiffres hexa).
acl-full = La liste d'accès aurait { $count } clés ; le noeud en garde { $max } et en retirerait lui-même. Retirez-en d'abord.
acl-set-line = accès : { $key } en { $role }
acl-remove-line = accès : { $key } retiré
key-acl-role = rôle
key-acl-add = ajouter depuis les contacts
key-acl-give = donner l'accès
acl-pick-title = Donner l'accès : vos contacts  ·  { $count } cochés
acl-pick-empty = Votre carnet d'adresses est vide : lisez d'abord un compagnon (ses contacts y arrivent), puis donnez-leur un rôle sur la page Contacts (r).
acl-pick-pasted = Entrée : ajouter la clé { $key } (dans aucun de vos contacts).
acl-pick-note = Chacun reçoit le rôle que vous lui avez donné sur la page Contacts (r) ; sans rôle, le rôle affiché, sinon lecture. ← → un rôle · Espace cocher · + tous ceux affichés · tapez pour chercher un nom (ou collez une clé entière).
node-regions-title = { $count ->
    [0] Régions
    [one] 1 région
   *[other] { $count } régions
}
node-no-regions = Le noeud n'a pas encore de région. En édition (e), a en ajoute une de votre liste.
col-flood = flood
regions-flood = permis
regions-no-flood = bloqué
regions-home = locale
regions-default = portée défaut
regions-not-in-list = hors de la liste
regions-in-list = dans la liste
regions-root = tout paquet sans région
regions-root-flood-only = * désigne tout paquet sans région : seul son flood peut changer
regions-offer-line = dans votre liste, pas sur ce noeud
regions-offer-add = Enter : ajouter
col-from = de
regions-cut = La réponse du noeud a été coupée (160 caractères) : des régions peuvent manquer ici.
key-region-take = l'ajouter
key-region-offer = ajouter celle sous la ligne
key-region-to-list = dans ma liste
key-region-add = ajouter région
key-region-flood = flood on / off
key-region-home = locale on / off
key-region-default = portée défaut on / off
node-backups-title = { $count ->
    [one] { $count } sauvegarde de cet appareil
   *[other] { $count } sauvegardes de cet appareil
}
node-no-backups = Pas encore de sauvegarde de cet appareil : appuyez sur b. Les sauvegardes suivent l'appareil (son n° de série USB, sinon sa clé) : renommer le noeud les garde ici.
contact-chat = chat
contact-repeater = répéteur
contact-room = salon
contact-sensor = capteur
contact-other = autre
route-flood = flood
route-direct = direct
route-hops = { $n ->
    [one] { $n } saut
   *[other] { $n } sauts
}

## Sauvegarde
job-backup = Sauvegarde de { $name }
backup-title = Sauvegarde
backup-ask = Sauvegarder { $name } ({ $board }) maintenant ?
backup-ask-many = Sauvegarder ces { $count } noeuds maintenant, en parallèle ?
backup-left-out = Laissés de côté (aucun noeud n'y répond) : { $ports }
backup-ask-secrets = La sauvegarde contient la clé privée du noeud et les clés de canal : gardez le dossier des sauvegardes pour vous.
backup-not-a-node = Ce port n'a aucun noeud MeshCore qui répond : rien à sauvegarder.
backup-no-answer = Le noeud n'a pas répondu
backup-already = Cette lecture du noeud est déjà sauvegardée : r la relit, puis b
backup-unwritable = Impossible d'écrire { $file }
backup-complete = complète
backup-partial = partielle
backup-verified = vérifiée
backup-newer = plus récente
backup-unverified = { $file } ne s'est pas relu tel qu'écrit : la sauvegarde n'est pas gardée
backup-newer-format = Cette sauvegarde vient d'une version plus récente ({ $tool }, format { $format }) : mettez ce programme à jour pour l'utiliser. Elle est gardée telle quelle.
backup-damaged = { $file } n'est plus tel qu'enregistré (modifié ou abîmé depuis) : cette sauvegarde est inutilisable
backup-stopped-answering = le noeud a cessé de répondre ({ $step }) : rien n'a été gardé, réessayez
backup-board-changed = un autre appareil répond sur le port (ou aucun) : rien n'a été gardé
reason-unknown = -
reason-manual = manuelle
section-self = identité et radio
section-device = infos appareil
section-tuning = réglage radio
section-autoadd = ajout auto contacts
section-flood-scope = portée flood
section-custom-vars = variables perso
section-private-key = clé privée
section-channels = canaux
section-contacts = contacts
section-console = réglages
section-acl = liste d'accès
section-regions = régions
section-address-book = vos noms et rôles
restore-book = { $count ->
    [one] 1 changement à vous revient dans le carnet d'adresses (jamais sur le noeud) :
   *[other] { $count } changements à vous reviennent dans le carnet d'adresses (jamais sur le noeud) :
}
restore-book-name = nom perso de { $who } : { $old } → { $new }
restore-book-role = rôle de { $who } : { $old } → { $new }
restore-book-done = { $count ->
    [one] Le nom perso / rôle d'1 contact remis dans le carnet d'adresses.
   *[other] Les noms perso / rôles de { $count } contacts remis dans le carnet d'adresses.
}
compare-book = Vos noms et rôles
restore-left = { $count ->
    [one] Cet appareil n'a pas pris 1 réglage : il garde le sien.
   *[other] Cet appareil n'a pas pris { $count } réglages : il garde les siens.
}
restore-left-line = { $what } ({ $why })
key-restore-here = restaurer sur ce noeud
restore-other-note = Sur un autre appareil : ce qu'il ne peut pas prendre (autre puce radio, moins de puissance, réglage absent de son firmware) reste comme il l'a, et est listé à la fin.
book-unreadable = Le carnet d'adresses { $file } est illisible : il est laissé tel quel (rien n'y est écrit) jusqu'à ce qu'il soit réparé ou déplacé.
book-unwritable = Le carnet d'adresses { $file } ne peut être écrit.
reason-before-config = auto (édition)
reason-before-restore = auto (restauration)
reason-before-firmware = auto (firmware)
reason-before-reset = auto (reset)
backup-unreadable = illisible
backup-contents = { $sections } parties, { $items } éléments
backup-secret-mark = * contient une clé privée, une clé de canal ou un mot de passe
backup-missing = Absent de cette sauvegarde : { $missing }

## Page Sauvegardes
backups-nodes = Noeuds  ·  ● présents
backups-of = Sauvegardes de { $name }
backups-of-gone = Sauvegardes de { $name } (absent : R en restaure une sur un autre appareil)
backups-details = Sauvegarde choisie
backups-empty = Pas encore de sauvegarde. Ouvrez Flotte, choisissez un noeud et appuyez sur b.
key-compare = comparer à la précédente
key-compare-checked = comparer les 2 cochées
compare-title = Comparer
key-compare-node = comparer au noeud actuel
compare-node-now = le noeud actuel
compare-node-absent = Le noeud de cette sauvegarde n'est pas là (branché, ou entendu en Bluetooth) : impossible de le comparer maintenant. c la compare à la sauvegarde précédente.
job-compare-node = Comparaison avec le noeud sur { $port }
job-connecting = connexion…
compare-close = ↑↓ PgUp PgDn: défiler    Esc / Enter: fermer
compare-head = { $name } : du { $older } au { $newer }
compare-count = { $count ->
    [0] Aucune différence : les deux sont identiques.
    [one] 1 différence
   *[other] { $count } différences
}
compare-settings = Réglages
compare-channels = Canaux
compare-acl = Liste d'accès
compare-acl-added = { $key } ajouté ({ $role })
compare-acl-removed = { $key } retiré (était { $role })
compare-acl-changed = { $key } : { $old } → { $new }
compare-contacts = Contacts
compare-notes = Notes
compare-only-older = Seulement dans l'ancienne : { $parts }
compare-only-newer = Seulement dans la récente : { $parts }
compare-not-in-older = Absent de l'ancienne : { $what }
compare-channel-added = slot { $slot } : { $name }
compare-channel-removed = slot { $slot } : { $name }
compare-channel-renamed = slot { $slot } : { $old } → { $new }
compare-channel-key = slot { $slot } ({ $name }) : clé remplacée (non affichée)
compare-contact-added = { $name }
compare-contact-removed = { $name }
compare-contact-renamed = { $old } → { $new }
compare-contact-favourite = { $name } : désormais favori
compare-contact-not-favourite = { $name } : plus favori
compare-contact-type = { $name } : autre type
compare-kinds-differ = Ces deux sauvegardes sont de types de noeud différents : impossible de les comparer.
compare-no-older = C'est la plus ancienne sauvegarde du noeud : rien avant pour comparer. Cochez deux sauvegardes (Space) pour les comparer.
compare-pick-two = Cochez exactement deux sauvegardes (Space) pour les comparer, ou aucune pour comparer celle choisie à la précédente.
key-delete-node-backups = tout effacer
key-delete-n = effacer { $count }
delete-title = Effacer des sauvegardes
delete-ask = { $count ->
    [one] Effacer cette sauvegarde de { $name } ?
   *[other] Effacer ces { $count } sauvegardes de { $name } ?
}
delete-none-left = Il ne restera aucune sauvegarde de { $name }.
delete-no-verified-left = Il ne restera aucune sauvegarde vérifiée de ce noeud.
delete-way-back = Certaines datent d'avant un changement (édition, restauration, firmware, reset) : c'est le moyen de revenir à l'état d'avant du noeud.
delete-cannot-undo = Les fichiers sont effacés du disque : c'est irréversible.
delete-sure = Vraiment effacer { $count } sauvegardes de { $name }, pour de bon ?
delete-yes = { $count ->
    [one] l'effacer
   *[other] effacer { $count }
}
log-delete-done = Sauvegarde { $stamp } effacée
log-delete-failed = Sauvegarde { $stamp } non effacée : { $error }
log-delete-refused = Sauvegarde { $stamp } non effacée : hors du dossier des sauvegardes
log-delete-needed = Sauvegarde { $stamp } non effacée : une restauration interrompue en a encore besoin

## Restauration
restore-title = Restauration
job-restore-check = Restauration : lecture du noeud sur { $port } pour montrer ce qui changerait (rien n'est encore écrit)
job-restore = Restauration sur le noeud de { $port }
restore-onto = Sur { $name } ({ $board })
restore-kind-differs = La sauvegarde est d'un { $backup }, ce noeud est un { $target } : ils ne correspondent pas.
restore-no-target = Branchez d'abord le noeud à restaurer (un noeud du même type, qui répond).
restore-choose-target = Restaurer cette sauvegarde sur quel noeud ?
restore-target-own = { $name } (le sien)
restore-choose-mode = Cette sauvegarde vient d'un autre appareil. Que doit-il en prendre ?
restore-move-means = Déplacer : il devient ce noeud, un clone (même identité, clé privée, nom, position). Pour un appareil qui en remplace un cassé ou perdu. Jamais les deux allumés en même temps : deux noeuds d'une même identité cassent les routes, les messages et les connexions sur le mesh.
restore-copy-means = Copier : elle reçoit réglages, canaux et contacts, et garde sa propre identité, son nom, sa position et son PIN Bluetooth.
restore-choose-move = déplacer (son identité)
restore-choose-copy = copier (garde la sienne)
restore-mode-same = Sa propre sauvegarde : tout revient comme avant.
restore-mode-move = Déplacer : ce noeud prend l'identité du noeud sauvegardé.
restore-mode-copy = Copier : ce noeud garde son identité, nom, position et PIN Bluetooth.
restore-firmware = Sauvegarde faite avec le firmware { $backup }, le noeud a le { $node } : les réglages absents de ce firmware sont omis.
restore-nothing = Le noeud contient déjà cette sauvegarde : rien à changer.
restore-changes = { $count ->
    [one] { $count } changement :
   *[other] { $count } changements :
}
restore-more = … et { $count } de plus
restore-cannot = Non restaurable : { $what }
restore-how = Seul ce qui diffère est envoyé, puis relu et vérifié.
restore-yes = restaurer
restore-stopped = La restauration n'a pas fini :
restore-stopped-how = Le noeud contient une partie de la sauvegarde. Restaurez à nouveau : seul le reste est envoyé.
restore-move-title = Prendre une autre identité ?
restore-move-sure = Ce noeud va prendre l'identité du noeud sauvegardé (sa clé privée). Sa propre identité n'est gardée que dans la sauvegarde faite juste avant. L'ancien appareil ne doit plus servir sur le mesh. Sûr ?
restore-move-yes = oui, prendre l'identité
restore-no-key = la sauvegarde ne contient pas de clé privée (gardée seulement si réglé dans config.toml)
restore-no-slot = canal { $name } (aucun slot libre)
restore-channel = slot de canal { $slot }
restore-channels = { $count ->
    [one] { $count } canal
   *[other] { $count } canaux
}
restore-contact = contact { $name }
restore-contacts-back = { $count ->
    [one] contact remis : { $names }
   *[other] { $count } contacts remis : { $names }
}
restore-contacts-fav = { $count ->
    [one] favori comme dans la sauvegarde : { $names }
   *[other] favoris comme dans la sauvegarde ({ $count }) : { $names }
}
restore-contacts-full = { $count ->
    [one] laissé tel quel : { $names } n'est plus sur le noeud (sa liste de contacts est pleine, un contact plus récent a pris sa place)
   *[other] laissés tels quels : { $count } contacts ne sont plus sur le noeud ({ $names }) : sa liste est pleine, des contacts plus récents ont pris leur place
}
restore-contacts-kept = { $count ->
    [one] gardé : 1 contact ajouté par le noeud depuis la sauvegarde
   *[other] gardés : { $count } contacts ajoutés par le noeud depuis la sauvegarde
}
restore-and-more = { $names } et { $count } de plus
set-flood-scope = portée flood par défaut
restore-regions-cut = retrait des régions (une liste de régions a été coupée à la lecture)
restore-waiting = Attente du retour du noeud ({ $secs } s)…
restore-gone = Le noeud n'est pas revenu.
restore-restarting = Redémarrage du noeud pour les nouveaux réglages…
restore-refused = Le noeud a refusé { $what } : { $why }
restore-not-settled = Le noeud ne garde pas les valeurs envoyées.
restore-not-started = Rien n'a été écrit : le noeud est comme avant.
restore-done = Restauré et vérifié : le noeud contient la sauvegarde.
restore-done-move = Cet appareil est maintenant un clone de l'ancien : jamais les deux allumés en même temps (même identité : routes, messages et connexions cassés). Effacez l'ancien ou laissez-le éteint.
restore-clone-warning = ! Un clone : cet appareil aura la même identité que { $name }. Jamais les deux allumés en même temps ; si l'ancien revient, effacez-le ou laissez-le éteint.
restore-put-back = La restauration n'a pas abouti. Le noeud a été remis comme avant, et vérifié. Pourquoi :
restore-stuck = La restauration n'a pas abouti, et le noeud n'a pas pu être remis comme avant non plus. Pourquoi :
restore-stuck-how = Rebranchez le noeud : au prochain démarrage, l'app propose de finir de le remettre. La sauvegarde faite avant est dans ses sauvegardes ("avant restauration").
restore-interrupted-title = Une restauration a été interrompue
restore-interrupted = Une restauration sur { $name } (lancée { $started }) n'a pas abouti : le programme ou l'ordinateur s'est arrêté.
restore-was-putting-back = Elle remettait le noeud comme avant.
restore-interrupted-how = Branchez le noeud, puis remettez-le comme avant (le plus sûr) ou finissez la restauration. Plus tard : redemandé au prochain démarrage.
restore-put-back-button = le remettre comme avant
restore-finish-button = finir de restaurer
restore-later = après
restore-journal-open = Une restauration de cet appareil a été interrompue : finissez-la ou remettez d'abord le noeud comme avant (demandé au démarrage du programme).

## Page Appareils
boards-title = { $count } appareils connus
boards-usb-title = { $count } ids USB connus

## Journal
log-empty = Rien ne s'est encore passé.
log-found = { $port } : { $kind } { $name }
log-backup-done = Sauvegarde de { $name } enregistrée : { $stamp }
log-backup-failed = Sauvegarde de { $name } échouée : { $error }
job-inspect = Lecture du noeud sur { $port }
log-job-waiting = { $port } : une autre tâche travaille sur cet appareil ; celle-ci démarre à sa fin
log-job-crashed = { $port } : la tâche s'est arrêtée sur une erreur inattendue (un bug : merci de le signaler). L'appareil est libre ; rien de plus n'a été envoyé.
log-job-not-started = Une tâche n'a pas pu démarrer (le système n'a plus de place pour un thread) : rien n'a été fait.
jobs-more = (+{ $count } de plus)
quit-title = Des tâches tournent
quit-ask = { $count ->
    [one] { $count } tâche travaille encore sur un noeud.
   *[other] { $count } tâches travaillent encore sur des noeuds.
}
quit-why = Quitter maintenant les coupe : une sauvegarde n'est pas gardée, une écriture peut être à moitié envoyée, une restauration est reproposée au prochain démarrage (pour la finir, ou remettre le noeud comme avant).
quit-yes = quitter
quit-wait = les attendre
log-inspect-companion = { $name } lu : { $contacts } contacts, { $channels } canaux
log-inspect-console = { $name } lu : { $settings } réglages
job-apply = Écriture sur { $name }
log-edit-done = { $what } : fait
log-edit-refused = { $what } : refusé ({ $why })
log-edit-silent = { $what } : pas de réponse. Fait ou non, on ne sait pas ; ce n'est pas renvoyé.
log-edit-not-sent = { $count ->
    [one] { $count } changement non envoyé (le noeud a cessé de répondre)
   *[other] { $count } changements non envoyés (le noeud a cessé de répondre)
}
log-edit-summary = { $name } : { $done } sur { $total } changements faits
log-edit-failed = Rien écrit sur { $name } : { $error }
log-edit-no-backup = Rien écrit sur { $name } : la sauvegarde d'avant a échoué
log-inspect-failed = { $name } n'a pas pu être lu : { $error }
