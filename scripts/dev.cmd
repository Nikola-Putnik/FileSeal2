@echo off
rem Lance FileSeal en mode developpement (rechargement a chaud de l'interface).
setlocal
cd /d "%~dp0.."
call npm run tauri dev
