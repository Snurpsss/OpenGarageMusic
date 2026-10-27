# Installs everything needed to build OpenGarageMusic inside .toolchain.
# Nothing is installed system-wide and PATH is not modified.
#
#   powershell -ExecutionPolicy Bypass -File tools\setup-toolchain.ps1
#
# What it installs:
#   - Rust (stable, x86_64-pc-windows-gnu) through rustup, from rust-lang.org
#   - w64devkit (portable MinGW-w64), from which only three files are used:
#     as.exe and dlltool.exe (needed by rustc to link Windows libraries) and
#     libshlwapi.a (an import library Rust's bundled MinGW does not ship).

$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'

$root = Split-Path $PSScriptRoot -Parent
$tc = Join-Path $root '.toolchain'
$env:RUSTUP_HOME = Join-Path $tc 'rustup'
$env:CARGO_HOME = Join-Path $tc 'cargo'

$w64Version = '2.10.0'
$w64Sha256 = '18d0a4c71a166f8401ab6305781bec5882b40b5e06ba9807c61cb5f3b3c6325e'

function Get-Verified($url, $path, $sha256) {
    Invoke-WebRequest -Uri $url -OutFile $path -UseBasicParsing
    $actual = (Get-FileHash $path -Algorithm SHA256).Hash.ToLower()
    if ($actual -ne $sha256) {
        Remove-Item $path -Confirm:$false
        throw "Checksum mismatch for $url (expected $sha256, got $actual)"
    }
}

# --- Rust ---------------------------------------------------------------
if (-not (Test-Path (Join-Path $env:CARGO_HOME 'bin\cargo.exe'))) {
    Write-Host 'Installing Rust...'
    $dl = Join-Path $tc 'dl'
    New-Item -ItemType Directory -Force $dl | Out-Null
    $base = 'https://static.rust-lang.org/rustup/dist/x86_64-pc-windows-msvc/rustup-init.exe'
    $sum = (Invoke-WebRequest -Uri "$base.sha256" -UseBasicParsing).Content
    if ($sum -is [byte[]]) { $sum = [Text.Encoding]::ASCII.GetString($sum) }
    $init = Join-Path $dl 'rustup-init.exe'
    Get-Verified $base $init $sum.Trim().Split(' ')[0]
    & $init -y --no-modify-path --profile minimal --default-host x86_64-pc-windows-gnu --default-toolchain stable
    if ($LASTEXITCODE -ne 0) { throw 'rustup-init failed' }
} else {
    Write-Host 'Rust already installed.'
}

# --- as.exe, dlltool.exe, libshlwapi.a ------------------------------------
$binutils = Join-Path $tc 'binutils'
$selfContained = Join-Path $env:RUSTUP_HOME 'toolchains\stable-x86_64-pc-windows-gnu\lib\rustlib\x86_64-pc-windows-gnu\lib\self-contained'
$needed = @((Join-Path $binutils 'as.exe'), (Join-Path $binutils 'dlltool.exe'), (Join-Path $binutils 'windres.exe'), (Join-Path $selfContained 'libshlwapi.a'))

if ($needed | Where-Object { -not (Test-Path $_) }) {
    $w64 = Join-Path $tc 'w64devkit'
    if (-not (Test-Path (Join-Path $w64 'bin\as.exe'))) {
        Write-Host 'Downloading w64devkit...'
        $dl = Join-Path $tc 'dl-mingw'
        New-Item -ItemType Directory -Force $dl | Out-Null
        $archive = Join-Path $dl "w64devkit-x64-$w64Version.7z.exe"
        Get-Verified "https://github.com/skeeto/w64devkit/releases/download/v$w64Version/w64devkit-x64-$w64Version.7z.exe" $archive $w64Sha256
        # The file is a self-extracting 7z archive; it is unpacked, never run.
        tar -xf $archive -C $tc
        if ($LASTEXITCODE -ne 0) { throw 'Could not extract w64devkit' }
    }
    New-Item -ItemType Directory -Force $binutils | Out-Null
    Copy-Item (Join-Path $w64 'bin\as.exe'), (Join-Path $w64 'bin\dlltool.exe'), (Join-Path $w64 'bin\windres.exe') $binutils -Force
    Copy-Item (Join-Path $w64 'lib\libshlwapi.a') $selfContained -Force
} else {
    Write-Host 'Linker helpers already installed.'
}

Write-Host 'Toolchain ready. Build with: dev build --release -p opengarage'
