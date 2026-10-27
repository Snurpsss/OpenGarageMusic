# OpenGarageMusic

*[English version](../en/README.md)*

Un studio de musique écrit de zéro en Rust, au code source publié et gratuit pour un
usage personnel (licence PolyForm Noncommercial : voir la licence dans le README du
projet ; un usage professionnel demande une licence à l'auteur), inspiré des
fonctionnalités de
GarageBand. Projet indépendant, sans lien avec Apple ; aucun code, son ni élément
graphique d'Apple n'est utilisé (voir [Contribuer](contributing.md)).

## Ce qu'il fait

- **Pistes** : instrument logiciel, audio, guitare ou basse avec ampli, batteur virtuel,
  piste principale ; muet, solo, verrou, indicateur de niveau ; le pictogramme de
  l'instrument que joue la piste, sur lequel un clic choisit sa couleur.
- **Sons** : bibliothèque de 36 sons (synthétiseur, cordes pincées, batteries), réglables
  par potentiomètres, enregistrables dans « Mes sons » ; arpégiateur.
- **Instruments échantillonnés** : banques SoundFont (`.sf2`, `.sf3`) et instruments SFZ
  (`.sfz`). Le menu Bibliothèque les télécharge depuis des sites qui les distribuent sous
  licence libre, ou les importe depuis le disque ; aucune n'est fournie.
- **Greffons** : effets et instruments aux formats CLAP (`.clap`), LV2 (dossier `.lv2`)
  et VST3 (fichier ou dossier `.vst3`), importés depuis le disque, trouvés dans les
  dossiers standard du système, ou téléchargés depuis des projets libres (menu
  Bibliothèque), puis ajoutés à une piste depuis les réglages du son. Un greffon se
  règle par curseurs, ou dans sa propre fenêtre quand il en a une.
- **Fenêtre de téléchargement** : les sites à gauche, ce qu'ils proposent sous licence
  libre au milieu, le rangement à droite. Ce qui est déjà téléchargé est coché.
- **Réglages du son** : chaque boîte (son, arpégiateur, greffons, égaliseur…) se fait
  glisser par son titre à côté, au-dessus ou en dessous d'une autre, se masque par clic
  droit, et se réaffiche par clic droit sur le fond. La disposition est conservée.
- **Langues** : anglais (par défaut), français, allemand, espagnol, italien, portugais,
  dans Présentation → Langue. Chaque langue est un fichier gettext `.po` : voir
  [Traduire](translating.md).
- **Batteur virtuel** : sept styles, complexité, intensité, swing, breaks, éléments du
  kit ; sa grosse caisse peut suivre les notes d'une autre piste (la basse, par exemple).
- **Régions** : créer, déplacer (y compris vers une autre piste), redimensionner, boucler,
  scinder, joindre, couper, copier, coller. Glissée avec Alt ou Ctrl enfoncé, une région
  ou une note est copiée au lieu d'être déplacée.
- **Éditeurs** : notes (sélection multiple, vélocité, quantification, transposition),
  partition, tablature pour une piste de guitare ou de basse (l'accordage et le doigté
  propres à la piste quand un import Songsterr les a donnés, les noms d'accords en haut,
  chacun avec une grille de doigté), batteur, audio (transposition sans changer la
  durée, justesse, recalage des attaques sur la grille). Les éditeurs de notes zooment
  avec Ctrl et la molette, ou avec le
  « - » et « + » en bas à droite, en largeur seulement et aussi loin qu'on veut dans
  les deux sens (la valeur entre eux est le zoom, qu'un clic remet à quatre mesures en
  largeur ; une tablature commence à quatre temps) ; la tablature a un curseur « Taille du texte » pour ses
  numéros de case, ses noms d'accords et ses grilles d'accords. La partition écrit
  l'armure et le chiffrage, les valeurs des notes avec ligatures, points et triolets,
  les silences, et les techniques d'une tablature. La case « Deux lignes » dispose la
  partition ou la tablature sur deux lignes de mesures entières, comme sur une page ;
  le plein écran dispose toute la région ainsi, ligne après ligne, et suit le morceau.
  Ctrl et un clic sur les en-têtes de pistes mettent plusieurs pistes dans l'éditeur,
  l'une sous l'autre ; le bouton « Changement auto » n'en montre alors qu'une à la
  fois, celle qui joue, la première choisie ayant la priorité, et passe à la suivante,
  dès la dernière note de celle affichée, quand plus rien ne vient sur elle dans les
  mesures réglées dans Affichage → « Changer de piste après ».
  Maj et un clic sur un temps de la partition ou de la tablature, puis sur un autre,
  choisissent les temps qui les séparent, et « Boucler les temps » les joue en boucle.
  Pendant la lecture, les
  notes défilent sous une tête de lecture maintenue à un cinquième de la largeur depuis
  la gauche ; Affichage → « Tête de lecture fixe dans l'éditeur » désactive cela, et
  l'éditeur tourne alors la page.
- **Enregistrement** : notes jouées au clavier de l'ordinateur ou sur un clavier MIDI,
  audio depuis l'entrée de la carte son avec prises multiples et écoute de l'entrée,
  décompte, métronome, accordeur.
- **Boucles** : 42 boucles de notes qui suivent le tempo et la tonalité, avec filtres,
  écoute et favoris. Une boucle se glisse du navigateur sur une piste, ou sous les
  pistes, où une piste est créée pour elle. Vos propres enregistrements s'ajoutent
  comme **boucles audio** (« Ajouter des fichiers… » dans le navigateur) : elles sont
  étirées au tempo du projet. Aucune boucle enregistrée n'est fournie.
- **Mixage** : égaliseur (une courbe à trois points à déplacer), compresseur, écho et
  réverbération par piste, amplis et une chaîne de pédales choisies une par une
  (overdrive, noise gate, wah, auto-wah, chorus, flanger, trémolo, phaser, trois
  échos), effets sur la sortie générale, une salle ou une plaque comme réverbération
  partagée. Automation du volume et du panoramique, et des effets de chaque piste :
  réverbération, écho, compresseur, les bandes de l'égaliseur, le gain de l'ampli, les
  pédales (dont la pédale du wah).
- **Captures et réponses impulsionnelles** : l'ampli d'une piste peut exécuter une
  capture d'un vrai ampli ou d'une vraie pédale, un fichier `.nam` de Neural Amp
  Modeler ou un `.json` de Proteus ou d'AIDA-X, à la place de son étage de saturation ;
  une réponse impulsionnelle (un fichier WAV ou FLAC) peut tenir lieu de son baffle, et
  une autre de la salle de la réverbération partagée. Vous apportez les fichiers, ou
  les téléchargez depuis TONE3000 (avec votre compte) et la ToneLibrary de GuitarML par
  la fenêtre des sites ; aucun n'est fourni.
- **Dépôt de fichiers** : un fichier audio ou MIDI glissé depuis l'explorateur de
  fichiers sur les pistes se pose sur la mesure sous le pointeur (l'audio sur une piste
  audio, ou une nouvelle) ; une banque de sons rejoint la bibliothèque ; un fichier de
  projet s'ouvre.
- **Arrangement** : pistes de tempo et de transposition, sections qui se déplacent avec
  leur contenu, zone de boucle. Pendant la lecture, la tête de lecture reste au milieu
  et les pistes glissent dessous ; Présentation → « Tête de lecture maintenue au milieu
  des pistes » désactive cela, et la tête de lecture traverse alors les pistes pendant
  que la vue tourne la page.
- **Fichiers** : un projet est un seul fichier (`.ogproj`, une archive du projet en
  JSON et de ses prises), enregistré dans le dossier `OpenGarageMusic` de vos documents
  sauf si vous choisissez ailleurs ; les derniers sont listés dans le menu Fichier et
  au démarrage. Le menu Fichier commence de nouveaux projets
  (« Nouveau… », « Nouveau depuis un site de tablatures… », « Nouveau depuis un
  fichier Guitar Pro ou MusicXML… »), les ouvre (« Ouvrir… », « Ouvrir un projet
  récent », « Sessions précédentes… »), les enregistre, et fait entrer et sortir des
  fichiers :
  - « Importer dans ce projet… » ouvre une fenêtre qui ajoute un fichier audio (WAV,
    MP3, AAC) sur la piste audio choisie, ou une nouvelle, à la tête de lecture, ou un
    fichier MIDI, dont les pistes de notes deviennent de nouvelles pistes, ou « Un
    enregistrement du morceau… », qui est calé sur la partition (voir **Son
    d'origine**). On peut toujours déposer des fichiers sur la fenêtre ; les banques de
    sons s'importent depuis le menu Bibliothèque.
  - « Exporter vers… » ouvre une fenêtre qui liste les formats, chacun avec quelques
    mots sur son usage : WAV et AIFF en 16 ou 24 bits, MP3 et AAC (`.m4a`) avec leur
    qualité (128, 192, 256 ou 320 kb/s, ce dernier pour le MP3 seulement), et MIDI
    (`.mid`). Le fichier MIDI contient les notes des pistes telles qu'elles sont jouées
    (swing, trémolo, trilles… compris) et aucun son : un Standard MIDI File au format 1,
    une piste pour chaque piste qui a des notes, avec le tempo et ses changements, la
    signature rythmique, le programme General MIDI d'une piste qui joue un son
    échantillonné, et les pistes de batterie et de batteur sur le canal 10. Tout le
    morceau est exporté, de son début à son repère de fin.
- **Songsterr** : Fichier → « Nouveau depuis un site de tablatures… », Songsterr
  choisi en haut de la fenêtre, cherche sur songsterr.com, liste les pistes d'une tablature (guitares, basse, batterie, chant…) et
  fait un nouveau projet de celles qui sont cochées, une piste de notes chacune, avec le
  tempo, la signature rythmique, la tonalité et les sections. Chaque piste reçoit
  l'instrument que nomme la tablature, pris dans une banque General MIDI installée
  quand il y en a une (Bibliothèque → Télécharger des sons), ou un son intégré de la
  même famille sinon. Ce que la tablature écrit autour de ses notes vient aussi :
  glissés, bends, hammer-on et pull-off, vibrato, harmoniques, notes étouffées et
  fantômes, palm mute, let ring… Ils sont dessinés dans la vue tablature et joués : un
  bend ou un glissé fait bouger la hauteur de la note pendant qu'elle sonne. Aucun
  compte n'est nécessaire : le programme lit ce que le site montre à tout le monde.
- **Fichiers Guitar Pro, partitions MusicXML et autres sites de tablatures** : la même
  fenêtre, Fichier → « Nouveau depuis un site de tablatures… », cherche aussi sur
  Guitariff et Tablature Hub : le site se choisit en haut, une recherche liste les
  tablatures Guitar Pro trouvées, en choisir une lit son fichier en arrière-plan et
  liste ses parties avec des cases à cocher, et « Créer le projet » fait le projet de
  celles qui sont cochées. Ultimate Guitar n'est qu'ouvert dans le navigateur, comme
  ses conditions le demandent. Chaque site est décrit par un petit fichier JSON :
  d'autres sites s'ajoutent dans le dossier `sites` à côté du programme (voir
  [Décrire un site de tablatures](sites.md)) ; le programme demande une fois avant de
  lire un site. Fichier → « Nouveau depuis un fichier Guitar Pro ou MusicXML… » et le
  bouton en bas de la fenêtre font de même un projet d'un fichier Guitar Pro (.gp3,
  .gp4, .gp5, .gpx ou .gp) ou d'une partition MusicXML (.musicxml, .xml ou .mxl), et un
  tel fichier peut être déposé sur la fenêtre. La section « Son d'origine » de la
  colonne des pistes propose, pour le morceau choisi, son enregistrement quand votre
  bibliothèque musicale l'a (« Votre enregistrement : … », coché ; ✕ le refuse), ou un
  bouton « Un enregistrement du morceau… » pour choisir un fichier ; la case « Les
  paroles du morceau », cochée au départ (voir **Paroles du morceau**) ; et, pour
  Songsterr, la case « Vidéo YouTube calée sur la tablature », décochée au départ (voir
  **Son d'origine**).
- **Son d'origine** : un morceau peut se jouer avec son enregistrement. Un
  enregistrement est une piste : une piste audio qui contient un fichier audio à vous
  (MP3, OGG, FLAC, M4A/AAC, WAV), ou une piste vidéo qui contient une vidéo YouTube,
  chacune avec son calage, la seconde où tombe chaque temps de la partition. Le bouton
  « Son d'origine » de la barre de transport, affiché quand le projet a un
  enregistrement, passe à tout moment d'une façon de jouer à l'autre :
  - désactivé, le morceau joue tel qu'écrit, au tempo de la partition, et les
    enregistrements sont muets ;
  - activé, le morceau joue au rythme de l'enregistrement qui mène : le tempo vient de
    son calage, mesure par mesure, de sorte qu'un groupe qui accélère ou qui traîne
    est suivi. L'enregistrement s'entend, et les autres pistes se taisent sauf si leur
    bouton M est désactivé : dans ce mode, M veut dire « muette pendant qu'on joue avec
    l'enregistrement », et le désactiver fait jouer cette piste par-dessus
    l'enregistrement. Il n'y a pas de décompte ; les éditeurs défilent avec le morceau
    comme d'habitude. Avec plusieurs enregistrements, l'en-tête de chaque piste
    d'enregistrement dit lequel mène, et un clic sur un autre le fait mener.

  *Un fichier audio à vous* : Fichier → « Importer dans ce projet… » → « Un
  enregistrement du morceau… ». En arrière-plan, le programme trouve où tombent les
  temps de la partition dans l'enregistrement : vingt fois par seconde, il compare
  quelles notes de la gamme sonnent dans la partition (d'après ses notes) et dans
  l'enregistrement (d'après son spectre, sur environ un tiers de seconde), et où les
  notes sont attaquées, la batterie mise de côté ; un alignement temporel dynamique
  (« dynamic time warping ») apparie les deux, de sorte que la partition peut
  commencer n'importe où dans l'enregistrement (une intro, un silence) et que son
  rythme peut changer. Un morceau de quatre minutes prend environ deux secondes. Le
  programme dit à quel point les deux se ressemblent, en % ; sous 30 %, il prévient que
  ce n'est peut-être pas le même morceau ou la même version. L'enregistrement va sur sa
  propre piste audio, qui mène, et le morceau passe aussitôt au son d'origine ;
  l'éditeur de cette piste a « ◀ 0,1 s » et « 0,1 s ▶ » pour décaler l'enregistrement
  par rapport à la partition. Le fichier reste où il est : le projet y renvoie par son
  chemin, il n'est pas copié, et rien n'est envoyé nulle part.

  *Votre bibliothèque musicale* : Fichier → Options → « Dossier de la bibliothèque
  musicale… ». Le programme lit ce dossier et ses sous-dossiers en arrière-plan (MP3,
  OGG, FLAC, M4A, AAC, MP4, WAV, jusqu'à 100 000 fichiers), chaque fichier connu par
  l'artiste et le titre de ses étiquettes (ID3, commentaires Vorbis, MP4), ou sinon par
  son nom (« Artiste - Titre.mp3 », un numéro de piste au début étant sauté). L'index
  est gardé à côté des réglages (`library.json`), et seuls les fichiers qui ont changé
  sont relus. Les noms sont comparés simplement : la casse, les accents, ce qui est
  entre parenthèses ou crochets (« (Remastered 2011) », « [Live] »), « feat. » et ce
  qui suit, et un « The » au début ne comptent pas. Quand un morceau est choisi dans
  la fenêtre des sites de tablatures, son enregistrement est proposé, et une fois le
  projet créé il est calé sur la partition comme ci-dessus.

  *Une vidéo YouTube de Songsterr* : quand « Vidéo YouTube calée sur la tablature »
  est cochée, le programme demande au site, comme le fait son lecteur public, les
  vidéos du morceau et la seconde où commence chaque mesure dans chacune : les points
  que posent les utilisateurs du site (152 points pour les 152 mesures jouées de
  Nothing Else Matters, reprises dépliées). La vidéo originale, ou sinon la première,
  devient une piste vidéo ; si les nombres de mesures diffèrent de plus d'une, un
  message dit qu'elle peut dériver. Sélectionnée, la piste vidéo montre la vidéo dans
  l'éditeur, aussi grande que le panneau le permet en 16:9 ; Ctrl et un clic sur
  l'en-tête d'une piste de guitare affichent la vidéo et la tablature l'une sous
  l'autre. Avec le son d'origine activé et la vidéo qui mène, le lecteur de YouTube
  lui-même la joue calée sur le morceau : le programme la lance et l'arrête avec le
  morceau, la suit plutôt que de l'envoyer ailleurs (elle s'arrêterait et se rechargerait) : quand les deux s'écartent de plus d'un huitième de seconde, le morceau va là où est la vidéo ; le morceau joue avec la vidéo dès que le projet est créé, ses autres pistes muettes, et un clic sur Lecture dans le lecteur de YouTube lance le morceau avec elle ; et le
  morceau attend pendant que la vidéo se charge ou montre une publicité, puis repart
  avec elle. Quand l'éditeur ne montre pas la vidéo, le bouton « Vidéo », à gauche de « Plein écran », l'affiche dans un coin de la fenêtre (il est désactivé au départ ; YouTube demande que son lecteur soit visible pendant qu'il joue).
  L'éditeur de la piste vidéo a les boutons de décalage et « Retard du lecteur vidéo »
  (en ms, un réglage de l'ordinateur, pour toutes les vidéos). Si YouTube ne lit pas
  une vidéo (retirée, bloquée dans votre pays ou hors de YouTube), l'éditeur le dit
  avec son code d'erreur. YouTube Music ne peut pas servir : il n'a pas de lecteur qui
  puisse être intégré. Le programme ne garde aucune vidéo ni rien de son son : rien
  n'est téléchargé. Le lecteur est `og-video.exe`, un petit programme à côté du
  programme qui montre le lecteur de YouTube dans une vue web WebView2 (présente dans
  Windows 10 et 11) dessinée dans la fenêtre ; il lui faut `WebView2Loader.dll` à côté
  de lui, et il garde ses données dans `%LOCALAPPDATA%\OpenGarageMusic\WebView2`. Comme
  le lecteur est un processus à part, le programme démarre sans cette DLL ; seule la
  vidéo ne démarre pas.
- **Paroles du morceau** : le bouton « Paroles » de la barre du panneau du bas, à
  gauche de « Vidéo » (quand le projet a une vidéo) et de « Plein écran », affiche ou
  masque un bandeau des paroles sous l'éditeur, au-dessus de la barre d'état. Il est
  affiché au départ, seulement avec l'onglet Éditeur, et disparaît avec le panneau du
  bas ; sa hauteur s'ajoute au panneau au lieu d'être prise à l'éditeur.
  Présentation → « Paroles du morceau » a « Affichées sous le panneau » et deux
  façons : « Les lignes en train d'être chantées » (la ligne chantée, en grand, les syllabes déjà
  chantées allumées, avec la ligne d'avant, pâle, et celle d'après) ou « Tout le
  texte, qui défile » (toutes les lignes, celle qui est chantée allumée, le texte
  défilant jusqu'à elle). Un clic sur une ligne y emmène le morceau, et la vidéo aussi
  quand il y en a une qui joue. La ligne au-dessus du bandeau se fait
  glisser pour le grandir ou le réduire, sous les pistes comme en plein écran. Les paroles viennent de la première de ces sources qui
  les a :
  - *La tablature* : les paroles de sa partie chantée, chaque syllabe sur sa note. De
    Songsterr, les paroles de la partie chantée sont lues même quand sa piste n'est pas
    cochée ; d'un fichier Guitar Pro (versions 3 à 8), les paroles du fichier, cinq
    lignes commençant chacune à une mesure, sur la piste à laquelle elles
    appartiennent ; d'une partition MusicXML, les `<lyric>` des notes, premier couplet.
    Les règles sont celles qu'écrit Guitar Pro : chaque syllabe va sur le temps suivant
    qui attaque une note (les silences, les notes d'ornement et les notes seulement
    liées n'en prennent pas) ; une espace ou un trait d'union termine une syllabe
    (« laugh-ter » en fait deux) ; un trait d'union ou un tiret bas seul prolonge la
    syllabe d'avant d'une note ; deux espaces laissent une note sans paroles ; une
    mesure rejouée chante de nouveau ses syllabes ; une ligne du texte n'est qu'une
    coupure entre lignes. Le texte est écrit à la main et ne suit pas toujours ces
    règles : les syllabes sont ajustées aux notes sur tout le morceau à la fois, une
    ligne commençant après un silence du chant, une espace ou un trait d'union de trop
    prenant une note ou non, une note restant sans syllabe quand une voyelle est tenue ;
    une erreur du texte ne décale pas toutes les syllabes qui la suivent. Les voyelles
    tenues entre parenthèses (« u-po-(o)n ») se lisent sans elles (« upon »). Les paroles sont ensuite mises en
    lignes : une ligne commence à un mot écrit avec une majuscule (pas « I », « I'm »,
    « Lord », « God »…), après une pause d'une mesure et demie, ou quand elle
    dépasserait environ 48 caractères. Les paroles chantées suivent la partition, que
    le morceau joue tel qu'écrit ou avec son son d'origine.
  - *LRCLIB* ([lrclib.net](https://lrclib.net)), une base libre de paroles calées
    ligne par ligne, sans compte : elle est interrogée avec l'artiste et le titre du
    projet (et la durée de l'enregistrement quand il y en a un, pour prendre la bonne
    version), d'elle-même quand une tablature ou un fichier Guitar Pro ou MusicXML sans
    paroles devient un projet, et avec le bouton « Chercher sur LRCLIB » du bandeau.
    Les lignes calées suivent les secondes de l'enregistrement qui mène (ou du premier
    enregistrement, ou le tempo de la partition quand il n'y en a pas) ; « ◀ 0,1 s » et
    « 0,1 s ▶ » les décalent.
  - *Les vôtres* : un fichier LRC ou texte du même nom à côté de votre enregistrement
    du morceau (`Morceau.lrc` ou `Morceau.txt` à côté de `Morceau.mp3`), lu quand
    LRCLIB n'a rien ou quand un enregistrement est lié à un morceau sans paroles ; un
    fichier choisi avec « Depuis un fichier… » ; ou des paroles tapées ou collées avec
    « Taper… ». Les lignes qui commencent par un temps comme `[01:23.45]` sont calées,
    et le `[offset:+250]` d'un fichier LRC est compris.

  Les paroles sont gardées dans le projet. Comme les tablatures, elles sont le plus
  souvent protégées par le droit d'auteur : elles sont là pour votre propre pratique.
- **Sons** : le bouton du son dans l'en-tête d'une piste ouvre la bibliothèque sur ce
  son, dans sa section. Une banque téléchargée ou importée une seconde fois est rangée
  là où on le demande cette fois (par famille, dans sa propre section, ou dans une seule
  section).
- **Périphériques audio** : le menu **Audio** choisit la sortie de toutes les pistes,
  l'entrée, et le canal de l'entrée qui est enregistré (entrée 1, entrée 2… ou tous
  mélangés). Une piste peut aussi être envoyée vers sa propre sortie (un casque pour
  le batteur, une seconde carte son…) et une piste audio peut enregistrer sa propre
  entrée et son propre canal : clic droit sur l'en-tête de la piste.
- **Aide rapide** : le bouton « ? » à droite de la barre de transport. Tant qu'il est
  activé, la barre du bas dit ce que le pointeur survole, et les bulles des boutons
  s'affichent tout de suite.
- **Thèmes** : l'aspect propre au programme, un sombre et un clair, dans
  Présentation → Thème. Un thème est un petit fichier JSON ; on peut importer le
  sien : voir [Thèmes](themes.md).
- **Disposition** : les réglages du son et l'éditeur sont sous les pistes, à une
  hauteur qui ne change que si l'on fait glisser son bord supérieur ; « Plein écran »
  affiche le panneau seul, « Remettre sous les pistes » le replace.
- **MIDI Pilot** : Fichier → « MIDI Pilot… », une fenêtre à part (sur un second écran en
  concert) avec un gros bouton par chanson d'un concert ou d'une répétition. Une liste
  s'importe d'un fichier CSV (numéro, artiste, chanson, et texte du bouton,
  facultatif) ou s'écrit chanson par chanson. Choisir une chanson envoie le numéro de
  son preset en MIDI à un switcher de pédales comme The GigRig G3S (control change 0
  ou program change, sur la sortie et le canal réglés dans le menu Réglages de la
  fenêtre), ou, dans le mode de la chaîne, ses propres messages (program changes,
  control changes, system exclusive) aux appareils de la chaîne, chacun sur son canal,
  et, avec « Ouvrir le projet de la chanson » coché, ouvre son projet. Les BOSS DD-500
  et RV-500 ont un module (Appareils → « Configurer… ») qui édite leurs patches par
  MIDI, leur table des program changes et leur système, enregistrés dans un fichier et
  renvoyés ; The GigRig G3S en a un qui édite un fichier de sauvegarde du switcher
  (banques, presets, MIDI) que son éditeur restaure. Une IA à qui l'on donne le guide
  (Liste → « Guide pour une IA… », `ai-guide.md`, en anglais, aussi `docs/en/ai-guide.md`)
  écrit des scripts pour tout cela,
  appliqués par « Appliquer un script d'IA… » après relecture ; un agent sur la machine
  peut passer par une API locale (Réglages → « API pour une IA… »). Un
  clic droit sur une chanson lui donne une image à vous ou un projet, ou trouve son
  projet, dans les sessions précédentes ou sur un site de tablatures ; « Trouver les
  projets de toute la liste » prépare un concert d'un coup. Les boutons montrent les
  pochettes trouvées sur Deezer, ce qui peut se désactiver.
- **Sessions précédentes** : le programme garde de lui-même les derniers projets sur
  lesquels on a travaillé, sans qu'on ait rien à enregistrer. Chaque projet a sa
  session, un dossier au nom de l'identifiant du projet qui le contient tel qu'il était,
  enregistré ou non, avec ses prises enregistrées, la disposition de la fenêtre, et un
  petit `about.json` pour la liste. Une session est écrite quand le programme se
  ferme, avant qu'un autre projet prenne la place de celui qui est ouvert (Nouveau,
  Ouvrir, réouverture d'une session…), et toutes les deux minutes tant que le projet
  change. Un projet ouvert depuis un fichier ou fait depuis un site de tablatures, un
  fichier Guitar Pro ou MusicXML est gardé aussitôt, modifié ou non ; seul un projet
  vierge laissé intact (la démonstration au démarrage, un nouveau projet depuis un
  modèle) ne l'est pas. Fichier → « Sessions précédentes… »
  les liste, la plus récente en premier, une ligne chacune :
  `Artiste – Titre · N pistes · m:ss · quand · fichier`, le moment étant « à
  l'instant », « il y a 12 min », « il y a 3 h » ou une date, et le fichier le chemin
  du fichier du projet ou « jamais enregistré dans un fichier » ; un projet sans
  artiste ni titre montre son nom. « Rouvrir » ou un double-clic en ramène une, après
  avoir gardé celle qui est ouverte ; « Supprimer » en retire une. La fenêtre règle
  aussi combien en sont gardées (« Garder les [20] projets », de 5 à 100) : les plus
  anciennes au-delà partent, jamais celle qui est ouverte. Elle montre le nombre de
  sessions et la place qu'elles prennent (et la taille de chacune sous le pointeur),
  « Ouvrir le dossier » les montre dans l'explorateur de fichiers, et « Changer de
  dossier… » les déplace ailleurs ; par défaut elles sont dans
  `%USERPROFILE%\OpenGarageMusic\Sessions`.
  Au démarrage, la dernière session revient telle qu'on l'a laissée. Si l'on décoche
  Fichier → Options → « Rouvrir le programme tel que je l'ai laissé », le programme
  démarre plutôt sur le projet de démonstration, et les projets restent tout de même
  dans la liste. Quitter ne demande rien tant que la session peut être écrite ;
  seulement quand elle ne le peut pas, et que le projet a des modifications, le
  programme propose de l'enregistrer. La session laissée par une version précédente
  revient encore au premier démarrage.
- **Artiste et titre** : un projet peut avoir un artiste et un titre, remplis par les
  sites de tablatures et par les fichiers Guitar Pro et MusicXML qui les donnent, et
  modifiables en haut du Bloc-notes (Présentation → Bloc-notes, à droite). Ils
  nomment le projet dans la liste des sessions précédentes.

L'avancement détaillé et les écarts connus : [Fonctionnalités](features.md). La
provenance et la licence de tout ce qui n'a pas été écrit pour le projet :
[Fichiers externes](external-files.md). Ce que l'auteur a demandé ensuite, et ce qui en
est advenu : [À faire](todo.md).

## Lancer

```
run.cmd
```

La première compilation prend quelques minutes. Le programme se trouve ensuite dans
`target\release\opengaragemusic.exe`.

## Aide-mémoire

| Action | Geste |
|---|---|
| Lecture / arrêt | Espace |
| Retour au début | Entrée |
| Enregistrer sur la piste sélectionnée | R |
| Boucle, métronome | C, K |
| Placer la tête de lecture | Clic dans la règle |
| Tracer la zone de boucle | Glisser dans le haut de la règle |
| Créer une région | Double-clic dans une piste d'instrument ou de batteur |
| Déplacer une région | Glisser (vers une autre piste aussi) |
| Changer sa durée / la boucler | Glisser le bas / le haut de son bord droit |
| Scinder, joindre, boucler | Ctrl+T, Ctrl+J, L |
| Couper, copier, coller | Ctrl+X, Ctrl+C, Ctrl+V |
| Poser une boucle sur les pistes | La glisser du navigateur de boucles sur une piste, ou sous les pistes |
| Ajouter une note | Clic dans l'éditeur |
| Remettre les notes sur la grille | Q (notes sélectionnées, ou toutes) |
| Déplacer un point de l'égaliseur | Le glisser ; un double-clic remet son gain à 0 |
| Sélectionner des notes | Glisser dans le vide, Maj+clic, Ctrl+A |
| Supprimer une note | Clic droit, ou Suppr sur la sélection |
| Jouer au clavier de l'ordinateur | Ctrl+K, puis la rangée centrale (Q S D F… ou A S D F…) |
| Bibliothèque, réglages du son, éditeur, boucles | Y, B, E, O |
| Automation | A, puis clic dans une piste pour poser un point |
| Zoom horizontal | Ctrl + molette, ou le curseur près de « + Piste » |
| Défilement horizontal | Maj + molette |
| Annuler / rétablir | Ctrl+Z / Ctrl+Y |
| Déplacer une boîte des réglages du son | Glisser son titre sur une autre boîte : tiers gauche ou droit pour la mettre à côté, milieu pour la mettre au-dessus ou en dessous |
| Masquer / réafficher une boîte | Clic droit sur la boîte / clic droit sur le fond |
| Fenêtre propre d'un greffon | Clic droit sur la boîte du greffon, ou son bouton « Ouvrir la fenêtre du greffon » |

## Greffons

| Format | Ce qui est pris en charge | Où ils sont cherchés |
|---|---|---|
| CLAP | effets et instruments, curseurs, fenêtre propre, état enregistré dans le projet | téléchargements, fichiers importés, `Common Files\CLAP` |
| VST3 | effets et instruments, curseurs (de 0 à 1, comme le format les définit), fenêtre propre, état enregistré dans le projet, calcul en 32 ou 64 bits, toutes les sorties mélangées | téléchargements, fichiers importés, `Common Files\VST3` |
| LV2 | effets et instruments, curseurs, fenêtre propre quand le greffon en décrit une pour Windows (`ui:WindowsUI`), état enregistré dans le projet | téléchargements, dossiers importés, `Common Files\LV2`, `%APPDATA%\LV2` |

Un greffon démarre avec les curseurs de l'application. Sa propre fenêtre s'ouvre à la
demande, et tant qu'elle est ouverte les curseurs lui cèdent la place.

**Chaque greffon tourne dans son propre processus** (Fichier → Options, « Lancer chaque
greffon dans son propre processus », activé par défaut). Un greffon est le programme
de quelqu'un d'autre : s'il plante ou ne répond plus, seul son processus disparaît.
L'effet est contourné ou l'instrument se tait, le projet reste ouvert, et la boîte du
greffon propose de le relancer. Cela protège l'application d'un greffon défectueux ;
ce n'est pas une barrière de sécurité : le greffon s'exécute toujours avec vos droits
et n'est pas vérifié.

## Développer

Tout l'outillage est installé dans `.toolchain`, rien sur le système :

```
powershell -ExecutionPolicy Bypass -File tools\setup-toolchain.ps1
dev test --workspace
dev build --release -p opengaragemusic
```

- `crates/og-engine` : modèle de projet, générateurs de sons, effets, batteur, boucles,
  séquenceur et mixeur, lecture et écriture WAV et MIDI. Aucune dépendance à l'interface.
- `apps/opengarage` : l'application (egui pour l'interface, cpal pour le son, midir pour
  le MIDI, les hôtes CLAP, LV2 et VST3, l'export compressé, le client Songsterr, les
  traductions dans `lang`).
- `apps/og-video` : le lecteur vidéo, `og-video.exe`, un processus à part qui montre le
  lecteur de YouTube dans une vue web (wry, WebView2) dans la fenêtre du programme.
- `apps/midi-pilot` : `MidiPilot.exe`, un lanceur qui démarre le programme sur sa seule
  fenêtre MIDI Pilot (`--midi-pilot`), pour la scène.
- `tools/og-clap-test` : une petite bibliothèque qui est à la fois un fichier CLAP, un
  programme LV2 et un fichier VST3, avec les deux mêmes greffons de test dans chaque
  format. Les hôtes sont testés avec elle.
- `tools/screenshot.ps1` : ouvre l'application sur une vue donnée et enregistre une image
  de sa fenêtre, pour vérifier le rendu. `OPENGARAGE_LANG=de` choisit une langue pour
  un lancement, `OPENGARAGE_THEME=light` un thème.

Les règles du projet sont dans [Contribuer](contributing.md).
