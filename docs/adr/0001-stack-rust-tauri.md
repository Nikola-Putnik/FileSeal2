# ADR 0001 — Stack : Rust (moteur) + Tauri 2 + Svelte 5 / TypeScript (interface)

**Statut** : accepté — 2026-10-22

## Contexte

Application desktop locale de contrôle d'intégrité : fichiers de plusieurs centaines de Go, collections de dizaines de milliers de fichiers, Windows d'abord, portage macOS/Linux préparé. Priorités : fiabilité (aucune erreur masquée, aucune perte de référence), performance, accessibilité, livraison simple. L'utilisateur connaît surtout Python et Java, et reste ouvert à d'autres langages.

## Options comparées

| Critère | Rust + Tauri 2 | Python + PySide6 | Java 25 + JavaFX |
|---|---|---|---|
| Langage connu | non | oui | oui |
| Fiabilité (erreurs, types) | excellente : chaque erreur et chaque statut doivent être traités | moyenne : typage optionnel | bonne |
| Performances | excellentes (SHA matériel, mémoire bornée sans GC) | bonnes (hashlib en C) | très bonnes |
| Taille livrée | ~10 Mo (WebView2 déjà présent sur Windows 11) | 100 Mo ou plus | ~70 Mo |
| Accessibilité | très bonne si HTML sémantique | très bonne (Qt) | plus limitée |
| Portage | bon (Linux : WebKitGTK à valider) | très bon | bon |
| Tests sans interface | `cargo test` | pytest | JUnit |
| Livraison | installateur et portable fournis par Tauri | PyInstaller, faux positifs antivirus | jpackage + WiX |

## Décision

Rust + Tauri 2 + Svelte 5 / TypeScript.
- Toute la logique métier est dans la crate `fileseal-core`, sans dépendance à Tauri ni à l'UI.
- La coque `src-tauri` se limite à exposer le moteur.
- `fileseal-cli` est une ligne de commande de développement qui prouve que le moteur ne dépend pas de l'interface.

## Conséquences

- Rust et TypeScript : le code commente les raisons et les cas délicats, et la documentation est en français.
- Frontière IPC : types TypeScript générés depuis Rust par `ts-rs` (stable), plutôt que `tauri-specta` (encore en version candidate en octobre 2026).
- Commandes Tauri toujours `async` : une commande synchrone tourne sur le fil principal et gèle la fenêtre.
- Linux : WebKitGTK devra être validé avant d'annoncer le portage.
