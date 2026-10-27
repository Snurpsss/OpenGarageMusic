# Publishes the packed release on GitLab: uploads the archive made by
# tools\package.ps1 and creates the release that offers it for download.
#
#   powershell -ExecutionPolicy Bypass -File tools\release.ps1
#
# It asks for a GitLab access token with the "api" scope (the token used to
# push code, with "write_repository" only, is not enough). The token is typed
# when asked, or taken from the GITLAB_TOKEN environment variable; it is never
# written anywhere.
#
# Before running it: build the archive (tools\package.ps1) and push the code
# (git push origin main), since a release points at a commit of the server.
param(
    # What the release says, under its title. A default text is written otherwise.
    [string]$Notes = '',
    # The same, read from a file (Markdown).
    [string]$NotesFile = ''
)
$ErrorActionPreference = 'Stop'
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
$root = Split-Path $PSScriptRoot -Parent
$utf8 = New-Object Text.UTF8Encoding $false
if ($NotesFile) { $Notes = [IO.File]::ReadAllText($NotesFile, $utf8) }
$git = Join-Path $root '.toolchain\git\cmd\git.exe'
if (-not (Test-Path $git)) { $git = 'git' }

# --- what is published -------------------------------------------------------------
$manifest = [IO.File]::ReadAllText((Join-Path $root 'Cargo.toml'), $utf8)
if ($manifest -notmatch '(?m)^version = "([^"]+)"') { throw 'No version in Cargo.toml.' }
# A fourth number is written "+1" for Cargo (0.9.9+1 is 0.9.9.1).
$version = $Matches[1] -replace '\+', '.'
$tag = "v$version"
$file = "OpenGarageMusic-$version-windows-x64.zip"
$archive = Join-Path $root "dist\$file"
if (-not (Test-Path $archive)) { throw "There is no $file in dist: run tools\package.ps1 first." }
$hash = (Get-FileHash $archive -Algorithm SHA256).Hash.ToLower()
$commit = (& $git -C $root rev-parse HEAD).Trim()
$pending = & $git -C $root status --short
if ($pending) { throw 'Some changes are not committed: commit them, push, and pack again, so that the archive matches the code.' }

# --- where ---------------------------------------------------------------------------
$remote = (& $git -C $root remote get-url origin).Trim()
if ($remote -notmatch '^(https?://[^/]+)/(.+?)(\.git)?/?$') { throw "The address of the repository is not understood: $remote" }
$server = $Matches[1]
$project = $Matches[2]
$api = "$server/api/v4/projects/$([Uri]::EscapeDataString($project))"

"Release $tag of $project on $server"
"  archive : $file ($([math]::Round((Get-Item $archive).Length / 1MB, 1)) MB)"
"  sha256  : $hash"
"  commit  : $commit"

# --- the token -----------------------------------------------------------------------
$token = $env:GITLAB_TOKEN
if (-not $token) {
    $typed = Read-Host -AsSecureString 'GitLab access token with the "api" scope (nothing shows while typing)'
    $token = [Runtime.InteropServices.Marshal]::PtrToStringAuto([Runtime.InteropServices.Marshal]::SecureStringToBSTR($typed))
}
if (-not $token) { throw 'No token given.' }
$headers = @{ 'PRIVATE-TOKEN' = $token.Trim() }

function Ask($method, $url, $body) {
    try {
        if ($body) {
            return Invoke-RestMethod -Method $method -Uri $url -Headers $headers -ContentType 'application/json; charset=utf-8' -Body ([Text.Encoding]::UTF8.GetBytes(($body | ConvertTo-Json -Depth 6)))
        }
        return Invoke-RestMethod -Method $method -Uri $url -Headers $headers
    } catch {
        $status = 0
        if ($_.Exception.Response) { $status = [int]$_.Exception.Response.StatusCode }
        return @{ failed = $status; message = $_.Exception.Message }
    }
}

# --- checks before anything is sent -----------------------------------------------
$found = Ask Get $api $null
if ($found.failed -eq 401) { throw 'The server refuses this token: it is wrong or expired.' }
if ($found.failed -eq 403) { throw 'This token lacks the "api" scope.' }
if ($found.failed) { throw "The project was not found on the server ($($found.failed)): $($found.message)" }

$onServer = Ask Get "$api/repository/commits/$commit" $null
if ($onServer.failed) { throw "The commit $($commit.Substring(0, 8)) is not on the server yet: run  git push origin main  first." }

$existingTag = Ask Get "$api/repository/tags/$([Uri]::EscapeDataString($tag))" $null
if (-not $existingTag.failed -and $existingTag.commit.id -ne $commit) {
    throw "The tag $tag already exists on the server, on another commit ($($existingTag.commit.short_id)). Raise the version in Cargo.toml, pack again, and run this script again."
}
$existingRelease = Ask Get "$api/releases/$([Uri]::EscapeDataString($tag))" $null
if (-not $existingRelease.failed) { throw "The release $tag already exists: $($existingRelease._links.self)" }

# --- the archive, in the project's package registry -------------------------------
# Sent with curl (part of Windows): unlike PowerShell's own client, it reports
# what the server answers when it turns a large upload down. The token goes to
# curl in a settings file of the user's temporary folder, deleted at once, so
# that it never shows on a command line.
function Upload($path, $url) {
    $settings = [IO.Path]::GetTempFileName()
    try {
        [IO.File]::WriteAllText($settings, "header = `"PRIVATE-TOKEN: $($headers['PRIVATE-TOKEN'])`"`n", (New-Object Text.UTF8Encoding $false))
        $answer = & curl.exe --config $settings --silent --show-error --output NUL --write-out '%{http_code}' --upload-file $path $url 2>&1 | Out-String
    } finally {
        Remove-Item -Force $settings -ErrorAction SilentlyContinue
    }
    $code = 0
    if ($answer -match '(\d{3})\s*$') { $code = [int]$Matches[1] }
    return @{ code = $code; text = $answer.Trim() }
}
function Explain($sent, $what) {
    switch ($sent.code) {
        201 { return }
        200 { return }
        401 { throw "$what was refused: the token is not accepted for uploads." }
        403 { throw "$what was refused (403): the token lacks the `"api`" scope, or the package registry is turned off for the project (Settings > General > Visibility, project features, permissions > Package registry)." }
        404 { throw "$what was refused (404): the package registry is turned off for the project (Settings > General > Visibility, project features, permissions > Package registry), or for the whole server." }
        413 { throw "$what was refused (413): the server, or a proxy in front of it, does not accept a file of this size. On the web server in front of GitLab, raise the limit on the size of requests (nginx: client_max_body_size)." }
        default { throw "$what could not be uploaded: $($sent.text)" }
    }
}
$packageBase = "$api/packages/generic/opengaragemusic/$version"
$packageUrl = "$packageBase/$file"
# A few bytes first: if they pass and the archive does not, its size is the reason.
$checksum = "$archive.sha256"
if (-not (Test-Path $checksum)) { [IO.File]::WriteAllText($checksum, "$hash  $file`n", $utf8) }
'Uploading the checksum...'
Explain (Upload $checksum "$packageBase/$file.sha256") 'The checksum file'
"Uploading the archive ($([math]::Round((Get-Item $archive).Length / 1MB, 1)) MB)..."
$sent = Upload $archive $packageUrl
if ($sent.code -notin 200, 201 -and $sent.code -notin 401, 403, 404, 413) {
    throw "The small checksum file was accepted but the archive was not ($($sent.text)). Its size is the likely reason: on the web server in front of GitLab, raise the limit on the size of requests (nginx: client_max_body_size, 1 MB by default) to 50 MB or more, then run this script again."
}
Explain $sent 'The archive'

# --- the release -------------------------------------------------------------------
if (-not $Notes) {
    $Notes = "OpenGarageMusic $version for Windows (64 bits).`n`nUnpack the archive anywhere and start OpenGarageMusic.exe; nothing is installed.`n`nSHA-256 of $file :`n``$hash``"
}
$release = Ask Post "$api/releases" @{
    name        = "OpenGarageMusic $version"
    tag_name    = $tag
    ref         = $commit
    description = $Notes
    assets      = @{ links = @(@{ name = $file; url = $packageUrl; link_type = 'package' }) }
}
if ($release.failed) { throw "The archive is uploaded, but the release could not be created ($($release.failed)): $($release.message)" }
''
"Done: $($release._links.self)"
