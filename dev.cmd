@echo off
rem Runs cargo with the toolchain installed in .toolchain, e.g.:
rem   dev test --workspace
rem   dev build --release -p opengaragemusic
setlocal
set "RUSTUP_HOME=%~dp0.toolchain\rustup"
set "CARGO_HOME=%~dp0.toolchain\cargo"
set "PATH=%CARGO_HOME%\bin;%~dp0.toolchain\binutils;%PATH%"
if not exist "%CARGO_HOME%\bin\cargo.exe" (
    echo Toolchain missing. Run: powershell -ExecutionPolicy Bypass -File tools\setup-toolchain.ps1
    exit /b 1
)
cd /d "%~dp0"
"%CARGO_HOME%\bin\cargo.exe" %*
