# Installation et développement

## Prérequis (Windows)

- Rust stable, cible `x86_64-pc-windows-msvc` (installé par `rustup`).
- Visual Studio Build Tools 2022, charge de travail « Développement Desktop en C++ ».
- Node.js 24 et npm.
- WebView2, présent d'office sur Windows 11.
- Git for Windows. Ses `sha256sum` et `md5sum` servent aux tests de compatibilité, qui sont ignorés si ces outils sont absents.

Si `cargo` n'est pas trouvé dans un shell Git Bash : `export PATH="$HOME/.cargo/bin:$PATH"`.

## Commandes

| Commande | Effet |
|---|---|
| `npm install` | Dépendances de l'interface et CLI Tauri. |
| `scripts\dev.cmd` (ou `npm run tauri dev`) | L'application, avec rechargement à chaud de l'interface. Une modification du code Rust relance l'application. |
| `scripts\verify.cmd` | Toutes les vérifications, dans l'ordre : Prettier, ESLint, svelte-check, Vitest, `cargo fmt`, `cargo clippy -D warnings`, `cargo test`. |
| `cargo test --workspace` | Tests Rust. **Régénère aussi les types TypeScript** dans `ui/src/lib/generated/` : à relancer après toute modification d'un type Rust exporté. |
| `cargo run -p fileseal-cli -- …` | La ligne de commande de test : `init-db`, `create`, `verify`, `search` (fichiers et dossiers, traités en un lot : [lots.md](lots.md)), `scan` (sélection : fichiers retenus, exclusions, erreurs, récapitulatif). `create --existing skip-complete|skip-any` saute les fichiers déjà référencés, et `--import-hash-files` ajoute leurs fichiers de hash à la base ; sans `--existing`, la CLI relit tout, comme avant (les scripts de test en dépendent). |
| `cargo run -p fileseal-core --release --example throughput -- 2048` | Débits de calcul sur un fichier de 2 Gio en cache, par algorithme. Résultats à consigner dans [performances.md](performances.md). |
| `cargo run -p fileseal-core --release --example disk_read -- <dossier> 4096` | Débits avec le disque : fichiers écrits sans passer par le cache, lus par le disque seul puis par FileSeal. |
| `cargo run -p fileseal-core --release --example costs -- <dossier>` | Coût des appels système faits pour chaque fichier (petits fichiers). |
| `powershell -ExecutionPolicy Bypass -File tools\manual\real-cases.ps1` | Les statuts produits par des situations réelles, réseau compris ([tests-build-livraison.md](tests-build-livraison.md)). |
| `scripts\build.cmd` | La livraison Windows : installateur NSIS, zip portable et leurs fichiers `.sha256`, dans `target\release\bundle\` ([tests-build-livraison.md](tests-build-livraison.md)). `npm run tauri build` ne fait que l'installateur. |

Les scripts sont des `.cmd`, car les scripts PowerShell `.ps1` sont bloqués par la stratégie d'exécution par défaut de Windows.

## Conventions

- Code et commentaires en anglais ; documentation (`docs/`) en français. Interface en anglais par défaut, français en option ([ADR 0002](adr/0002-langue-interface.md)).
- Les commentaires expliquent les raisons et les cas délicats, pas ce que le code dit déjà.
- Les textes de l'interface sont dans `ui/src/lib/i18n/en.ts` (référence) et `fr.ts` (mêmes clés, vérifiées par TypeScript et par `i18n.test.ts`), jamais écrits en dur dans un composant. Les icônes et tons des statuts sont dans `ui/src/lib/labels.ts`.
- Commits au format conventionnel, en anglais (`feat(core): …`), avec un paragraphe qui explique le pourquoi.
- Chaque version ajoute une entrée à [CHANGELOG.md](../CHANGELOG.md). Une décision structurante donne lieu à un ADR dans [adr/](adr/).
- Le moteur n'a aucune dépendance à Tauri. Le code propre à un système d'exploitation reste dans `platform.rs`.

## Tests

- **Unitaires et scénarios** (`cargo test`) : hash connus, lecture en flux, fichiers de hash, base, créations et vérifications de bout en bout dans le moteur. Les pannes (écriture refusée, disque plein…) sont simulées par `faults.rs`, compilé seulement en test.
- **Lots** : plusieurs « volumes » sont simulés par des dossiers ; disque débranché, erreur de lecture, base pleine et disque lent sont injectés (`faults.rs`, armé par dossier pour atteindre les fils des lecteurs).
- **Volume qui disparaît pour de vrai** (procédure manuelle) : `subst W: <dossier de test>`, lancer `fileseal-cli verify W:\… --parallel 1` sur un gros fichier suivi de petits, puis `subst W: /D` pendant la lecture. Attendu : le fichier en cours conforme (son handle reste valide), le suivant « introuvable », les autres « non traité », et le volume `W:\` signalé perdu.
- **Liens symboliques** : leur test n'est exécuté que si Windows permet d'en créer (mode développeur activé, ou droit « Créer des liens symboliques ») ; sinon il le signale et passe. Les jonctions, elles, sont toujours testées.
- **Test long** : `cargo test -p fileseal-core --release -- --ignored fifty_thousand --nocapture` parcourt 50 000 fichiers et affiche les temps.
- **Compatibilité GNU** (`crates/fileseal-core/tests/gnu_compat.rs`) : `sha256sum -c` et `md5sum -c` de Git for Windows valident les fichiers produits.
- **Interface** (`npm run test`) : fonctions de mise en forme.
- **De bout en bout dans l'application** : on pilote la vraie fenêtre par le protocole DevTools de WebView2.
  1. Lancer une instance isolée en arrière-plan :
     `FILESEAL_CONFIG_DIR=<dossier temporaire> WEBVIEW2_USER_DATA_FOLDER=<dossier temporaire> WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9232 npm run tauri dev`
     `FILESEAL_CONFIG_DIR` remplace le dossier des réglages (`settings.json`) : une instance de test ne touche jamais aux réglages réels.
  2. `cdp.mjs` n'agit que sur une page intitulée « FileSeal ». **Ports de FileSeal** : 1430 pour le serveur de développement (Vite, `vite.config.ts` et `devUrl` de `tauri.conf.json`) et 9232 pour le débogage (`cdp.mjs`). Ce ne sont pas les ports par défaut (1420 du modèle Tauri, 9222 habituel) : d'autres projets Tauri en développement sur le même ordinateur les prennent, et la fenêtre charge ce qui répond sur son `devUrl`. Si l'un d'eux est quand même occupé, ne pas lancer `tauri dev` : construire avec `npx tauri build --no-bundle`, lancer `target\release\fileseal.exe` avec `--remote-debugging-port=9233`, et ajouter `CDP_PORT=9233` aux commandes ci-dessous.
  3. Piloter avec `node tools/e2e/cdp.mjs eval @script.js` et capturer l'écran avec `node tools/e2e/cdp.mjs shot capture.png [full] [1920x1040] [prefers-color-scheme=light]`. La taille et les caractéristiques émulées ne valent que pour cette capture. Autres commandes : `keys Tab Shift+Tab ArrowDown Control+a…` (vraies frappes, puis l'élément qui a le focus), `ax grid row tab…` (arbre d'accessibilité : rôles, noms, états), `media forced-colors=active` (émulation d'une media query). Dans la page, `window.__TAURI_INTERNALS__.invoke('commande', {…})` appelle une commande Rust. Pour une commande à `Channel`, importer le module de l'interface : `const { api } = await import('/src/lib/api.ts')` (servi par Vite en développement).
  4. La demande de fermeture de la fenêtre se provoque comme un vrai clic sur la croix, depuis PowerShell : `(Get-Process -Id <pid de fileseal.exe>).CloseMainWindow()`.
  5. Un glisser-déposer se simule par `invoke('plugin:event|emit', { event: 'tauri://drag-drop', payload: { paths: [...], position: { x: 0, y: 0 } } })`.

  Règles :
  - **ne jamais cliquer sur un bouton qui ouvre une boîte de dialogue native** (elle s'ouvrirait à l'écran et bloquerait le pilotage) : appeler la commande directement, par exemple `database_create` ;
  - travailler sur des fichiers et des bases de test dans un dossier temporaire, jamais sur des données réelles ;
  - avant de lancer, vérifier que les ports 1430 et 9232 sont libres (`curl http://127.0.0.1:9232/json`) : un script envoyé à la fenêtre d'une autre application y agirait pour de vrai ;
  - arrêter ensuite ses propres processus **par PID** (`fileseal.exe`, `cargo run`, `vite`), jamais par nom.

## Journaux

Fichiers quotidiens `fileseal.<date>.log` dans `%LOCALAPPDATA%\org.fileseal.desktop\logs\`, 14 jours conservés. Niveau réglable par la variable `FILESEAL_LOG` (par exemple `FILESEAL_LOG=debug`). Les journaux contiennent des chemins, des statuts et des erreurs, jamais le contenu des fichiers.
