# OpenGarageMusic

*[Version française](../fr/README.md)*

A music studio written from scratch in Rust, its source published and free for
personal use (PolyForm Noncommercial: see the licence in the README of the project;
a professional use needs a licence from the author), inspired by the features of
GarageBand. An independent project, with no link to Apple; no code, sound or
graphic element from Apple is used (see [Contributing](contributing.md)).

## What it does

- **Tracks**: software instrument, audio, guitar or bass with an amp, virtual drummer,
  master track; mute, solo, lock, level meter; the pictogram of the instrument the
  track plays, a click on which chooses its colour.
- **Sounds**: a library of 36 sounds (synthesizer, plucked strings, drum kits), set
  with knobs, saved to “My sounds”; arpeggiator.
- **Sampled instruments**: SoundFont banks (`.sf2`, `.sf3`) and SFZ instruments
  (`.sfz`). The Library menu downloads them from sites that distribute them under a
  free licence, or imports them from the disk; none is shipped.
- **Plug-ins**: effects and instruments in the CLAP (`.clap`), LV2 (`.lv2` folder) and
  VST3 (`.vst3` file or folder) formats, imported from the disk, found in the standard
  folders of the system, or downloaded from free projects (Library menu), then added
  to a track from the sound controls. A plug-in is set with sliders, or in its own
  window when it has one.
- **Download window**: sites on the left, what they offer under a free licence in the
  middle, where it goes on the right. What is already downloaded is ticked.
- **Sound controls**: each panel (sound, arpeggiator, plug-ins, equaliser…) is dragged
  by its title next to, above or below another, hidden with a right-click, and shown
  again with a right-click on the background. The layout is remembered.
- **Languages**: English (default), French, German, Spanish, Italian, Portuguese, in
  View → Language. Each language is one gettext `.po` file: see
  [Translating](translating.md).
- **Virtual drummer**: seven styles, complexity, intensity, swing, fills, kit pieces;
  its kick drum can follow the notes of another track (the bass, say).
- **Regions**: create, move (to another track too), resize, loop, split, join, cut,
  copy, paste. Dragged with Alt or Ctrl held, a region or a note is copied instead of
  moved.
- **Editors**: notes (multiple selection, velocity, quantizing, transposition), score,
  tablature for a guitar or bass track (the track's own tuning and fingering when a
  Songsterr import gave them, chord names along the top, each with a grid of the
  fingering), drummer, audio (transposition without changing the length, tuning,
  snapping attacks to the grid). The note editors zoom with Ctrl and the mouse wheel,
  or with the "-" and "+" at their bottom right, sideways only and as far as wanted
  either way (the value between them is the zoom, which a click puts back to four bars
  across; a tablature starts at four beats); the tablature has a “Text size” slider for
  its fret numbers, chord names and chord grids. The score writes the key and time
  signatures, the values of the notes with beams, dots and triplets, the rests, and
  the techniques of a tablature. The “Two lines” box lays the score or the tablature
  over two lines of whole bars, as on a page; the full window lays the whole region out
  that way, line after line, and follows the song. Ctrl and a click on track headers
  put several tracks in the editor, one under the other; the “Auto-switch” button then
  shows one at a time, the one being played, the first chosen having priority, and
  moves on, as the last note of the one shown is struck, when nothing more comes on it
  within the bars set in View → “Auto-switch after”. Shift and a click on a beat of
  the score or the tablature, then on another, choose the beats between them, and
  “Loop the beats” plays them over and over. While
  the song plays, the notes scroll under a playhead kept a fifth of the way from the
  left; View → “Playhead kept in place in the editor” turns that off, and the editor
  then pages instead.
- **Recording**: notes played on the computer keyboard or on a MIDI keyboard, audio
  from the sound card input with multiple takes and input monitoring, count-in,
  metronome, tuner.
- **Loops**: 42 loops of notes that follow the tempo and the key, with filters,
  listening and favourites. A loop is dragged from the browser onto a track, or under
  the tracks, where a track is made for it. Your own recordings can be added as
  **audio loops** (“Add files…” in the browser): they are stretched to the tempo of
  the project. No recorded loop is shipped.
- **Mixing**: equaliser (a curve with three points to drag), compressor, echo and
  reverb per track, amps and a chain of pedals chosen one by one (overdrive, noise
  gate, wah, auto-wah, chorus, flanger, tremolo, phaser, three echoes), effects on
  the master output, a room or a plate as the shared reverb. Automation of volume
  and pan, and of the effects of each track: reverb, echo, compressor, the bands of
  the equaliser, the amp's gain, the pedals (the wah's pedal among them).
- **Captures and impulse responses**: the amp of a track may run a capture of a real
  amplifier or pedal, a `.nam` file of Neural Amp Modeler or a `.json` one of
  Proteus or AIDA-X, in place of its drive stage; an impulse response (a WAV or FLAC
  file) may stand for its cabinet, and another for the room of the shared reverb.
  You bring the files, or download them from TONE3000 (with your account) and
  GuitarML's ToneLibrary through the window of the sites; none is shipped.
- **Dropping files**: an audio or MIDI file dragged from the file explorer onto the
  tracks lands on the bar under the pointer (audio on an audio track, or a new one); a
  sound bank joins the library; a project file opens.
- **Arrangement**: tempo and transposition tracks, sections that move with their
  contents, loop area. While the song plays, the playhead stays in the middle and the
  tracks slide under it; View → “Playhead kept in the middle of the tracks” turns that
  off, and the playhead then crosses the tracks while the view turns the page.
- **Files**: a project is one file (`.ogproj`, an archive of the project as JSON and
  of its takes), saved in the `OpenGarageMusic` folder of your documents unless you
  choose elsewhere; the latest ones are listed in the File menu and at start-up. The File menu starts new projects (“New…”, “New from a
  tablature site…”, “New from a Guitar Pro or MusicXML file…”), opens them (“Open…”,
  “Open a recent project”, “Previous sessions…”), saves them, and brings files in and
  out:
  - “Import into this project…” opens a window that adds an audio file (WAV, MP3, AAC)
    onto the selected audio track, or a new one, at the playhead, or a MIDI file, whose
    tracks of notes become new tracks, or “A recording of the song…”, which is found in
    step with the score (see **Original sound**). Files can still be dropped onto the
    window; sound banks are imported from the Library menu.
  - “Export to…” opens a window that lists the formats, each with a few words on what
    it is for: WAV and AIFF in 16 or 24 bits, MP3 and AAC (`.m4a`) with their quality
    (128, 192, 256 or 320 kb/s, the last for MP3 only), and MIDI (`.mid`). The MIDI
    file holds the notes of the tracks as they are played (swing, tremolo picking,
    trills… included) and no sound: a Standard MIDI File of format 1, one track for
    each track that has notes, with the tempo and its changes, the time signature, the
    General MIDI program of a track that plays a sampled preset, and the drum and
    drummer tracks on channel 10. The whole song is exported, from its start to its
    end marker.
- **Songsterr**: File → “New from a tablature site…”, with Songsterr chosen at the
  top of the window, searches songsterr.com, lists the tracks of a tablature (guitars, bass, drums, vocals…) and makes a new project of the
  ticked ones, one track of notes each, with the tempo, the time signature, the key and
  the sections. Each track gets the instrument the tablature names, from an installed
  General MIDI bank when there is one (Library → Download sounds), or a built-in sound
  of the same family otherwise. What the tablature writes around its notes comes too:
  slides, bends, hammer-ons and pull-offs, vibrato, harmonics, dead and ghost notes,
  palm muting, let ring… They are drawn on the tablature view and played: a bend or
  a slide moves the pitch of the note while it sounds. No account is needed: the
  program reads what the site shows to anybody.
- **Guitar Pro files, MusicXML scores and the other tablature sites**: the same window,
  File → “New from a tablature site…”, searches Guitariff and Tablature Hub too: the
  site is chosen at the top, a search lists the Guitar Pro tablatures found, choosing
  one reads its file in the background and lists its parts with check boxes, and
  “Create the project” makes the project of the ticked ones. Ultimate Guitar is only
  opened in the browser, as its terms ask. Each site is described by a small JSON file:
  more sites can be added in the `sites` folder next to the program (see
  [Describing a tablature site](sites.md)); the program asks once before reading a site.
  File → “New from a Guitar Pro or MusicXML file…” and the button at the bottom of the
  window make a project of a Guitar Pro file (.gp3, .gp4, .gp5, .gpx or .gp) or a
  MusicXML score (.musicxml, .xml or .mxl) the same way, and such a file can be dropped
  onto the window. The “Original sound” section of the tracks column offers, for the
  song chosen, its recording when your music library has it (“Your recording: …”,
  ticked; ✕ refuses it), or a button “A recording of the song…” to choose a file;
  the box “The words of the song”, ticked at first (see **Words of the song**); and,
  for Songsterr, the box “YouTube video in step with the tablature”, unticked at first
  (see **Original sound**).
- **Original sound**: a song can be played with its recording. A recording is a track:
  an audio track holding an audio file of yours (MP3, OGG, FLAC, M4A/AAC, WAV), or a
  video track holding a YouTube video, each with its timing, the second each beat of
  the score falls at in it. The “Original sound” button of the transport bar, shown
  when the project has a recording, switches at any time between two ways of playing:
  - off, the song plays as written, at the tempo of the score, and the recordings are
    silent;
  - on, the song plays at the pace of the recording that leads: the tempo comes from
    its timing, bar by bar, so that a band that speeds up or drags is followed. The
    recording is heard, and the other tracks are silent unless their M button is
    turned off: in this mode M means “silent while playing along”, and turning it off
    plays that track over the recording. There is no count-in; the editors scroll with
    the song as usual. With several recordings, the header of each recording track says
    which one leads, and a click on another makes it lead.

  *An audio file of yours*: File → “Import into this project…” → “A recording of the
  song…”. In the background, the program finds where the beats of the score fall in
  it: twenty times a second, it compares which notes of the scale sound in the score
  (from its notes) and in the recording (from its spectrum, over about a third of a
  second), and where notes are struck, drums left out; dynamic time warping pairs the
  two, so the score may start anywhere in the recording (an intro, silence) and its
  pace may change. A song of four minutes takes about two seconds. The program says
  how alike the two are, in %; under 30 % it warns that it may not be the same song or
  the same version. The recording goes on an audio track of its own, which leads, and
  the song switches to the original sound at once; the editor of that track has
  “◀ 0.1 s” and “0.1 s ▶” to shift the recording against the score. The file stays
  where it is: the project refers to it by its path, it is not copied, and nothing is
  uploaded.

  *Your music library*: File → Options → “Music library folder…”. The program reads
  that folder and its subfolders in the background (MP3, OGG, FLAC, M4A, AAC, MP4,
  WAV, up to 100 000 files), each file known by the artist and the title of its tags
  (ID3, Vorbis comments, MP4), or else by its name (“Artist - Title.mp3”, a leading
  track number skipped). The index is kept beside the settings (`library.json`), and
  only the files that changed are read again. Names are compared plainly: the case,
  the accents, what is in brackets (“(Remastered 2011)”, “[Live]”), “feat.” and what
  follows it, and a leading “The” do not count. When a song is chosen in the window of
  the tablature sites, its recording is offered, and once the project is made it is
  found in step with the score as above.

  *A YouTube video from Songsterr*: with “YouTube video in step with the tablature”
  ticked, the program asks the site, as its public player does, for the videos of the
  song and the second each bar starts at in each: the points the site's users set
  (152 points for the 152 bars played of Nothing Else Matters, repeats unfolded). The
  original video, or else the first, becomes a video track; if the numbers of bars
  differ by more than one, a message says it may drift. Selected, the video track shows
  the video in the editor, as large as the panel allows at 16:9; Ctrl and a click on a
  guitar track's header show the video and the tablature one under the other. With the
  original sound on and the video leading, YouTube's own player plays it in step with
  the song: the program starts and stops it with the song, follows it rather than sending it elsewhere (which would make it stop and load again): when the two part by more than an eighth of a second, the song moves to where the video is; the song plays with the video as soon as the project is made, its other tracks silent, and a click on Play in YouTube's player starts the song with it; and the song waits while the video loads or shows
  an advertisement, then starts again with it. When the editor does not show the video, the “Video” button, left of “Full window”, shows it in a corner of the window (it is off at first; YouTube asks that its player be seen while it plays). The editor of the video track has the shift buttons
  and “Delay of the video player” (in ms, a setting of the computer, for every video).
  If YouTube does not play a video (removed, blocked in your country or outside
  YouTube), the editor says so with its error code. YouTube Music cannot be used: it
  has no player that can be built in. The program keeps no video and none of its
  sound: nothing is downloaded. The player is `og-video.exe`, a small program next to
  the program that shows YouTube's player in a WebView2 web view (part of Windows 10
  and 11) drawn inside the window; it needs `WebView2Loader.dll` next to it, and keeps
  its data in `%LOCALAPPDATA%\OpenGarageMusic\WebView2`. As the player is a process of
  its own, the program starts without that DLL; only the video does not.
- **Words of the song**: the “Words” button in the bar of the bottom panel, left of
  “Video” (when the project has a video) and of “Full window”, shows or hides a band
  of the words under the editor, above the status bar. It is shown at first, only with
  the Editor tab, and goes with the bottom panel; its height is added to the panel
  rather than taken from the editor. View → “Words of the song” has “Shown under the
  panel” and two ways: “The lines being sung” (the line being sung, large, the
  syllables already sung lit, with the line before, faint, and the line after) or “The
  whole text, scrolling” (every line, the one being sung lit, scrolled to it). A click
  on a line takes the song there, and the video too when one plays. The line over the band is
  dragged to make it taller or lower, under the tracks as in the full window. The words come
  from the first of these that has them:
  - *The tablature*: the words of its singing part, each syllable on its note. From
    Songsterr, the words of the singing part are read even when that track is not
    ticked; from a Guitar Pro file (versions 3 to 8), the lyrics of the file, five lines
    each starting at a bar, on the track they belong to; from a MusicXML score, the
    `<lyric>` of the notes, first verse. The rules are those Guitar Pro writes by: each
    syllable goes on the next beat that strikes a note (rests, grace notes and notes
    only tied over take none); a space or a hyphen ends a syllable (“laugh-ter” is
    two); a hyphen or an underscore alone holds the syllable before over one more note;
    two spaces leave a note without words; a bar played again sings its syllables
    again; a line of the text is only a break between lines. The text is written by
    hand and does not always keep these rules: the syllables are fitted to the notes
    over the whole song at once, a line starting after a silence of the singing, a
    space or a hyphen too many taking a note or not, a note left without a syllable
    when a vowel is held; one mistake of the text does not move every syllable after
    it. Vowels held in brackets (“u-po-(o)n”) are read without them (“upon”). The words are then laid out in lines: a line starts at a word written with a
    capital (not “I”, “I'm”, “Lord”, “God”…), after a pause of a bar and a half, or when
    it would grow past about 48 characters. The sung words follow the score, whether the
    song plays as written or with its original sound.
  - *LRCLIB* ([lrclib.net](https://lrclib.net)), a free base of words timed line by
    line, with no account: it is asked by the artist and the title of the project (and
    the length of the recording when there is one, to pick the right version), by
    itself when a tablature or a Guitar Pro or MusicXML file without words becomes a
    project, and with the band's “Search LRCLIB” button. The timed lines follow the
    seconds of the recording that leads (or of the first recording, or the tempo of the
    score when there is none); “◀ 0.1 s” and “0.1 s ▶” shift them.
  - *Yours*: an LRC or text file of the same name beside your recording of the song
    (`Song.lrc` or `Song.txt` next to `Song.mp3`), read when LRCLIB has nothing or when
    a recording is linked to a song without words; a file chosen with “From a file…”;
    or words typed or pasted with “Type…”. Lines that start with a time such as
    `[01:23.45]` are timed, and the `[offset:+250]` of an LRC file is understood.

  The words are kept in the project. Like tablatures, they are mostly under copyright:
  they are there for your own practice.
- **Sounds**: the sound button of a track header opens the library on that sound, in
  its section. A bank downloaded or imported again is filed where it is now asked
  (by family, in its own section, or in one section).
- **Audio devices**: the **Audio** menu chooses the output of all the tracks, the
  input, and the channel of the input that is recorded (input 1, input 2… or all of
  them mixed). A track can also be sent to an output of its own (a headphone amp for
  the drummer, a second sound card…) and an audio track can record an input and a
  channel of its own: right-click the track's header.
- **Quick help**: the “?” button at the right of the transport bar. While it is on,
  the bar at the bottom tells what the pointer is over, and the tips of the buttons
  show at once.
- **Themes**: the program's own look, a dark one and a light one, in View → Theme. A
  theme is a small JSON file; yours can be imported: see [Themes](themes.md).
- **Layout**: the sound controls and the editor sit under the tracks, at a height
  that only changes when its top edge is dragged; “Full window” shows the panel
  alone, “Back under the tracks” puts it back.
- **MIDI Pilot**: File → “MIDI Pilot…”, a window of its own (on a second screen on
  stage) with a large button for each song of a concert or a rehearsal. A list is
  imported from a CSV file (number, artist, song, and the text of the button,
  optional) or written song by song. Choosing a song sends the number of its preset
  by MIDI to a pedal switcher such as The GigRig G3S (control change 0 or program
  change, on the output and the channel set in the window's Settings menu), or, in
  the chain's mode, its own messages (program changes, control changes, system
  exclusive) to the devices of the chain, each on its channel, and,
  with “Open the song's project” ticked, opens its project. The BOSS DD-500 and
  RV-500 have a module (Devices → “Configure…”) that edits their patches over MIDI,
  their program change map and their system, saved in a file and sent back; The GigRig
  G3S has one that edits a backup file of the switcher (banks, presets, MIDI) for its
  editor to restore. An AI given the guide (List → “Guide for an AI…”, `ai-guide.md`,
  also `docs/en/ai-guide.md` for this version)
  writes scripts for all of that, applied with “Apply an AI script…” after a review;
  an agent on the computer can use a local API instead (Settings → “API for an AI…”). A right-click on a song
  gives it a picture of your own or a project, or finds its project, in the
  previous sessions or on a tablature site; “Find the projects of the whole list”
  prepares a concert in one go. The buttons show the covers found on Deezer, which
  can be turned off.
- **Previous sessions**: the program keeps the last projects you worked on by itself,
  without your saving anything. Each project has its session, a folder named by the
  project's identifier that holds it as it was, saved or not, with its recorded
  takes, the arrangement of the window, and a small `about.json` for the list. A
  session is written when the program quits, before another project takes the place
  of the one open (New, Open, reopening a session…), and every two minutes while the
  project changes. A project opened from a file or made from a tablature site, a Guitar
  Pro or a MusicXML file is kept at once, changed or not; only a blank one left
  untouched (the demonstration as it starts, a new one from a template) is not. File → “Previous sessions…” lists them, the
  latest first, one line each: `Artist – Title · N tracks · m:ss · when · file`, the
  time being “just now”, “12 min ago”, “3 h ago” or a date, and the file the path of
  the project file or “never saved in a file”; a project with no artist and no title
  shows its name. “Reopen” or a double-click brings one back, after keeping the one
  open; “Delete” removes one. The window also sets how many are kept (“Keep the last
  [20] projects”, from 5 to 100): the oldest beyond that go, never the one open. It
  shows how many sessions there are and the room they take (and the size of each
  under the pointer), “Open the folder” shows them in the file explorer, and “Change
  the folder…” moves them elsewhere; by default they are in
  `%USERPROFILE%\OpenGarageMusic\Sessions`.
  At start-up the latest session comes back as it was left. With File → Options →
  “Reopen the program as I left it” turned off, the program starts on the
  demonstration project instead, and the projects stay in the list all the same.
  Quitting asks nothing while the session can be written; only when it cannot, and
  the project has changes, does the program offer to save it. The session left by an
  earlier version still comes back at the first start.
- **Artist and title**: a project can have an artist and a title, filled in by the
  tablature sites and by the Guitar Pro and MusicXML files that give them, and changed
  at the top of the Notepad (View → Notepad, on the right). They name the project in
  the list of previous sessions.

Detailed progress and known gaps: [Features](features.md). Origin and licence of
everything that was not written for the project: [External files](external-files.md).
What the author asked for next, and what became of it: [To do](todo.md).

## Running it

```
run.cmd
```

The first build takes a few minutes. The program is then at
`target\release\opengaragemusic.exe`.

## Quick reference

| Action | Gesture |
|---|---|
| Play / stop | Space |
| Back to the start | Enter |
| Record on the selected track | R |
| Loop, metronome | C, K |
| Put the playhead | Click in the ruler |
| Draw the loop area | Drag in the top of the ruler |
| Create a region | Double-click in an instrument or drummer track |
| Move a region | Drag (to another track too) |
| Change its length / loop it | Drag the bottom / the top of its right edge |
| Split, join, loop | Ctrl+T, Ctrl+J, L |
| Cut, copy, paste | Ctrl+X, Ctrl+C, Ctrl+V |
| Put a loop on the tracks | Drag it from the loop browser onto a track, or under the tracks |
| Add a note | Click in the editor |
| Put the notes back on the grid | Q (selected notes, or all of them) |
| Move a point of the equaliser | Drag it; double-click puts its gain back to 0 |
| Select notes | Drag in empty space, Shift+click, Ctrl+A |
| Delete a note | Right-click, or Del on the selection |
| Play on the computer keyboard | Ctrl+K, then the middle row (A S D F…) |
| Library, sound controls, editor, loops | Y, B, E, O |
| Automation | A, then click in a track to put a point |
| Horizontal zoom | Ctrl + wheel, or the slider next to “+ Track” |
| Horizontal scroll | Shift + wheel |
| Undo / redo | Ctrl+Z / Ctrl+Y |
| Move a panel of the sound controls | Drag its title onto another panel: left or right third to go beside it, middle to go above or below |
| Hide / show a panel | Right-click the panel / right-click the background |
| A plug-in's own window | Right-click the plug-in's box, or its “Open the plug-in window” button |

## Plug-ins

| Format | What is supported | Where they are looked for |
|---|---|---|
| CLAP | effects and instruments, sliders, own window, state saved in the project | downloads, imported files, `Common Files\CLAP` |
| VST3 | effects and instruments, sliders (0 to 1, as the format defines them), own window, state saved in the project, 32- or 64-bit processing, every output bus mixed | downloads, imported files, `Common Files\VST3` |
| LV2 | effects and instruments, sliders, own window when the plug-in describes one for Windows (`ui:WindowsUI`), state saved in the project | downloads, imported folders, `Common Files\LV2`, `%APPDATA%\LV2` |

A plug-in starts with the host's sliders. Its own window is opened on request, and
while it is open the sliders give way to it.

**Each plug-in runs in a process of its own** (File → Options, “Run each plug-in in a
process of its own”, on by default). A plug-in is somebody else's program: if it
crashes or stops answering, only its process goes. The effect is bypassed or the
instrument falls silent, the project stays open, and the plug-in's box offers to start
it again. This protects the application from a faulty plug-in; it is not a security
barrier: the plug-in still runs with your rights and is not checked.

## Developing

All the tooling is installed in `.toolchain`, nothing on the system:

```
powershell -ExecutionPolicy Bypass -File tools\setup-toolchain.ps1
dev test --workspace
dev build --release -p opengaragemusic
```

- `crates/og-engine`: project model, sound generators, effects, drummer, loops,
  sequencer and mixer, reading and writing WAV and MIDI. No dependency on the interface.
- `apps/opengarage`: the application (egui for the interface, cpal for sound, midir for
  MIDI, the CLAP, LV2 and VST3 hosts, compressed export, the Songsterr client,
  translations in `lang`).
- `apps/og-video`: the video player, `og-video.exe`, a process of its own that shows
  YouTube's player in a web view (wry, WebView2) inside the program's window.
- `apps/midi-pilot`: `MidiPilot.exe`, a launcher that starts the program on its MIDI
  Pilot window alone (`--midi-pilot`), for the stage.
- `tools/og-clap-test`: one small library that is at once a CLAP file, an LV2 program
  and a VST3 file, with the same two test plug-ins in each format. The hosts are
  tested against it.
- `tools/screenshot.ps1`: opens the application on a given view and saves a picture of
  its window, to check the rendering. `OPENGARAGE_LANG=de` picks a language for one
  run, `OPENGARAGE_THEME=light` a theme.

The rules of the project are in [Contributing](contributing.md).
