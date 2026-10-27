# Builds the release and packs it for distribution:
#   dist\OpenGarageMusic-<version>-windows-x64.zip
# The archive holds the program, the documentation, what a translator needs,
# and the list of the libraries built into the program with their licences.
#   powershell -ExecutionPolicy Bypass -File tools\package.ps1
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
$utf8 = New-Object Text.UTF8Encoding $false
Set-Location $root

$manifest = [IO.File]::ReadAllText((Join-Path $root 'Cargo.toml'), $utf8)
if ($manifest -notmatch '(?m)^version = "([^"]+)"') { throw 'no version in Cargo.toml' }
# A fourth number is written "+1" for Cargo (0.9.9+1 is 0.9.9.1).
$version = $Matches[1] -replace '\+', '.'
$name = "OpenGarageMusic-$version"
"Packaging $name"

& (Join-Path $root 'dev.cmd') build --release -p opengaragemusic -p og-video -p midi-pilot
if ($LASTEXITCODE -ne 0) { throw 'the build failed' }

$dist = Join-Path $root 'dist'
$stage = Join-Path $dist $name
if (Test-Path $stage) { Remove-Item -Recurse -Force $stage }
New-Item -ItemType Directory -Force $stage, (Join-Path $stage 'lang') | Out-Null
# The program just built: in CARGO_TARGET_DIR when it is set, as cargo does.
$target = Join-Path $root 'target'
if ($env:CARGO_TARGET_DIR) {
    $target = $env:CARGO_TARGET_DIR
    if (-not [IO.Path]::IsPathRooted($target)) { $target = Join-Path $root $target }
}
Copy-Item (Join-Path $target 'release\opengaragemusic.exe') (Join-Path $stage 'OpenGarageMusic.exe')
# The video player, in a process of its own, and the loader of Microsoft's web
# view it needs (BSD licence; building og-video puts it next to it).
Copy-Item (Join-Path $target 'release\og-video.exe'), (Join-Path $target 'release\WebView2Loader.dll') $stage
# The MIDI Pilot alone, for the stage: a launcher that starts the program on that window.
Copy-Item (Join-Path $target 'release\MidiPilot.exe') $stage
Copy-Item (Join-Path $root 'README.md'), (Join-Path $root 'LICENSE.md'), (Join-Path $root 'COMMERCIAL.md') $stage
# The guide for an AI, written by the program itself from the descriptions of the pedals.
& (Join-Path $target 'release\opengaragemusic.exe') --ai-guide (Join-Path $stage 'ai-guide.md')
if ($LASTEXITCODE -ne 0) { throw 'the guide was not written' }
Copy-Item (Join-Path $root 'docs') $stage -Recurse
# The descriptions of the tablature sites, next to the program, where the user
# adds or changes them (the program has its own copy of the ones shipped).
Copy-Item (Join-Path $root 'crates\og-engine\sites') (Join-Path $stage 'sites') -Recurse
# The descriptions of the pedals, the same way (the program has its own copy too).
Copy-Item (Join-Path $root 'crates\og-engine\devices') (Join-Path $stage 'devices') -Recurse
# A translator starts from the template; the built-in languages are in the program.
Copy-Item (Join-Path $root 'apps\opengarage\lang\template.pot'), (Join-Path $root 'apps\opengarage\lang\README.md') (Join-Path $stage 'lang')

# The libraries built into the program, from what cargo knows of them.
$meta = & (Join-Path $root 'dev.cmd') metadata --format-version 1 --filter-platform x86_64-pc-windows-gnu | Out-String | ConvertFrom-Json
$ours = @('opengaragemusic', 'og-engine', 'og-video', 'midi-pilot', 'og-clap-test')
$lines = @(
    "OpenGarageMusic $version - libraries built into the program",
    '',
    'Each library is the work of its authors and is used under the licence named',
    'here. Its source and the full text of its licence are at the address given.',
    ''
)
foreach ($p in ($meta.packages | Where-Object { $ours -notcontains $_.name } | Sort-Object name, version)) {
    $licence = if ($p.license) { $p.license } else { 'see the repository' }
    $where = if ($p.repository) { $p.repository } else { "https://crates.io/crates/$($p.name)" }
    $lines += ('{0} {1}  -  {2}  -  {3}' -f $p.name, $p.version, $licence, $where)
}
# The pictograms of the tracks, with the authors their licence asks to name.
$lines += @(
    '',
    'Pictograms of the track headers (apps/opengarage/track-icons): Game Icons,',
    'https://game-icons.net, under the Creative Commons Attribution 3.0 licence,',
    'https://creativecommons.org/licenses/by/3.0/ - by Caro Asercion (grand piano,',
    'drum kit), Delapouite (piano keys, bass guitar head, trumpet, saxophone,',
    'xylophone, microphone, musical notes, film strip), Lorc (guitar), Skoll (sound',
    'waves) and Zajkonur (violin).',
    '',
    'WebView2Loader.dll, next to og-video.exe: the loader of the WebView2 runtime, from',
    'the WebView2 SDK of Microsoft Corporation (https://www.nuget.org/packages/Microsoft.Web.WebView2),',
    'redistributed under its BSD 3-Clause licence: Copyright (C) Microsoft Corporation.',
    'All rights reserved. Redistribution and use in source and binary forms, with or',
    'without modification, are permitted provided that the following conditions are met:',
    'redistributions of source code must retain the above copyright notice, this list of',
    'conditions and the following disclaimer; redistributions in binary form must',
    'reproduce the above copyright notice, this list of conditions and the following',
    'disclaimer in the documentation and/or other materials provided with the',
    'distribution; the name of Microsoft Corporation, or the names of its contributors,',
    'may not be used to endorse or promote products derived from this software without',
    'specific prior written permission. THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT',
    'HOLDERS AND CONTRIBUTORS "AS IS" AND ANY EXPRESS OR IMPLIED WARRANTIES ARE',
    'DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT OWNER OR CONTRIBUTORS BE LIABLE FOR ANY',
    'DAMAGES ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE.',
    '',
    "Videos are played by YouTube's own player (the YouTube IFrame Player API), under",
    "YouTube's terms of service; the program keeps no video and none of its sound."
)
[IO.File]::WriteAllLines((Join-Path $stage 'THIRD-PARTY-LICENSES.txt'), $lines, $utf8)

$zip = Join-Path $dist "$name-windows-x64.zip"
if (Test-Path $zip) { Remove-Item -Force $zip }
Compress-Archive -Path $stage -DestinationPath $zip -CompressionLevel Optimal
$hash = (Get-FileHash $zip -Algorithm SHA256).Hash.ToLower()
[IO.File]::WriteAllText("$zip.sha256", "$hash  $(Split-Path $zip -Leaf)`n", $utf8)
'{0}  ({1:N1} MB)' -f $zip, ((Get-Item $zip).Length / 1MB)
"sha256 $hash"
"libraries listed: $($lines.Count - 5)"
