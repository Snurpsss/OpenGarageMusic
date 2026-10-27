@echo off
rem Builds (if needed) and starts OpenGarageMusic.
call "%~dp0dev.cmd" run --release -p opengaragemusic %*
