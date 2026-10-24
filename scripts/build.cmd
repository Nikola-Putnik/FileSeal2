@echo off
rem Construit la livraison Windows, dans target\release\bundle :
rem   nsis\FileSeal_<version>_x64-setup.exe        installateur par utilisateur
rem   portable\FileSeal_<version>_x64-portable.zip  version portable
rem et, a cote de chacun, son fichier .sha256 ecrit par FileSeal lui-meme.
rem Procedure de publication : docs\tests-build-livraison.md.
setlocal
cd /d "%~dp0.."

for /f "tokens=3" %%v in ('findstr /b /c:"version = " Cargo.toml') do set VERSION=%%~v
if "%VERSION%"=="" (echo Version introuvable dans Cargo.toml & exit /b 1)
echo === FileSeal %VERSION%

call npm run tauri build || exit /b 1
cargo build --release -p fileseal-cli || exit /b 1

set BUNDLE=target\release\bundle
set SETUP=%BUNDLE%\nsis\FileSeal_%VERSION%_x64-setup.exe
set ZIP=%BUNDLE%\portable\FileSeal_%VERSION%_x64-portable.zip
if not exist "%SETUP%" (echo Installateur introuvable : %SETUP% & exit /b 1)

rem Version portable : le programme, portable.flag et le mode d'emploi,
rem dans un dossier FileSeal.
if exist "%BUNDLE%\portable" rmdir /s /q "%BUNDLE%\portable"
mkdir "%BUNDLE%\portable\FileSeal" || exit /b 1
copy /y target\release\fileseal.exe "%BUNDLE%\portable\FileSeal\FileSeal.exe" >nul || exit /b 1
copy /y scripts\portable\portable.flag "%BUNDLE%\portable\FileSeal\" >nul || exit /b 1
copy /y scripts\portable\README.txt "%BUNDLE%\portable\FileSeal\" >nul || exit /b 1
"%SystemRoot%\System32\tar.exe" -a -cf "%ZIP%" -C "%BUNDLE%\portable" FileSeal || exit /b 1
rmdir /s /q "%BUNDLE%\portable\FileSeal"

rem Empreintes : FileSeal ne remplace jamais un fichier de hash existant.
del /q "%SETUP%.sha256" 2>nul
target\release\fileseal-cli.exe create "%SETUP%" "%ZIP%" || exit /b 1

echo.
echo Livraison :
dir /b "%BUNDLE%\nsis\FileSeal_%VERSION%_*" "%BUNDLE%\portable\*"
