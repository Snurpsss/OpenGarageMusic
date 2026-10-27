# External files

*[Version franÃ§aise](../fr/external-files.md)*

Everything in the repository that was not written for OpenGarageMusic: where it comes
from, under which licence, and where it was downloaded. Rule of the project: only
freely licensed contents enter it (see [Contributing](contributing.md)). One thing the
program reads at the user's request is not free and never enters the repository: the
tablatures of Songsterr, described below.

## Sounds

**No sound is shipped with OpenGarageMusic.** The built-in sounds are computed by the
program. Banks of sampled instruments are downloaded by the user, on request, from the
Library menu; they are not part of the repository.

Those downloads go to `%LOCALAPPDATA%\OpenGarageMusic`, in two separate folders:
`Banques` for sound banks, `Effets` for plug-ins. Each holds one sub-folder per site,
then per item, with a `SOURCE.txt` file written by the application: name, author,
licence, site, page, exact download address, date and SHA-256 checksum of the file
received.

Sites offered by the download window:

| Site | What the application lists there | Licence |
|---|---|---|
| MuseScore (mirror <https://ftp.osuosl.org/pub/musescore/soundfont/>) | FluidR3 GM, MuseScore General (SF3) | MIT |
| Musical Artifacts (<https://musical-artifacts.com/>) | SF2 and SF3 banks whose declared licence is free (public domain, CC BY, CC BY-SA, WTFPL, GPL) | declared by the person who uploaded the file, not verified |
| SFZ Instruments (<https://github.com/sfzinstruments>) | Repositories for which GitHub recognises a free licence (CC0, CC BY, CC BY-SA, Unlicenseâ¦) | that of the repository |
| Polyphone, FreePats | Nothing directly: the application points to the site | specific to each file |

## External effects and instruments (plug-ins)

**No plug-in is shipped with OpenGarageMusic.** The user can import a `.clap` file, an
`.lv2` folder or a `.vst3` file they have, or download plug-ins from the Library menu.
A plug-in is a program: the application does not check it. It runs it in a process of
its own, which protects the application from its crashes, not you from its intentions.

| Site | What the application lists there | Licence |
|---|---|---|
| GitHub, the authors' repositories | The latest 64-bit Windows version of: Dragonfly Reverb, Wolf Shaper, Six Sines, Dexed, Surge XT | GPL-3.0 or MIT depending on the project (licence of the repository, noted on 11 October 2026) |
| OpenAudio (<https://github.com/webprofusion/OpenAudio>) | The list of projects (one file, read from GitHub). For the chosen project only, the application asks GitHub for its licence and its latest version: the download is only offered when the licence is free and a `.zip` archive for 64-bit Windows exists | that of each project's repository, noted when it is chosen |
| x42-plugins (<https://x42-plugins.com/x42/>), CLAP Database (<https://clapdb.tech/>) | Nothing directly: the application points to the site; plug-ins installed in the standard LV2, CLAP and VST3 folders are found | specific to each plug-in |

From a downloaded archive the application keeps what it can use: `.clap` files and
`.vst3` files or folders. These plug-ins are loaded at the user's request and are not
distributed with OpenGarageMusic.

During development, two banks (FluidR3_GM from Debian, MuseScore_General from the
MuseScore mirror, both under the MIT licence) were downloaded into the repository for
trials, then deleted. Nothing of them remains.
## Tablatures (Songsterr)

**No tablature is shipped with OpenGarageMusic**, and none enters the repository. File
â âNew from a tablature siteâ¦â, with Songsterr chosen, asks <https://www.songsterr.com>
for the songs that match a search and reads the parts of the one the user chooses; the
notes go into the user's project and nowhere else.

| Site | What the application reads there | Licence |
|---|---|---|
| Songsterr (<https://www.songsterr.com>) | The list of songs of a search, the description of a song (which track is the singing one: its field `isVocalTrack`, read as the field `vocal` of the description of the site), and the files of its parts, which the site's own player loads from its servers; when the user asks for the words of the song (the box âThe words of the songâ, ticked at first), the file of the singing part, for its words (`newLyrics`), even when that part is not ticked; when the user asks for it, the YouTube videos of the song with the second each bar starts at in them (`/api/video-points/â¦`, the points its users set, noted on 16 October 2026) | **not free**: the tablatures are written by the users of the site, of songs that are for the most part under copyright; the conditions of use of the site apply, and the program cannot check rights |

This is the one exception to the rule that only freely licensed contents are offered,
and it is the user's to use: what is imported is for their own practice, and is not to
be distributed with a project. The addresses and the format of the files are not
documented by the site; they were worked out from what its public player loads (noted
on 12 October 2026) and can change without notice. The tests of the project read parts written
for them and a few lines of the site's lists (titles and numbers of songs), not
tablatures of the site; the few tests that ask the site are turned off by default.

The program does not sign in to the site and keeps no account: it reads what the site
shows to anybody.

## Other tablature sites, and Guitar Pro files

File â âNew from a tablature siteâ¦â names three more sites. What their conditions
of use and their `robots.txt` files say was read on 14 October 2026. Two of them,
Guitariff and Tablature Hub, are searched by the program itself, which fetches the
Guitar Pro file of the song the user chooses, at that moment and at their decision,
with its own User-Agent (`OpenGarageMusic/<version>`) and no account; the third,
Ultimate Guitar, is only opened in the browser. Whatever the site, the program also
imports a Guitar Pro file the user downloaded themselves.

Since version 0.9.9, each site, Songsterr included, is read through a description in
JSON (`crates/og-engine/sites`, written for the project and part of it), which says
how to search it and where the files are. A user may add descriptions of other sites
in the `sites` folder next to the program ([Describing a tablature site](sites.md)):
such a site was not studied by the project, and whether it may be read by a program is
the user's to check, as the window says when it asks, once, whether to read it.

| Site | What the program does there | Conditions |
|---|---|---|
| Ultimate Guitar (<https://www.ultimate-guitar.com>) | Opens the site | Its terms of service allow only printing a text tablature for personal use, forbid reproducing the contents âonto any physical medium, memory or deviceâ and replicating them, and its `robots.txt` closes the search page and the download paths to programs. **Not free**, and not to be read by a program |
| Tablature Hub (<https://www.tablatureguitare.net>) | Searches the site (`/recherche?q=â¦`, which knows the titles only, and its index of artists by letter, `/tablature-partition/<letter>.html`, then the pages of at most three matching artists) and fetches the Guitar Pro file of the song the user chooses (`/download.php?id=â¦`), with the program's User-Agent and no account | Its conditions reserve the tablatures, âpersonal and approximate interpretations of songs under copyrightâ, to personal and educational use, and say nothing of programs. Its `robots.txt` disallows `/download.php`, the address its Guitar Pro 3 and 4 files are served from: the author of the project chose to read it anyway, for the personal use of whoever asks for a song, one file at a time, never as a crawl. **Not free**; what is read goes into the user's project only |
| Guitariff (<https://www.guitariff.net>) | Searches the site (`/tabs/search.php?q=â¦`), reads the page of the song the user chooses and fetches the ZIP archive it offers (`/ddl_tabs/â¦`), with the program's User-Agent and no account | Its conditions ask that a tablature be used âfor personal purposes, research or school workâ only, and say nothing of programs; its `robots.txt` only closes its statistics. **Not free**; what is read goes into the user's project only |

The program reads **Guitar Pro files** itself (`crates/og-engine/src/guitar_pro.rs`),
whatever site or program they come from: GP3, GP4 and GP5 files, the GP6 ones (.gpx)
and the GP7 and GP8 ones (.gp). The formats are not published by their maker; the
reader follows the public descriptions of them and was written for the project (no
code was taken from the free programs that read them). No tablature is shipped and
none enters the repository: the tests read files that they write themselves.

| Description used | For | Licence |
|---|---|---|
| The description of the formats in the documentation of PyGuitarPro (<https://pyguitarpro.readthedocs.io/en/stable/pyguitarpro/format.html>), read on 15 October 2026 | Checking the reader of GP3, GP4 and GP5 field by field | Read to understand the formats only: PyGuitarPro's code is under a copyleft licence and none of it was taken |
| The README of rust-gpx-reader (<https://github.com/Antti/rust-gpx-reader>) and `gpx_filesystem.go` of go-guitar-pro (<https://github.com/CaliLuke/go-guitar-pro>), read on 14 October 2026 | The container of GP6 files: its compression and its file system (`crates/og-engine/src/gpx.rs`, written anew in Rust) | go-guitar-pro: MIT; the README states no licence and only its description was used |

The program also reads **MusicXML scores** (`crates/og-engine/src/musicxml.rs`),
following the published specification of the format (MusicXML 4.0, W3C Music Notation
Community Group, <https://www.w3.org/2021/06/musicxml40/>), an open format meant to
be implemented by any program. The reader was tried, outside the repository, on the
212 MusicXML files that alphaTab carries for its tests (among them the MusicXML test
suite and the samples published with the format); none of them is in the repository.

The GP6 reader was also tried, outside the repository, on the 38 GP6 files of the
alphaTab test suite (MPL-2.0) that go-guitar-pro carries; they are not in the
repository, and a test turned off by default reads such files from a folder of the
user's (`OPENGARAGE_SCORES_DIR`), as it does the other formats.

## Captures and impulse responses (TONE3000, GuitarML)

Library → “Captures and impulse responses from the sites…” names two sites, each
described to the program the way the tablature sites are (`crates/og-engine/sites`),
read on 17 October 2026:

- **TONE3000** (<https://www.tone3000.com>): captures of amplifiers and pedals in the
  Neural Amp Modeler, AIDA-X and Proteus formats, and impulse responses, shared by
  its users, each under the licence its author chose (most under the site's T3K
  licence: use and publish what you play freely, do not pass the file on). The site
  is free with an account, and its API terms for free programs (read the same day)
  allow the user's favourites, the trending and the latest tones, downloaded one by
  one at the user's request, never in bulk, and ask each program for a key that the
  user makes in their own account; the program keeps that key and the token the
  site gives it in the settings, and asks nothing of the site without them.
- **GuitarML's ToneLibrary** (<https://guitarml.com/tonelibrary/tonelib-pro.html>):
  captures for Proteus, free and without an account, each a `.json` file on GitHub.

What is downloaded goes into the library's folders `Captures` and `Impulsions`, in a
folder of the site and of the tone, with a `SOURCE.txt` that says where it came
from, from whom and under which licence. The program ships no capture and no impulse.
The `.nam` format and its reference reader are open source (MIT); the program reads
`.nam` files with the `nam-rs` crate and the Proteus and AIDA-X ones with its own code.

## Recordings and videos of a song

A song can be played with its original sound: a recording of the user's, or a YouTube
video. **No recording and no video is shipped, and none enters the repository.**

- An audio file of the user's (MP3, OGG, FLAC, M4A/AAC, WAV) stays where it is: the
  project refers to it by its path, it is not copied, and nothing is uploaded. The
  music library is a folder of the user's that the program reads; its index
  (`library.json`, beside the settings) holds paths, artists and titles, nothing of
  the sound.
- A YouTube video is played by YouTube's own player, inside the program's window,
  while it is seen. The program keeps no video and none of its sound: nothing is
  downloaded. YouTube Music is not used: it has no player that can be built in.

| File or service | What it is | Origin and licence |
|---|---|---|
| `og-video.exe` | The video player: a small program next to `OpenGarageMusic.exe` that shows YouTube's player in a web view drawn inside the program's window, in a process of its own so that the program never needs `WebView2Loader.dll` (it starts without it; only the video does not) | Built from `apps/og-video`, written for the project. The web view keeps its data in `%LOCALAPPDATA%\OpenGarageMusic\WebView2` |
| `WebView2Loader.dll` | The loader of Microsoft's WebView2 runtime, the web view that is part of Windows 10 and 11 | Microsoft Corporation, from the WebView2 SDK (<https://www.nuget.org/packages/Microsoft.Web.WebView2>), under the BSD 3-Clause licence. **Not in the repository**: building `og-video` copies it, unchanged, from the crate `webview2-com-sys`, which carries it; it is shipped in the download next to `og-video.exe`, its licence written in `THIRD-PARTY-LICENSES.txt` |
| `MidiPilot.exe` | The MIDI Pilot alone, for the stage: a launcher that starts `OpenGarageMusic.exe`, next to it, on that window only (no sound card, no project; a song whose project is to be opened starts the whole program on it) | Built from `apps/midi-pilot`, written for the project |
| YouTube's IFrame Player API (<https://developers.google.com/youtube/iframe_api_reference>) | The player of YouTube videos that a page can build in | A service of YouTube, loaded by the video player from youtube.com when it runs, under YouTube's terms of service. Nothing of it is in the repository |
| `screenshots/video-and-tablature.png` | The screenshot of the video and the tablature of a song | The video shown is *Big Buck Bunny* (Â© 2008 Blender Foundation, <https://peach.blender.org>), under the [Creative Commons Attribution 3.0](https://creativecommons.org/licenses/by/3.0/) licence, played from YouTube for the demonstration |

## Words of the song

**No words of a song are shipped, and none enter the repository**, except those
written for the demonstration project. The words of a song come from its
tablature (Songsterr, a Guitar Pro file, a MusicXML score: see above), from LRCLIB, or
from the user (a file beside their recording, a file they choose, words they type).
They are kept in the user's project and nowhere else: like tablatures, they are for
the most part under copyright, and are there for the user's own practice.

| File or service | What the program does there | Origin and conditions |
|---|---|---|
| LRCLIB (<https://lrclib.net>) | Asks its public interface for the words of a song (`/api/search?track_name=â¦&artist_name=â¦`), by itself when a tablature or a Guitar Pro or MusicXML file without words becomes a project, and with âSearch LRCLIBâ. It sends the artist and the title of the project, with the User-Agent `OpenGarageMusic/<version>`, and nothing else: no account, no key; the length of the recording only chooses, on the computer, among the versions it answers | A free service with no account, whose words, timed line by line, are given by its users. **Not free**: the words are those of songs for the most part under copyright; what is read goes into the user's project only. Nothing of it is in the repository: the test that asks the real service is turned off by default |
| `screenshots/words.png` | The screenshot of the band of the words, the line being sung | Made for the project: the demonstration project, with words written for it |

## Pedal descriptions (MIDI Pilot)

The modules of the BOSS DD-500 and RV-500 pedals are driven by `crates/og-engine/devices/dd500.json` and `rv500.json`, written for the project from the makers' public MIDI implementation documents (Roland Corporation, downloaded on 25 October 2026): [DD-500 Ver.2.00 MIDI Implementation](https://static.roland.com/assets/media/pdf/DD-500_Ver-200_MIDI_Imple.pdf) and [RV-500 MIDI Implementation](https://static.roland.com/assets/media/pdf/RV-500_MIDI_Imple_jpn01_W.pdf) (Japanese edition, its tables readable). They hold the model numbers, the map of the memory (system exclusive addresses, the requests and data sets of the Roland dialect), and each parameter of a patch, of the system and of a bank with its place, its range, the names of its values and the range as the document shows it. Nothing of the makers' software is in them: the parameter names and lists are those the documents print, and the modules were checked against the documents' ranges and addresses. The user's pedal files (`*.pedal.json`) and the backups of the BOSS editors (`*.bel`, JSON) are read and written by the program and never enter the repository. The names BOSS, DD-500 and RV-500 belong to Roland Corporation; the project is neither affiliated with it nor endorsed by it.

## G3S backup files (MIDI Pilot)

The module of The GigRig G3S pedal switcher reads and writes its backup files (`.g3b`), the ones its editor makes and restores. The maker publishes no description of that format: what the program knows of it (system exclusive messages of 50 bytes, 7-bit packing, checksums and a sum at the end, the records of the global settings, the presets, the extended MIDI and the songs, and in a preset the loops, their order, the name and the MIDI lines) was deduced from backup files the switcher itself made, and checked by restoring files written this way in the switcher; nothing was taken from the maker's software. What is not understood (most of the global settings, the MIDI map) is carried over unchanged. The files are the user's and never enter the repository: the tests work on files the program makes up. Writing a backup is the user's risk: the switcher checks each file it restores (checksums, sequence, sum), and “Check” runs the same checks beforehand. The names The GigRig, G3 and G3S belong to their owner; the project is neither affiliated with it nor endorsed by it.

## Set list pictures

**No picture of an album or an artist is shipped, and none enters the repository.**
The buttons of the MIDI Pilot show a picture the user chose, or else one looked for on
Deezer, which the user can turn off (Settings > “Covers and photographs from Deezer”).

| File or service | What the program does there | Origin and conditions |
|---|---|---|
| Deezer (<https://api.deezer.com>) | Asks its public search, with no account and no key, for the cover of the album of a song of the MIDI Pilot (`/search?q=<artist> <title>&limit=1`) and the photograph of an artist (`/search/artist?q=<artist>&limit=1`), one question at a time, then fetches the picture it points to. It sends the artist and the title of the song, and nothing else. The pictures are kept in the `Covers` folder of the program's data folder (`%LOCALAPPDATA%\OpenGarageMusic`), so that they show on stage without the Internet; Settings > “Empty the pictures kept” deletes them | **Not free**: covers and photographs belong to their authors and publishers; they are shown to the user only, on the button of their song. Nothing of it is in the repository, and the tests ask nothing of Deezer |
| `screenshots/midi-pilot.png` | The screenshot of the window of the MIDI Pilot | Made for the project: songs and artists invented for the demonstration, no picture from Deezer; the one picture is the program's own loading photograph |
| `screenshots/midi-pilot-pedal.png` | The screenshot of the module of a BOSS DD-500 in the MIDI Pilot | Made for the project, on a pedal made up by the program: the patches named are invented |
| `screenshots/midi-pilot-script.png` | The screenshot of an AI script reviewed in the MIDI Pilot | Made for the project: the script is invented |

## Build tools

Installed in `.toolchain` (not versioned) by `tools/setup-toolchain.ps1`.

| Tool | Licence | Downloaded from |
|---|---|---|
| Rust (rustup, compiler, cargo) | MIT or Apache-2.0 | <https://static.rust-lang.org/rustup/dist/x86_64-pc-windows-msvc/rustup-init.exe> |
| w64devkit 2.10.0 (only `as.exe`, `dlltool.exe`, `windres.exe` and `libshlwapi.a` are used) | a collection of free tools; `as` and `dlltool` come from GNU binutils (GPL) and are not redistributed with OpenGarageMusic | <https://github.com/skeeto/w64devkit/releases/download/v2.10.0/w64devkit-x64-2.10.0.7z.exe> |

## Code libraries

Downloaded by Cargo from <https://crates.io>; the exact list and the versions are in
`Cargo.lock`. Those that matter for licences:

| Library | Role | Licence |
|---|---|---|
| egui / eframe | interface | MIT or Apache-2.0 |
| cpal | audio input and output | Apache-2.0 |
| midir | MIDI keyboards, and the MIDI output of the MIDI Pilot | MIT |
| rfd | file dialogs | MIT |
| rustysynth | playing SoundFont banks | MIT |
| symphonia | decoding MP3, AAC, FLAC, Ogg Vorbis | MPL-2.0 (used unmodified) |
| clap-sys | definitions of the CLAP plug-in standard | MIT or Apache-2.0 |
| vst3, com-scrape-types | definitions of the VST 3 plug-in interfaces (generated from the VST 3 SDK, itself under the MIT licence since version 3.8) | MIT or Apache-2.0 |
| libloading | loading plug-in files | ISC |
| ureq, native-tls | downloads (secure connection by the system) | MIT or Apache-2.0 |
| sha2 | checksums of downloads | MIT or Apache-2.0 |
| zip | opening downloaded archives | MIT |
| windows | calls to the MP3 and AAC encoders of Windows, plug-in windows, the window of the video player | MIT or Apache-2.0 |
| wry (by the Tauri project) | the web view of the video player | MIT or Apache-2.0 |
| webview2-com, webview2-com-sys | definitions of Microsoft's WebView2 interfaces, used by wry | MIT |
| serde, serde_json | project files | MIT or Apache-2.0 |
| image, zune-jpeg, png | reading the picture of the loading screen, and the pictures of the MIDI Pilot | MIT or Apache-2.0 (zune-jpeg: or Zlib) |
| regex | the rules of the descriptions of the sites | MIT or Apache-2.0 |
| nam-rs | running Neural Amp Modeler captures (`.nam`); a pure Rust rewrite of NeuralAmpModelerCore, itself MIT | MIT |
| rustfft | the transforms of the convolution with impulse responses | MIT or Apache-2.0 |

The LV2 host uses no library: it is written against the public LV2 specification (ISC
licence) and reads the plug-ins' description files itself. The translations are read
by the program's own reader of gettext `.po` files.

## Icon

The program's icon is the body of a blue electric guitar on a dark tile, in
`apps/opengarage/icon`.

| File | What it is | Origin |
|---|---|---|
| `icon.png` and the `*.rgba` files (the same picture at 16, 24, 32, 48, 64 and 256 pixels) | The icon | A drawing made by an AI image generator at the request of the project's author, who supplied it on 12 October 2026; it was downloaded from nowhere. `tools/icon.ps1` took out its background, put it on the tile and wrote it at each size |

The drawing shows a guitar with two cutaways, without the head, and bears no maker's
name or mark. It is a drawing, not a photograph.

## Track pictograms

The pictogram at the left of each track header, which says what the track plays,
is a drawing of [Game Icons](https://game-icons.net), under the
[Creative Commons Attribution 3.0](https://creativecommons.org/licenses/by/3.0/)
licence, which asks for each author to be named. The files, in
`apps/opengarage/track-icons`, were taken unchanged on 15 October 2026 (the film strip
on 16 October 2026) from the copy Iconify keeps of the collection
(<https://api.iconify.design/game-icons.json>); the author of each was checked on its page of game-icons.net. The program fills them at
the size of the header, in a dark ink over the colour of the track.

| File | Drawing | Author |
|---|---|---|
| `grand-piano.svg` | Piano, keyboards | Caro Asercion |
| `piano-keys.svg` | Synthesizer | Delapouite |
| `guitar-bass-head.svg` | Bass | Delapouite |
| `guitar.svg` | Guitar | Lorc |
| `violin.svg` | Strings | Zajkonur |
| `trumpet.svg` | Brass | Delapouite |
| `saxophone.svg` | Winds | Delapouite |
| `xylophone.svg` | Mallets | Delapouite |
| `drum-kit.svg` | Drums | Caro Asercion |
| `microphone.svg` | Voice | Delapouite |
| `sound-waves.svg` | Audio | Skoll |
| `musical-notes.svg` | Other instrument | Delapouite |
| `film-strip.svg` | Video | Delapouite |

## Loading screen

| File | What it is | Origin |
|---|---|---|
| `apps/opengarage/splash/loading.jpg` | The picture shown while the program starts, 1672 by 941 pixels; the loading screen is half that size | A picture of the author's own studio with the program's name written over it, supplied by the project's author on 14 October 2026 for this use (`load.png`, turned into JPEG for the program; the PNG file stays out of the repository); it was downloaded from nowhere |

The instruments and devices in the picture bear the names of their makers, as they
were bought; the project is affiliated with none of them.

## Translations

The catalogues in `apps/opengarage/lang` were written for the project: French from
the program's original texts, German, Spanish, Italian and Portuguese with the help of
an AI. They are part of the project and follow its licence.

## What is not in the repository

- The MP3 and AAC encoders are those of Windows; no codec is shipped.
- No sound, loop, icon or text from Apple.
- `WebView2Loader.dll` is copied at build time and is only in the download.
- No video and none of its sound: YouTube's player plays them. Your recordings and
  your music library stay on your disk, where they are.
- The SoundFont or SFZ banks and the CLAP, LV2 or VST3 plug-ins that you import or
  download yourself stay on your disk, outside the repository; check their licence
  before distributing a project that contains them.
