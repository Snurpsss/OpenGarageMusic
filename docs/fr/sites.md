# Décrire un site de tablatures

*[English version](../en/sites.md)*

La fenêtre de Fichier → « Nouveau depuis un site de tablatures… » liste les sites que
connaît le programme. Chacun est décrit par un petit fichier JSON : comment chercher sur
le site, comment découper sa réponse en morceaux (et en pistes, quand le site les
liste), comment suivre ses pages jusqu'au fichier d'un morceau, et lequel des lecteurs
du programme lit ce fichier. Une description contient des règles, jamais de code.

Les descriptions de Songsterr, Guitariff, Tablature Hub et Ultimate Guitar sont
intégrées au programme, et des copies sont dans le dossier `sites` à côté
d'`OpenGarageMusic.exe`. Pour ajouter un site, déposez sa description dans ce dossier ;
pour modifier l'un de ceux livrés, déposez-y une description du même `name` : elle
prend la place de celle intégrée. Le dossier est lu au démarrage du programme. Un
fichier illisible est signalé dans la fenêtre, avec ce qui ne va pas.

**Avant d'utiliser un site**, le programme vous demande une fois s'il doit le lire.
C'est à vous de vérifier qu'un site peut être lu par un programme, dans ses conditions
d'utilisation et son `robots.txt` : le programme le lit à votre demande, pour votre
usage. Ce qu'il importe va dans votre projet seulement.

## Une description

```json
{
  "format_version": 2,
  "name": "Exemple",
  "home": "https://www.exemple.org",
  "offers": { "en": "Guitar Pro files of its users.", "fr": "Les fichiers Guitar Pro de ses utilisateurs." },
  "encoding": "utf-8",
  "headers": { "Referer": "https://www.exemple.org/" },
  "search": [
    { "steps": [ {
      "get": "{home}/recherche?q={query}",
      "within": { "after": "Résultats", "before": "</ul>" },
      "items": { "regex": "(?s)<li>.*?</li>" },
      "fields": {
        "url":    { "regex": "href=\"([^\"]+)\"", "absolute": true },
        "artist": { "regex": "(?s)<b>(.*?)</b>", "text": true },
        "title":  { "regex": "(?s)<i>(.*?)</i>", "text": true },
        "format": { "regex": "\\.(gp[345x]?)\"", "uppercase": true, "default": "GP" }
      },
      "require": ["url", "title"],
      "where": { "format": "^GP" }
    } ] }
  ],
  "open": [
    { "get": "{url}", "fields": { "file": { "regex": "href=\"([^\"]+\\.zip)\"", "absolute": true } } },
    { "download": "{file}" }
  ],
  "videos": {
    "get": "{home}/videos?morceau={url}",
    "items": { "json": "videos" },
    "fields": { "video": { "json": "youtube" }, "points": { "json": "mesures" }, "label": { "json": "sorte" } },
    "require": ["video", "points"]
  },
  "reader": "guitar-pro",
  "display": { "label": "{artist} – {title}", "badge": "{format}" }
}
```

| Clé | Ce qu'elle dit |
|---|---|
| `format_version` | La version du langage des descriptions dans laquelle le fichier est écrit : voir [Versions](#versions). `1` quand elle manque. |
| `name` | Le nom affiché dans la fenêtre ; il identifie aussi le site. |
| `home` | L'adresse du site, en `https://` seulement. Toute adresse demandée doit être en `https://`. |
| `offers` | Ce qu'offre le site, en une phrase : un texte, ou un par code de langue (`en`, `fr`, `de`, `es`, `it`, `pt`). |
| `open_only` | `true` : le site est seulement ouvert dans le navigateur, jamais lu (`search` et `open` sont alors omis). |
| `encoding` | `utf-8` (par défaut) ou `latin1`, pour les pages des sites plus anciens. |
| `headers` | Des en-têtes envoyés avec chaque requête, quand un site les attend. |
| `search` | Comment chercher : des chaînes d'étapes, dont les morceaux sont réunis, chacun une fois. |
| `open` | Comment aller d'un morceau trouvé à son fichier. |
| `videos` | Depuis la version 2, et facultative : où se trouvent les vidéos YouTube d'un morceau, calées sur ses mesures. Voir [Vidéos](#vidéos). |
| `reader` | `guitar-pro` (par défaut : fichiers Guitar Pro 3 à 8 et MusicXML, aussi dans une archive ZIP) ou `songsterr` (les parties du lecteur de Songsterr). |
| `display` | Comment un morceau est listé : `label` et `badge`, des modèles (un texte, ou un par langue). |

## Chercher

Une chaîne est une liste d'étapes. Chaque étape demande une page (`get`, un modèle), la
découpe en éléments, et lit des champs dans chacun. La première étape se sert de la
recherche ; chaque étape suivante est faite une fois pour chaque élément gardé par la
précédente (douze pages au plus), avec les champs de cet élément comme variables. Les
éléments de la dernière étape sont les morceaux.

| Clé d'une étape | Ce qu'elle fait |
|---|---|
| `get` | L'adresse de la page, un modèle. |
| `within` | Ne regarde que la partie de la page après le texte `after` et avant le texte `before`. |
| `items` | `{"regex": "…"}` : chaque correspondance est un élément. `{"json": "a.b"}` : chaque élément de la liste à ce chemin (quand le chemin manque et que la page est une liste, la page elle-même). Sans `items`, la page entière est un élément. |
| `fields` | Les champs lus dans chaque élément (voir plus bas). |
| `require` | Les champs qu'un élément doit avoir, sinon il est écarté. |
| `where` | Des champs qui doivent correspondre à une expression régulière : `{"format": "^GP"}`. |
| `match_query` | Un champ qui doit contenir chaque mot cherché, sans tenir compte des majuscules ni des accents. |
| `unique` | Garde un seul élément pour chaque valeur de ce champ. |
| `limit` | Garde au plus ce nombre d'éléments. |
| `tracks` | Les pistes d'un morceau, quand le site les liste avec lui (JSON seulement) : `{"json": "tracks", "fields": {"name": …, "detail": …, "drums": …, "vocal": …}}` ; `vocal` marque la partie chantée, récupérée pour les paroles du morceau même quand elle n'est pas cochée. Les pistes d'un tel morceau sont cochées sans lire son fichier ; seules celles cochées sont récupérées. |

Un champ est lu de l'une de deux façons, puis nettoyé :

| Clé d'un champ | Ce qu'elle fait |
|---|---|
| `regex` | Une expression régulière sur l'élément ; la valeur est son premier groupe, ou le groupe `group`. |
| `json` | Un chemin dans un élément JSON : `artist`, `tracks.0.name`. |
| `template` | Construit à partir des autres champs une fois lus : `"{home}/download.php?id={id}"`. |
| `text` | Retire les balises et les entités HTML, et les espaces doublés. |
| `url_decode` | Défait les `%2C` d'une adresse. |
| `replace` | Des paires de textes à remplacer : `[[",", " "]]`. |
| `uppercase` | En majuscules. |
| `absolute` | Rend un lien absolu par rapport à `home`. |
| `equals` | La valeur devient `"1"` quand elle égale ce texte, sinon `""` (pour `drums` et `vocal`). |
| `default` | Un modèle qui remplace une valeur vide. |

Les expressions régulières sont celles de la bibliothèque Rust `regex` : pas
d'assertions avant ou arrière ; `(?s)` permet à `.` de prendre aussi les retours à la
ligne.

## Variables

Les modèles prennent `{nom}` pour une variable : `{home}` ; `{query}` (la recherche,
prête pour une adresse), `{query_text}` (telle que tapée), `{query_letter}` (sa première
lettre, en minuscule, sans accent, ou `9` quand elle commence par autre chose) ; chaque
champ trouvé ; et `{tracks}`, le nombre de pistes que liste un morceau. Une variable
inconnue est omise.

## Jusqu'au fichier

`open` est une liste d'étapes depuis le morceau choisi :

| Clé | Ce qu'elle fait |
|---|---|
| `get` | Demande une page et y lit des `fields`, comme une étape de recherche (en JSON quand un champ a un chemin `json`). |
| `download` | L'adresse du fichier, un modèle. |
| `each_track` | Télécharge un fichier par piste cochée, `{track}` étant sa place dans la liste (0 pour la première). |
| `try` | D'autres valeurs pour une variable, essayées tour à tour quand l'une échoue : `{"host": ["a", "b"]}`. |

## Vidéos

`videos` est une étape, écrite comme une étape de `search` (`get`, `within`, `items`,
`fields`, `require`, `where`…). Elle est faite une fois le morceau ouvert, et seulement
quand « Vidéo YouTube calée sur la tablature » est cochée dans la fenêtre, avec pour
variables les champs du morceau et ceux que ses étapes `open` ont lus. Ses éléments
sont les vidéos du morceau, avec ces champs :

| Champ | Ce qu'il contient |
|---|---|
| `video` | L'identifiant YouTube de la vidéo (le `F4E7DSusV9Y` de son adresse). |
| `points` | La seconde où commence chaque mesure dans la vidéo, en liste JSON (`[0.11, 3.17, 6.29]`), une par mesure jouée, reprises dépliées. |
| `label` | Le nom que le site donne à la vidéo, quand il en donne un. Celle qui n'en a pas, ou qui s'appelle `original`, est prise ; sinon la première. |

La vidéo devient une piste vidéo du projet, et ses points le calage de cette piste. La
description de Songsterr livrée demande `{home}/api/video-points/{id}/{revision}/list`,
une page qui est elle-même une liste (`"items": {"json": ""}`), et garde les vidéos
dont le `status` est `done`.

## Versions

Le langage des descriptions a une version, `format_version` ; cette version du programme
lit les versions 1 et 2. La version 2 a ajouté `videos`, dont une description de
version 1 ne peut pas se servir ; la description de Songsterr livrée est de version 2,
les autres de version 1. Ce n'est pas la version du site : quand un site change ses
pages, sa description est corrigée et garde son numéro. Le numéro n'augmente que lorsqu'une nouvelle
version d'OpenGarageMusic apprend quelque chose de nouveau au langage (une clé, une façon
de lire), et les descriptions qui s'en servent indiquent le nouveau numéro.

Un programme lit toutes les versions jusqu'à la sienne. Une description écrite pour un
programme plus récent n'est pas lue : la fenêtre nomme le fichier et dit de mettre le
programme à jour. Ainsi, une description partagée par quelqu'un qui a un OpenGarageMusic
plus récent ne fonctionne qu'une fois que vous l'avez aussi, tandis que les descriptions
plus anciennes continuent de fonctionner avec chaque nouvelle version.

## Limites

Huit étapes au plus dans une chaîne, huit chaînes, huit étapes jusqu'au fichier ; 500
éléments lus dans une page ; douze pages demandées par une étape ; des fichiers de
20 Mo au plus. Une description illisible, ou dont les expressions régulières sont
fausses, est refusée au démarrage du programme ; une page qui ne contient pas ce qui
est attendu ne donne rien.

Les descriptions livrées, dans `crates/og-engine/sites` des sources, sont des exemples
complets : Guitariff (une page de résultats, puis la page du morceau pour son archive),
Tablature Hub (deux recherches réunies : par titre, et par un index des artistes) et
Songsterr (une API JSON, avec les pistes de chaque morceau, un fichier par piste, et
les vidéos du morceau).
