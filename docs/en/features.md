# Features: target and progress

*[Version française](../fr/features.md)*

The list follows the chapters of the public user guide of GarageBand for Mac (table of
contents consulted on 11 October 2026), put in our own words. It serves as a roadmap.

Legend: **done** · **partial** · **to do**

**Count: 94 lines “done” out of 99, or 94 %.** “Partial” lines are not counted. This
count measures whether features are present, not how deep they go: see “Known gaps” at
the bottom of the page.

## Projects (7 of 7)

| Feature | State |
|---|---|
| Create, open, save a project | done (`.ogproj` format: one file, an archive holding the project as JSON and, under `media/`, the audio of its takes and the files of its effects that came with it, so that a project travels as one file; a file of the user's elsewhere on the disk, such as the recording of the song from the music library, stays where it is, named by its path; projects written before, JSON with their audio in a folder beside them, still open. Saved in the `OpenGarageMusic` folder of the user's documents unless File → Options → “Projects folder…” says elsewhere. At save, the audio no region plays any more, not even as another take, goes; in the editor, the “Takes” menu of an audio region forgets a take with its ✕) |
| Tempo | done |
| Time signature | done (1 to 16 beats, over 4 or over 8) |
| Key and scale of the project | done (loops follow it) |
| End point of the project | done (automatic or set to a bar) |
| Master volume | done |
| Project templates | done (7 templates) |

## Playback and navigation (7 of 7)

| Feature | State |
|---|---|
| Play, stop, back to the start | done |
| Move the playhead in the ruler | done |
| Loop area | done (drawn in the top of the ruler) |
| Following the playhead | done (page by page) |
| Zoom and scrolling of the track area | done |
| Metronome | done |
| Count-in before recording | done (one bar) |

## Tracks (10 of 10)

| Feature | State |
|---|---|
| Create, rename, delete, duplicate a track | done |
| Mute, solo, volume, pan | done |
| Reorder tracks | done (move up, move down) |
| Track icon and colour | done (the icon is that of the instrument the track plays, found from its kind and its sound: piano, synthesizer, bass, guitar, strings, brass, winds, mallets, drums, voice, audio or video, drawn by Game Icons; a click on it chooses one of 8 colours) |
| Lock a track | done |
| Software instrument track | done |
| Audio track (microphone, line) | done |
| Guitar / bass track with an amp | done |
| Virtual drummer track | done |
| Master track with effects | done (volume that can be automated, equaliser, compressor, limiter) |

## Sounds (6 of 6)

| Feature | State |
|---|---|
| Choose a sound per track | done (36 built-in sounds) |
| Sound library with categories and search | done |
| Save one's own sound settings | done (“My sounds”) |
| Simplified controls per sound (knobs) | done (panels arranged in rows, that can be moved and hidden) |
| Sampled instruments (piano, drums…) | done: SoundFont banks (.sf2, .sf3) and SFZ instruments (.sfz), imported from the disk or downloaded from the Library menu; no bank is shipped |
| Arpeggiator | done (written, recorded or live notes) |

## Recording (2 of 5)

| Feature | State |
|---|---|
| Play on the computer keyboard | done |
| Record what is played into a region | done |
| External MIDI keyboard | partial (written, never tried with a device) |
| Audio recording | partial (multiple takes in a loop, input monitoring; checked by tests without a sound card, never tried with a microphone) |
| Tuner | partial (pitch detection checked by a test, never tried with a microphone) |

## Regions and arrangement (9 of 9)

| Feature | State |
|---|---|
| Create, select, move, resize, delete | done (dragged with Alt or Ctrl held, a region or a note is copied) |
| Snapping to the grid | done |
| Rename a region | done |
| Cut, copy, paste | done |
| Split, join | done |
| Loop a region | done |
| Move a region to another track | done |
| Arrangement markers (intro, verse, chorus) | done (move, repeat or delete a section with its contents) |
| Tempo track, transposition track | done |

## Editors (12 of 12)

| Feature | State |
|---|---|
| Note editor: add, move, lengthen, delete | done |
| Quantizing | done (from whole notes to thirty-second notes) |
| Transposition | done (semitone or octave) |
| Note velocity | done |
| Multiple selection of notes | done |
| Score editor | done (reading and simple editing; no printing. Treble and bass staves with the key signature and the time signature; the notes with the accidentals the key calls for, their values from the whole note to the sixty-fourth, with dots, beams over the eighths and shorter notes of a beat, a 3 over the triplets, one stem for the notes struck together, ties for the notes that cross a bar line or last no single value, brackets over the triplets, and rests where nothing sounds; the names of the chords and of the sections over the staff, and what a tablature writes around its notes drawn here too) |
| Tablature | done (for a track that plays a guitar or a bass: the notes as fret numbers on the strings, read-only; a click selects a note. The strings are the track's own tuning when it has one, as a song imported from Songsterr gives it with the string of each note, else the standard tuning. Chord names run along the top: the ones the tablature carries, or else the ones the notes make; over each, a chord grid shows where the fingers go: what the strings play there when it shows the chord within four frets, or else a usual way to play it; the chord being played stays at the left edge of the view while it lasts. A slider sets the size of the fret numbers, the chord names and the grids. A beat is always wide enough for the fret numbers of a string not to overlap: the least zoom shows as many bars as can be read so, ten at most, and the most zoom four beats. Chord names written with the signs of printed music (B♭7, F♯m, CΔ7, Cø…) are understood and written with letters (Bb7, F#m). What a tablature writes around its notes is drawn too: slides, hammer-ons and pull-offs, bends with their height, vibrato, harmonics, dead and ghost notes, staccato, accents, palm muting and let ring, the slashes of tremolo picking and the "tr" of trills; under the strings, where the swing starts and stops; the names of the sections of the song run over the chords) |
| Score and tablature over several lines | done (the “Two lines” box, off at first, lays them over two lines of whole bars, as on a page: the next two come when both are played, or with the wheel. The full window lays the whole region out that way, over as many lines as it takes, and scrolls to the line being played) |
| Several tracks in the editor | done (Ctrl and a click on track headers put several tracks in the editor, one under the other, in the order chosen; a note selected in another track's editor brings that track to the front. A track deleted leaves the editor, and none takes its place; a track moved stays in it. The “Auto-switch” button shows one track at a time: the one being played, the first chosen having priority; when nothing more comes on the one shown within the bars set in View → “Auto-switch after” (1, 2, 4 or 8), the next one that plays takes its place as its last note is struck) |
| Looping beats from the score or the tablature | done (Shift and a click on a beat, then on another, choose the beats between them, shaded; with Shift held, the beats a click would choose are shaded under the pointer. “Loop the beats”, in the editor's bar, sets the loop area of the song to them and plays; again, the loop stops. In the panel or the full window, for each track of the editor; Escape lets the beats go. In the tablature, Ctrl and a click now add a note to the selection) |
| Audio editor, pitch and timing correction | done (transposition without changing the length, tuning, snapping attacks to the grid) |
| Virtual drummer editor | done |

## Loops and files (6 of 7)

| Feature | State |
|---|---|
| Loop browser with filters, search, favourites, listening | done (42 loops) |
| Loops adapt to the tempo and the key | done |
| Import of audio files | done (WAV, MP3, AAC/M4A; from the window of File → “Import into this project…”, onto the selected audio track or a new one, at the playhead, or dropped from the file explorer onto the tracks, like MIDI files, sound banks and projects; the same window finds a recording of the song in step with the score: see “Beyond the guide”) |
| Import of MIDI files | done (from the same window, its tracks of notes as new tracks, or dropped onto the window) |
| A new project from a Guitar Pro file | done (GP3, GP4 and GP5 files, the GP6 ones (.gpx) and the GP7 and GP8 ones (.gp): one track a part, with the tuning and the string of each note, the tempo and its changes, the time signature, the key, the sections, the repeats unfolded, the chord names and what the tablature writes around its notes, as the Songsterr import gives them; the jumps of the score (D.C., D.S., coda, fine) followed in the order of play, the swing of the song or of each bar played, tremolo picking and trills at their speed, grace notes at their length, before the beat or on it; the instrument of each track from an installed General MIDI bank, or a built-in sound of its family. From the File menu, from the window of the tablature sites, or dropped from the file explorer) |
| A new project from a MusicXML score | done (`.musicxml` and `.xml` files, partwise or timewise, and `.mxl` archives, the format most score editors exchange: one track a part, with the notes, ties, repeats and alternate endings, tempo, time signature, key, sections, chord names, dynamics, and the string, fret and techniques of a part written as tablature, whose tuning it keeps; where a part has both a staff of notes and a staff of tablature, only the tablature is read. From the same menu entry as Guitar Pro, or dropped from the file explorer) |
| Import from other tablature sites | done (File → “New from a tablature site…” is one window for Songsterr, Guitariff and Tablature Hub: the site to search chosen at the top, then the same search, list of songs and choice of the tracks for all three. On Guitariff and Tablature Hub the program searches the site, fetches the Guitar Pro file of the chosen song in the background and lists its parts, each with a check box; “Create the project” makes the project of the ticked ones. Ultimate Guitar is only opened in the browser: its terms forbid programs. The window also says in a sentence what each site offers, opens it, and keeps the “Import a Guitar Pro file…” button. See [External files](external-files.md)) |
| Tablature sites described by files | done (each site of the window is described by a JSON file: how to search it, how to cut its answers into songs and tracks, how to follow its pages to the file, and which reader reads it, with regular expressions and JSON paths and no code; the four sites shipped are described so, and the user adds or replaces sites in the `sites` folder next to the program; the program asks once before reading a site, asks only `https://` addresses and bounds every count. See [Describing a tablature site](sites.md)) |
| A loop dragged from the browser into the tracks | done (onto a track that can take it, or under the tracks, where a track is made for it; it lands on the bar under the pointer) |
| Loops of recorded sound | done for the user's own files (WAV, MP3, AAC, FLAC, Ogg), kept in a folder of the program and stretched to the tempo of the project; a tempo in the file's name (“Funk 98 bpm”) is used, otherwise it is worked out from the length. **No recorded loop is shipped** |
| Soundtrack of a movie | to do |

## Mixing and effects (5 of 6)

| Feature | State |
|---|---|
| Volume and pan per track | done |
| Level meters | done (per track and on the output) |
| Equaliser, compressor, reverb, echo | done (the equaliser is a curve with three points to drag: sideways for the frequency, up and down for the gain; the shared reverb is a room, or Dattorro's plate, or an impulse response of a room, a plate or a spring of your own, âRoom IRâ¦â in the Master panel, by convolution) |
| Automation | done (volume and pan per track, master volume; reverb, echo, compressor, the three bands of the equaliser, the amp's gain and the pedals of each track) |
| Amps and effect pedals | done (4 amps, and a capture of a real amplifier or pedal in place of the amp's drive stage, âCaptureâ¦â in the Amp panel: a `.nam` file of Neural Amp Modeler (Architectures 1 and 2) or a `.json` one of Proteus or AIDA-X, run at its own rate whatever the card's, the gain knob setting its level, the tone knobs behind it and the speaker filter unless the capture holds its cabinet; an impulse response of a cabinet in place of the speaker filter, âCabinet IRâ¦â; 11 pedals: overdrive, noise gate, wah (its pedal the first knob, which automation moves), auto-wah, chorus, flanger, tremolo, phaser, echo, tape echo, analog echo; the pedals are chosen one by one into a chain, on any track) |
| Captures and impulse responses from the sites | done (Library â âCaptures and impulse responses from the sitesâ¦â: the window of the sites lists them by kind. TONE3000, captures and impulse responses, is read with your account: the program asks it for a code you enter on the site, signed in, and keeps the token; its terms for free programs ask each for a key you make in your account, and allow your favourites, the trending and the latest, downloaded one by one. GuitarML's ToneLibrary (Proteus captures) needs no account. The files go into the library's folders `Captures` and `Impulsions`, with where they came from and their licence) |
| Third-party plug-ins | partial (effects and instruments in the CLAP, LV2 and VST3 formats, imported, found in the standard folders or downloaded from the Library menu; sliders, own window and saved state — see “External formats”; only ever run with the project's test plug-ins, never with a real third-party plug-in) |

## Sharing (3 of 3)

| Feature | State |
|---|---|
| WAV and AIFF export | done (16 or 24 bits, 44.1 kHz; File → “Export to…” opens a window that lists the formats, each with a few words on what it is for; the whole song is exported, from its start to its end marker) |
| Compressed export (MP3, AAC) | done on Windows (encoders of the system; 128, 192, 256 or 320 kb/s, the last for MP3 only, chosen in the same window when MP3 or AAC is) |
| MIDI export | done (in the same window: a Standard MIDI File (`.mid`) of format 1 that holds the notes of the tracks as they are played, swing, tremolo picking and trills included, and no sound; one track for each track that has notes, the tempo and its changes, the time signature, the General MIDI program of a track that plays a sampled preset, the drum and drummer tracks on channel 10) |

## Beyond the guide

| Feature | State |
|---|---|
| Interface in several languages | done (English by default, French, German, Spanish, Italian, Portuguese; one language = one gettext `.po` file) |
| Download of freely licensed sounds and plug-ins | done (download window, which marks what is already downloaded) |
| Choice of the audio output and input; an output or an input per track | done (tracks sent to another output bypass the effects of the main output; two sound cards never run at exactly the same speed, so a second output may drop or repeat a few milliseconds now and then; one input is open at a time) |
| The program reopens as it was left | done (the latest session: project, saved or not, and arrangement of the window; can be turned off, in which case the program starts on the demonstration project; quitting asks nothing, unless the session cannot be written and the project has changes) |
| Previous sessions | done (the last projects worked on, 20 by default, from 5 to 100, kept without saving: a folder each, with the project, saved or not, its takes and the arrangement of the window, written on quitting, before another project replaces the one open, and every two minutes while it changes; a project opened or imported is kept at once, changed or not, and only a blank one left untouched (the demonstration as it starts, a new one from a template) is not. File → “Previous sessions…” lists them, the latest first, by artist and title, tracks, length, time and file, reopens one (after keeping the one open) or deletes one, sets how many are kept, shows the room they take, opens their folder and moves them to another; by default `%USERPROFILE%\OpenGarageMusic\Sessions`. The artist and the title of a project come from the tablature sites and the files imported, and are changed at the top of the Notepad) |
| Recent projects | done (File menu and start window; the ten latest) |
| Position in minutes and seconds | done (next to bars and beats) |
| Channel of the sound card that is recorded | done (Audio menu for the program, track menu for one track; all channels mixed by default) |
| Audio menu | done (output of all the tracks, input, channel) |
| Q quantizes the notes of the editor | done |
| Zoom of the note editors | done (at the bottom right of the piano roll, the score and the tablature: “-”, the zoom as a percentage (100 % is 40 pixels a beat), and “+”, each press a quarter further out or in, as far as wanted either way; a click on the value puts the zoom back to four bars across (four beats in the tablature), which is how an editor starts. Ctrl and the wheel zoom too, around the pointer. The zoom is the pixels a beat, the same on every track and every song, and kept in the settings. It is sideways only: the height of the notes and the size of the text do not follow it, the tablature having its own “Text size”; zoomed far out, the fret numbers of a tablature may run into one another. The first beat of a tablature stands a little in from the left, so that its fret number is whole) |
| Drummer that follows another track | done (its kick drum plays where the notes of the chosen track begin, moved to the nearest sixteenth; the rest of the kit keeps the style's pattern) |
| Quick help | done (the “?” button: the status bar tells what the pointer is over, by area of the window, and the tips of the buttons show at once) |
| Themes | done (the program's own look, a dark one and a light one; a theme is a JSON file, the user's own can be imported — see [Themes](themes.md)) |
| A new project from a Songsterr tablature | done (in the window of File → “New from a tablature site…”, with Songsterr chosen: search on songsterr.com, choice of the tracks, one track of notes each; tempo and its changes, time signature, key, sections, repeats unfolded; the instrument of each track from an installed General MIDI bank, or a built-in sound of its family; the tuning and the string of each note; slides, bends, hammer-ons and pull-offs, vibrato, harmonics, dead and ghost notes, staccato, accents, palm muting, let ring and the tremolo bar, shown on the tablature and played: bends, slides and vibrato move the pitch of the note while it sounds, hammered notes are softer, palm-muted and staccato notes shorter, let ring longer; no account is used, only what the site shows to anybody) |
| Play with the original sound (recording that leads, tempo from it, tracks played along) | done (a recording of the song is a track: an audio track holding an audio file of the user's, or a video track holding a YouTube video, each with its timing, the second each beat of the score falls at in it. The “Original sound” button of the transport bar, shown when the project has a recording, switches at any time between two ways of playing: off, the song plays as written, at the tempo of the score, the recordings silent; on, it plays at the pace of the recording that leads, the tempo taken from its timing bar by bar, so that a band that speeds up or drags is followed, the recording is heard, and the other tracks are silent unless their M is turned off: in this mode M means “silent while playing along”, and turning it off plays that track over the recording. No count-in in this mode; the editors scroll with the song as usual. With several recordings, the header of each says which one leads, and a click on another makes it lead) |
| A recording found in step with the score (audio file) | done (File → “Import into this project…” → “A recording of the song…”, for an MP3, OGG, FLAC, M4A/AAC or WAV file. In the background, the program compares twenty times a second which notes of the scale sound in the score, from its notes, and in the recording, from its spectrum over about a third of a second, and where notes are struck, drums left out; dynamic time warping pairs the two, so the score may start anywhere in the recording (an intro, silence) and its pace may change. Four minutes of song take about two seconds. The program says how alike the two are, in %, and warns under 30 % that it may not be the same song or version. The recording goes on an audio track of its own, which leads, and the song switches to the original sound at once; the editor of the track shifts it against the score by a tenth of a second (“◀ 0.1 s”, “0.1 s ▶”). The file stays where it is: the project refers to it by its path, it is not copied, and nothing is uploaded) |
| Music library | done (File → Options → “Music library folder…”: the program reads that folder and its subfolders in the background, MP3, OGG, FLAC, M4A, AAC, MP4 and WAV files, up to 100 000, each known by the artist and title of its tags (ID3, Vorbis comments, MP4) or else by its name (“Artist - Title.mp3”, a leading track number skipped). The index is kept beside the settings (`library.json`), and only the files that changed are read again. Names are compared plainly: case, accents, what is in brackets (“(Remastered 2011)”, “[Live]”), “feat.” and what follows, a leading “The” do not count. When a song is chosen in the window of the tablature sites, its recording, if the library has it, is offered in the “Original sound” section of the tracks column (“Your recording: …”, ticked; ✕ refuses it); otherwise “A recording of the song…” chooses a file. Once the project is made, the recording is found in step with the score as above) |
| YouTube video in step with the tablature (Songsterr) | done (in the same section, for a site that gives videos, the box “YouTube video in step with the tablature”, unticked at first. Ticked, the program asks the site, as its public player does, for the videos of the song and the second each bar starts at in each, the points the site's users set (152 points for the 152 bars played of Nothing Else Matters, repeats unfolded); the original video, or else the first, becomes a video track; when the site gives several, the others are kept in the track, saved with the project, and the list at the top of its editor (“Video 1: the original”, “Video 2: …”, with what the site calls each) plays another instead, at the same place of the song, each keeping its own shift; and a message says it may drift when the numbers of bars differ by more than one. The editor of the video track shows the video, as large as the panel allows at 16:9; with Ctrl and a click on a guitar track's header, the video and the tablature show one under the other. With the original sound on and the video leading, YouTube's own player plays it in step with the song: the program starts and stops it with the song, follows it rather than sending it elsewhere (which would make it stop and load again): when the two part by more than an eighth of a second, the song moves to where the video is; the song plays with the video as soon as the project is made, its other tracks silent, and a click on Play in YouTube's player starts the song with it (and Stop stops both, even when the video was started that way); and the song waits while the video loads or shows an advertisement, then starts again with it. When the editor does not show the video, the “Video” button, left of “Full window”, shows it in a corner of the window (it is off at first; YouTube asks that its player be seen while it plays). The editor of the video track has the shift buttons and “Delay of the video player” (in ms, a setting of the computer, for every video), and says, with its error code, when YouTube does not play a video (removed, blocked in the country or outside YouTube). YouTube Music cannot be used: it has no player that can be built in. No video and none of its sound is kept: nothing is downloaded. The player is `og-video.exe`, next to the program, a process of its own that shows YouTube's IFrame player in a WebView2 web view drawn inside the program's window; the video is played with the mouse, and the keyboard always stays the program's (given back at once when the web view takes it, as YouTube's page does when it loads or is clicked), so that a field being typed in and the space bar keep working; see [External files](external-files.md)) |
| Words of the song from the tablature (Songsterr, Guitar Pro, MusicXML), sung syllable by syllable | done (the words of the singing part, each syllable on its note: from Songsterr, read even when the singing track is not ticked; from Guitar Pro files, versions 3 to 8, the lyrics of the file, five lines each starting at a bar, on the track they belong to; from MusicXML, the `<lyric>` of the notes, first verse. As Guitar Pro writes them, each syllable goes on the next beat that strikes a note (rests, grace notes and notes only tied over take none), a space or a hyphen ends a syllable (“laugh-ter” is two), a hyphen or an underscore alone holds the syllable before over one more note, two spaces leave a note without words, and a bar played again sings its syllables again; the syllables are fitted to the notes over the whole song at once, each line after a silence of the singing, so that a mistake of the hand-written text does not move every syllable after it; vowels held in brackets are read without them. Lines start at a word written with a capital (not “I”, “I'm”, “Lord”, “God”…), after a pause of a bar and a half, or when a line would grow past about 48 characters. The sung words follow the score, as written or with the original sound. In the window of the tablature sites, the “Original sound” section has the box “The words of the song”, ticked at first) |
| Words from LRCLIB, timed | done ([LRCLIB](https://lrclib.net), a free base of words timed line by line, no account: asked by the artist and the title of the project, and the length of the recording when there is one to pick the right version; asked by itself when a tablature or a Guitar Pro or MusicXML file without words becomes a project, and with “Search LRCLIB” in the band. The lines follow the seconds of the recording that leads, or of the first recording, or the tempo of the score when there is none; “◀ 0.1 s” and “0.1 s ▶” shift them. Only the artist and the title are sent: see [External files](external-files.md)) |
| Words of the user's (file beside the recording, file chosen, typed) | done (an LRC or text file of the same name beside the recording of the song, `Song.lrc` or `Song.txt` next to `Song.mp3`, read when LRCLIB has nothing or when a recording is linked to a song without words; “From a file…”; “Type…”, for words typed or pasted. Lines starting with a time such as `[01:23.45]` are timed; an LRC `[offset:+250]` is understood. The words are kept in the project; like tablatures, they are mostly under copyright and are there for one's own practice) |
| Band of the words, two ways | done (the band is made taller or lower by dragging the line over it, under the tracks as in the full window; the “Words” button of the bottom panel's bar, left of “Video” and “Full window”, shows or hides a band under the editor, above the status bar: shown at first, only with the Editor tab, gone with the bottom panel, its height added to the panel rather than taken from the editor. View → “Words of the song”: “Shown under the panel”, and “The lines being sung” (the line being sung, large, the syllables already sung lit, the line before, faint, and the line after) or “The whole text, scrolling” (every line, the one being sung lit and scrolled to). A click on a line takes the song there, and the video too when one plays) |
| MIDI Pilot (the songs of a concert, sent to a pedal switcher by MIDI) | done (File → “MIDI Pilot…” opens a window of its own, which can go on a second screen, with a large button for each song: its text, or “Artist - Title”, over the cover of its album, or a picture of the user's, or else a colour for each artist; the song chosen is outlined in green, and a yellow dot says a song has its project. The list is read from a CSV file of four columns, number, artist, song and the text of the button (optional), separated by commas or semicolons, a first line that is not a song being a header (the format of G3S Pilot), or written song by song (List → “Add a song…”, a right-click → “Change…”), and written back as CSV; it is kept in the user's settings. Choosing a song sends the number of its preset to the MIDI output chosen, on the channel chosen, as a control change 0 (bank select, the way The GigRig G3S changes bank) or a program change, with “Numbers from 1” when the first song is number 1: that is the switcher's mode. In the chain's mode (Settings → Mode), the devices of the chain are listed under Settings → “Devices…”, each with a name, a model (any device, The GigRig G3S, BOSS DD-500, BOSS RV-500) and a MIDI channel, and each song carries its own messages, sent in order when it is chosen, each on the channel of its device and followed by a pause of its own: a program change (after a bank select, control changes 0 and 32, when a bank is given), a control change, or a system exclusive message written in hexadecimal; “Change…” edits them and “Send now” tries them. A column of the CSV file named after a device gives each song the program that device receives, and the export writes those columns; the devices a file names join the chain, and a file with programs switches to the chain's mode. The status line shows the bytes last sent when hovered. A device whose model is the BOSS DD-500 or RV-500 has a module (Devices → “Configure…”), which talks to the pedal by system exclusive messages over the MIDI output and the MIDI input chosen (Settings → “MIDI input”), as the makers' public MIDI implementation documents describe, from a description file of the pedal shipped with the program (`devices/dd500.json`, `devices/rv500.json`; one of a `devices` folder next to the program takes the place of the shipped one of the same key): the memories listed (01A to 99C, their names read on request), the patch of the memory chosen read and edited parameter by parameter, by group, each change sent at once to the temporary patch the pedal plays (lists of values, sliders shown as the pedal shows them, the name), “Store…” writing it in the memory after what it held is kept in `Pedal backups` of the folder of the list and the memory is read back and compared, “Init”; the program change map (which memory each bank and program calls) and the system settings, read and written; a file of the program's (JSON: the patches by memory, the map, the system) saved and loaded, every memory read into it, the whole file sent back to the pedal, and the backups of the BOSS editors (`.bel`) read as well; the songs' messages go through the module while it is open, for Windows lets one program at a time use a port. A device whose model is The GigRig G3S has a module too (Devices → “Configure…”), which edits a backup file of the switcher (`.g3b`, made by the switcher's editor and restored by it; nothing goes to the switcher from the program): its banks (added, removed, moved, named on two lines of ten, the song mode), the 28 presets of each (14 on the footswitches, 14 hidden), and for the preset chosen its name, the loops on, the order of the twelve loops in the signal, and its MIDI messages (up to twenty: program change or control change on a channel, when the switch is pressed, released or held, from the kinds the switcher knows); “Banks from the songs” names a bank a song of the list after the first bank, adding banks as needed, “Songs from the banks” does the reverse; “Pedalboard” describes the board itself, kept in the settings since the switcher holds nothing of it: the pedals in each loop (name, kind, and the device of the chain when the pedal is MIDI), the order the signal goes through the loops, the footswitches that change banks, and the loops are then named so in the presets; “Check” checks the file as the switcher does (checksums, sequence, sum, banks declared), “Save as…” writes it. The format of the file is not published: see [External files](external-files.md). An AI can write all of that: List → “Guide for an AI…” writes `ai-guide.md` (also in the archive, in the documents as `docs/en/ai-guide.md` for the version of the code, and `OpenGarageMusic.exe --ai-guide file.md`; the guide says which version of the program and of the format it is for), made by the program from the descriptions of the pedals so that it names every parameter with its range or its values, and teaching a JSON format of actions (a memory of a pedal set parameter by parameter with the values the pedal shows, from its current patch or from init, with a name; the program change map; the system; a G3S bank named, a preset with its loops, their order and its MIDI lines, a bank removed; a song of the list added, changed or removed, with its number, the text of its button and its messages to the devices of the chain); List → “Apply an AI script…” reads such a file, shows each action in a sentence with what is wrong with it (a pedal not described, no device of that model in the chain, a parameter or a value the pedal does not have, a memory that is not one), and “Apply” does them in order through the pedal module (the memory read first when the action starts from it, written, kept before and read back after) and the G3S module (the backup edited, to be saved); “Export for an AI…” on a patch, on the file of a module or on a G3S backup, and List → “Export the songs for an AI…”, write the same format for the AI to change; List → “Export the pedalboard for an AI…” writes one file of everything an AI needs before it writes: a `pedalboard` section (the devices of the chain, the switcher's loops, signal order and bank switches, how songs are sent) and, as actions, the songs, the switcher's loops and backup, and every memory and program change map known of the pedals; what a pedal module reads or writes is kept in `Pedals` of the program's data for that, so the pedals need not be connected. The scripts also take `{ "g3s": "loops", … }`. An agent of an AI running on the same computer (Claude Code with `curl`, a script) can do all of that through a local API: Settings → “API for an AI…” turns it on (a port on `127.0.0.1` only, 7311 unless changed, a token of 32 characters the agent sends as `Authorization: Bearer`, made anew on request), it listens while the MIDI Pilot window is open, and answers JSON in the format of the scripts: `GET /` (what runs and the routes), `/guide`, `/devices`, `/pedals/{key}` (the parameters), `/pedals/{key}/memories` (the names), `GET|PUT /pedals/{key}/memories/{01A}` (a memory as an action; written from the body `{ name, from, set }`), `POST /pedals/{key}/try` (values played without being written), `GET|PUT /pedals/{key}/pc_map` and `/system`, `GET /g3s` (the backup loaded as actions), `POST /g3s/actions`, `POST /g3s/save` (the file written, at `path` or where it was loaded), `GET /songs` (the songs as actions), `GET /pedalboard` (the pedalboard file), `POST /actions` (a whole script, reviewed and applied). A write into a pedal waits for the user's Yes at the bottom of the window (202 after two minutes without it, 403 on No), unless “Write the pedals without asking me each time” is ticked; the pedal module is opened on the device of that model as needed, and one request is served at a time (409 meanwhile). An AI in a web page cannot reach the API: the guide and the script file are its way. The output is opened only while the window is open, for Windows lets one program at a time use it. With “Open the song's project” ticked, choosing a song also opens its project in the main window, at once: the project open until then is kept in the previous sessions. A right-click on a song gives it a picture (over the one found, until “Remove my picture”), a project file, the project open now, or finds its project: one kept in the previous sessions with the same artist and title (written alike up to case and signs), otherwise its tablature on the site of Settings → “Site searched”, searched at once in the window of the tablature sites, where the user chooses the song and its tracks; the project made is saved, once its words and its recording have come, in the folder of the list (OpenGarageMusic\MIDI Pilot of the user's folder, or another), so that it opens on stage without the Internet. List → “Find the projects of the whole list” does that for every song without a project, one after the other, and stops when the window of the sites is closed. The pictures of the albums and the photographs of the artists (Order → “By artist” shows the artists first) come from Deezer, can be turned off, and are kept on the disk; see [External files](external-files.md). No keyboard shortcut: on stage, the pedal switcher is where songs are changed) |

## Out of scope

Piano and guitar lessons, remote control from a phone, CD burning, sharing to online
services.

## External formats

- **Instruments**: SoundFont 2 (`.sf2`), its compressed variant SF3 (`.sf3`) and SFZ
  (`.sfz`). No bank is shipped: the Library menu downloads some on request (see
  [External files](external-files.md)). Banks that rely on SoundFont modulators (the
  pianos of MuseScore General, for instance) sound poor with the current player.
- **SFZ**: the common opcodes of SFZ 1 are read (sample, key and velocity ranges,
  tuning, volume, loop, envelope, groups, `#include`, `#define`); WAV, FLAC or Ogg
  Vorbis samples. Effects and filters described in an SFZ file are ignored: SFZ
  describes instruments, not stand-alone effects.
- **CLAP plug-ins**: an open standard under the MIT licence. Effects and instruments,
  sliders, the plug-in's window (on Windows) and its state saved in the project.
- **VST3 plug-ins**: interfaces published by Steinberg, under the MIT licence since
  version 3.8 of the SDK. A plug-in is a file `name.vst3`, alone or in a folder of the
  same name. Effects and instruments; sliders from 0 to 1 (the format only gives
  normalised values); the plug-in's window (on Windows); the state of both halves of
  the plug-in saved in the project; messages between those two halves; processing in
  32 bits, or in 64 bits for plug-ins that only do that; every output bus mixed into
  the track; what the plug-in reports while computing sound (a meter, a parameter it
  moved) passed on to its window. Not supported: side-chain inputs (they stay silent),
  context menus, and the plug-in's units or programs.
- **LV2 plug-ins**: an open standard under the ISC licence. An LV2 plug-in is a folder
  `name.lv2`; the application reads its description files itself. Effects and
  instruments, set with sliders by default. With a right-click on the plug-in's box,
  its own window opens when it describes one for Windows (`ui:WindowsUI`), with the
  host features “parent”, “resize”, “idle”, instance access and data access. What the
  plug-in keeps outside its ports (LV2 “state”) is saved in the project, for values
  that are plain data. Not supported: the other kinds of windows (X11, Gtk, Qt: they
  belong to Linux and macOS and do not exist on Windows), state that refers to files,
  and plug-ins that require host features other than the identifier table, the
  options and the bounded block length.
- **Isolation**: each plug-in, whatever its format, runs in a second process started
  by the application, which sends it sound and notes and gets processed sound back
  over a connection of the computer alone (a socket on 127.0.0.1), so that whatever
  the plug-in prints never gets into the conversation.
  A plug-in that crashes or stops answering for two seconds is dropped: the effect is
  bypassed, the instrument falls silent, the project stays open and the plug-in can be
  started again. The plug-in's window then belongs to that process. Can be turned off
  in File → Options, for a shorter start-up.
- **JACK applications**: these are not plug-ins but stand-alone programs connected to
  one another by an audio server (JACK). They cannot be loaded into the application;
  connecting them would mean making OpenGarageMusic a JACK client.
- **Audio Units** (GarageBand's format): specific to macOS and iOS, impossible here.

## Known gaps

- **The built-in sounds** are synthesized; realistic instruments come from banks that
  the user downloads or imports.
- **Downloads**: on Musical Artifacts, the licence is the one declared by the person
  who uploaded the file; the application cannot verify it.
- **The library** has 36 sounds and 42 loops, against thousands in the reference
  software.
- **Audio correction**: simple time-domain methods; large changes of pitch or timing
  can be heard.
- **Plug-ins**: the three hosts have only been run with the project's test plug-ins: a
  real plug-in may reveal cases that were not foreseen. Running each plug-in in its
  own process protects against crashes, not against a malicious plug-in, and costs a
  round trip between two processes for every block of sound.
- **Translations**: German, Spanish, Italian and Portuguese have not been reviewed by
  native speakers; a few error messages of the audio engine stay in English.
- **The audio thread** shares a lock with the interface; on a large project, a change
  during playback can produce a click.
- **Sources**: the table of contents of the public guide, and what is said in three
  public video tutorials (their spoken commentary was read as text; the picture was
  not watched). Details of interaction that are only shown, not said, are unchecked.
- **Loops of recorded sound**: the program ships none (nothing is bundled that is not
  the project's own work); the stretching to the tempo is a simple time-domain
  method, and the key of an audio loop is not changed.
- **Drummer that follows**: only the kick drum follows, and only tracks of notes can
  be followed (not recorded sound).
- **Quick help** describes areas of the window, not each control: the controls have
  their own tips.
- **Themes** set the colours, not the sizes, the fonts or the shapes.
- **Songsterr**: the tablatures are written by the users of the site and the program
  cannot check their rights; the format is not documented and was worked out from the
  files the site's player loads, so a change on the site can break the import. Only
  the public tablatures are read: the program does not sign in to the site.
- **Guitar Pro files**: the formats are not published; the reader follows their public
  descriptions and was tried on files written by its own tests, not on many files of
  the programs themselves. The pictures of the chords, the sound settings of the tracks
  and the layout of the pages are passed over; the lyrics are read. The files of Guitar Pro 1,
  2 and 6 are recognised and refused by name: their layouts are not described
  anywhere public.
- **Words of the song**: the lines of a tablature's words are guessed from capitals,
  pauses and length, and may not fall where the song breaks them; only the first verse
  of a MusicXML score is read; words from LRCLIB are timed by line, not by syllable,
  and are only as right as the people who timed them.
- **The score** writes one voice a staff, and chooses sharps or flats from the key
  alone; only triplets get a bracket, other tuplets are written as the nearest value.
