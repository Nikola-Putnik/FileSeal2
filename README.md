# FileSeal

Application de bureau pour **contrôler l'intégrité de fichiers dans le temps** : FileSeal enregistre l'empreinte (hash) d'un fichier, puis vérifie plus tard que son contenu n'a pas changé. Les références peuvent être stockées :

- dans un **fichier de hash** à côté du fichier (`film.mkv.sha256`), compatible avec `sha256sum -c` et `md5sum -c` ;
- dans une **base FileSeal** (fichier SQLite `.fileseal`), qui garde l'historique des vérifications.

Les fichiers sources ne sont jamais modifiés. Une vérification ne remplace jamais une référence.

**État** : version 2.4.0, pour Windows 11 (64 bits). macOS et Linux ne sont pas encore pris en charge.
L'interface est en anglais, avec une traduction française au choix (bouton ⚙). Une seule fenêtre : on choisit l'action, on ajoute des fichiers et des dossiers (ou on les glisse dans la fenêtre), on lance, puis on lit les résultats et on décide ([docs/interface.md](docs/interface.md)). Nouveautés : [CHANGELOG.md](CHANGELOG.md). Décisions : [docs/adr/](docs/adr/).

## Installer

- **Installateur** : `FileSeal_2.4.0_x64-setup.exe`, pour l'utilisateur courant, sans droits administrateur.
- **Version portable** (clé USB) : `FileSeal_2.4.0_x64-portable.zip`, à décompresser, puis lancer `FileSeal.exe`. Rien n'est écrit dans le profil Windows.

Mode d'emploi complet : [docs/guide-utilisateur.md](docs/guide-utilisateur.md). Construire ces fichiers : `scripts\build.cmd` ([docs/tests-build-livraison.md](docs/tests-build-livraison.md)).

## Lancer en développement

Prérequis : Rust stable (MSVC), Visual Studio Build Tools 2022, Node.js 24, WebView2 (présent sur Windows 11).

```
npm install
scripts\dev.cmd          (ou npm run tauri dev)
scripts\verify.cmd       toutes les vérifications : format, lint, types, tests
```

Ligne de commande de test, sans interface :

```
cargo run -p fileseal-cli -- init-db essais.fileseal --name Essais
cargo run -p fileseal-cli -- create "film.mkv" --db essais.fileseal --algo sha256 --algo md5
cargo run -p fileseal-cli -- verify "film.mkv" --db essais.fileseal
cargo run -p fileseal-cli -- verify "D:\Films" "E:\Sauvegarde" --db essais.fileseal --parallel 2
cargo run -p fileseal-cli -- scan "D:\Films" --ext mkv --ext avi
```

## Organisation

| Dossier | Rôle |
|---|---|
| `crates/fileseal-core` | Le moteur : lecture et calcul, fichiers de hash, bases, règles. Sans interface, testé par `cargo test`. |
| `crates/fileseal-cli` | Ligne de commande de développement et de test. |
| `src-tauri` | La coque Tauri : expose le moteur à l'interface. |
| `ui` | L'interface (Svelte 5, TypeScript). |
| `docs` | Documentation en français : [guide utilisateur](docs/guide-utilisateur.md), [cadrage](docs/cadrage.md), [architecture](docs/architecture.md), [formats des fichiers de hash](docs/formats-hash.md), [rapprochement](docs/rapprochement.md), [rapport CSV](docs/rapport-csv.md), [sélection](docs/selection.md), [lots](docs/lots.md), [interface](docs/interface.md), [écran Bases](docs/bases.md), [modèle de données](docs/modele-donnees.md), [développement](docs/installation-developpement.md), [limites connues](docs/limites.md), [décisions](docs/adr/). |
