# À faire

Demandes de l'auteur, notées les 13 et 14 octobre 2026, et ce qu'elles
sont devenues.

1. **Mode deux lignes** pour les vues Partition et Tab de l'éditeur — fait : la case
   « Deux lignes » de l'éditeur, décochée par défaut.
2. **Éditeur en plein écran** déroulé sur plusieurs lignes, comme Songsterr dispose un
   morceau — fait pour la partition comme pour la tablature : toute la région, ligne
   après ligne, la vue suivant le morceau.
3. **Meilleur rendu de la partition** — fait : armure et chiffrage, toutes les valeurs
   de notes avec points, ligatures et triolets, silences, hampes communes, et les
   annotations que donnent les sites (glissés, bends, hammer-on, vibrato, palm mute,
   let ring…, noms d'accords, sections).
4. **Import depuis d'autres sites** — fait en 0.9.7 : Fichier → « Importer depuis un
   site de tablatures… » cherche sur Guitariff et Tablature Hub dans le programme, comme
   sur Songsterr, et lit le fichier Guitar Pro du morceau choisi ; Ultimate Guitar, dont
   les conditions l'interdisent, n'est qu'ouvert (voir
   [Fichiers externes](external-files.md)). L'entrée est désormais Fichier → « Nouveau
   depuis un site de tablatures… ».
5. **Import de fichiers Guitar Pro** — fait : Fichier → « Nouveau depuis un fichier
   Guitar Pro… », un fichier déposé sur la fenêtre, ou le bouton de la fenêtre des
   sites.
6. **Plusieurs pistes dans l'éditeur** (demandé le 14 octobre 2026) — fait en 0.9.7 :
   Ctrl et un clic sur les en-têtes de pistes, les pistes l'une sous l'autre, et le
   bouton « Changement auto » qui n'en suit qu'une à la fois (la première choisie
   d'abord, puis la suivante qui joue, dès la dernière note de celle affichée, quand
   plus rien ne vient sur elle dans les mesures réglées dans Affichage → « Changer de
   piste après »).

## Demandes du 14 octobre 2026, faites en 0.9.9

7. **Écran de chargement** — fait : l'image fournie par l'auteur, à la moitié de sa
   taille, la version et une barre qui suit le démarrage du programme ; il reste au
   moins deux secondes.
8. **Moteur de sites de tablatures extensible** — fait : chaque site est décrit par un
   fichier JSON (comment y chercher, découper ses réponses en morceaux et en pistes,
   suivre ses pages jusqu'au fichier, l'afficher), avec des expressions régulières et
   des chemins JSON, sans code. Songsterr, Guitariff, Tablature Hub et Ultimate Guitar
   sont décrits ainsi ; l'utilisateur ajoute ou remplace des sites dans le dossier
   `sites` à côté du programme, sous sa responsabilité, et le programme demande une fois
   avant de lire un site. Voir [Décrire un site de tablatures](sites.md).
9. **Import MusicXML** — fait : `.musicxml`, `.xml` et `.mxl`, par le même menu que
   Guitar Pro et en déposant un fichier, avec les cordes et les cases d'une partie
   écrite en tablature.
10. **Import Guitar Pro 6 (`.gpx`)** — fait : son conteneur est ouvert et sa partition
    lue par le lecteur de GP7 et GP8.
11. **Relecture du lecteur GP3, GP4 et GP5** — faite d'après la description des formats
    de PyGuitarPro et 102 vrais fichiers : aucun fichier GP5 n'était lu jusque-là (un
    neuvième texte dans l'en-tête, pas de saut de ligne après la dernière mesure, la
    taille d'un son de mixage en 5.00) ; les harmoniques artificielles et tapées sonnent
    désormais à leur hauteur.

## Trouvé par la relecture du lecteur Guitar Pro, fait le 15 octobre 2026

12. **Sauts de la partition** — fait : D.C., D.S. et D.S.S., al fine, al coda et al
    double coda, les signes segno, coda et « to coda », tels que les écrivent les
    fichiers GP5 (dans leur en-tête), les fichiers GP6 à GP8 et les partitions MusicXML.
    L'ordre de lecture les suit : les reprises jusqu'au saut, puis le passage une fois,
    sur ses dernières fins, jusqu'au fine ou jusqu'à la coda.
13. **Swing (« triplet feel »)** — fait : écrit pour tout le morceau en GP3 et GP4, par
    mesure en GP5 à GP8 et en MusicXML, en croches ou en doubles croches. Les notes
    restent écrites droites et sont jouées en swing ; la tablature et la partition
    écrivent « Swing (croches) » ou « Droit » là où le feeling change.
14. **Trémolo, trilles et notes d'ornement tels qu'écrits** — fait : le trémolo est joué
    à sa vitesse (croches, doubles, triples croches) et dessiné avec ses barres ; une
    trille alterne avec son autre note à sa vitesse et est marquée « tr » ; une note
    d'ornement a sa durée écrite, avant le temps ou sur le temps, la note venant alors
    d'autant plus tard.

## Demandé le 15 octobre 2026

15. **Boucler des temps depuis l'éditeur** — fait en 0.9.9 : Maj et un clic sur les temps
    de la partition ou de la tablature, éclairés au passage de la souris quand Maj est
    enfoncée, et le bouton « Boucler les temps » de l'éditeur, dans le panneau ou en
    plein écran.
16. **Icônes des pistes** — fait en 0.9.9 : le pictogramme d'une piste est celui de
    l'instrument qu'elle joue, trouvé par le programme (douze sortes, dessinées par Game
    Icons) et non plus choisi à la main ; un clic dessus choisit la couleur de la piste.
17. **Icône dans la barre des tâches** — fait en 0.9.9 : la guitare bleue s'affiche aussi
    dans la barre des tâches de Windows, dès l'écran de chargement.
18. **Zoom des éditeurs de notes** — fait en 0.9.9 : de dix mesures par ligne, au zoom le
    plus faible, à quatre, au plus fort, dans le piano roll, la partition et la
    tablature.
19. **Sessions précédentes, menu Fichier, export MIDI** — fait : le programme garde les
    derniers projets sur lesquels on a travaillé (20 par défaut, de 5 à 100),
    enregistrés ou non, avec leurs prises et la disposition de la fenêtre, sans que
    l'utilisateur ait rien à enregistrer, et Fichier → « Sessions précédentes… » les
    liste, les rouvre ou les supprime, et déplace leur dossier. Le menu Fichier est
    regroupé en nouveaux projets (« Nouveau… », « Nouveau depuis un site de
    tablatures… », « Nouveau depuis un fichier Guitar Pro ou MusicXML… »), ouverture,
    enregistrement, et deux fenêtres : « Importer dans ce projet… » pour un fichier
    audio ou MIDI, « Exporter vers… » pour les formats, chacun avec quelques mots sur
    son usage. Le MIDI les rejoint : les notes des pistes telles qu'elles sont jouées,
    dans un Standard MIDI File. Un projet a un artiste et un titre, remplis par les
    imports et modifiables dans le Bloc-notes, qui le nomment dans la liste des
    sessions. Un projet ouvert ou importé est gardé aussitôt, modifié ou non. Les
    descriptions des sites de tablatures indiquent la version de leur langage
    (`format_version`), et une description écrite pour un programme plus récent n'est
    pas lue ; voir [Décrire un site de tablatures](sites.md#versions). La barre des
    tâches montre l'icône du programme : la fenêtre la demandait à l'exécutable sous un
    mauvais numéro.
20. **Zoom de la tablature, plein écran, noms d'accords** — fait : le curseur et Ctrl
    avec la molette zooment la tablature, d'autant de mesures que ses numéros de case
    restent lisibles (dix au plus) à quatre temps, dans son panneau et en plein
    écran ; le temps sous la souris y reste. En plein écran, la vue va aussitôt à la
    ligne jouée. Les noms d'accords écrits avec ♭, ♯, Δ, ° ou ø sont compris et
    affichés avec des lettres, et davantage de sortes d'accords ont une grille (7sus4,
    7sus2, 9, m9, maj9, madd9, mmaj7, 7#5, 7b5).
21. **Jouer un morceau avec son son d'origine** — fait, en quatre parties. La base : un
    enregistrement du morceau est une piste (une piste audio, ou une nouvelle sorte, la
    piste vidéo) avec la seconde où tombe chaque temps de la partition, et le bouton
    « Son d'origine » de la barre de transport joue le morceau soit tel qu'écrit, au
    tempo de la partition, soit au rythme de l'enregistrement qui mène, qu'on entend
    pendant que les autres pistes se taisent sauf si leur M est désactivé. Un fichier
    audio de l'utilisateur : Fichier → « Importer dans ce projet… » → « Un
    enregistrement du morceau… » le cale de lui-même sur la partition, en
    arrière-plan, en comparant les notes qui sonnent dans l'un et l'autre, et dit à
    quel point ils se ressemblent. Une bibliothèque musicale : un dossier de
    l'utilisateur, lu avec les étiquettes de ses fichiers, où l'enregistrement d'un
    morceau choisi sur un site de tablatures est cherché et proposé. Une vidéo YouTube
    de Songsterr : « Vidéo YouTube calée sur la tablature » prend la vidéo du morceau
    avec la seconde où commence chaque mesure, que posent les utilisateurs du site, et
    le lecteur de YouTube lui-même la joue calée sur le morceau, dans l'éditeur ou dans
    un coin, par un petit programme à part, `og-video.exe` ; rien n'est téléchargé. Le
    langage des descriptions des sites de tablatures passe à la version 2, qui ajoute
    `videos`, et la description de Songsterr s'en sert ; voir
    [Décrire un site de tablatures](sites.md#vidéos). Pendant que la vidéo joue, la tête de lecture va
    là où l'on clique (la règle, un clic dans le vide de la tablature, Alt et un clic
    dans la partition ou le piano roll, ou la barre du lecteur de YouTube), et la vidéo
    y va avec elle. La ligne entre les pistes montrées ensemble dans l'éditeur se fait
    glisser pour partager autrement sa hauteur.
22. **Les paroles du morceau** — fait : un bandeau sous l'éditeur, affiché ou masqué
    par le bouton « Paroles » du panneau du bas, chante les paroles avec le morceau,
    soit la ligne chantée, en grand, ses syllabes allumées à mesure qu'elles sont
    chantées, soit tout le texte, qui défile jusqu'à elle (Présentation → « Paroles du
    morceau ») ; un clic sur une ligne y emmène le morceau, et la vidéo. Les paroles
    viennent d'abord de la tablature, chaque syllabe sur sa note : la partie chantée
    de Songsterr, lue même quand elle n'est pas cochée, les paroles des fichiers
    Guitar Pro (versions 3 à 8) et les `<lyric>` des partitions MusicXML, placées
    comme Guitar Pro les place et mises en lignes. Sinon de LRCLIB, une base libre de
    paroles calées ligne par ligne, interrogée avec l'artiste et le titre. Sinon de
    l'utilisateur : un fichier LRC ou texte à côté de l'enregistrement, un fichier
    choisi, ou des paroles tapées. La fenêtre des sites de tablatures a la case « Les
    paroles du morceau », cochée au départ, et la description de Songsterr donne à ses
    pistes un champ `vocal`, lu dans `isVocalTrack`. Les paroles sont gardées dans le
    projet.

## Demandé le 17 octobre 2026, fait en 0.9.10

23. **Stop avec la vidéo** — fait : une vidéo lancée depuis le lecteur de YouTube ne
    relance plus le morceau quand on appuie sur Stop ; et le clavier reste toujours au
    programme (le champ de la recherche de tablatures ne le perd plus quand la page de
    YouTube se charge).
24. **Choisir parmi les vidéos d'un morceau** — fait : quand Songsterr en donne
    plusieurs, elles sont toutes gardées dans la piste vidéo, et une liste en haut de
    son éditeur en fait jouer une autre.
25. **Set List** (venue du programme G3S Pilot de l'auteur ; nommée MIDI Pilot depuis
    la 0.9.18) — fait : Fichier →
    « Set List… », les chansons d'un concert dans une fenêtre à part, lues du fichier
    CSV de G3S Pilot ou écrites à la main ; une chanson choisie envoie le numéro de son
    preset en MIDI au switcher de pédales (control change 0 ou program change) et, si
    on le demande, ouvre son projet ; projets associés à la main ou trouvés dans les
    sessions précédentes ou sur un site de tablatures ; pochettes de Deezer, ou une
    image de l'utilisateur par un clic droit. Voir [Fonctionnalités](features.md).

## Demandé le 17 octobre 2026, fait en 0.9.15

26. **Zoom des éditeurs sur tout écran** — fait : le zoom est en largeur seulement et
    sans fin dans les deux sens, réglé par « - » et « + » autour de sa valeur en bas à
    droite (un clic sur la valeur : de nouveau quatre mesures), les mêmes pixels par
    temps sur toutes les pistes et tous les morceaux, et gardé dans les réglages ; le
    premier numéro de case d'une tablature n'est plus coupé au bord gauche.
27. **Une relecture du code** — fait : aucune référence à l'auteur ni à un serveur
    privé ne reste dans le code ou les documents ; onze défauts trouvés par la
    relecture sont corrigés (« Enregistrer sous » dans un autre dossier qui perdait
    l'audio, un enregistrement en cours de calage qui atterrissait sur le projet
    suivant, la fenêtre de modification de la Set List après une suppression, des fils
    de téléchargement morts qui laissaient la fenêtre attendre, les paroles à côté
    d'un enregistrement sauvegardé jamais trouvées, « majeur »/« mineur » non
    traduits, Q qui quantifiait la mauvaise piste à plusieurs dans l'éditeur, les
    curseurs qui ne suivaient pas la fenêtre d'un plug-in CLAP, les bornes du tempo,
    un fichier Guitar Pro capable de planter le lecteur, et un fichier VST3 fermé
    sans `ExitDll`).

## Demandé le 17 octobre 2026, fait en 0.9.16

28. **Les restes de la relecture** — fait : une prise n'alloue plus rien dans le
    callback d'entrée (le callback remplit des morceaux que l'application lui prête
    et les lui rend sans attendre ; le son perdu faute de morceau est compté et
    signalé) ; le processus du greffon parle par une connexion propre à l'ordinateur
    (un socket sur 127.0.0.1) au lieu de sa sortie standard, où un greffon peut
    imprimer ce qu'il veut, et par un fil d'écriture à lui, de sorte que le fil audio
    n'attend jamais la connexion ; le code de l'ancienne fenêtre Songsterr, les
    helpers écrits deux fois (hexadécimal, hachages, chunks RIFF, extensions, dossiers
    à côté du programme, couleurs du thème), les dialogues de fichiers et les listes
    radio des menus sont chacun à un seul endroit ; les hôtes de greffons partagent
    leurs limites et le chargement des fichiers ; la partition ne dessine que les
    lignes visibles et calcule une fois ce que toutes les lignes partagent. Les deux
    versions du crate `windows` restent : les bibliothèques de carte son et de MIDI
    ont chacune la leur, et passer le programme à celle du lecteur vidéo en
    ajouterait une troisième.

## Demandé le 17 octobre 2026, fait en 0.9.17

29. **Des moteurs d'effets intégrés au programme, et leurs fichiers depuis les
    sites** — fait. En plus des greffons, les effets d'une piste ont des moteurs à
    eux, dans la chaîne de la piste (pédales → ampli → égaliseur → compresseur). Deux
    sortes :
    - *Les moteurs qui lisent un fichier apporté par l'utilisateur*, chacun avec son
      dossier dans la bibliothèque (`Captures`, `Impulsions`) :
      1. **Captures d'amplis, Neural Amp Modeler** (`.nam`, Architectures 1 et 2,
         lues par le crate `nam-rs`), **Proteus et AIDA-X** (`.json`, le LSTM lu par
         le programme) : le bouton « Capture… » du bloc « Ampli » ; la capture prend
         la place de l'étage de saturation maison, le bouton de gain règle son niveau,
         les boutons de tonalité restent derrière comme un égaliseur, le filtre
         « haut-parleur » seulement quand la capture n'a pas de baffle (`gear_type`).
         Exécutée dans le processus, sur le fil audio, à sa propre fréquence par un
         rééchantillonneur à sinus cardinal fenêtré quand la carte ou l'export tournent
         ailleurs ; un fil de travail par piste capturée reste en réserve si la charge
         le demande. Les captures couvrent aussi les pédales.
      2. **Convolution par réponse impulsionnelle** (`.wav`, `.flac`) : un moteur,
         deux usages : le baffle de l'ampli (« IR de baffle… », à la place du filtre
         haut-parleur) et une réverbération à convolution (« IR de salle… » du master,
         à la place de celle du programme). Convolution partitionnée uniforme par FFT
         (le crate `rustfft`), impulsions jusqu'à cinq secondes.
      Le fichier est gardé par son chemin, relatif, copié à côté du projet à
      « Enregistrer sous » ; le programme n'en embarque aucun. Un modèle et une
      impulsion minuscules générés par les tests vérifient chargement, traitement et
      sauvegarde.
    - *Les moteurs faits de code seul, rien à télécharger* : un noise gate, un wah
      (sa pédale est le premier bouton, que l'automation déplace), un auto-wah, un
      flanger, un écho à bande et un écho analogique (bucket-brigade) ; l'overdrive
      et l'étage à lampes de l'ampli par antiderivative antialiasing ; la plaque de
      Dattorro comme seconde réverbération partagée à côté de la salle. Écrits
      d'après les références publiées (Zavalishin, Parker, Dattorro, Smith), sans
      code GPL ; les modèles de circuits de pédales nommées (wave digital filters)
      sont laissés pour plus tard.
    - *Le téléchargement depuis les sites*, chaque fois par les descriptions des
      sites (`crates/og-engine/sites/*.json`), jamais par du code propre à un site :
      le format des descriptions est en `format_version` 3, avec ce qu'un site donne
      (`"gives": "tablature" | "capture" | "impulse"`), les lecteurs `capture` et
      `impulse`, des étapes dont la page liste plusieurs fichiers (`items`), et un
      `login` par le device flow d'OAuth 2 pour un site qui demande un compte ; un
      site dont la recherche ne demande rien à la requête liste ce qu'il a, et un mot
      trie la liste. La fenêtre des sites les liste par sorte et, pour un site qui
      donne des fichiers, liste les fichiers de l'élément choisi et télécharge ceux
      cochés dans la bibliothèque avec leur provenance. TONE3000 (captures ; une
      seconde description pour ses réponses impulsionnelles) est lu avec le compte de
      l'utilisateur : les conditions de son API pour les intégrations gratuites
      permettent les favoris, les captures du moment et les dernières, téléchargées
      une par une, et demandent à chaque programme une clé que l'utilisateur crée dans
      son compte, que le programme garde ; la ToneLibrary de GuitarML (Proteus) ne
      demande pas de compte. OpenAIR n'a pas été ajouté : son site est suspendu.

## Demandé le 24 octobre 2026, fait en 0.9.18

30. **Un projet en un seul fichier, enregistré dans les documents, plus léger** —
    fait. Un `.ogproj` est désormais une archive (ZIP) contenant `project.json` et,
    sous `media/`, les prises et les fichiers des effets venus avec le projet ; il
    voyage en un seul fichier. Le projet continue de travailler sur ses propres
    fichiers (une prise encore dans le dossier temporaire passe dans celui de la
    session à l'enregistrement), et un fichier de l'utilisateur ailleurs sur le disque
    reste nommé par son chemin. Les projets écrits avant, en JSON avec un dossier
    voisin, s'ouvrent toujours. Les projets sont enregistrés dans le dossier
    `OpenGarageMusic` des documents de l'utilisateur, ou là où Fichier → Options →
    « Dossier des projets… » le dit. À l'enregistrement, l'audio qu'aucune région ne
    joue plus, même comme autre prise, est oublié (il partait avec le projet pour
    rien) ; une prise peut être oubliée depuis le menu « Prises » d'une région. Les
    prises étaient déjà écrites sans perte, en WAV 24 bits.

31. **MIDI Pilot** — fait : la Set List s'appelle MIDI Pilot (menu, fenêtre, documents,
    son dossier dans celui de l'utilisateur, l'ancien gardé quand il existe), et
    `MidiPilot.exe`, à côté du programme, ouvre cette fenêtre seule, pour la scène :
    le programme lancé avec `--midi-pilot`, sans carte son, sans écran de chargement
    ni projet ; une chanson dont le projet doit s'ouvrir lance le programme entier
    dessus (un fichier de projet donné sur la ligne de commande s'ouvre au
    démarrage). Deux fenêtres du programme à la fois sont abandonnées ; importer les
    pistes d'un autre projet sera la façon de copier entre projets, à faire quand
    voulu.

## Demandé le 25 octobre 2026, fait en 0.9.19

32. **MIDI Pilot : le mode de la chaîne** — fait : deux modes sous Réglages → Mode. Celui
    du switcher, comme avant (un numéro par chanson, sélection de banque ou program
    change). Celui de la chaîne : les appareils de la chaîne (Réglages →
    « Appareils… » : nom, modèle, canal), et chaque chanson ses propres messages
    (program change avec ou sans sélection de banque, control change, system
    exclusive en hexadécimal), dans l'ordre, avec un délai après chacun, édités par
    « Modifier… » et essayés par « Envoyer maintenant » ; une colonne CSV nommée comme
    un appareil donne le programme qu'il reçoit, l'export écrit ces colonnes ; les
    octets envoyés se montrent au survol de la ligne d'état. Ensuite : les modules des
    pédales (BOSS DD-500 et RV-500 par SysEx, d'après les documents publics BOSS :
    patches, PC map, système, un fichier enregistré et rechargé, les sauvegardes de
    l'éditeur BOSS lues), puis le G3S (une sauvegarde `.g3b` écrite pour que
    l'éditeur du fabricant la restaure : banques, presets, boucles, lignes MIDI,
    globaux).

33. **MIDI Pilot : les modules des BOSS DD-500 et RV-500** — fait : Appareils →
    « Configurer… » sur un appareil de ce modèle parle à la pédale en system exclusive
    par la sortie et l'entrée MIDI choisies, d'après un fichier de description écrit
    depuis le document public du fabricant (`devices/dd500.json`, `rv500.json`, un
    dossier `devices` à côté du programme pour d'autres) : les mémoires et leurs noms,
    le patch choisi édité paramètre par paramètre et entendu aussitôt, enregistré
    après une sauvegarde et une relecture, la table des program changes, le système,
    un fichier enregistré et chargé, toutes les mémoires lues, le fichier renvoyé, les
    sauvegardes des éditeurs BOSS lues. Ensuite : le G3S (une sauvegarde `.g3b` écrite
    pour que l'éditeur du fabricant la restaure).

34. **MIDI Pilot : le module de The GigRig G3S** — fait : Appareils → « Configurer… »
    sur un appareil de ce modèle édite un fichier de sauvegarde du switcher (`.g3b`) :
    banques (ajoutées, retirées, déplacées, nommées, le mode chanson), les 28 presets de
    chacune, et pour le preset choisi son nom, ses boucles, l'ordre des boucles et ses
    messages MIDI ; banques nommées d'après les chansons et chansons d'après les
    banques ; « Vérifier » comme le switcher vérifie ; le fichier écrit pour que
    l'éditeur du switcher le restaure. Le format, non publié, a été déduit des
    sauvegardes du switcher lui-même (voir Fichiers externes).

35. **MIDI Pilot : scripts pour une IA** — fait : un format JSON d'actions (mémoires des
    pédales BOSS réglées avec les valeurs qu'elles affichent, table des program changes,
    système, banques et presets du G3S), le guide `ai-guide.md` écrit par le programme
    d'après les descriptions (Liste → « Guide pour une IA… », `--ai-guide`, dans
    l'archive), « Appliquer un script d'IA… » avec relecture de chaque action avant
    d'écrire les pédales, « Exporter pour une IA… » sur un patch, le fichier d'un module
    ou une sauvegarde G3S. Ensuite : une API REST locale pour les agents sur la machine
    (même format), puis peut-être un pont MCP.

36. **MIDI Pilot : une API locale pour l'agent d'une IA** — fait : Réglages → « API pour
    une IA… » (port sur 127.0.0.1, jeton, écritures sans demander ou le Oui de
    l'utilisateur dans la fenêtre), des routes qui lisent et écrivent les pédales et la
    sauvegarde G3S au format des scripts, une requête à la fois, documentées dans le
    guide. Peut-être plus tard : un pont MCP par-dessus.

37. **MIDI Pilot : les chansons dans les scripts d'IA** — fait : des actions `song`
    ajoutent, modifient et retirent les chansons de la liste (numéro, texte du bouton,
    messages aux appareils de la chaîne), Liste → « Exporter les chansons pour une
    IA… » et `GET /songs` les réécrivent ; dans le guide.

38. **MIDI Pilot : le pedalboard décrit pour une IA** — fait : dans la page du G3S, les
    pédales de chaque boucle (nom, genre, l'appareil de la chaîne si MIDI), l'ordre du
    signal et les switches de banque, gardés dans les réglages et affichés dans les
    presets ; ce que les modules de pédales lisent est gardé dans les données du
    programme ; Liste → « Exporter le pedalboard pour une IA… » et `GET /pedalboard`
    écrivent un seul fichier (la chaîne, le switcher, et tout ce qui est connu en
    actions) ; `{ "g3s": "loops" }` dans les scripts.
