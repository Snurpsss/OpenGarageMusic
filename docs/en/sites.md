# Describing a tablature site

*[Version française](../fr/sites.md)*

The window of File → “New from a tablature site…” lists the sites the program
knows. Each one is described by a small JSON file: how to search the site, how to cut
its answer into songs (and their tracks, when the site lists them), how to follow its
pages to the file of a song, and which of the program's readers reads that file. A
description holds rules, never code.

The descriptions of Songsterr, Guitariff, Tablature Hub and Ultimate Guitar are built
into the program, and copies of them are in the `sites` folder next to
`OpenGarageMusic.exe`. To add a site, put a description of it in that folder; to change
one of those shipped, put a description with the same `name` there: it takes the place
of the built-in one. The folder is read when the program starts. A file that cannot be
read is named in the window, with what is wrong with it.

**Before using a site**, the program asks you once whether to read it. Whether a site
may be read by a program is for you to check, in its terms of use and its
`robots.txt`: the program reads it at your request, for your own use. What it imports
goes into your project only.

## A description

```json
{
  "format_version": 2,
  "name": "Example",
  "home": "https://www.example.org",
  "offers": { "en": "Guitar Pro files of its users.", "fr": "Les fichiers Guitar Pro de ses utilisateurs." },
  "encoding": "utf-8",
  "headers": { "Referer": "https://www.example.org/" },
  "search": [
    { "steps": [ {
      "get": "{home}/search?q={query}",
      "within": { "after": "Results", "before": "</ul>" },
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
    "get": "{home}/videos?song={url}",
    "items": { "json": "videos" },
    "fields": { "video": { "json": "youtube" }, "points": { "json": "bars" }, "label": { "json": "kind" } },
    "require": ["video", "points"]
  },
  "reader": "guitar-pro",
  "display": { "label": "{artist} – {title}", "badge": "{format}" }
}
```

| Key | What it says |
|---|---|
| `format_version` | The version of the language of descriptions the file is written in: see [Versions](#versions). `1` when it is missing. |
| `name` | The name shown in the window; it also identifies the site. |
| `home` | The address of the site, `https://` only. Every address asked must be `https://`. |
| `offers` | What the site offers, in a sentence: one text, or one per language code (`en`, `fr`, `de`, `es`, `it`, `pt`). |
| `open_only` | `true`: the site is only opened in the browser, never read (`search` and `open` are then left out). |
| `encoding` | `utf-8` (the default) or `latin1`, for the pages of older sites. |
| `headers` | Headers sent with every request, when a site wants them. |
| `search` | How to search: pipelines, whose songs are put together, each once. |
| `open` | How to go from a song found to its file. |
| `videos` | Since version 2, and optional: where the YouTube videos of a song are found, in step with its bars. See [Videos](#videos). |
| `reader` | `guitar-pro` (the default: Guitar Pro 3 to 8 and MusicXML files, also inside a ZIP archive) or `songsterr` (the parts of Songsterr's player). |
| `display` | How a song is listed: `label` and `badge`, templates (one text or one per language). |

## Searching

A pipeline is a list of steps. Each step asks for a page (`get`, a template), cuts it
into items, and reads fields from each item. The first step uses the query; each next
step is run once for every item the step before kept (at most twelve pages), with that
item's fields as variables. The items of the last step are the songs.

| Key of a step | What it does |
|---|---|
| `get` | The address of the page, a template. |
| `within` | Looks only at the part of the page after the text `after` and before the text `before`. |
| `items` | `{"regex": "…"}`: each match is an item. `{"json": "a.b"}`: each element of the list at that path (when the path is not there and the page is a list, the page itself). Without `items`, the whole page is one item. |
| `fields` | The fields read from each item (see below). |
| `require` | The fields an item must have, or it is dropped. |
| `where` | Fields that must match a regular expression: `{"format": "^GP"}`. |
| `match_query` | A field that must hold every word searched, whatever the case and the accents. |
| `unique` | Keeps one item for each value of this field. |
| `limit` | Keeps that many items at most. |
| `tracks` | The tracks of a song, when the site lists them with it (JSON only): `{"json": "tracks", "fields": {"name": …, "detail": …, "drums": …, "vocal": …}}`; `vocal` marks the singing part, fetched for the words of the song even when it is not ticked. Such a song's tracks are ticked without reading its file; only the ticked ones are fetched. |

A field is read in one of two ways, then cleaned:

| Key of a field | What it does |
|---|---|
| `regex` | A regular expression on the item; the value is its first group, or the group `group`. |
| `json` | A path in a JSON item: `artist`, `tracks.0.name`. |
| `template` | Built from the other fields once they are read: `"{home}/download.php?id={id}"`. |
| `text` | Takes out the HTML tags and entities, and doubled spaces. |
| `url_decode` | Undoes the `%2C` of an address. |
| `replace` | Pairs of texts to swap: `[[",", " "]]`. |
| `uppercase` | In capitals. |
| `absolute` | Makes a link absolute against `home`. |
| `equals` | The value becomes `"1"` when it equals this text, else `""` (for `drums` and `vocal`). |
| `default` | A template that stands for an empty value. |

The regular expressions are those of the Rust `regex` library: no look-around; `(?s)`
lets `.` take line breaks too.

## Variables

Templates take `{name}` for a variable: `{home}`; `{query}` (the query, ready for an
address), `{query_text}` (as typed), `{query_letter}` (its first letter, lowercase,
without accent, or `9` when it starts with something else); every field found; and
`{tracks}`, the number of tracks a song lists. A variable that is not known is left
out.

## To the file

`open` is a list of steps from the song chosen:

| Key | What it does |
|---|---|
| `get` | Asks for a page and reads `fields` from it, as a search step would (JSON when a field has a `json` path). |
| `download` | The address of the file, a template. |
| `each_track` | Downloads one file per ticked track, `{track}` being its place in the list (0 for the first). |
| `try` | Other values for a variable, tried in turn when one fails: `{"host": ["a", "b"]}`. |

## Videos

`videos` is one step, written like a step of `search` (`get`, `within`, `items`,
`fields`, `require`, `where`…). It is run once a song is opened, and only when “YouTube
video in step with the tablature” is ticked in the window, with the fields of the song
and those its `open` steps read as variables. Its items are the videos of the song,
with these fields:

| Field | What it holds |
|---|---|
| `video` | The YouTube identifier of the video (the `F4E7DSusV9Y` of its address). |
| `points` | The second each bar starts at in the video, as a JSON list (`[0.11, 3.17, 6.29]`), one for each bar played, repeats unfolded. |
| `label` | What the site calls the video, when it names it. The one with no label, or labelled `original`, is taken; else the first. |

The video becomes a video track of the project, and its points the timing of that
track. The description of Songsterr shipped asks
`{home}/api/video-points/{id}/{revision}/list`, a page that is itself a list
(`"items": {"json": ""}`), and keeps the videos whose `status` is `done`.

## Versions

The language of descriptions has a version, `format_version`; this version of the
program reads versions 1 and 2. Version 2 added `videos`, which a description of
version 1 may not use; the description of Songsterr shipped is of version 2, the others
of version 1. A version is not the version of the site: when a site changes its pages,
its description is corrected and keeps its number. The number goes up only when a new
version of OpenGarageMusic teaches the language something new (a key, a way of reading),
and the descriptions that use it say the new number.

A program reads every version up to its own. A description written for a newer program
is not read: the window names the file and says to update the program. So a description
shared by someone who has a newer OpenGarageMusic works only once you have it too, while
the older descriptions keep working with every new version.

## Limits

Eight steps at most in a pipeline, eight pipelines, eight steps to the file; 500 items
read from a page; twelve pages asked by one step; files of 20 MB at most. A description
that does not parse, or whose regular expressions are wrong, is refused when the
program starts; a page that does not hold what is expected gives nothing.

The descriptions shipped, in `crates/og-engine/sites` of the source, are complete
examples: Guitariff (a page of results, then the page of the song for its archive),
Tablature Hub (two searches put together: by title, and through an index of artists),
and Songsterr (a JSON API, with the tracks of each song, one file per track, and the
videos of the song).
