@echo off
rem Construit la livraison Windows, dans target\release\bundle :
rem   nsis\FileSeal_<version>_x64-setup.exe        installateur par utilisateur
rem   portable\FileSeal_<version>_x64-portable.zip  version portable
rem et, a cote de chacun, son fichier .sha256 ecrit par FileSeal lui-meme ;
rem avec la cle des mises a jour, la signature de l'installateur (.sig) et
rem nsis\latest.json pour la release.
rem Procedure de publication : docs\tests-build-livraison.md.
setlocal
cd /d "%~dp0.."

for /f "tokens=3" %%v in ('findstr /b /c:"version = " Cargo.toml') do set VERSION=%%~v
if "%VERSION%"=="" (echo Version introuvable dans Cargo.toml & exit /b 1)
echo === FileSeal %VERSION%

rem Un programme publie ne garde aucun chemin de cet ordinateur. Rust garde
rem ceux des sources des dependances (dans le profil Windows, donc avec le
rem nom de session) pour ses messages d'erreur internes : ils deviennent
rem relatifs. Le controle apres la construction le garantit.
set "RUSTFLAGS=--remap-path-prefix=%USERPROFILE%=~ --remap-path-prefix=%CD%=."

rem Les licences du code des autres projets qui sera dans le programme,
rem mesure sur ce qui est construit : jamais perimees.
node tools\third-party-notices.mjs || exit /b 1

rem Mises a jour : avec la cle privee du projet, gardee hors du depot dans le
rem profil, l'installateur est signe pour le module de mise a jour de Tauri
rem (.sig), et latest.json decrit la version pour la release. Sans la cle,
rem une livraison sans fichiers de mise a jour.
set "UPDATER_KEY=%USERPROFILE%\.tauri\fileseal-updater.key"
set "UPDATER_CONFIG="
del /q "target\release\bundle\nsis\latest.json" 2>nul
if exist "%UPDATER_KEY%" (
  set "TAURI_SIGNING_PRIVATE_KEY=%UPDATER_KEY%"
  set "TAURI_SIGNING_PRIVATE_KEY_PASSWORD="
  set "UPDATER_CONFIG=--config src-tauri\tauri.updater.conf.json"
) else (
  echo Pas de cle de mise a jour : livraison sans fichiers de mise a jour.
)

call npm run tauri build -- %UPDATER_CONFIG% || exit /b 1
cargo build --release -p fileseal-cli || exit /b 1

rem Controle : le chemin du profil ne doit figurer dans rien de ce qui est livre.
powershell -NoProfile -Command "$profil = $env:USERPROFILE; foreach ($f in 'target\release\fileseal.exe', 'target\release\fileseal-cli.exe', 'THIRD-PARTY-NOTICES.txt') { $texte = [Text.Encoding]::GetEncoding(28591).GetString([IO.File]::ReadAllBytes($f)); if ($texte.IndexOf($profil, [StringComparison]::OrdinalIgnoreCase) -ge 0) { Write-Host ($f + ' contient le chemin du profil Windows : livraison refusee.'); exit 1 } }" || exit /b 1

set BUNDLE=target\release\bundle
set SETUP=%BUNDLE%\nsis\FileSeal_%VERSION%_x64-setup.exe
set ZIP=%BUNDLE%\portable\FileSeal_%VERSION%_x64-portable.zip
if not exist "%SETUP%" (echo Installateur introuvable : %SETUP% & exit /b 1)

rem Version portable : le programme, portable.flag, le mode d'emploi, la
rem licence et celles des autres projets, dans un dossier FileSeal.
if exist "%BUNDLE%\portable" rmdir /s /q "%BUNDLE%\portable"
mkdir "%BUNDLE%\portable\FileSeal" || exit /b 1
copy /y target\release\fileseal.exe "%BUNDLE%\portable\FileSeal\FileSeal.exe" >nul || exit /b 1
copy /y scripts\portable\portable.flag "%BUNDLE%\portable\FileSeal\" >nul || exit /b 1
copy /y scripts\portable\README.txt "%BUNDLE%\portable\FileSeal\" >nul || exit /b 1
copy /y LICENSE "%BUNDLE%\portable\FileSeal\LICENSE.txt" >nul || exit /b 1
copy /y THIRD-PARTY-NOTICES.txt "%BUNDLE%\portable\FileSeal\" >nul || exit /b 1
"%SystemRoot%\System32\tar.exe" -a -cf "%ZIP%" -C "%BUNDLE%\portable" FileSeal || exit /b 1
rmdir /s /q "%BUNDLE%\portable\FileSeal"

rem Empreintes : FileSeal ne remplace jamais un fichier de hash existant.
del /q "%SETUP%.sha256" 2>nul
target\release\fileseal-cli.exe create "%SETUP%" "%ZIP%" || exit /b 1

if defined UPDATER_CONFIG (node tools\update-manifest.mjs %VERSION% || exit /b 1)

echo.
echo Livraison :
dir /b "%BUNDLE%\nsis\FileSeal_%VERSION%_*" "%BUNDLE%\portable\*"
if exist "%BUNDLE%\nsis\latest.json" dir /b "%BUNDLE%\nsis\latest.json"
