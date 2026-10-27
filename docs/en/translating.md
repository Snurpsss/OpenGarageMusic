# Translating

*[Version française](../fr/translating.md)*

The texts in the code are English, and English is what is shown when no other language
is chosen. Every other language is one
[gettext](https://www.gnu.org/software/gettext/manual/html_node/PO-Files.html)
catalogue in `apps/opengarage/lang`: `fr.po`, `de.po`, `es.po`, `it.po`, `pt.po`.
The language is chosen in **View → Language** and remembered.

## Adding a language

1. Copy `apps/opengarage/lang/template.pot` to `<code>.po`, where `<code>` is the
   language code (`nl`, `pl`, `ja`…).
2. In the header, set `Language:` to the code and `X-Language-Name:` to the name of
   the language in that language (`Nederlands`). That name is what the menu shows.
3. Fill in each `msgstr`. Any gettext editor (Poedit, Lokalize, Weblate…) can do it,
   and so can a text editor.
4. Either rebuild the program — every `.po` file of the folder is built in, nothing
   else has to be changed — or, without rebuilding, put the file in a `lang` folder
   next to the program, or in `%APPDATA%\OpenGarageMusic\lang`.

A file on the disk with the code of a built-in language replaces it: that is the way
to correct a translation without rebuilding.

## Rules

- A text without a translation (empty `msgstr`, or no entry) is shown in English.
- Keep the placeholders as they are: `{}`, `{count}`, `{:.1}`. They may move inside the
  sentence. When two `{}` have to swap, number them from zero: `{1} … {0}`.
- Menu entries carry their keyboard shortcut after a run of spaces
  (`Undo        Ctrl+Z`): keep the spaces and the shortcut.
- Spaces at the start or the end of a text are part of it.
- What plug-ins and sound banks say about themselves (their names, the names of their
  parameters) is shown as they give it, usually in English; it is not translated.
- A few low-level error messages of the audio engine (unreadable WAV, MIDI or
  SoundFont file) are English only.

## For developers

- A text is written `tr("Text")`. With values: `tf!("Track {}", number)`; values the
  text names are listed in brackets: `tf!("{count} notes recorded." [count])`.
- Names from the engine (sounds, loops, templates, kinds of track) are English
  identifiers; they go through `tr(name)` where they are shown, never where they are
  stored or compared.
- A new text must be added to `template.pot` and to every `.po` file of the folder.
  `dev test -p opengaragemusic` fails, naming the missing texts, until each built-in
  language has every text of the code with its placeholders intact.

## State of the translations

French repeats the texts the program had before it was translated. German, Spanish,
Italian and Portuguese were written with the help of an AI and have not been reviewed
by native speakers: corrections are welcome.
