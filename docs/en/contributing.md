# Contributing — rules for humans and AIs

*[Version française](../fr/contributing.md)*

OpenGarageMusic is a music studio, written from scratch in Rust, published under the
PolyForm Noncommercial License 1.0.0 (free for personal use, a licence from the author
for professional use). It is inspired by
the features of Apple's GarageBand without taking anything from it. These rules have
no exceptions.

## 1. Clean room

Allowed sources for knowing what the software should do:

- Apple's public documentation (user guide, support pages);
- public videos and articles showing the software in use;
- observing the software by using it normally, with a legally obtained copy;
- the public literature on signal processing and the public specifications of formats
  (WAV, MIDI, SoundFont, SFZ, CLAP, LV2, VST 3, gettext…).

Forbidden:

- reading, disassembling, decompiling or copying anything in Apple's applications;
- copying the text of Apple's documentation: behaviours are described in our own words;
- copying code under the GPL, LGPL or AGPL (Ardour, LMMS, etc.). Reading those projects
  to understand an idea is allowed, copying or translating their code is not.

## 2. Contents

- No sound, loop, sampled instrument, preset, icon, image or screenshot from Apple, even
  retouched or redrawn after the original.
- The names of sounds and presets are ours, not Apple's.
- Anything added (sound, font, icon) is either made for the project or under a free
  licence. Its author, its licence, the address it was downloaded from and the date are
  written in [External files](external-files.md) (in both languages) before it enters
  the repository.
- No sound and no plug-in is shipped: the user downloads them, on request, from sites
  that distribute them under a free licence.
- One exception, which is the user's to use: the tablatures of Songsterr, read at their
  request into their own project. They are not free, are never added to the repository
  (not even as test data), and are described in [External files](external-files.md).
- Reference screenshots serve to understand a behaviour; they are never added to the
  repository.
- “GarageBand” and “Apple” are only used to describe compatibility or inspiration. The
  project is neither affiliated with Apple nor endorsed by it.

## 3. Code

- The engine (`crates/og-engine`) never depends on the interface (`apps/opengarage`).
- Dependencies under permissive licences (MIT, Apache-2.0, BSD, ISC, Zlib), or MPL-2.0
  used unmodified (the Symphonia decoders). No LGPL or GPL code linked into the program:
  that is why MP3 and AAC export go through the encoders of the system.
- `unsafe` code is kept to the borders with the outside (the encoders of Windows, the
  plug-in hosts); each block says why it is correct.
- Third-party plug-ins downloaded for a check are inspected, not run: the hosts are
  tested with the project's own test plug-ins.
- An invalid project file, a missing audio output or an out-of-range input produces an
  error message, never a crash.
- Every feature of the engine has a test; the interactions of the interface are tested
  without a screen (`apps/opengarage/src/tests.rs`).
- Before committing: `dev test --workspace`.

## 4. Texts

- The texts in the code are English, wrapped in `tr("…")`, or `tf!("… {}", value)` when
  values go into them. Names that come from the engine (sounds, loops, templates) are
  English too and go through `tr` where they are shown.
- Each of the built-in languages (`apps/opengarage/lang/*.po`) must translate every
  text: a test checks it, and checks that the placeholders are kept.
  See [Translating](translating.md).
- The documentation exists in English (`docs/en`) and in French (`docs/fr`); a change
  to one is made to the other.
- Source files are UTF-8. On Windows, do not rewrite them with PowerShell 5 commands
  that read or write in the system code page.

## 5. Licence and contributions

- The program is published under the PolyForm Noncommercial License 1.0.0
  (`LICENSE.md`): free for non-commercial use, a licence from the author for any
  commercial use (`COMMERCIAL.md`).
- By proposing a contribution (a merge request, a patch, a translation, a description
  of a site or of a pedal), the contributor gives the author of OpenGarageMusic the
  right to use, change, publish and license it under any terms, including the
  commercial licences, and confirms it is their own work or under a licence that
  allows this. Their name stays in the history of the repository.
- Contributions under the GPL, LGPL or AGPL, or copied from such code, cannot be taken.
