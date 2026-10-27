# To do

Requests of the author, noted on 13 and 14 October 2026, and what became
of them.

1. **Two-line mode** for the Score and Tab views of the editor — done: the “Two lines”
   box of the editor, off by default.
2. **Editor in full window** running over several lines, as Songsterr lays a song out —
   done for the score and for the tablature alike: the whole region, line after line,
   the view following the song.
3. **Better score rendering** — done: key and time signatures, every note value with
   dots, beams and triplets, rests, shared stems, and the annotations the sites give
   (slides, bends, hammer-ons, vibrato, palm mute, let ring…, chord names, sections).
4. **Importing from other sites** — done in 0.9.7: File → “Import from a tablature
   site…” searches Guitariff and Tablature Hub in the program, as it does Songsterr,
   and reads the Guitar Pro file of the chosen song; Ultimate Guitar, whose terms
   forbid it, is only opened (see [External files](external-files.md)). The entry is
   now File → “New from a tablature site…”.
5. **Import of Guitar Pro files** — done: File → “New from a Guitar Pro file…”, a file
   dropped onto the window, or the button of the sites window.
6. **Several tracks in the editor** (asked on 14 October 2026) — done in 0.9.7: Ctrl and a
   click on track headers, the tracks one under the other, and the “Auto-switch”
   button that follows one at a time (the first chosen first, then the next one that
   plays, as the last note of the one shown is struck, when nothing more comes on it
   within the bars set in View → “Auto-switch after”).

## Requests of 14 October 2026, done in 0.9.9

7. **Loading screen** — done: the picture supplied by the author, at half its size, the
   version and a bar that follows the start of the program; it stays two seconds at
   least.
8. **Extensible engine for the tablature sites** — done: each site is described by a
   JSON file (how to search it, how to cut its answers into songs and tracks, how to
   follow its pages to the file, how to show it), with regular expressions and JSON
   paths and no code. Songsterr, Guitariff, Tablature Hub and Ultimate Guitar are
   described so; the user adds or replaces sites in the `sites` folder next to the
   program, on their own responsibility, and the program asks once before reading a
   site. See [Describing a tablature site](sites.md).
9. **MusicXML import** — done: `.musicxml`, `.xml` and `.mxl`, from the same menu as
   Guitar Pro and by dropping a file, with the strings and frets of a part written as
   tablature.
10. **Guitar Pro 6 import (`.gpx`)** — done: its container is opened and its score read
    by the reader of GP7 and GP8.
11. **Review of the GP3, GP4 and GP5 reader** — done against PyGuitarPro's description
    of the formats and 102 real files: no GP5 file was read before (a ninth text in
    the header, no line break after the last bar, the size of a mixer sound in 5.00);
    artificial and tapped harmonics now sound at their pitch.

## Found by the review of the Guitar Pro reader, done on 15 October 2026

12. **Jumps of the score** — done: D.C., D.S. and D.S.S., al fine, al coda and al double
    coda, the segno, the coda and "to coda" signs, as GP5 files (in their header),
    GP6 to GP8 files and MusicXML scores write them. The order of play follows them:
    the repeats up to the jump, then the passage once, on its last endings, to the
    fine or over to the coda.
13. **Swing (triplet feel)** — done: written for the whole song in GP3 and GP4, per bar
    in GP5 to GP8 and in MusicXML, in eighth notes or sixteenths. The notes stay
    written straight and are played swung; the tablature and the score write
    "Swing (eighths)" or "Straight" where the feel changes.
14. **Tremolo picking, trills and grace notes as written** — done: tremolo picking is
    played at its speed (eighth notes, sixteenths, thirty-seconds) and drawn with
    its slashes; a trill alternates with its other note at its speed and is marked
    "tr"; a grace note has its written length, before the beat or on it, the note
    then coming that much later.

## Asked on 15 October 2026

15. **Loop beats from the editor** — done in 0.9.9: Shift and a click on the beats of
    the score or the tablature, shaded as the pointer passes over them with Shift
    held, and the “Loop the beats” button of the editor, in the panel or the full
    window.
16. **Icons of the tracks** — done in 0.9.9: the pictogram of a track is that of the
    instrument it plays, found by the program (twelve kinds, drawn by Game Icons) and
    no longer chosen by hand; a click on it chooses the colour of the track.
17. **Icon on the task bar** — done in 0.9.9: the blue guitar shows on the Windows task
    bar too, from the loading screen on.
18. **Zoom of the note editors** — done in 0.9.9: from ten bars a line, at the least
    zoom, to four, at the most, in the piano roll, the score and the tablature.
19. **Previous sessions, File menu, MIDI export** — done: the program keeps the last
    projects worked on (20 by default, from 5 to 100), saved or not, with their takes
    and the arrangement of the window, without the user saving anything, and File →
    “Previous sessions…” lists them, reopens or deletes them, and moves their folder.
    The File menu is grouped into new projects (“New…”, “New from a tablature
    site…”, “New from a Guitar Pro or MusicXML file…”), opening, saving, and two
    windows: “Import into this project…” for an audio or MIDI file, “Export to…” for
    the formats, each with a few words on what it is for. MIDI joins them: the notes
    of the tracks as they are played, in a Standard MIDI File. A project has an
    artist and a title, filled in by the imports and changed in the Notepad, which
    name it in the list of sessions. A project opened or imported is kept at once,
    changed or not. The descriptions of the tablature sites say the version of their
    language (`format_version`), and one written for a newer program is not read; see
    [Describing a tablature site](sites.md#versions). The task bar shows the icon of
    the program: the window was asking the executable for it under a wrong number.
20. **Zoom of the tablature, full window, chord names** — done: the slider and Ctrl with
    the wheel zoom the tablature, from as many bars as its fret numbers can be read at
    (ten at most) to four beats, in its panel and in the full window; the beat under
    the pointer stays there. In the full window, the view goes at once to the line
    being played. Chord names written with ♭, ♯, Δ, ° or ø are understood and shown
    with letters, and more kinds of chord have a grid (7sus4, 7sus2, 9, m9, maj9,
    madd9, mmaj7, 7#5, 7b5).
21. **Playing a song with its original sound** — done, in four parts. The base: a
    recording of the song is a track (an audio track, or a new kind, the video track)
    with the second each beat of the score falls at in it, and the “Original sound”
    button of the transport bar plays the song either as written, at the tempo of the
    score, or at the pace of the recording that leads, which is heard while the other
    tracks are silent unless their M is turned off. An audio file of the user's: File
    → “Import into this project…” → “A recording of the song…” finds it in step with
    the score by itself, in the background, comparing the notes that sound in both,
    and says how alike they are. A music library: a folder of the user's, read with
    the tags of its files, where the recording of a song chosen on a tablature site is
    looked for and offered. A YouTube video from Songsterr: “YouTube video in step
    with the tablature” takes the video of the song with the second each bar starts
    at, which the site's users set, and YouTube's own player plays it in step with the
    song, in the editor or in a corner, through a small program of its own,
    `og-video.exe`; nothing is downloaded. The language of the descriptions of the
    tablature sites goes to version 2, which adds `videos`, and the description of
    Songsterr uses it; see
    [Describing a tablature site](sites.md#videos). While the video plays, the playhead moves
    wherever it is clicked (the ruler, a click on nothing in the tablature, Alt and a
    click in the score or the piano roll, or YouTube's own bar), and the video goes
    there with it. The line between the tracks shown together in the editor is dragged
    to share its height otherwise.
22. **The words of the song** — done: a band under the editor, shown or hidden with
    the “Words” button of the bottom panel, sings the words with the song, either the
    line being sung, large, its syllables lit as they are sung, or the whole text
    scrolling to it (View → “Words of the song”); a click on a line takes the song,
    and the video, there. The words come first from the tablature, each syllable on
    its note: the singing part of Songsterr, read even when it is not ticked, the
    lyrics of Guitar Pro files (versions 3 to 8) and the `<lyric>` of MusicXML
    scores, placed as Guitar Pro places them and laid out in lines. Otherwise from
    LRCLIB, a free base of words timed line by line, asked by the artist and the
    title. Otherwise from the user: an LRC or text file beside the recording, a file
    chosen, or words typed. The window of the tablature sites has the box “The words
    of the song”, ticked at first, and the description of Songsterr gives its tracks
    a field `vocal`, read from `isVocalTrack`. The words are kept in the project.

## Asked on 17 October 2026, done in 0.9.10

23. **Stop with the video** — done: a video started from YouTube's own player no longer
    starts the song again when Stop is pressed; and the keyboard always stays the
    program's (the field of the tablature search kept it no longer when YouTube's page
    loaded).
24. **Choosing among the videos of a song** — done: when Songsterr gives several, they
    are all kept in the video track, and a list at the top of its editor plays another.
25. **Set List** (from the author's program G3S Pilot; named MIDI Pilot since the
    0.9.18) — done: File → “Set List…”, the
    songs of a concert in a window of their own, read from the CSV file of G3S Pilot or
    written by hand; a song chosen sends its preset number by MIDI to the pedal
    switcher (control change 0 or program change) and, when asked, opens its project;
    projects linked by hand or found in the previous sessions or on a tablature site;
    covers from Deezer, or a picture of the user's from a right-click. See
    [Features](features.md).

## Asked on 17 October 2026, done in 0.9.15

26. **Zoom of the editors on any screen** — done: the zoom is sideways only and
    without end either way, set with "-" and "+" around its value at the bottom right
    (a click on the value: four bars again), the same pixels a beat on every track and
    song, and kept in the settings; the first fret number of a tablature is no longer
    cut at the left edge.
27. **A review of the code** — done: no reference to the author or to a private
    server is left in the code or the documents; eleven faults found by the review
    are corrected (“Save as” into another folder losing the audio, a recording being
    aligned landing on the next project, the set list's edit window after a removal,
    download threads that died leaving the window waiting, the words beside a saved
    recording never found, “major”/“minor” untranslated, Q quantizing the wrong track
    with several in the editor, the sliders not following a CLAP plug-in's own
    window, the tempo bounds, a Guitar Pro file able to crash the reader, and a VST3
    file shut down without `ExitDll`).

## Asked on 17 October 2026, done in 0.9.16

28. **The review's leftovers** — done: a take no longer allocates in the input
    callback (the callback fills chunks the application lends and hands them back
    without waiting; sound lost for want of a chunk is counted and told); the
    plug-in's process talks over a connection of the computer alone (a socket on
    127.0.0.1) instead of its standard output, which a plug-in may print to as it
    likes, and through a writing thread of its own, so the audio thread never waits
    for the connection; the code of the old Songsterr window, the helpers written
    twice (hex, hashes, RIFF chunks, extensions, folders beside the program, the
    colours of the theme), the file dialogs and the radio lists of the menus are
    each in one place; the plug-in hosts share their limits and the loading of the
    files; the score draws only the lines in view and finds once what every line
    shares. The two versions of the `windows` crate stay: the sound card and MIDI
    libraries each need their own, and moving the program to the video player's
    would add a third.

## Asked on 17 October 2026, done in 0.9.17

29. **Effect engines built into the program, and their files from the sites** —
    done. Besides the plug-ins, the effects of a track have engines of their own, in
    the chain of the track (pedals → amp → equaliser → compressor). Two kinds:
    - *Engines that read a file the user brings*, each with its folder in the
      library (`Captures`, `Impulsions`):
      1. **Amplifier captures, Neural Amp Modeler** (`.nam`, Architectures 1 and 2,
         read by the `nam-rs` crate), **Proteus and AIDA-X** (`.json`, the LSTM
         read by the program): the “Capture…” button of the “Amp” block; the capture
         takes the place of the built-in drive stage, the gain knob sets its level,
         the tone knobs stay behind as an equaliser, the speaker filter only when the
         capture has no cabinet (`gear_type`). Run in the process, on the audio thread,
         at the capture's own rate through a windowed-sinc resampler when the card or
         the export runs elsewhere; a worker thread a captured track stays in reserve
         should the load call for it. Captures cover pedals too.
      2. **Convolution by impulse response** (`.wav`, `.flac`): one engine, two uses:
         the cabinet of the amp (“Cabinet IR…”, in place of the speaker filter) and a
         convolution reverb (“Room IR…” of the master, in place of the built-in one).
         Uniformly partitioned convolution by FFT (the `rustfft` crate), impulses up
         to five seconds.
      The file is kept by its path, relative, copied beside the project on “Save
      as”; the program ships none. A tiny model and a tiny impulse generated by the
      tests check loading, processing and saving.
    - *Engines made of code alone, nothing to download*: a noise gate, a wah (its
      pedal the first knob, which automation moves), an auto-wah, a flanger, a tape
      echo and an analog (bucket-brigade) echo; the overdrive and the amp's valve
      stage through antiderivative antialiasing; Dattorro's plate as a second shared
      reverb beside the room. Written from the published references (Zavalishin,
      Parker, Dattorro, Smith) with no GPL code; circuit models of named pedals
      (wave digital filters) are left for later.
    - *Downloading from the sites*, every time through the descriptions of the
      sites (`crates/og-engine/sites/*.json`), never by code of a site's own: the
      format of the descriptions is at `format_version` 3, with what a site gives
      (`"gives": "tablature" | "capture" | "impulse"`), the readers `capture` and
      `impulse`, steps whose page lists several files (`items`), and a `login` by the
      device flow of OAuth 2 for a site that asks for an account; a site whose search
      asks nothing of the query lists what it has, and a word sifts the list. The
      window of the sites lists them by kind and, for a site that gives files, lists
      the files of the one chosen and downloads the ticked ones into the library
      with where they came from. TONE3000 (captures; a second description for its
      impulse responses) is read with the user's account: its API terms for free
      integrations allow the favourites, the trending and the latest, downloaded one
      by one, and ask each program for a key the user makes in their account, which
      the program keeps; GuitarML's ToneLibrary (Proteus) needs no account. OpenAIR
      was not added: its site is suspended.

## Asked on 24 October 2026, done in 0.9.18

30. **A project in one file, saved in the documents, lighter** — done. A `.ogproj`
    is now an archive (ZIP) holding `project.json` and, under `media/`, the takes
    and the files of the effects that came with the project; it travels as one
    file. The project keeps working from its own files (a take still in the
    temporary folder moves to the session's at save), and a file of the user's
    elsewhere on the disk stays named by its path. Projects written before, JSON
    with a folder beside them, still open. Projects are saved in the
    `OpenGarageMusic` folder of the user's documents, or where File → Options →
    “Projects folder…” says. At save, the audio no region plays any more, not even
    as another take, is forgotten (it used to go with the project for nothing); a
    take can be forgotten from the “Takes” menu of a region. The takes were already
    written without loss, as 24-bit WAV.

31. **MIDI Pilot** — done: the Set List is named MIDI Pilot (menu, window, documents,
    its folder in the user's folder, the old one kept when it is there), and
    `MidiPilot.exe`, next to the program, opens that window alone, for the stage:
    the program started with `--midi-pilot`, with no sound card, no loading screen
    and no project; a song whose project is to be opened starts the whole program
    on it (a project file given on the command line opens at start). Two windows of
    the program at once are given up; importing the tracks of another project is
    the way to copy between projects, to do when wanted.

## Asked on 25 October 2026, done in 0.9.19

32. **MIDI Pilot: the chain's mode** — done: two modes under Settings → Mode. The
    switcher's, as before (one number a song, bank select or program change). The
    chain's: the devices of the chain (Settings → “Devices…”: name, model, channel),
    and each song its own messages (program change with or without bank select,
    control change, system exclusive in hexadecimal), in order, with a pause after
    each, edited with “Change…” and tried with “Send now”; a CSV column named after a
    device gives the program it receives, the export writes those columns; the bytes
    sent are shown when the status line is hovered. Next: the modules of the pedals
    (BOSS DD-500 and RV-500 by SysEx, from the public BOSS documents: patches, PC
    map, system, a file saved and loaded, the BOSS editor's backups read), then the
    G3S (a `.g3b` backup written for the vendor's editor to restore: banks,
    presets, loops, MIDI lines, globals).

33. **MIDI Pilot: the modules of the BOSS DD-500 and RV-500** — done: Devices →
    “Configure…” on a device of that model talks to the pedal by system exclusive
    over the MIDI output and the MIDI input chosen, from a description file written
    from the maker's public document (`devices/dd500.json`, `rv500.json`, a folder
    `devices` next to the program for others): memories and their names, the patch
    chosen edited parameter by parameter and heard at once, stored after a backup
    and a read-back, the program change map, the system, a file saved and loaded,
    every memory read, the file sent back, the BOSS editors' backups read. Next:
    the G3S (a `.g3b` backup written for the vendor's editor to restore).

34. **MIDI Pilot: the module of The GigRig G3S** — done: Devices → “Configure…” on a
    device of that model edits a backup file of the switcher (`.g3b`): banks (added,
    removed, moved, named, the song mode), the 28 presets of each, and for the preset
    chosen its name, loops, order of the loops and MIDI messages; banks named after
    the songs and songs from the banks; “Check” as the switcher checks; the file
    written for the switcher's editor to restore. The format, unpublished, was
    deduced from the switcher's own backups (see External files).

35. **MIDI Pilot: scripts for an AI** — done: a JSON format of actions (memories of the
    BOSS pedals set with the values they show, program change map, system, G3S banks
    and presets), the guide `ai-guide.md` written by the program from the descriptions
    (List → “Guide for an AI…”, `--ai-guide`, in the archive), “Apply an AI script…”
    with a review of each action before the pedals are written, “Export for an AI…”
    on a patch, a module's file or a G3S backup. Next: a local REST API for agents on
    the computer (same format), then perhaps an MCP bridge.

36. **MIDI Pilot: a local API for an AI's agent** — done: Settings → “API for an AI…”
    (port on 127.0.0.1, token, unattended writes or the user's Yes in the window),
    routes that read and write the pedals and the G3S backup in the format of the
    scripts, one request at a time, documented in the guide. Perhaps later: an MCP
    bridge over it.

37. **MIDI Pilot: the songs in the AI scripts** — done: `song` actions add, change
    and remove the songs of the list (number, text of the button, messages to the
    devices of the chain), List → “Export the songs for an AI…” and `GET /songs`
    write them back; in the guide.

38. **MIDI Pilot: the pedalboard described for an AI** — done: in the G3S page, the
    pedals of each loop (name, kind, the device of the chain when MIDI), the signal
    order and the bank switches, kept in the settings and shown in the presets; what
    the pedal modules read is kept in the program's data; List → “Export the
    pedalboard for an AI…” and `GET /pedalboard` write one file (the chain, the
    switcher, and every setting known as actions); `{ "g3s": "loops" }` in the scripts.
