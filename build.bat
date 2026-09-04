@echo off
setlocal EnableExtensions EnableDelayedExpansion
cd /d "%~dp0"

where py >nul 2>&1
if errorlevel 1 (
  echo ERROR: Python launcher ^(py^) not found on PATH.
  echo Install Python and add it to PATH, or check your Environment Variables.
  echo.
  pause
  exit /b 1
)

if not exist main.py (
  echo ERROR: main.py not found in current directory.
  echo.
  pause
  exit /b 1
)

set "NAME=WebFavoritesFastScrnshot"
set "EXE_OUT=dist\%NAME%.exe"
set "BAR_WIDTH=30"
set /a TOTAL_STEPS=5
set /a CURRENT_STEP=0

cls

call :progress "Cleaning build artifacts..."
if exist dist rmdir /s /q dist >nul 2>&1
if exist build rmdir /s /q build >nul 2>&1
if exist "%NAME%.spec" del /f /q "%NAME%.spec" >nul 2>&1
mkdir dist 2>nul

call :progress "Analyzing dependencies..."
py -3 -c "import PyInstaller" >nul 2>&1
if errorlevel 1 goto :fail

call :progress "Compiling main.py to executable..."
py -3 -m PyInstaller --noconfirm --clean --onefile --windowed --name %NAME% --icon assets/icon.ico --add-data "assets/icon.ico;assets" main.py >nul 2>&1
if errorlevel 1 goto :fail

call :progress "Cleaning temporary build files..."
if exist build rmdir /s /q build >nul 2>&1
if exist "%NAME%.spec" del /f /q "%NAME%.spec" >nul 2>&1

call :progress "Finalizing..."

echo.
echo BUILD OK  -^> %EXE_OUT%
echo.

timeout /t 3 /nobreak >nul
exit /b 0

:fail
echo.
echo BUILD FAILED
echo.
pause
exit /b 1

:progress
set /a CURRENT_STEP+=1
set /a "pct=CURRENT_STEP*100/TOTAL_STEPS"
set /a "filled=CURRENT_STEP*BAR_WIDTH/TOTAL_STEPS"
if !filled! gtr %BAR_WIDTH% set "filled=%BAR_WIDTH%"

set "bar="
for /L %%i in (1,1,%BAR_WIDTH%) do (
    if %%i LEQ !filled! (set "bar=!bar!=") else (set "bar=!bar! ")
)

echo [!bar!] !pct!%%  %~1
goto :eof