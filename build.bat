@echo off
setlocal
rem Build Meshcore Manager on Windows.
rem   build.bat                a menu
rem   build.bat manager        for Windows: dist\manager\mcmanager.exe
rem   build.bat manager-linux  for Linux: dist\manager-linux\x86_64\mcmanager (PC)
rem                            and dist\manager-linux\aarch64\mcmanager (64-bit ARM: Raspberry Pi 3/4/5...)
rem                            Needs, once: rustup target add x86_64-unknown-linux-musl aarch64-unknown-linux-musl
rem                                         cargo install --locked cargo-zigbuild   (and zig on the PATH)
rem   build.bat all            both
cd /d "%~dp0"

set "DO_MANAGER="
set "DO_MANAGER_LINUX="
set "MENU="
set "WHAT=%~1"
if "%WHAT%"=="" goto :menu
if /I "%WHAT%"=="manager" set "DO_MANAGER=1"
if /I "%WHAT%"=="manager-linux" set "DO_MANAGER_LINUX=1"
if /I "%WHAT%"=="all" (set "DO_MANAGER=1" & set "DO_MANAGER_LINUX=1")
if not defined DO_MANAGER if not defined DO_MANAGER_LINUX (
  echo Usage: build.bat [manager^|manager-linux^|all]
  echo        build.bat alone shows a menu.
  exit /b 1
)
goto :run

:menu
set "MENU=1"
echo.
echo   Meshcore Manager build (Windows)
echo.
echo     1  for Windows
echo     2  for Linux: PC and 64-bit ARM, any distribution
echo     A  both
echo.
set "CHOICE="
set /p "CHOICE=  Which ones? e.g. 1, 12 or A (Enter = A): "
if not defined CHOICE set "CHOICE=A"
set "REST=%CHOICE%"
:pick
if not defined REST goto :picked
set "C=%REST:~0,1%"
set "REST=%REST:~1%"
if "%C%"=="1" set "DO_MANAGER=1"
if "%C%"=="2" set "DO_MANAGER_LINUX=1"
if /I "%C%"=="a" (set "DO_MANAGER=1" & set "DO_MANAGER_LINUX=1")
goto :pick
:picked
if not defined DO_MANAGER if not defined DO_MANAGER_LINUX (
  echo   Nothing chosen.
  pause
  exit /b 1
)

:run
call :build
set "RC=%errorlevel%"
echo.
if "%RC%"=="0" (echo Done.) else (echo Stopped: a step failed, see above.)
if defined MENU pause
exit /b %RC%

:build
where cargo >nul 2>&1 || (
  echo cargo is missing: install Rust from https://rustup.rs, then open a new window
  exit /b 1
)
if defined DO_MANAGER call :manager || exit /b 1
if defined DO_MANAGER_LINUX call :manager_linux || exit /b 1
exit /b 0

:manager
echo === Meshcore Manager for Windows
cargo build --release -p meshcore-manager || exit /b 1
if not exist "dist\manager" mkdir "dist\manager"
copy /Y "target\release\mcmanager.exe" "dist\manager\mcmanager.exe" >nul || (
  echo Could not replace dist\manager\mcmanager.exe - is it running? Close it and retry
  exit /b 1
)
copy /Y meshcore-manager\README.md dist\manager\README.md >nul || exit /b 1
echo Built dist\manager\mcmanager.exe
exit /b 0

rem Linux, built from Windows with zig as the linker (cargo-zigbuild). musl: one file with nothing
rem to install, on any Linux. libdbus (Bluetooth) is compiled in.
:manager_linux
echo === Meshcore Manager for Linux
where cargo-zigbuild >nul 2>&1 || (
  echo cargo-zigbuild is missing: cargo install --locked cargo-zigbuild   ^(and zig on the PATH^)
  exit /b 1
)
rem zig's linker notes about "-O1" are harmless: not shown
setlocal
set "RUSTFLAGS=%RUSTFLAGS% -A linker_messages"
call :linux x86_64-unknown-linux-musl x86_64 || exit /b 1
call :linux aarch64-unknown-linux-musl aarch64 || exit /b 1
copy /Y meshcore-manager\README.md dist\manager-linux\README.md >nul || exit /b 1
exit /b 0

rem :linux <target> <folder in dist\manager-linux>
:linux
cargo zigbuild --release -p meshcore-manager --target %~1 || exit /b 1
if not exist "dist\manager-linux\%~2" mkdir "dist\manager-linux\%~2"
copy /Y "target\%~1\release\mcmanager" "dist\manager-linux\%~2\mcmanager" >nul || exit /b 1
echo Built dist\manager-linux\%~2\mcmanager
exit /b 0
