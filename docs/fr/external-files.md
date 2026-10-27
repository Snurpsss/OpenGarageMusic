# Fichiers externes

*[English version](../en/external-files.md)*

Tout ce qui, dans le dépôt, n'a pas été écrit pour OpenGarageMusic : d'où il vient,
sous quelle licence, et où il a été téléchargé. Règle du projet : seuls des contenus
sous licence libre y entrent (voir [Contribuer](contributing.md)). Une chose que le
programme lit à la demande de l'utilisateur n'est pas libre et n'entre jamais dans le
dépôt : les tablatures de Songsterr, décrites plus bas.

## Sons

**Aucun son n'est fourni avec OpenGarageMusic.** Les sons intégrés sont calculés par le
programme. Les banques d'instruments échantillonnés sont téléchargées par
l'utilisateur, à sa demande, depuis le menu Bibliothèque ; elles ne font pas partie
du dépôt.

Ces téléchargements vont dans `%LOCALAPPDATA%\OpenGarageMusic`, en deux dossiers
distincts : `Banques` pour les banques de sons, `Effets` pour les greffons. Chacun
contient un sous-dossier par site puis par contenu, avec un fichier `SOURCE.txt` écrit
par l'application : nom, auteur, licence, site, page, adresse exacte du téléchargement,
date et somme de contrôle SHA-256 du fichier reçu.

Sites proposés par la fenêtre de téléchargement :

| Site | Ce que l'application y liste | Licence |
|---|---|---|
| MuseScore (miroir <https://ftp.osuosl.org/pub/musescore/soundfont/>) | FluidR3 GM, MuseScore General (SF3) | MIT |
| Musical Artifacts (<https://musical-artifacts.com/>) | Banques SF2 et SF3 dont la licence déclarée est libre (domaine public, CC BY, CC BY-SA, WTFPL, GPL) | déclarée par la personne qui a déposé le fichier, non vérifiée |
| SFZ Instruments (<https://github.com/sfzinstruments>) | Dépôts dont GitHub reconnaît une licence libre (CC0, CC BY, CC BY-SA, Unlicense…) | celle du dépôt |
| Polyphone, FreePats | Rien en direct : l'application renvoie vers le site | propre à chaque fichier |

## Effets et instruments externes (greffons)

**Aucun greffon n'est fourni avec OpenGarageMusic.** L'utilisateur peut importer un
fichier `.clap`, un dossier `.lv2` ou un fichier `.vst3` qu'il possède, ou télécharger
des greffons depuis le menu Bibliothèque. Un greffon est un programme : l'application
ne le vérifie pas. Elle le lance dans un processus à part, ce qui la protège de ses
plantages, pas vous de ses intentions.

| Site | Ce que l'application y liste | Licence |
|---|---|---|
| GitHub, dépôts des auteurs | La dernière version Windows 64 bits de : Dragonfly Reverb, Wolf Shaper, Six Sines, Dexed, Surge XT | GPL-3.0 ou MIT selon le projet (licence du dépôt, relevée le 11 octobre 2026) |
| OpenAudio (<https://github.com/webprofusion/OpenAudio>) | La liste des projets (un seul fichier, lu sur GitHub). Pour le projet choisi seulement, l'application demande à GitHub sa licence et sa dernière version : le téléchargement n'est proposé que si la licence est libre et qu'une archive `.zip` pour Windows 64 bits existe | celle du dépôt de chaque projet, relevée au moment du choix |
| x42-plugins (<https://x42-plugins.com/x42/>), CLAP Database (<https://clapdb.tech/>) | Rien en direct : l'application renvoie vers le site ; les greffons installés dans les dossiers LV2, CLAP et VST3 standard sont détectés | propre à chaque greffon |

D'une archive téléchargée, l'application garde ce qu'elle sait utiliser : les fichiers
`.clap` et les fichiers ou dossiers `.vst3`. Ces greffons sont chargés à la demande de
l'utilisateur et ne sont pas distribués avec OpenGarageMusic.

Pendant la mise au point, deux banques (FluidR3_GM depuis Debian, MuseScore_General
depuis le miroir de MuseScore, toutes deux sous licence MIT) ont été téléchargées
dans le dépôt pour des essais, puis supprimées. Il n'en reste rien.
## Tablatures (Songsterr)

**Aucune tablature n'est fournie avec OpenGarageMusic**, et aucune n'entre dans le
dépôt. Fichier → « Nouveau depuis un site de tablatures… », Songsterr choisi, demande à
<https://www.songsterr.com> les morceaux qui correspondent à une recherche et lit les
parties de celui que l'utilisateur choisit ; les notes vont dans le projet de
l'utilisateur et nulle part ailleurs.

| Site | Ce que l'application y lit | Licence |
|---|---|---|
| Songsterr (<https://www.songsterr.com>) | La liste des morceaux d'une recherche, la description d'un morceau (quelle piste est celle du chant : son champ `isVocalTrack`, lu comme le champ `vocal` de la description du site), et les fichiers de ses parties, que le lecteur du site charge depuis ses serveurs ; quand l'utilisateur demande les paroles du morceau (la case « Les paroles du morceau », cochée au départ), le fichier de la partie chantée, pour ses paroles (`newLyrics`), même quand cette partie n'est pas cochée ; quand l'utilisateur le demande, les vidéos YouTube du morceau avec la seconde où commence chaque mesure dans chacune (`/api/video-points/…`, les points que posent ses utilisateurs, constaté le 16 octobre 2026) | **non libre** : les tablatures sont écrites par les utilisateurs du site, pour des morceaux le plus souvent protégés par le droit d'auteur ; les conditions d'utilisation du site s'appliquent, et le programme ne peut pas vérifier les droits |

C'est la seule exception à la règle qui veut que seuls des contenus sous licence libre
soient proposés, et son usage relève de l'utilisateur : ce qui est importé sert à sa
pratique personnelle et ne doit pas être distribué avec un projet. Les adresses et le
format des fichiers ne sont pas documentés par le site ; ils ont été déduits de ce que
charge son lecteur public (constaté le 12 octobre 2026) et peuvent changer sans
préavis. Les tests du projet lisent des parties écrites pour eux et quelques lignes des
listes du site (titres et numéros de morceaux), pas des tablatures du site ; les
quelques tests qui interrogent le site sont désactivés par défaut.

Le programme ne se connecte pas au site et ne conserve aucun compte : il lit ce que le
site montre à tout le monde.

## Autres sites de tablatures, et fichiers Guitar Pro

Fichier → « Nouveau depuis un site de tablatures… » nomme trois autres sites. Ce que
disent leurs conditions d'utilisation et leurs fichiers `robots.txt` a été lu le
14 octobre 2026. Deux d'entre eux, Guitariff et Tablature Hub, sont cherchés par le
programme lui-même, qui va lire le fichier Guitar Pro du morceau que l'utilisateur
choisit, à ce moment-là et sur sa décision, avec son propre User-Agent
(`OpenGarageMusic/<version>`) et sans compte ; le troisième, Ultimate Guitar, n'est
qu'ouvert dans le navigateur. Quel que soit le site, le programme importe aussi un
fichier Guitar Pro que l'utilisateur a téléchargé lui-même.

Depuis la version 0.9.9, chaque site, Songsterr compris, est lu à travers une
description en JSON (`crates/og-engine/sites`, écrite pour le projet et qui en fait
partie), qui dit comment y chercher et où sont les fichiers. L'utilisateur peut ajouter
des descriptions d'autres sites dans le dossier `sites` à côté du programme
([Décrire un site de tablatures](sites.md)) : un tel site n'a pas été étudié par le
projet, et c'est à l'utilisateur de vérifier qu'il peut être lu par un programme, comme
le dit la fenêtre quand elle demande, une fois, si elle doit le lire.

| Site | Ce que le programme y fait | Conditions |
|---|---|---|
| Ultimate Guitar (<https://www.ultimate-guitar.com>) | Ouvre le site | Ses conditions d'utilisation ne permettent que d'imprimer une tablature en texte pour un usage personnel, interdisent d'en reproduire le contenu « sur tout support, mémoire ou appareil » et de le répliquer, et son `robots.txt` ferme la page de recherche et les chemins de téléchargement aux programmes. **Non libre**, et pas à lire par un programme |
| Tablature Hub (<https://www.tablatureguitare.net>) | Cherche sur le site (`/recherche?q=…`, qui ne connaît que les titres, et son index des artistes par lettre, `/tablature-partition/<lettre>.html`, puis les pages de trois artistes correspondants au plus) et va lire le fichier Guitar Pro du morceau que l'utilisateur choisit (`/download.php?id=…`), avec le User-Agent du programme et sans compte | Ses conditions réservent les tablatures, « représentations d'interprétations personnelles, approximatives de chansons protégées par droits d'auteur », à un usage personnel et pédagogique, et ne disent rien des programmes. Son `robots.txt` interdit `/download.php`, l'adresse qui sert ses fichiers Guitar Pro 3 et 4 : l'auteur du projet a choisi de le lire quand même, pour l'usage personnel de qui demande un morceau, un fichier à la fois, jamais en parcourant le site. **Non libre** ; ce qui est lu ne va que dans le projet de l'utilisateur |
| Guitariff (<https://www.guitariff.net>) | Cherche sur le site (`/tabs/search.php?q=…`), lit la page du morceau que l'utilisateur choisit et va prendre l'archive ZIP qu'elle propose (`/ddl_tabs/…`), avec le User-Agent du programme et sans compte | Ses conditions demandent de n'utiliser une tablature « qu'à des fins personnelles, pour une recherche ou un travail scolaire », et ne disent rien des programmes ; son `robots.txt` ne ferme que ses statistiques. **Non libre** ; ce qui est lu ne va que dans le projet de l'utilisateur |

Le programme lit lui-même les **fichiers Guitar Pro** (`crates/og-engine/src/guitar_pro.rs`),
de quelque site ou programme qu'ils viennent : fichiers GP3, GP4 et GP5, ceux de GP6
(.gpx) et ceux de GP7 et GP8 (.gp). Les formats ne sont pas publiés par leur éditeur ;
le lecteur suit leurs descriptions publiques et a été écrit pour le projet (aucun code
n'a été pris aux programmes libres qui les lisent). Aucune tablature n'est livrée et
aucune n'entre dans le dépôt : les tests lisent des fichiers qu'ils écrivent eux-mêmes.

| Description utilisée | Pour | Licence |
|---|---|---|
| La description des formats dans la documentation de PyGuitarPro (<https://pyguitarpro.readthedocs.io/en/stable/pyguitarpro/format.html>), lue le 15 octobre 2026 | Vérifier champ par champ le lecteur de GP3, GP4 et GP5 | Lue seulement pour comprendre les formats : le code de PyGuitarPro est sous une licence copyleft et rien n'en a été pris |
| Le README de rust-gpx-reader (<https://github.com/Antti/rust-gpx-reader>) et `gpx_filesystem.go` de go-guitar-pro (<https://github.com/CaliLuke/go-guitar-pro>), lus le 14 octobre 2026 | Le conteneur des fichiers GP6 : sa compression et son système de fichiers (`crates/og-engine/src/gpx.rs`, réécrit en Rust) | go-guitar-pro : MIT ; le README n'indique pas de licence et seule sa description a servi |

Le programme lit aussi les **partitions MusicXML** (`crates/og-engine/src/musicxml.rs`),
d'après la spécification publiée du format (MusicXML 4.0, W3C Music Notation Community
Group, <https://www.w3.org/2021/06/musicxml40/>), un format ouvert fait pour être
implémenté par tout programme. Le lecteur a été essayé, hors du dépôt, sur les 212
fichiers MusicXML qu'alphaTab porte pour ses tests (parmi eux la suite de tests MusicXML
et les exemples publiés avec le format) ; aucun n'est dans le dépôt.

Le lecteur GP6 a aussi été essayé, hors du dépôt, sur les 38 fichiers GP6 de la suite de
tests d'alphaTab (MPL-2.0) que porte go-guitar-pro ; ils ne sont pas dans le dépôt, et un
test désactivé par défaut lit de tels fichiers dans un dossier de l'utilisateur
(`OPENGARAGE_SCORES_DIR`), comme pour les autres formats.

## Captures et réponses impulsionnelles (TONE3000, GuitarML)

Bibliothèque → « Captures et réponses impulsionnelles depuis les sites… » nomme deux
sites, chacun décrit au programme comme le sont les sites de tablatures
(`crates/og-engine/sites`), lus le 17 octobre 2026 :

- **TONE3000** (<https://www.tone3000.com>) : captures d'amplis et de pédales aux
  formats Neural Amp Modeler, AIDA-X et Proteus, et réponses impulsionnelles,
  partagées par ses utilisateurs, chacune sous la licence choisie par son auteur (la
  plupart sous la licence T3K du site : utiliser et publier ce que l'on joue librement,
  ne pas redistribuer le fichier). Le site est gratuit avec un compte, et les
  conditions de son API pour les programmes gratuits (lues le même jour) permettent
  les favoris de l'utilisateur, les captures du moment et les dernières, téléchargées
  une par une à la demande de l'utilisateur, jamais en masse, et demandent à chaque
  programme une clé que l'utilisateur crée dans son propre compte ; le programme garde
  cette clé et le jeton que le site lui donne dans les réglages, et ne demande rien au
  site sans eux.
- **La ToneLibrary de GuitarML** (<https://guitarml.com/tonelibrary/tonelib-pro.html>) :
  captures pour Proteus, gratuites et sans compte, chacune un fichier `.json` sur
  GitHub.

Ce qui est téléchargé va dans les dossiers `Captures` et `Impulsions` de la
bibliothèque, dans un dossier du site et de l'élément, avec un `SOURCE.txt` qui dit
d'où il vient, de qui et sous quelle licence. Le programme n'embarque aucune capture ni
aucune impulsion. Le format `.nam` et son lecteur de référence sont open source (MIT) ;
le programme lit les fichiers `.nam` avec le crate `nam-rs` et ceux de Proteus et
d'AIDA-X avec son propre code.

## Enregistrements et vidéos d'un morceau

Un morceau peut se jouer avec son son d'origine : un enregistrement de l'utilisateur,
ou une vidéo YouTube. **Aucun enregistrement ni aucune vidéo n'est fourni, et aucun
n'entre dans le dépôt.**

- Un fichier audio de l'utilisateur (MP3, OGG, FLAC, M4A/AAC, WAV) reste où il est :
  le projet y renvoie par son chemin, il n'est pas copié, et rien n'est envoyé nulle
  part. La bibliothèque musicale est un dossier de l'utilisateur que le programme
  lit ; son index (`library.json`, à côté des réglages) contient des chemins, des
  artistes et des titres, rien du son.
- Une vidéo YouTube est jouée par le lecteur de YouTube lui-même, dans la fenêtre du
  programme, tant qu'elle est visible. Le programme ne garde aucune vidéo ni rien de
  son son : rien n'est téléchargé. YouTube Music ne sert pas : il n'a pas de lecteur
  qui puisse être intégré.

| Fichier ou service | Ce que c'est | Origine et licence |
|---|---|---|
| `og-video.exe` | Le lecteur vidéo : un petit programme à côté d'`OpenGarageMusic.exe` qui montre le lecteur de YouTube dans une vue web dessinée dans la fenêtre du programme, dans son propre processus pour que le programme n'ait jamais besoin de `WebView2Loader.dll` (il démarre sans elle ; seule la vidéo ne démarre pas) | Compilé depuis `apps/og-video`, écrit pour le projet. La vue web garde ses données dans `%LOCALAPPDATA%\OpenGarageMusic\WebView2` |
| `WebView2Loader.dll` | Le chargeur du runtime WebView2 de Microsoft, la vue web présente dans Windows 10 et 11 | Microsoft Corporation, du SDK WebView2 (<https://www.nuget.org/packages/Microsoft.Web.WebView2>), sous licence BSD 3-Clause. **Pas dans le dépôt** : la compilation d'`og-video` la copie, sans changement, depuis la crate `webview2-com-sys`, qui la porte ; elle est livrée dans le téléchargement à côté d'`og-video.exe`, sa licence écrite dans `THIRD-PARTY-LICENSES.txt` |
| `MidiPilot.exe` | Le MIDI Pilot seul, pour la scène : un lanceur qui démarre `OpenGarageMusic.exe`, à côté de lui, sur cette fenêtre seulement (pas de carte son, pas de projet ; une chanson dont le projet doit s'ouvrir lance le programme entier dessus) | Construit depuis `apps/midi-pilot`, écrit pour le projet |
| L'API IFrame Player de YouTube (<https://developers.google.com/youtube/iframe_api_reference>) | Le lecteur de vidéos YouTube qu'une page peut intégrer | Un service de YouTube, chargé par le lecteur vidéo depuis youtube.com au moment où il tourne, sous les conditions d'utilisation de YouTube. Rien n'en est dans le dépôt |
| `screenshots/video-and-tablature.png` | La capture d'écran de la vidéo et de la tablature d'un morceau | La vidéo montrée est *Big Buck Bunny* (© 2008 Blender Foundation, <https://peach.blender.org>), sous la licence [Creative Commons Attribution 3.0](https://creativecommons.org/licenses/by/3.0/deed.fr), jouée depuis YouTube pour la démonstration |

## Paroles du morceau

**Aucunes paroles de morceau ne sont fournies, et aucunes n'entrent dans le dépôt**,
sauf celles écrites pour le projet de démonstration. Les paroles d'un morceau viennent
de sa tablature (Songsterr, un fichier Guitar Pro, une partition MusicXML : voir
plus haut), de LRCLIB, ou de l'utilisateur (un fichier à côté de son enregistrement, un
fichier qu'il choisit, des paroles qu'il tape). Elles sont gardées dans le projet de
l'utilisateur et nulle part ailleurs : comme les tablatures, elles sont le plus
souvent protégées par le droit d'auteur, et servent à la pratique personnelle de
l'utilisateur.

| Fichier ou service | Ce que le programme y fait | Origine et conditions |
|---|---|---|
| LRCLIB (<https://lrclib.net>) | Demande à son interface publique les paroles d'un morceau (`/api/search?track_name=…&artist_name=…`), de lui-même quand une tablature ou un fichier Guitar Pro ou MusicXML sans paroles devient un projet, et avec « Chercher sur LRCLIB ». Il envoie l'artiste et le titre du projet, avec le User-Agent `OpenGarageMusic/<version>`, et rien d'autre : ni compte, ni clé ; la durée de l'enregistrement sert seulement à choisir, sur l'ordinateur, parmi les versions qu'il répond | Un service gratuit, sans compte, dont les paroles, calées ligne par ligne, sont déposées par ses utilisateurs. **Non libre** : ce sont les paroles de morceaux le plus souvent protégés par le droit d'auteur ; ce qui est lu ne va que dans le projet de l'utilisateur. Rien n'en est dans le dépôt : le test qui interroge le vrai service est désactivé par défaut |
| `screenshots/words.png` | La capture d'écran du bandeau des paroles, la ligne chantée | Faite pour le projet : le projet de démonstration, avec des paroles écrites pour lui |

## Descriptions de pédales (MIDI Pilot)

Les modules des pédales BOSS DD-500 et RV-500 sont pilotés par `crates/og-engine/devices/dd500.json` et `rv500.json`, écrits pour le projet d'après les documents publics d'implémentation MIDI des fabricants (Roland Corporation, téléchargés le 25 octobre 2026) : [DD-500 Ver.2.00 MIDI Implementation](https://static.roland.com/assets/media/pdf/DD-500_Ver-200_MIDI_Imple.pdf) et [RV-500 MIDI Implementation](https://static.roland.com/assets/media/pdf/RV-500_MIDI_Imple_jpn01_W.pdf) (édition japonaise, tables lisibles). Ils contiennent les numéros de modèle, la carte de la mémoire (adresses system exclusive, requêtes et envois de données du dialecte Roland), et chaque paramètre d'un patch, du système et d'une banque avec sa place, sa plage, les noms de ses valeurs et la plage telle que le document l'affiche. Rien des logiciels des fabricants n'y figure : les noms de paramètres et les listes sont ceux que les documents impriment, et les modules ont été vérifiés contre les plages et adresses des documents. Les fichiers de pédale de l'utilisateur (`*.pedal.json`) et les sauvegardes des éditeurs BOSS (`*.bel`, JSON) sont lus et écrits par le programme et n'entrent jamais dans le dépôt. Les noms BOSS, DD-500 et RV-500 appartiennent à Roland Corporation ; le projet n'est ni affilié ni approuvé par elle.

## Fichiers de sauvegarde du G3S (MIDI Pilot)

Le module du switcher de pédales The GigRig G3S lit et écrit ses fichiers de sauvegarde (`.g3b`), ceux que son éditeur fait et restaure. Le fabricant ne publie pas de description de ce format : ce que le programme en sait (messages system exclusive de 50 octets, empaquetage sur 7 bits, checksums et somme finale, les enregistrements des réglages globaux, des presets, du MIDI étendu et des chansons, et dans un preset les boucles, leur ordre, le nom et les lignes MIDI) a été déduit de fichiers de sauvegarde faits par le switcher lui-même, et vérifié en restaurant dans le switcher des fichiers écrits ainsi ; rien n'a été pris au logiciel du fabricant. Ce qui n'est pas compris (la plupart des réglages globaux, la table MIDI) est recopié tel quel. Les fichiers sont ceux de l'utilisateur et n'entrent jamais dans le dépôt : les tests travaillent sur des fichiers que le programme invente. Écrire une sauvegarde est au risque de l'utilisateur : le switcher vérifie chaque fichier qu'il restaure (checksums, séquence, somme), et « Vérifier » fait les mêmes contrôles avant. Les noms The GigRig, G3 et G3S appartiennent à leur propriétaire ; le projet n'est ni affilié ni approuvé par lui.

## Images du MIDI Pilot

**Aucune image d'album ni d'artiste n'est fournie, et aucune n'entre dans le dépôt.**
Les boutons du MIDI Pilot montrent une image choisie par l'utilisateur, ou sinon une
image cherchée sur Deezer, ce que l'utilisateur peut désactiver (Réglages > « Pochettes et
photos de Deezer »).

| Fichier ou service | Ce que le programme y fait | Origine et conditions |
|---|---|---|
| Deezer (<https://api.deezer.com>) | Demande à sa recherche publique, sans compte ni clé, la pochette de l'album d'une chanson du MIDI Pilot (`/search?q=<artiste> <titre>&limit=1`) et la photo d'un artiste (`/search/artist?q=<artiste>&limit=1`), une question à la fois, puis télécharge l'image indiquée. Il envoie l'artiste et le titre de la chanson, et rien d'autre. Les images sont gardées dans le dossier `Covers` du dossier de données du programme (`%LOCALAPPDATA%\OpenGarageMusic`), pour s'afficher sur scène sans Internet ; Réglages > « Vider les images gardées » les efface | **Non libre** : pochettes et photos appartiennent à leurs auteurs et éditeurs ; elles ne sont montrées qu'à l'utilisateur, sur le bouton de sa chanson. Rien n'en est dans le dépôt, et les tests ne demandent rien à Deezer |
| `screenshots/midi-pilot.png` | La capture d'écran de la fenêtre du MIDI Pilot | Faite pour le projet : chansons et artistes inventés pour la démonstration, aucune image de Deezer ; la seule image est la photo de chargement du programme |
| `screenshots/midi-pilot-pedal.png` | La capture d'écran du module d'une BOSS DD-500 dans le MIDI Pilot | Faite pour le projet, sur une pédale simulée par le programme : les patches nommés sont inventés |
| `screenshots/midi-pilot-script.png` | La capture d'écran d'un script d'IA relu dans le MIDI Pilot | Faite pour le projet : le script est inventé |

## Outils de compilation

Installés dans `.toolchain` (non versionné) par `tools/setup-toolchain.ps1`.

| Outil | Licence | Téléchargé depuis |
|---|---|---|
| Rust (rustup, compilateur, cargo) | MIT ou Apache-2.0 | <https://static.rust-lang.org/rustup/dist/x86_64-pc-windows-msvc/rustup-init.exe> |
| w64devkit 2.10.0 (seuls `as.exe`, `dlltool.exe`, `windres.exe` et `libshlwapi.a` servent) | assemblage d'outils libres ; `as` et `dlltool` viennent de GNU binutils (GPL) et ne sont pas redistribués avec OpenGarageMusic | <https://github.com/skeeto/w64devkit/releases/download/v2.10.0/w64devkit-x64-2.10.0.7z.exe> |

## Bibliothèques de code

Téléchargées par Cargo depuis <https://crates.io> ; la liste exacte et les versions
sont dans `Cargo.lock`. Celles qui comptent pour les licences :

| Bibliothèque | Rôle | Licence |
|---|---|---|
| egui / eframe | interface | MIT ou Apache-2.0 |
| cpal | entrée et sortie audio | Apache-2.0 |
| midir | claviers MIDI, et la sortie MIDI du MIDI Pilot | MIT |
| rfd | boîtes de dialogue de fichiers | MIT |
| rustysynth | lecture des banques SoundFont | MIT |
| symphonia | décodage MP3, AAC, FLAC, Ogg Vorbis | MPL-2.0 (utilisée sans modification) |
| clap-sys | définitions du standard de greffons CLAP | MIT ou Apache-2.0 |
| vst3, com-scrape-types | définitions des interfaces de greffons VST 3 (générées depuis le SDK VST 3, lui-même sous licence MIT depuis sa version 3.8) | MIT ou Apache-2.0 |
| libloading | chargement des fichiers de greffons | ISC |
| ureq, native-tls | téléchargements (connexion sécurisée par le système) | MIT ou Apache-2.0 |
| sha2 | sommes de contrôle des téléchargements | MIT ou Apache-2.0 |
| zip | ouverture des archives téléchargées | MIT |
| windows | appels aux encodeurs MP3 et AAC de Windows, fenêtres des greffons, fenêtre du lecteur vidéo | MIT ou Apache-2.0 |
| wry (du projet Tauri) | la vue web du lecteur vidéo | MIT ou Apache-2.0 |
| webview2-com, webview2-com-sys | définitions des interfaces WebView2 de Microsoft, dont se sert wry | MIT |
| serde, serde_json | fichiers de projet | MIT ou Apache-2.0 |
| image, zune-jpeg, png | lecture de l'image de l'écran de chargement, et des images du MIDI Pilot | MIT ou Apache-2.0 (zune-jpeg : ou Zlib) |
| regex | les règles des descriptions des sites | MIT ou Apache-2.0 |
| nam-rs | exécution des captures Neural Amp Modeler (`.nam`) ; une réécriture en Rust pur de NeuralAmpModelerCore, lui-même MIT | MIT |
| rustfft | les transformées de la convolution par réponses impulsionnelles | MIT ou Apache-2.0 |

L'hôte LV2 n'utilise aucune bibliothèque : il est écrit d'après la spécification
publique de LV2 (licence ISC) et lit lui-même les fichiers de description des greffons.
Les traductions sont lues par le lecteur de fichiers gettext `.po` du programme.

## Icône

L'icône du programme est le corps d'une guitare électrique bleue sur une tuile sombre,
dans `apps/opengarage/icon`.

| Fichier | Ce que c'est | Origine |
|---|---|---|
| `icon.png` et les fichiers `*.rgba` (la même image en 16, 24, 32, 48, 64 et 256 pixels) | L'icône | Un dessin produit par un générateur d'images par IA à la demande de l'auteur du projet, qui l'a fourni le 12 octobre 2026 ; il n'a été téléchargé de nulle part. `tools/icon.ps1` en a retiré le fond, l'a posé sur la tuile et l'a écrit à chaque taille |

Le dessin montre une guitare à deux échancrures, sans la tête, et ne porte ni nom ni
marque de fabricant. C'est un dessin, pas une photographie.

## Pictogrammes des pistes

Le pictogramme à gauche de l'en-tête de chaque piste, qui dit ce qu'elle joue, est un
dessin de [Game Icons](https://game-icons.net), sous la licence
[Creative Commons Attribution 3.0](https://creativecommons.org/licenses/by/3.0/deed.fr),
qui demande de nommer chaque auteur. Les fichiers, dans `apps/opengarage/track-icons`,
ont été repris sans changement le 15 octobre 2026 (la pellicule le 16 octobre 2026) de
la copie de la collection que tient Iconify
(<https://api.iconify.design/game-icons.json>) ; l'auteur de chacun a été vérifié sur sa page de game-icons.net. Le programme les remplit à la taille de
l'en-tête, d'une encre sombre sur la couleur de la piste.

| Fichier | Dessin | Auteur |
|---|---|---|
| `grand-piano.svg` | Piano, claviers | Caro Asercion |
| `piano-keys.svg` | Synthétiseur | Delapouite |
| `guitar-bass-head.svg` | Basse | Delapouite |
| `guitar.svg` | Guitare | Lorc |
| `violin.svg` | Cordes | Zajkonur |
| `trumpet.svg` | Cuivres | Delapouite |
| `saxophone.svg` | Bois et vents | Delapouite |
| `xylophone.svg` | Percussions à lames | Delapouite |
| `drum-kit.svg` | Batterie | Caro Asercion |
| `microphone.svg` | Voix | Delapouite |
| `sound-waves.svg` | Audio | Skoll |
| `musical-notes.svg` | Autre instrument | Delapouite |
| `film-strip.svg` | Vidéo | Delapouite |

## Écran de chargement

| Fichier | Ce que c'est | Origine |
|---|---|---|
| `apps/opengarage/splash/loading.jpg` | L'image montrée pendant le démarrage du programme, 1672 sur 941 pixels ; l'écran de chargement fait la moitié de cette taille | Une image du studio de l'auteur avec le nom du programme écrit dessus, fournie par l'auteur du projet le 14 octobre 2026 pour cet usage (`load.png`, mise en JPEG pour le programme ; le fichier PNG reste hors du dépôt) ; elle n'a été téléchargée de nulle part |

Les instruments et appareils de l'image portent le nom de leur fabricant, tels qu'ils
ont été achetés ; le projet n'est lié à aucun d'eux.

## Traductions

Les catalogues de `apps/opengarage/lang` ont été écrits pour le projet : le français à
partir des textes d'origine du programme, l'allemand, l'espagnol, l'italien et le
portugais avec l'aide d'une IA. Ils font partie du projet et suivent sa licence.

## Ce qui n'est pas dans le dépôt

- Les encodeurs MP3 et AAC sont ceux de Windows ; aucun codec n'est fourni.
- Aucun son, boucle, icône ou texte d'Apple.
- `WebView2Loader.dll` est copiée à la compilation et n'est que dans le téléchargement.
- Aucune vidéo ni rien de son son : le lecteur de YouTube les joue. Vos enregistrements
  et votre bibliothèque musicale restent sur votre disque, là où ils sont.
- Les banques SoundFont ou SFZ et les greffons CLAP, LV2 ou VST3 que vous importez ou
  téléchargez vous-même restent sur votre disque, hors du dépôt ; vérifiez leur licence
  avant de diffuser un projet qui les contient.
