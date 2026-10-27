# Themes

[Français](../fr/themes.md)

A theme sets the colours of the interface. Three come with the program, in
`apps/opengarage/themes`:

| File | Name | What it is |
|---|---|---|
| `studio.json` | Studio | The program's own look, and the default |
| `dark.json` | Dark | Darker and more contrasted |
| `light.json` | Light | Dark text on a light background |

The theme is chosen in **View → Theme**. The choice is kept between sessions.

## Making your own

1. **View → Theme → Save this theme as a model…** writes the theme in use as a file
   that gives every colour.
2. Change the colours in a text editor.
3. **View → Theme → Import a theme (.json file)…** copies the file into the themes
   folder of the program (`%APPDATA%\OpenGarageMusic\themes`) and switches to it.

The name of the file, without `.json`, identifies the theme: importing a file with
the same name replaces it, and a file named like a built-in theme (`dark.json`)
takes its place. **View → Theme → Open the themes folder** shows the folder; a file
deleted there is no longer offered.

## The file

```json
{
  "name": "Evening sand",
  "base": "light",
  "colors": {
    "panel": "#e8dcc0",
    "record": "#aa2200"
  },
  "tracks": ["#336699", "#4fae69"]
}
```

- `name`: what the menu shows. Without it, the name of the file.
- `base`: `dark` or `light`. Everything the file does not give is taken from it, so
  a theme can be a few lines long.
- `colors`: any of the colours below, as `#rrggbb` (or `#rgb`, or `#rrggbbaa` with
  transparency). A colour this version does not know is ignored; a value that is
  not a colour makes the import fail, and the message names it.
- `tracks`: up to eight colours, for the tracks and their regions. The first ones
  are also the accents: the second one draws the knobs and the selected notes, the
  first one the level meters.

| Colour | What it paints |
|---|---|
| `window` | Background of windows and menus |
| `panel` | Background of the panels |
| `field` | Text fields, the tracks' lanes, the editors |
| `stripe` | Track headers and slightly raised areas |
| `button`, `button_hover`, `button_active` | Buttons: at rest, under the pointer, pressed |
| `button_text` | Text of the buttons |
| `border` | Lines between panels, borders of windows |
| `text` | Ordinary text |
| `text_strong` | Titles and emphasised text |
| `selection`, `selection_text` | What is selected or switched on |
| `link` | Links |
| `outline` | Line around the selected region, automation curves |
| `record` | The record button, errors |
| `warning` | Warnings |
| `done` | What succeeded or is already there |

A theme sets colours only: not sizes, fonts or shapes. Regions always carry dark
text, so the colours of `tracks` should stay rather light.
