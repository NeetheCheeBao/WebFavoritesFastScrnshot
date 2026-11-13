@echo off
setlocal EnableExtensions EnableDelayedExpansion
cd /d "%~dp0"

where cargo >nul 2>&1
if errorlevel 1 (
  echo ERROR: cargo not found on PATH.
  echo Install Rust and add cargo to PATH, or check your Environment Variables.
  echo.
  pause
  exit /b 1
)

if not exist Cargo.toml (
  echo ERROR: Cargo.toml not found in current directory.
  echo.
  pause
  exit /b 1
)

set "OUTDIR=dist"

cls

if exist "%OUTDIR%" rmdir /s /q "%OUTDIR%" >nul 2>&1
mkdir "%OUTDIR%" 2>nul

set "RUSTFLAGS=-C target-feature=+crt-static"

cargo build --release
if errorlevel 1 goto :fail

set "EXE_NAME="
for %%F in (target\release\*.exe) do (
  copy /y "%%F" "%OUTDIR%\" >nul 2>&1
  set "EXE_NAME=%%~nxF"
)
if not defined EXE_NAME goto :fail

echo.
echo BUILD OK  -^> %OUTDIR%\%EXE_NAME%
echo.

"%SystemRoot%\System32\timeout.exe" /t 3 /nobreak >nul 2>&1
exit /b 0

:fail
echo.
echo BUILD FAILED
echo.
pause
exit /b 1
