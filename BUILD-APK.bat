@echo off
cd /d "%~dp0"
set LOG=..\.tools\claude-tmp\build.log
echo Building RustyPrompter. This takes several minutes...
C:\Python311\python.exe scripts\build.py > "%LOG%" 2>&1
echo EXIT %ERRORLEVEL% >> "%LOG%"
echo Finished. Close this window.
pause
