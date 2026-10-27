# Contribuer — règles pour les humains et les IA

*[English version](../en/contributing.md)*

OpenGarageMusic est un studio de musique écrit de zéro en Rust, publié sous la licence
PolyForm Noncommercial 1.0.0 (gratuit pour un usage personnel, une licence de l'auteur
pour un usage professionnel). Il s'inspire des
fonctionnalités de GarageBand d'Apple sans rien en reprendre. Ces règles ne souffrent
aucune exception.

## 1. Salle blanche

Sources autorisées pour savoir ce que le logiciel doit faire :

- la documentation publique d'Apple (guide d'utilisation, pages d'assistance) ;
- des vidéos et articles publics montrant le logiciel en fonctionnement ;
- l'observation du logiciel en l'utilisant normalement, avec une copie obtenue légalement ;
- la littérature publique sur le traitement du signal et les spécifications publiques
  des formats (WAV, MIDI, SoundFont, SFZ, CLAP, LV2, VST 3, gettext…).

Interdit :

- lire, désassembler, décompiler ou copier quoi que ce soit dans les applications Apple ;
- copier le texte de la documentation d'Apple : on décrit les comportements avec nos mots ;
- copier du code sous licence GPL, LGPL ou AGPL (Ardour, LMMS, etc.). Lire ces projets
  pour comprendre une idée est permis, recopier ou traduire leur code ne l'est pas.

## 2. Contenus

- Aucun son, boucle, instrument échantillonné, préréglage, icône, image ou capture d'écran
  provenant d'Apple, même retouché ou redessiné d'après l'original.
- Les noms de sons et de préréglages sont les nôtres, pas ceux d'Apple.
- Tout contenu ajouté (son, police, icône) est soit créé pour le projet, soit sous licence
  libre. Son auteur, sa licence, l'adresse d'où il a été téléchargé et la date sont notés
  dans [Fichiers externes](external-files.md) (dans les deux langues) avant qu'il n'entre
  dans le dépôt.
- Aucun son ni aucun greffon n'est fourni : l'utilisateur les télécharge, à sa demande,
  depuis des sites qui les distribuent sous licence libre.
- Une exception, dont l'usage relève de l'utilisateur : les tablatures de Songsterr, lues
  à sa demande dans son propre projet. Elles ne sont pas libres, ne sont jamais ajoutées
  au dépôt (pas même comme données de test), et sont décrites dans
  [Fichiers externes](external-files.md).
- Les captures de référence servent à comprendre un comportement ; elles ne sont jamais
  ajoutées au dépôt.
- « GarageBand » et « Apple » ne servent qu'à décrire la compatibilité ou l'inspiration.
  Le projet n'est ni affilié à Apple ni approuvé par Apple.

## 3. Code

- Le moteur (`crates/og-engine`) ne dépend jamais de l'interface (`apps/opengarage`).
- Dépendances sous licences permissives (MIT, Apache-2.0, BSD, ISC, Zlib), ou MPL-2.0
  utilisée sans modification (les décodeurs Symphonia). Pas de LGPL ni de GPL liée au
  programme : c'est pourquoi l'export MP3 et AAC passe par les encodeurs du système.
- Le code `unsafe` est réservé aux frontières avec l'extérieur (encodeurs de Windows,
  hôtes de greffons) ; chaque bloc dit pourquoi il est correct.
- Les greffons tiers téléchargés pour une vérification sont inspectés, pas exécutés :
  les hôtes sont testés avec les greffons de test du projet.
- Un fichier de projet invalide, une sortie audio absente ou une saisie hors limites
  produit un message d'erreur, jamais un plantage.
- Chaque fonctionnalité du moteur a un test ; les interactions de l'interface sont
  testées sans écran (`apps/opengarage/src/tests.rs`).
- Avant de valider : `dev test --workspace`.

## 4. Textes

- Les textes du code sont en anglais, entourés de `tr("…")`, ou de `tf!("… {}", valeur)`
  quand des valeurs y sont insérées. Les noms qui viennent du moteur (sons, boucles,
  modèles) sont eux aussi en anglais et passent par `tr` là où ils sont affichés.
- Chacune des langues intégrées (`apps/opengarage/lang/*.po`) doit traduire tous les
  textes : un test le vérifie, ainsi que la conservation des emplacements de valeurs.
  Voir [Traduire](translating.md).
- La documentation existe en anglais (`docs/en`) et en français (`docs/fr`) ; une
  modification de l'une est reportée dans l'autre.
- Les fichiers sources sont en UTF-8. Sous Windows, ne pas les réécrire avec des
  commandes de PowerShell 5 qui lisent ou écrivent dans la page de codes du système.

## 5. Licence et contributions

- Le programme est publié sous la licence PolyForm Noncommercial 1.0.0 (`LICENSE.md`) :
  gratuit pour un usage non commercial, une licence de l'auteur pour tout usage
  commercial (`COMMERCIAL.md`).
- En proposant une contribution (demande de fusion, correctif, traduction, description
  d'un site ou d'une pédale), le contributeur donne à l'auteur d'OpenGarageMusic le droit
  de l'utiliser, la modifier, la publier et la licencier à toute condition, licences
  commerciales comprises, et confirme qu'elle est son œuvre ou sous une licence qui le
  permet. Son nom reste dans l'historique du dépôt.
- Les contributions sous GPL, LGPL ou AGPL, ou copiées d'un tel code, ne peuvent pas
  être prises.
