# ADR 0003 — Livraison Windows : installateur par utilisateur et version portable

**Statut** : accepté — 2026-10-22

## Contexte

Le cadrage (§11) prévoit un installateur NSIS par utilisateur, sans droits administrateur, avec WebView2 inclus pour une installation hors ligne, et une version portable : un zip avec l'exe et un fichier `portable.flag`, les réglages et les données WebView2 gardés à côté de l'exe. Les mises à jour se font à la main, en installant par-dessus.

## Décision

- **Installateur NSIS par utilisateur** (`installMode: currentUser`) : il installe dans `%LOCALAPPDATA%\FileSeal`, avec des raccourcis dans le menu Démarrer et sur le Bureau, et une entrée dans « Applications installées ». Il est en anglais et en français, selon la langue de Windows. La désinstallation garde les réglages, sauf si l'on coche la case prévue.
- **WebView2 n'est pas inclus** : écart par rapport au cadrage. L'installateur garde le mode par défaut de Tauri, qui télécharge WebView2 seulement s'il manque.
  - Pour : un installateur de 3,7 Mo au lieu de plus de 130 Mo. WebView2 fait partie de Windows 11, la cible, et Microsoft l'a aussi installé sur la plupart des Windows 10.
  - Contre : un Windows 10 sans WebView2 et sans Internet ne peut pas installer FileSeal.
  - Pour revenir au cadrage, une ligne suffit : `"webviewInstallMode": { "type": "offlineInstaller" }` dans `bundle.windows` de `tauri.conf.json`.
- **Version portable = le même programme**, plus `portable.flag` et un `README.txt`, dans un zip. Le fichier est cherché au démarrage (`src-tauri/src/portable.rs`) :
  - **réglages, journaux et données WebView2** vont dans `data\`, à côté du programme. Pour WebView2, la fenêtre est créée par le code et reçoit ce dossier ;
  - **les bases du disque du programme** sont enregistrées sans leur lettre. Une clé USB qui passe de E: à F: les retrouve ;
  - **une nouvelle base** est proposée à côté du programme ;
  - **dossier non inscriptible** : FileSeal démarre comme s'il était installé, plutôt que de refuser de démarrer.
- **Version 2.0.0**, en une seule source : `[workspace.package]` de `Cargo.toml`. Elle vaut pour le moteur (`APP_VERSION`, écrite dans les bases et les rapports), l'application et l'installateur, et pour les informations de version de Windows. `package.json` doit dire la même, ce qu'un test vérifie. « 2 », parce que c'est FileSeal 2 : l'ancien FileSeal en Python était la version 1.
- **Empreintes des fichiers livrés** : `scripts\build.cmd` les écrit avec FileSeal lui-même, en fichiers de hash GNU. On peut les contrôler avec FileSeal, `sha256sum -c` ou `certutil`.
- **Ni signature de code, ni mise à jour automatique** pour l'instant : SmartScreen avertit au premier lancement. La mise à jour automatique est l'évolution 9 du cadrage.

## Conséquences

- Une seule instance de FileSeal à la fois, installée et portable confondues : le verrou d'instance unique dépend de l'identifiant de l'application, pas de son emplacement.
- Publier une version suit la procédure de [tests-build-livraison.md](../tests-build-livraison.md) : version, vérifications, contrôles manuels, `scripts\build.cmd`, installation et désinstallation, journal, étiquette.
- Signer le programme demandera un certificat (payant), et changera la procédure de publication.
