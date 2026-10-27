# OpenGarageMusic — for contributors, human and AI

The rules of the project (clean room, contents, code, texts) are in
[docs/en/contributing.md](docs/en/contributing.md).
Les règles du projet (salle blanche, contenus, code, textes) sont dans
[docs/fr/contributing.md](docs/fr/contributing.md).

In short, and without exception:

- Nothing is read, decompiled or copied from Apple's applications; no sound, loop,
  image, text or name from Apple. Behaviours come from public documentation and are
  described in our own words.
- No sound and no plug-in is shipped. Anything external is under a free licence and
  recorded in `docs/en/external-files.md` and `docs/fr/external-files.md`.
- Texts in the code are English, through `tr("…")` / `tf!("…", …)`; every built-in
  language in `apps/opengarage/lang` translates every text.
- The documentation is kept in English (`docs/en`) and in French (`docs/fr`).
- Before committing: `dev test --workspace`.
- The program is under the PolyForm Noncommercial License 1.0.0 (`LICENSE.md`); a
  contribution gives the author the rights on it (see contributing.md § 5). No GPL,
  LGPL or AGPL code.
