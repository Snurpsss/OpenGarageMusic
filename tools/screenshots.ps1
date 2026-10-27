# Takes again the pictures shown on the project's front page, into "screenshots".
# The program is started as on a first run, in English, so nothing of the
# user's own settings or projects appears in them.
#   powershell -ExecutionPolicy Bypass -File tools\screenshots.ps1
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
$out = Join-Path $root 'screenshots'
New-Item -ItemType Directory -Force $out | Out-Null
$env:OPENGARAGE_LANG = 'en'
# View of the program, name of the picture, theme (empty for the usual one), and
# seconds to wait before the picture, and the title of the window taken when it is not
# the main one. "site-search" asks songsterr.com for real.
$views = @(
    @('notes', 'tracks-and-editor'),
    @('controls', 'sound-controls'),
    @('drummer', 'drummer'),
    @('score', 'score'),
    @('multitab', 'tablature-several-tracks'),
    @('site-search', 'tablature-sites', '', 8),
    @('loops', 'loops'),
    @('global', 'arrangement-and-automation'),
    @('sounds', 'downloads'),
    @('sessions', 'previous-sessions'),
    @('export', 'export-to'),
    @('video', 'video-and-tablature', '', 12),
    @('words', 'words'),
    @('setlist', 'midi-pilot', '', 10, 'MIDI Pilot'),
    @('setlist-pedal', 'midi-pilot-pedal', '', 10, 'MIDI Pilot'),
    @('setlist-script', 'midi-pilot-script', '', 10, 'MIDI Pilot'),
    @('controls', 'light-theme', 'light'),
    @('loading', 'loading-screen')  # saved as PNG; the front page shows it as JPEG
)
foreach ($view in $views) {
    $env:OPENGARAGE_THEME = if ($view.Count -gt 2 -and $view[2]) { $view[2] } else { 'studio' }
    $wait = if ($view.Count -gt 3) { $view[3] } else { 4 }
    $file = Join-Path $out "$($view[1]).png"
    $title = if ($view.Count -gt 4) { $view[4] } else { '' }
    $size = & powershell -NoProfile -ExecutionPolicy Bypass -File (Join-Path $PSScriptRoot 'screenshot.ps1') $view[0] $file release $wait $title
    '{0,-28} {1}' -f $view[1], $size
}
