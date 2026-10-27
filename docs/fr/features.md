# Fonctionnalités : cible et avancement

*[English version](../en/features.md)*

La liste suit les chapitres du guide d'utilisation public de GarageBand pour Mac
(table des matières consultée le 11 octobre 2026), reformulés avec nos mots.
Elle sert de feuille de route.

Légende : **fait** · **partiel** · **à faire**

**Décompte : 94 lignes « fait » sur 99, soit 94 %.** Les lignes « partiel » ne sont
pas comptées. Ce décompte mesure la présence des fonctions, pas leur profondeur :
voir « Écarts connus » en bas de page.

## Projets (7 sur 7)

| Fonction | État |
|---|---|
| Créer, ouvrir, enregistrer un projet | fait (format `.ogproj` : un seul fichier, une archive contenant le projet en JSON et, sous `media/`, l'audio de ses prises et les fichiers de ses effets venus avec lui, de sorte qu'un projet voyage en un fichier ; un fichier de l'utilisateur ailleurs sur le disque, comme l'enregistrement du morceau pris dans la bibliothèque musicale, reste où il est, nommé par son chemin ; les projets écrits avant, en JSON avec leur audio dans un dossier voisin, s'ouvrent toujours. Enregistrés dans le dossier `OpenGarageMusic` des documents de l'utilisateur sauf si Fichier → Options → « Dossier des projets… » dit ailleurs. À l'enregistrement, l'audio qu'aucune région ne joue plus, même comme autre prise, s'en va ; dans l'éditeur, le menu « Prises » d'une région audio oublie une prise avec son ✕) |
| Tempo | fait |
| Signature rythmique | fait (1 à 16 temps, sur 4 ou sur 8) |
| Tonalité et gamme du projet | fait (les boucles la suivent) |
| Point de fin du projet | fait (automatique ou fixé à une mesure) |
| Volume général | fait |
| Modèles de projet | fait (7 modèles) |

## Lecture et navigation (7 sur 7)

| Fonction | État |
|---|---|
| Lecture, arrêt, retour au début | fait |
| Déplacer la tête de lecture dans la règle | fait |
| Zone de lecture en boucle | fait (se trace dans le haut de la règle) |
| Suivi de la tête de lecture | fait (par pages) |
| Zoom et défilement de la zone de pistes | fait |
| Métronome | fait |
| Décompte avant enregistrement | fait (une mesure) |

## Pistes (10 sur 10)

| Fonction | État |
|---|---|
| Créer, renommer, supprimer, dupliquer une piste | fait |
| Muet, solo, volume, panoramique | fait |
| Réorganiser les pistes | fait (monter, descendre) |
| Icône et couleur de piste | fait (l'icône est celle de l'instrument que joue la piste, trouvé d'après son type et son son : piano, synthétiseur, basse, guitare, cordes, cuivres, bois, percussions à lames, batterie, voix, audio ou vidéo, dessinés par Game Icons ; un clic dessus choisit l'une des 8 couleurs) |
| Verrouiller une piste | fait |
| Piste d'instrument logiciel | fait |
| Piste audio (micro, ligne) | fait |
| Piste guitare / basse avec ampli | fait |
| Piste de batteur virtuel | fait |
| Piste principale avec effets | fait (volume automatisable, égaliseur, compresseur, limiteur) |

## Sons (6 sur 6)

| Fonction | État |
|---|---|
| Choisir un son par piste | fait (36 sons intégrés) |
| Bibliothèque de sons avec catégories et recherche | fait |
| Enregistrer ses propres réglages de son | fait (« Mes sons ») |
| Commandes simplifiées par son (potentiomètres) | fait (boîtes disposées en lignes, déplaçables et masquables) |
| Instruments échantillonnés (piano, batterie…) | fait : banques SoundFont (.sf2, .sf3) et instruments SFZ (.sfz), importés depuis le disque ou téléchargés depuis le menu Bibliothèque ; aucune banque n'est fournie |
| Arpégiateur | fait (notes écrites, enregistrées ou jouées en direct) |

## Enregistrement (2 sur 5)

| Fonction | État |
|---|---|
| Jouer au clavier de l'ordinateur | fait |
| Enregistrer ce qu'on joue dans une région | fait |
| Clavier MIDI externe | partiel (écrit, jamais essayé avec un appareil) |
| Enregistrement audio | partiel (prises multiples en boucle, écoute de l'entrée ; vérifié par tests sans carte son, jamais essayé avec un micro) |
| Accordeur | partiel (détection de hauteur vérifiée par test, jamais essayée avec un micro) |

## Régions et arrangement (9 sur 9)

| Fonction | État |
|---|---|
| Créer, sélectionner, déplacer, redimensionner, supprimer | fait (glissée avec Alt ou Ctrl enfoncé, une région ou une note est copiée) |
| Alignement sur la grille | fait |
| Renommer une région | fait |
| Couper, copier, coller | fait |
| Scinder, joindre | fait |
| Mettre une région en boucle | fait |
| Déplacer une région vers une autre piste | fait |
| Marqueurs d'arrangement (intro, couplet, refrain) | fait (déplacer, répéter ou supprimer une section avec son contenu) |
| Piste de tempo, piste de transposition | fait |

## Éditeurs (12 sur 12)

| Fonction | État |
|---|---|
| Éditeur de notes : ajouter, déplacer, allonger, supprimer | fait |
| Quantification | fait (de la ronde à la triple croche) |
| Transposition | fait (demi-ton ou octave) |
| Vélocité des notes | fait |
| Sélection multiple de notes | fait |
| Éditeur de partition | fait (lecture et édition simple ; pas d'impression. Portées de sol et de fa avec l'armure et le chiffrage ; les notes avec les altérations que demande la tonalité, leurs valeurs de la ronde à la quadruple croche, avec les points, les ligatures sur les croches et plus brèves d'un temps, un 3 sur les triolets, une hampe pour les notes frappées ensemble, des liaisons pour les notes qui passent une barre de mesure ou ne tiennent en aucune valeur, des crochets sur les triolets, et des silences là où rien ne sonne ; les noms des accords et des sections au-dessus de la portée, et ce qu'une tablature écrit autour de ses notes dessiné ici aussi) |
| Tablature | fait (pour une piste qui joue une guitare ou une basse : les notes en numéros de frette sur les cordes, en lecture seule ; un clic sélectionne une note. Les cordes sont l'accordage propre à la piste quand elle en a un, comme le donne un morceau importé de Songsterr avec la corde de chaque note, sinon l'accordage standard. Les noms d'accords courent en haut : ceux que porte la tablature, sinon ceux que forment les notes ; au-dessus de chacun, une grille d'accord montre où vont les doigts : ce que jouent les cordes à cet endroit quand cela montre l'accord sur quatre cases, sinon une façon courante de le jouer ; l'accord en cours reste au bord gauche de la vue tant qu'il dure. Un curseur règle la taille des numéros de frette, des noms d'accords et des grilles. Un temps est toujours assez large pour que les numéros de frette d'une corde ne se chevauchent pas : le zoom le plus faible montre autant de mesures qu'on peut en lire ainsi, dix au plus, et le plus fort quatre temps. Les noms d'accords écrits avec les signes de la musique imprimée (B♭7, F♯m, CΔ7, Cø…) sont compris et écrits avec des lettres (Sib7, Fa#m). Ce qu'une tablature écrit autour de ses notes est dessiné aussi : glissés, hammer-on et pull-off, bends avec leur hauteur, vibrato, harmoniques, notes étouffées et fantômes, staccato, accents, palm mute et let ring, les barres du trémolo et le « tr » des trilles ; sous les cordes, là où le swing commence et s'arrête ; les noms des sections du morceau courent au-dessus des accords) |
| Partition et tablature sur plusieurs lignes | fait (la case « Deux lignes », décochée au départ, les dispose sur deux lignes de mesures entières, comme sur une page : les deux suivantes viennent quand toutes deux sont jouées, ou avec la molette. Le plein écran dispose toute la région ainsi, sur autant de lignes qu'il faut, et défile jusqu'à la ligne jouée) |
| Plusieurs pistes dans l'éditeur | fait (Ctrl et un clic sur les en-têtes de pistes mettent plusieurs pistes dans l'éditeur, l'une sous l'autre, dans l'ordre choisi ; une note sélectionnée dans l'éditeur d'une autre piste fait passer cette piste devant. Une piste supprimée quitte l'éditeur sans être remplacée ; une piste déplacée y reste. Le bouton « Changement auto » montre une piste à la fois : celle qui joue, la première choisie ayant la priorité ; quand plus rien ne vient sur celle affichée dans les mesures à venir réglées dans Affichage → « Changer de piste après » (1, 2, 4 ou 8), la suivante qui joue prend sa place dès que sa dernière note est jouée) |
| Boucler des temps depuis la partition ou la tablature | fait (Maj et un clic sur un temps, puis sur un autre, choisissent les temps qui les séparent, en surbrillance ; Maj enfoncée, les temps qu'un clic choisirait s'éclairent sous la souris. « Boucler les temps », dans la barre de l'éditeur, place la zone de boucle du morceau sur eux et lance la lecture ; un second clic arrête la boucle. Dans le panneau ou en plein écran, pour chaque piste de l'éditeur ; Échap abandonne les temps choisis. Dans la tablature, Ctrl et un clic ajoutent désormais une note à la sélection) |
| Éditeur audio, correction de hauteur et de placement | fait (transposition sans changer la durée, justesse, recalage des attaques sur la grille) |
| Éditeur du batteur virtuel | fait |

## Boucles et fichiers (6 sur 7)

| Fonction | État |
|---|---|
| Navigateur de boucles avec filtres, recherche, favoris, écoute | fait (42 boucles) |
| Adaptation des boucles au tempo et à la tonalité | fait |
| Import de fichiers audio | fait (WAV, MP3, AAC/M4A ; depuis la fenêtre de Fichier → « Importer dans ce projet… », sur la piste audio choisie ou une nouvelle, à la tête de lecture, ou déposés depuis l'explorateur de fichiers sur les pistes, comme les fichiers MIDI, les banques de sons et les projets ; la même fenêtre cale un enregistrement du morceau sur la partition : voir « Au-delà du guide ») |
| Import de fichiers MIDI | fait (depuis la même fenêtre, ses pistes de notes en nouvelles pistes, ou déposés sur la fenêtre) |
| Nouveau projet depuis un fichier Guitar Pro | fait (fichiers GP3, GP4 et GP5, ceux de GP6 (.gpx) et ceux de GP7 et GP8 (.gp) : une piste par partie, avec l'accordage et la corde de chaque note, le tempo et ses changements, la signature rythmique, la tonalité, les sections, les reprises dépliées, les noms d'accords et ce que la tablature écrit autour de ses notes, comme les donne l'import Songsterr ; les sauts de la partition (D.C., D.S., coda, fine) suivis dans l'ordre de lecture, le swing du morceau ou de chaque mesure joué, le trémolo et les trilles à leur vitesse, les notes d'ornement à leur durée, avant le temps ou sur le temps ; l'instrument de chaque piste pris dans une banque General MIDI installée, ou un son intégré de sa famille. Depuis le menu Fichier, depuis la fenêtre des sites de tablatures, ou déposé depuis l'explorateur de fichiers) |
| Nouveau projet depuis une partition MusicXML | fait (fichiers `.musicxml` et `.xml`, par parties ou par mesures, et archives `.mxl`, le format qu'échangent la plupart des éditeurs de partitions : une piste par partie, avec les notes, liaisons, reprises et fins alternatives, le tempo, la signature rythmique, la tonalité, les sections, les noms d'accords, les nuances, et la corde, la case et les techniques d'une partie écrite en tablature, dont l'accordage est gardé ; quand une partie a une portée de notes et une portée de tablature, seule la tablature est lue. Depuis la même entrée de menu que Guitar Pro, ou déposé depuis l'explorateur de fichiers) |
| Import depuis d'autres sites de tablatures | fait (Fichier → « Nouveau depuis un site de tablatures… » est une seule fenêtre pour Songsterr, Guitariff et Tablature Hub : le site à chercher se choisit en haut, puis la même recherche, la même liste de morceaux et le même choix des pistes pour les trois. Sur Guitariff et Tablature Hub, le programme cherche sur le site, va lire en arrière-plan le fichier Guitar Pro du morceau choisi et liste ses parties, chacune avec une case à cocher ; « Créer le projet » fait le projet de celles qui sont cochées. Ultimate Guitar n'est qu'ouvert dans le navigateur : ses conditions interdisent les programmes. La fenêtre dit aussi en une phrase ce que chaque site offre, l'ouvre, et garde le bouton « Importer un fichier Guitar Pro… ». Voir [Fichiers externes](external-files.md)) |
| Sites de tablatures décrits par des fichiers | fait (chaque site de la fenêtre est décrit par un fichier JSON : comment y chercher, comment découper ses réponses en morceaux et en pistes, comment suivre ses pages jusqu'au fichier, et quel lecteur le lit, avec des expressions régulières et des chemins JSON, sans code ; les quatre sites livrés sont décrits ainsi, et l'utilisateur ajoute ou remplace des sites dans le dossier `sites` à côté du programme ; le programme demande une fois avant de lire un site, ne demande que des adresses `https://` et borne chaque nombre. Voir [Décrire un site de tablatures](sites.md)) |
| Boucle glissée du navigateur dans les pistes | fait (sur une piste qui peut la recevoir, ou sous les pistes, où une piste est créée pour elle ; elle se pose sur la mesure sous le pointeur) |
| Boucles de son enregistré | fait pour les fichiers de l'utilisateur (WAV, MP3, AAC, FLAC, Ogg), rangés dans un dossier du programme et étirés au tempo du projet ; un tempo dans le nom du fichier (« Funk 98 bpm ») est utilisé, sinon il est déduit de la durée. **Aucune boucle enregistrée n'est fournie** |
| Bande-son d'un film | à faire |

## Mixage et effets (5 sur 6)

| Fonction | État |
|---|---|
| Volume et panoramique par piste | fait |
| Indicateurs de niveau | fait (par piste et en sortie) |
| Égaliseur, compresseur, réverbération, écho | fait (l'égaliseur est une courbe à trois points à déplacer : de côté pour la fréquence, de haut en bas pour le gain ; la réverbération partagée est une salle, ou la plaque de Dattorro, ou une réponse impulsionnelle de salle, de plaque ou de ressort à vous, « IR de salle… » dans le panneau Master, par convolution) |
| Automatisation | fait (volume et panoramique par piste, volume général ; réverbération, écho, compresseur, les trois bandes de l'égaliseur, le gain de l'ampli et les pédales de chaque piste) |
| Amplis et pédales d'effets | fait (4 amplis, et une capture d'un vrai ampli ou d'une vraie pédale à la place de l'étage de saturation de l'ampli, « Capture… » dans le panneau Ampli : un fichier `.nam` de Neural Amp Modeler (Architectures 1 et 2) ou un `.json` de Proteus ou d'AIDA-X, exécuté à sa propre fréquence quelle que soit celle de la carte, le bouton de gain réglant son niveau, les boutons de tonalité derrière et le filtre haut-parleur sauf si la capture contient son baffle ; une réponse impulsionnelle de baffle à la place du filtre haut-parleur, « IR de baffle… » ; 11 pédales : overdrive, noise gate, wah (sa pédale est le premier bouton, que l'automation déplace), auto-wah, chorus, flanger, trémolo, phaser, écho, écho à bande, écho analogique ; les pédales se choisissent une par une dans une chaîne, sur n'importe quelle piste) |
| Captures et réponses impulsionnelles depuis les sites | fait (Bibliothèque → « Captures et réponses impulsionnelles depuis les sites… » : la fenêtre des sites les liste par sorte. TONE3000, captures et réponses impulsionnelles, est lu avec votre compte : le programme lui demande un code que vous entrez sur le site, connecté, et garde le jeton ; ses conditions pour les programmes gratuits demandent à chacun une clé que vous créez dans votre compte, et permettent vos favoris, les captures du moment et les dernières, téléchargées une par une. La ToneLibrary de GuitarML (captures Proteus) ne demande pas de compte. Les fichiers vont dans les dossiers `Captures` et `Impulsions` de la bibliothèque, avec leur provenance et leur licence) |
| Greffons tiers | partiel (effets et instruments aux formats CLAP, LV2 et VST3, importés, détectés dans les dossiers standard ou téléchargés depuis le menu Bibliothèque ; curseurs, fenêtre propre et état enregistré — voir « Formats externes » ; exécuté uniquement avec les greffons de test du projet, jamais avec un greffon tiers réel) |

## Partage (3 sur 3)

| Fonction | État |
|---|---|
| Export WAV et AIFF | fait (16 ou 24 bits, 44,1 kHz ; Fichier → « Exporter vers… » ouvre une fenêtre qui liste les formats, chacun avec quelques mots sur son usage ; tout le morceau est exporté, de son début à son repère de fin) |
| Export compressé (MP3, AAC) | fait sous Windows (encodeurs du système ; 128, 192, 256 ou 320 kb/s, ce dernier pour le MP3 seulement, choisis dans la même fenêtre quand le MP3 ou l'AAC l'est) |
| Export MIDI | fait (dans la même fenêtre : un Standard MIDI File (`.mid`) au format 1 qui contient les notes des pistes telles qu'elles sont jouées, swing, trémolo et trilles compris, et aucun son ; une piste pour chaque piste qui a des notes, le tempo et ses changements, la signature rythmique, le programme General MIDI d'une piste qui joue un son échantillonné, les pistes de batterie et de batteur sur le canal 10) |

## Au-delà du guide

| Fonction | État |
|---|---|
| Interface en plusieurs langues | fait (anglais par défaut, français, allemand, espagnol, italien, portugais ; une langue = un fichier gettext `.po`) |
| Téléchargement de sons et de greffons sous licence libre | fait (fenêtre de téléchargement, avec repérage de ce qui est déjà téléchargé) |
| Choix de la sortie et de l'entrée audio ; une sortie ou une entrée par piste | fait (les pistes envoyées vers une autre sortie ne passent pas par les effets de la sortie principale ; deux cartes son ne tournent jamais exactement à la même vitesse, une seconde sortie peut donc perdre ou répéter quelques millisecondes de temps à autre ; une seule entrée est ouverte à la fois) |
| Le programme se rouvre tel qu'on l'a laissé | fait (la dernière session : projet, enregistré ou non, et disposition de la fenêtre ; désactivable, auquel cas le programme démarre sur le projet de démonstration ; quitter ne demande rien, sauf si la session ne peut pas être écrite et que le projet a des modifications) |
| Sessions précédentes | fait (les derniers projets sur lesquels on a travaillé, 20 par défaut, de 5 à 100, gardés sans enregistrer : un dossier chacun, avec le projet, enregistré ou non, ses prises et la disposition de la fenêtre, écrit en quittant, avant qu'un autre projet remplace celui qui est ouvert, et toutes les deux minutes tant qu'il change ; un projet ouvert ou importé est gardé aussitôt, modifié ou non, et seul un projet vierge laissé intact (la démonstration au démarrage, un nouveau projet depuis un modèle) ne l'est pas. Fichier → « Sessions précédentes… » les liste, la plus récente en premier, par artiste et titre, pistes, durée, moment et fichier, en rouvre une (après avoir gardé celle qui est ouverte) ou en supprime une, règle combien en sont gardées, montre la place qu'elles prennent, ouvre leur dossier et les déplace dans un autre ; par défaut `%USERPROFILE%\OpenGarageMusic\Sessions`. L'artiste et le titre d'un projet viennent des sites de tablatures et des fichiers importés, et se modifient en haut du Bloc-notes) |
| Projets récents | fait (menu Fichier et fenêtre de démarrage ; les dix derniers) |
| Position en minutes et secondes | fait (à côté des mesures et des temps) |
| Canal de la carte son enregistré | fait (menu Audio pour le programme, menu de la piste pour une piste ; tous les canaux mélangés par défaut) |
| Menu Audio | fait (sortie de toutes les pistes, entrée, canal) |
| Q quantifie les notes de l'éditeur | fait |
| Zoom des éditeurs de notes | fait (en bas à droite du piano roll, de la partition et de la tablature : « - », le zoom en pourcentage (100 % font 40 pixels par temps), et « + », chaque pression un quart plus loin ou plus près, aussi loin qu'on veut dans les deux sens ; un clic sur la valeur remet le zoom à quatre mesures en largeur (quatre temps dans la tablature), ce par quoi un éditeur commence. Ctrl et la molette zooment aussi, autour du pointeur. Le zoom est le nombre de pixels par temps, le même sur toutes les pistes et tous les morceaux, et gardé dans les réglages. Il est en largeur seulement : la hauteur des notes et la taille du texte ne le suivent pas, la tablature ayant sa propre « Taille du texte » ; très dézoomés, les numéros de case d'une tablature peuvent se chevaucher. Le premier temps d'une tablature est un peu en retrait du bord gauche, pour que son numéro de case soit entier) |
| Batteur qui suit une autre piste | fait (sa grosse caisse joue là où commencent les notes de la piste choisie, ramenées à la double croche la plus proche ; le reste de la batterie garde le motif du style) |
| Aide rapide | fait (le bouton « ? » : la barre d'état dit ce que le pointeur survole, par zone de la fenêtre, et les bulles des boutons s'affichent tout de suite) |
| Thèmes | fait (l'aspect propre au programme, un sombre et un clair ; un thème est un fichier JSON, on peut importer le sien — voir [Thèmes](themes.md)) |
| Nouveau projet depuis une tablature Songsterr | fait (dans la fenêtre de Fichier → « Nouveau depuis un site de tablatures… », Songsterr choisi : recherche sur songsterr.com, choix des pistes, une piste de notes chacune ; tempo et ses changements, signature rythmique, tonalité, sections, reprises dépliées ; l'instrument de chaque piste pris dans une banque General MIDI installée, ou un son intégré de sa famille ; l'accordage et la corde de chaque note ; glissés, bends, hammer-on et pull-off, vibrato, harmoniques, notes étouffées et fantômes, staccato, accents, palm mute, let ring et barre de vibrato, affichés sur la tablature et joués : bends, glissés et vibrato font bouger la hauteur de la note pendant qu'elle sonne, les notes liées au marteau sont plus douces, les notes en palm mute et staccato plus courtes, let ring plus longues ; aucun compte n'est utilisé, seulement ce que le site montre à tout le monde) |
| Jouer avec le son d'origine (l'enregistrement mène, le tempo vient de lui, les pistes jouent par-dessus) | fait (un enregistrement du morceau est une piste : une piste audio qui contient un fichier audio de l'utilisateur, ou une piste vidéo qui contient une vidéo YouTube, chacune avec son calage, la seconde où tombe chaque temps de la partition. Le bouton « Son d'origine » de la barre de transport, affiché quand le projet a un enregistrement, passe à tout moment d'une façon de jouer à l'autre : désactivé, le morceau joue tel qu'écrit, au tempo de la partition, les enregistrements muets ; activé, il joue au rythme de l'enregistrement qui mène, le tempo pris dans son calage mesure par mesure, de sorte qu'un groupe qui accélère ou qui traîne est suivi, l'enregistrement s'entend, et les autres pistes se taisent sauf si leur M est désactivé : dans ce mode, M veut dire « muette pendant qu'on joue avec l'enregistrement », et le désactiver fait jouer cette piste par-dessus l'enregistrement. Pas de décompte dans ce mode ; les éditeurs défilent avec le morceau comme d'habitude. Avec plusieurs enregistrements, l'en-tête de chacun dit lequel mène, et un clic sur un autre le fait mener) |
| Un enregistrement calé sur la partition (fichier audio) | fait (Fichier → « Importer dans ce projet… » → « Un enregistrement du morceau… », pour un fichier MP3, OGG, FLAC, M4A/AAC ou WAV. En arrière-plan, le programme compare vingt fois par seconde quelles notes de la gamme sonnent dans la partition, d'après ses notes, et dans l'enregistrement, d'après son spectre sur environ un tiers de seconde, et où les notes sont attaquées, la batterie mise de côté ; un alignement temporel dynamique (« dynamic time warping ») apparie les deux, de sorte que la partition peut commencer n'importe où dans l'enregistrement (une intro, un silence) et que son rythme peut changer. Quatre minutes de morceau prennent environ deux secondes. Le programme dit à quel point les deux se ressemblent, en %, et prévient sous 30 % que ce n'est peut-être pas le même morceau ou la même version. L'enregistrement va sur sa propre piste audio, qui mène, et le morceau passe aussitôt au son d'origine ; l'éditeur de la piste le décale par rapport à la partition d'un dixième de seconde (« ◀ 0,1 s », « 0,1 s ▶ »). Le fichier reste où il est : le projet y renvoie par son chemin, il n'est pas copié, et rien n'est envoyé nulle part) |
| Bibliothèque musicale | fait (Fichier → Options → « Dossier de la bibliothèque musicale… » : le programme lit ce dossier et ses sous-dossiers en arrière-plan, fichiers MP3, OGG, FLAC, M4A, AAC, MP4 et WAV, jusqu'à 100 000, chacun connu par l'artiste et le titre de ses étiquettes (ID3, commentaires Vorbis, MP4) ou sinon par son nom (« Artiste - Titre.mp3 », un numéro de piste au début étant sauté). L'index est gardé à côté des réglages (`library.json`), et seuls les fichiers qui ont changé sont relus. Les noms sont comparés simplement : la casse, les accents, ce qui est entre parenthèses ou crochets (« (Remastered 2011) », « [Live] »), « feat. » et ce qui suit, un « The » au début ne comptent pas. Quand un morceau est choisi dans la fenêtre des sites de tablatures, son enregistrement, si la bibliothèque l'a, est proposé dans la section « Son d'origine » de la colonne des pistes (« Votre enregistrement : … », coché ; ✕ le refuse) ; sinon « Un enregistrement du morceau… » choisit un fichier. Une fois le projet créé, l'enregistrement est calé sur la partition comme ci-dessus) |
| Vidéo YouTube calée sur la tablature (Songsterr) | fait (dans la même section, pour un site qui donne des vidéos, la case « Vidéo YouTube calée sur la tablature », décochée au départ. Cochée, le programme demande au site, comme le fait son lecteur public, les vidéos du morceau et la seconde où commence chaque mesure dans chacune, les points que posent les utilisateurs du site (152 points pour les 152 mesures jouées de Nothing Else Matters, reprises dépliées) ; la vidéo originale, ou sinon la première, devient une piste vidéo ; quand le site en donne plusieurs, les autres sont gardées dans la piste, enregistrées avec le projet, et la liste en haut de son éditeur (« Vidéo 1 : l'originale », « Vidéo 2 : … », avec le nom que le site donne à chacune) en fait jouer une autre à la place, au même endroit du morceau, chacune gardant son propre décalage ; et un message dit qu'elle peut dériver quand les nombres de mesures diffèrent de plus d'une. L'éditeur de la piste vidéo montre la vidéo, aussi grande que le panneau le permet en 16:9 ; avec Ctrl et un clic sur l'en-tête d'une piste de guitare, la vidéo et la tablature s'affichent l'une sous l'autre. Avec le son d'origine activé et la vidéo qui mène, le lecteur de YouTube lui-même la joue calée sur le morceau : le programme la lance et l'arrête avec le morceau, la suit plutôt que de l'envoyer ailleurs (elle s'arrêterait et se rechargerait) : quand les deux s'écartent de plus d'un huitième de seconde, le morceau va là où est la vidéo ; le morceau joue avec la vidéo dès que le projet est créé, ses autres pistes muettes, et un clic sur Lecture dans le lecteur de YouTube lance le morceau avec elle (et Stop arrête les deux, même quand la vidéo a été lancée ainsi) ; et le morceau attend pendant que la vidéo se charge ou montre une publicité, puis repart avec elle. Quand l'éditeur ne montre pas la vidéo, le bouton « Vidéo », à gauche de « Plein écran », l'affiche dans un coin de la fenêtre (il est désactivé au départ ; YouTube demande que son lecteur soit visible pendant qu'il joue). L'éditeur de la piste vidéo a les boutons de décalage et « Retard du lecteur vidéo » (en ms, un réglage de l'ordinateur, pour toutes les vidéos), et dit, avec son code d'erreur, quand YouTube ne lit pas une vidéo (retirée, bloquée dans le pays ou hors de YouTube). YouTube Music ne peut pas servir : il n'a pas de lecteur qui puisse être intégré. Aucune vidéo ni rien de son son n'est gardé : rien n'est téléchargé. Le lecteur est `og-video.exe`, à côté du programme, un processus à part qui montre le lecteur IFrame de YouTube dans une vue web WebView2 dessinée dans la fenêtre du programme ; la vidéo se joue à la souris, et le clavier reste toujours au programme (rendu aussitôt quand la vue web le prend, comme le fait la page de YouTube quand elle se charge ou qu'on clique dedans), pour qu'un champ où l'on tape et la barre d'espace continuent de marcher ; voir [Fichiers externes](external-files.md)) |
| Paroles du morceau tirées de la tablature (Songsterr, Guitar Pro, MusicXML), chantées syllabe par syllabe | fait (les paroles de la partie chantée, chaque syllabe sur sa note : de Songsterr, lues même quand la piste de chant n'est pas cochée ; des fichiers Guitar Pro, versions 3 à 8, les paroles du fichier, cinq lignes commençant chacune à une mesure, sur la piste à laquelle elles appartiennent ; de MusicXML, les `<lyric>` des notes, premier couplet. Comme les écrit Guitar Pro, chaque syllabe va sur le temps suivant qui attaque une note (les silences, les notes d'ornement et les notes seulement liées n'en prennent pas), une espace ou un trait d'union termine une syllabe (« laugh-ter » en fait deux), un trait d'union ou un tiret bas seul prolonge la syllabe d'avant d'une note, deux espaces laissent une note sans paroles, et une mesure rejouée chante de nouveau ses syllabes ; les syllabes sont ajustées aux notes sur tout le morceau à la fois, chaque ligne après un silence du chant, pour qu'une erreur du texte écrit à la main ne décale pas toutes les syllabes qui la suivent ; les voyelles tenues entre parenthèses se lisent sans elles. Une ligne commence à un mot écrit avec une majuscule (pas « I », « I'm », « Lord », « God »…), après une pause d'une mesure et demie, ou quand elle dépasserait environ 48 caractères. Les paroles chantées suivent la partition, telle qu'écrite ou avec le son d'origine. Dans la fenêtre des sites de tablatures, la section « Son d'origine » a la case « Les paroles du morceau », cochée au départ) |
| Paroles de LRCLIB, calées | fait ([LRCLIB](https://lrclib.net), une base libre de paroles calées ligne par ligne, sans compte : interrogée avec l'artiste et le titre du projet, et la durée de l'enregistrement quand il y en a un pour prendre la bonne version ; interrogée d'elle-même quand une tablature ou un fichier Guitar Pro ou MusicXML sans paroles devient un projet, et avec « Chercher sur LRCLIB » dans le bandeau. Les lignes suivent les secondes de l'enregistrement qui mène, ou du premier enregistrement, ou le tempo de la partition quand il n'y en a pas ; « ◀ 0,1 s » et « 0,1 s ▶ » les décalent. Seuls l'artiste et le titre sont envoyés : voir [Fichiers externes](external-files.md)) |
| Paroles de l'utilisateur (fichier à côté de l'enregistrement, fichier choisi, tapées) | fait (un fichier LRC ou texte du même nom à côté de l'enregistrement du morceau, `Morceau.lrc` ou `Morceau.txt` à côté de `Morceau.mp3`, lu quand LRCLIB n'a rien ou quand un enregistrement est lié à un morceau sans paroles ; « Depuis un fichier… » ; « Taper… », pour des paroles tapées ou collées. Les lignes qui commencent par un temps comme `[01:23.45]` sont calées ; le `[offset:+250]` d'un fichier LRC est compris. Les paroles sont gardées dans le projet ; comme les tablatures, elles sont le plus souvent protégées par le droit d'auteur et sont là pour sa propre pratique) |
| Bandeau des paroles, deux façons | fait (le bandeau se grandit ou se réduit en faisant glisser la ligne au-dessus de lui, sous les pistes comme en plein écran ; le bouton « Paroles » de la barre du panneau du bas, à gauche de « Vidéo » et de « Plein écran », affiche ou masque un bandeau sous l'éditeur, au-dessus de la barre d'état : affiché au départ, seulement avec l'onglet Éditeur, parti avec le panneau du bas, sa hauteur ajoutée au panneau au lieu d'être prise à l'éditeur. Présentation → « Paroles du morceau » : « Affichées sous le panneau », et « Les lignes en train d'être chantées » (la ligne chantée, en grand, les syllabes déjà chantées allumées, la ligne d'avant, pâle, et celle d'après) ou « Tout le texte, qui défile » (toutes les lignes, celle qui est chantée allumée, le texte défilant jusqu'à elle). Un clic sur une ligne y emmène le morceau, et la vidéo aussi quand il y en a une qui joue) |
| MIDI Pilot (les chansons d'un concert, envoyées à un switcher de pédales en MIDI) | fait (Fichier → « MIDI Pilot… » ouvre une fenêtre à part, qui peut aller sur un second écran, avec un gros bouton par chanson : son texte, ou « Artiste - Titre », sur la pochette de son album, ou une image de l'utilisateur, ou sinon une couleur par artiste ; la chanson choisie est entourée de vert, et une pastille jaune dit qu'une chanson a son projet. La liste se lit d'un fichier CSV de quatre colonnes, numéro, artiste, chanson et texte du bouton (facultatif), séparées par des virgules ou des points-virgules, une première ligne qui n'est pas une chanson étant un en-tête (le format de G3S Pilot), ou s'écrit chanson par chanson (Liste → « Ajouter une chanson… », un clic droit → « Modifier… »), et se réécrit en CSV ; elle est gardée dans les réglages de l'utilisateur. Choisir une chanson envoie le numéro de son preset à la sortie MIDI choisie, sur le canal choisi, en control change 0 (sélection de banque, comme The GigRig G3S change de banque) ou en program change, avec « Numéros à partir de 1 » quand la première chanson est le numéro 1 : c'est le mode du switcher. Dans le mode de la chaîne (Réglages → Mode), les appareils de la chaîne sont listés sous Réglages → « Appareils… », chacun avec un nom, un modèle (appareil quelconque, The GigRig G3S, BOSS DD-500, BOSS RV-500) et un canal MIDI, et chaque chanson porte ses propres messages, envoyés dans l'ordre quand elle est choisie, chacun sur le canal de son appareil et suivi d'un délai propre : un program change (après une sélection de banque, control changes 0 et 32, quand une banque est donnée), un control change, ou un message system exclusive écrit en hexadécimal ; « Modifier… » les édite et « Envoyer maintenant » les essaie. Une colonne du fichier CSV nommée comme un appareil donne à chaque chanson le programme que cet appareil reçoit, et l'export écrit ces colonnes ; les appareils qu'un fichier nomme rejoignent la chaîne, et un fichier avec des programmes passe au mode de la chaîne. La ligne d'état montre les octets envoyés en dernier au survol. Un appareil dont le modèle est la BOSS DD-500 ou RV-500 a un module (Appareils → « Configurer… »), qui parle à la pédale par messages system exclusive sur la sortie et l'entrée MIDI choisies (Réglages → « Entrée MIDI »), comme le décrivent les documents publics d'implémentation MIDI des fabricants, d'après un fichier de description de la pédale livré avec le programme (`devices/dd500.json`, `devices/rv500.json` ; un fichier d'un dossier `devices` à côté du programme remplace celui livré de même clé) : les mémoires listées (01A à 99C, leurs noms lus à la demande), le patch de la mémoire choisie lu et édité paramètre par paramètre, par groupe, chaque changement envoyé aussitôt au patch temporaire que la pédale joue (listes de valeurs, curseurs affichés comme la pédale les affiche, le nom), « Enregistrer… » l'écrivant dans la mémoire après que ce qu'elle contenait est gardé dans `Pedal backups` du dossier de la liste et que la mémoire est relue et comparée, « Init » ; la table des program changes (quelle mémoire chaque banque et programme appelle) et les réglages système, lus et écrits ; un fichier du programme (JSON : les patches par mémoire, la table, le système) enregistré et chargé, toutes les mémoires lues dedans, le fichier entier renvoyé à la pédale, et les sauvegardes des éditeurs BOSS (`.bel`) lues aussi ; les messages des chansons passent par le module tant qu'il est ouvert, car Windows ne laisse un port qu'à un programme à la fois. Un appareil dont le modèle est The GigRig G3S a aussi un module (Appareils → « Configurer… »), qui édite un fichier de sauvegarde du switcher (`.g3b`, fait par l'éditeur du switcher et restauré par lui ; rien ne va au switcher depuis le programme) : ses banques (ajoutées, retirées, déplacées, nommées sur deux lignes de dix, le mode chanson), les 28 presets de chacune (14 sur les footswitches, 14 cachés), et pour le preset choisi son nom, les boucles actives, l'ordre des douze boucles dans le signal, et ses messages MIDI (jusqu'à vingt : program change ou control change sur un canal, à l'appui, au relâchement ou au maintien du switch, parmi les sortes que le switcher connaît) ; « Banques d'après les chansons » nomme une banque par chanson de la liste après la première banque, en ajoutant des banques au besoin, « Chansons d'après les banques » fait l'inverse ; « Pedalboard » décrit le board lui-même, gardé dans les réglages puisque le switcher n'en sait rien : les pédales de chaque boucle (nom, genre, et l'appareil de la chaîne quand la pédale est MIDI), l'ordre où le signal traverse les boucles, les footswitches qui changent de banque, et les boucles sont alors nommées ainsi dans les presets ; « Vérifier » contrôle le fichier comme le switcher le fait (checksums, séquence, somme, banques déclarées), « Enregistrer sous… » l'écrit. Le format du fichier n'est pas publié : voir [Fichiers externes](external-files.md). Une IA peut écrire tout cela : Liste → « Guide pour une IA… » écrit `ai-guide.md` (aussi dans l'archive, dans la documentation en `docs/en/ai-guide.md` pour la version du code, et `OpenGarageMusic.exe --ai-guide fichier.md` ; le guide dit pour quelle version du programme et du format il est), fait par le programme d'après les descriptions des pédales pour nommer chaque paramètre avec sa plage ou ses valeurs, et enseignant un format JSON d'actions (une mémoire d'une pédale réglée paramètre par paramètre avec les valeurs que la pédale affiche, depuis son patch actuel ou depuis init, avec un nom ; la table des program changes ; le système ; une banque du G3S nommée, un preset avec ses boucles, leur ordre et ses lignes MIDI, une banque retirée ; une chanson de la liste ajoutée, modifiée ou retirée, avec son numéro, le texte de son bouton et ses messages aux appareils de la chaîne) ; Liste → « Appliquer un script d'IA… » lit un tel fichier, montre chaque action en une phrase avec ce qui cloche (une pédale non décrite, aucun appareil de ce modèle dans la chaîne, un paramètre ou une valeur que la pédale n'a pas, une mémoire qui n'en est pas une), et « Appliquer » les fait dans l'ordre par le module de la pédale (la mémoire lue d'abord quand l'action part d'elle, écrite, gardée avant et relue après) et le module G3S (la sauvegarde modifiée, à enregistrer) ; « Exporter pour une IA… » sur un patch, sur le fichier d'un module ou sur une sauvegarde G3S, et Liste → « Exporter les chansons pour une IA… », écrivent le même format pour que l'IA le modifie ; Liste → « Exporter le pedalboard pour une IA… » écrit en un fichier tout ce qu'une IA doit savoir avant d'écrire : une section `pedalboard` (les appareils de la chaîne, les boucles du switcher, l'ordre du signal et les switches de banque, comment les chansons sont envoyées) et, en actions, les chansons, les boucles et la sauvegarde du switcher, et chaque mémoire et table des program changes connue des pédales ; ce qu'un module de pédale lit ou écrit est gardé dans `Pedals` des données du programme pour cela, les pédales n'ont pas à être branchées. Les scripts acceptent aussi `{ "g3s": "loops", … }`. Un agent d'IA qui tourne sur la même machine (Claude Code avec `curl`, un script) peut faire tout cela par une API locale : Réglages → « API pour une IA… » l'allume (un port sur `127.0.0.1` seulement, 7311 sauf changement, un jeton de 32 caractères que l'agent envoie en `Authorization: Bearer`, renouvelable), elle écoute tant que la fenêtre du MIDI Pilot est ouverte, et répond en JSON au format des scripts : `GET /` (ce qui tourne et les routes), `/guide`, `/devices`, `/pedals/{key}` (les paramètres), `/pedals/{key}/memories` (les noms), `GET|PUT /pedals/{key}/memories/{01A}` (une mémoire en action ; écrite d'après le corps `{ name, from, set }`), `POST /pedals/{key}/try` (des valeurs jouées sans être écrites), `GET|PUT /pedals/{key}/pc_map` et `/system`, `GET /g3s` (la sauvegarde chargée en actions), `POST /g3s/actions`, `POST /g3s/save` (le fichier écrit, à `path` ou là où il a été chargé), `GET /songs` (les chansons en actions), `GET /pedalboard` (le fichier pedalboard), `POST /actions` (un script entier, relu et appliqué). Une écriture dans une pédale attend le Oui de l'utilisateur en bas de la fenêtre (202 après deux minutes sans réponse, 403 sur Non), sauf si « Écrire les pédales sans me demander à chaque fois » est coché ; le module de la pédale s'ouvre sur l'appareil de ce modèle au besoin, et une requête est servie à la fois (409 pendant ce temps). Une IA dans une page web ne peut pas atteindre l'API : le guide et le fichier de script sont sa voie. La sortie n'est ouverte que tant que la fenêtre est ouverte, car Windows ne la laisse qu'à un programme à la fois. Avec « Ouvrir le projet de la chanson » coché, choisir une chanson ouvre aussi son projet dans la fenêtre principale, aussitôt : le projet ouvert jusque-là est gardé dans les sessions précédentes. Un clic droit sur une chanson lui donne une image (prioritaire sur celle trouvée, jusqu'à « Supprimer mon image »), un fichier de projet, le projet ouvert, ou trouve son projet : un projet gardé dans les sessions précédentes avec le même artiste et le même titre (écrits pareil aux majuscules et aux signes près), sinon sa tablature sur le site de Réglages → « Site cherché », cherchée aussitôt dans la fenêtre des sites de tablatures, où l'utilisateur choisit le morceau et ses pistes ; le projet créé est enregistré, une fois ses paroles et son enregistrement arrivés, dans le dossier de la liste (OpenGarageMusic\MIDI Pilot du dossier de l'utilisateur, ou un autre), pour s'ouvrir sur scène sans Internet. Liste → « Trouver les projets de toute la liste » fait cela pour chaque chanson sans projet, l'une après l'autre, et s'arrête quand la fenêtre des sites est fermée. Les pochettes des albums et les photos des artistes (Ordre → « Par artiste » montre d'abord les artistes) viennent de Deezer, peuvent être désactivées, et sont gardées sur le disque ; voir [Fichiers externes](external-files.md). Pas de raccourci clavier : sur scène, c'est le switcher de pédales qui change de chanson) |

## Hors périmètre

Cours de piano et de guitare, télécommande depuis un téléphone, gravure de CD,
partage vers des services en ligne.

## Formats externes

- **Instruments** : SoundFont 2 (`.sf2`), sa variante compressée SF3 (`.sf3`) et SFZ
  (`.sfz`). Aucune banque n'est fournie : le menu Bibliothèque en télécharge à la
  demande (voir [Fichiers externes](external-files.md)). Les banques qui s'appuient
  sur des modulateurs SoundFont (les pianos de MuseScore General, par exemple) sonnent
  mal avec le lecteur actuel.
- **SFZ** : les opcodes courants de SFZ 1 sont lus (échantillon, étendue de touches et
  de vélocités, accordage, volume, boucle, enveloppe, groupes, `#include`, `#define`) ;
  échantillons WAV, FLAC ou Ogg Vorbis. Les effets et filtres décrits dans un fichier
  SFZ sont ignorés : SFZ décrit des instruments, pas des effets autonomes.
- **Greffons CLAP** : standard ouvert sous licence MIT. Effets et instruments, curseurs,
  fenêtre du greffon (sous Windows) et sauvegarde de son état dans le projet.
- **Greffons VST3** : interfaces publiées par Steinberg, sous licence MIT depuis la
  version 3.8 du SDK. Un greffon est un fichier `nom.vst3`, seul ou dans un dossier du
  même nom. Effets et instruments ; curseurs de 0 à 1 (le format ne donne que des
  valeurs normalisées) ; fenêtre du greffon (sous Windows) ; état des deux moitiés du
  greffon enregistré dans le projet ; messages entre ces deux moitiés ; calcul en
  32 bits, ou en 64 bits pour les greffons qui ne font que cela ; toutes les sorties
  mélangées dans la piste ; ce que le greffon signale pendant le calcul du son (un
  niveau, un paramètre qu'il a déplacé) transmis à sa fenêtre. Ne sont pas pris en
  charge : les entrées latérales (« side-chain », elles restent silencieuses), les
  menus contextuels et les unités ou programmes du greffon.
- **Greffons LV2** : standard ouvert sous licence ISC. Un greffon LV2 est un dossier
  `nom.lv2` ; l'application lit elle-même ses fichiers de description. Effets et
  instruments, réglés par curseurs par défaut. Par clic droit sur la boîte du greffon,
  sa propre fenêtre s'ouvre quand il en décrit une pour Windows (`ui:WindowsUI`), avec
  les fonctions d'hôte « parent », « resize », « idle », accès à l'instance et accès
  aux données. Ce que le greffon garde en dehors de ses ports (« state » de LV2) est
  enregistré dans le projet, pour les valeurs qui sont de simples données. Ne sont pas
  pris en charge : les autres types de fenêtres (X11, Gtk, Qt : ils appartiennent à
  Linux et macOS et n'existent pas sous Windows), l'état qui renvoie à des fichiers,
  et les greffons qui exigent des fonctions d'hôte autres que la table
  d'identifiants, les options et la taille de bloc bornée.
- **Isolation** : chaque greffon, quel que soit son format, tourne dans un second
  processus lancé par l'application, qui lui envoie le son et les notes et reçoit le
  son traité par une connexion propre à l'ordinateur (un socket sur 127.0.0.1), de
  sorte que ce que le greffon imprime n'entre jamais dans la conversation. Un greffon qui plante ou ne répond plus pendant deux secondes est
  abandonné : l'effet est contourné, l'instrument se tait, le projet reste ouvert et
  le greffon peut être relancé. La fenêtre du greffon appartient alors à ce processus.
  Désactivable dans Fichier → Options, pour un démarrage plus court.
- **Applications JACK** : ce ne sont pas des greffons mais des programmes autonomes
  reliés entre eux par un serveur audio (JACK). Elles ne peuvent pas être chargées dans
  l'application ; les relier demanderait de faire d'OpenGarageMusic un client JACK.
- **Audio Units** (le format de GarageBand) : propre à macOS et iOS, impossible ici.

## Écarts connus

- **Les sons intégrés** sont synthétisés ; les instruments réalistes viennent de
  banques que l'utilisateur télécharge ou importe.
- **Téléchargements** : sur Musical Artifacts, la licence est celle que déclare la
  personne qui a déposé le fichier ; l'application ne peut pas la vérifier.
- **La bibliothèque** compte 36 sons et 42 boucles, contre des milliers dans le logiciel
  de référence.
- **Correction audio** : méthodes simples dans le domaine temporel ; de gros
  changements de hauteur ou de placement s'entendent.
- **Greffons** : les trois hôtes n'ont été exécutés qu'avec les greffons de test du
  projet : un vrai greffon peut révéler des cas non prévus. Lancer chaque greffon dans
  son propre processus protège des plantages, pas d'un greffon malveillant, et coûte
  un aller-retour entre deux processus pour chaque bloc de son.
- **Traductions** : l'allemand, l'espagnol, l'italien et le portugais n'ont pas été
  relus par des locuteurs natifs ; quelques messages d'erreur du moteur audio restent
  en anglais.
- **Le fil audio** partage un verrou avec l'interface ; sur un gros projet, une
  modification pendant la lecture peut produire un craquement.
- **Sources** : la table des matières du guide public, et ce qui est dit dans trois
  tutoriels vidéo publics (leur commentaire parlé a été lu sous forme de texte ;
  l'image n'a pas été regardée). Les détails d'interaction seulement montrés, et non
  dits, ne sont pas vérifiés.
- **Boucles de son enregistré** : le programme n'en fournit aucune (rien n'est livré
  qui ne soit le travail du projet) ; l'étirement au tempo est une méthode simple dans
  le domaine temporel, et la tonalité d'une boucle audio n'est pas modifiée.
- **Batteur qui suit** : seule la grosse caisse suit, et seules les pistes de notes
  peuvent être suivies (pas le son enregistré).
- **Aide rapide** : elle décrit des zones de la fenêtre, pas chaque commande ; les
  commandes ont leurs propres bulles.
- **Thèmes** : ils règlent les couleurs, pas les tailles, les polices ni les formes.
- **Songsterr** : les tablatures sont écrites par les utilisateurs du site et le
  programme ne peut pas vérifier leurs droits ; le format n'est pas documenté et a été
  déduit des fichiers que charge le lecteur du site, un changement du site peut donc
  casser l'import. Seules les tablatures publiques sont lues : le programme ne se
  connecte pas au site.
- **Fichiers Guitar Pro** : les formats ne sont pas publiés ; le lecteur suit leurs
  descriptions publiques et a été essayé sur des fichiers écrits par ses propres tests,
  pas sur beaucoup de fichiers des programmes eux-mêmes. Les dessins des accords, les
  réglages de son des pistes et la mise en page sont passés ; les paroles sont lues. Les fichiers
  de Guitar Pro 1, 2 et 6 sont reconnus et refusés en le disant : leur structure n'est
  décrite nulle part publiquement.
- **Paroles du morceau** : les lignes des paroles d'une tablature sont devinées
  d'après les majuscules, les pauses et la longueur, et peuvent ne pas tomber là où le
  morceau les coupe ; seul le premier couplet d'une partition MusicXML est lu ; les
  paroles de LRCLIB sont calées par ligne, pas par syllabe, et ne sont pas plus
  justes que le calage des personnes qui les ont déposées.
- **La partition** écrit une voix par portée, et choisit dièses ou bémols d'après la
  seule tonalité ; seuls les triolets ont un crochet, les autres n-olets sont écrits
  à la valeur la plus proche.
