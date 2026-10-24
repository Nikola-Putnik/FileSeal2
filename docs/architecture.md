# Architecture

Décision de stack : [ADR 0001](adr/0001-stack-rust-tauri.md). Cadre général : [cadrage.md](cadrage.md) §3.

## Vue d'ensemble

```
ui/ (Svelte 5, TypeScript)
   │  invoke('create_reference', …) + Channel de progression
   ▼
src-tauri/ (coque mince : commandes async, état, journaux)
   │  appels Rust directs
   ▼
crates/fileseal-core/ (moteur, sans interface)          crates/fileseal-cli/ (même moteur, en ligne de commande)
```

Aucune règle métier dans la coque ni dans l'interface : elles transmettent et affichent. Les types échangés sont définis en Rust et générés en TypeScript par `ts-rs` dans `ui/src/lib/generated/` à chaque `cargo test`.

## Le moteur (`fileseal-core`)

| Module | Rôle |
|---|---|
| `algorithm` | `Algorithm` (SHA-256, MD5) et ses métadonnées ; `MultiHasher` calcule plusieurs algorithmes en une lecture. Ajouter un algorithme = une variante + sa crate. |
| `reader` | Lecture en flux par blocs de 1 Mio (mémoire constante), annulation entre deux blocs, contrôle « modifié pendant le calcul » : état avant/après comparé, octets lus = taille. |
| `platform` | Tout ce qui dépend du système, seul module autorisé à utiliser `unsafe` (appels Windows) : <br>• ouverture qui interdit l'écriture, le renommage et la suppression par les autres programmes ; <br>• état d'un fichier ouvert (taille, dates, ChangeTime, identifiant volume + numéro de fichier) ; <br>• attributs (caché, système, « en ligne uniquement ») ; <br>• racine du volume d'un chemin (en se méfiant des lecteurs `subst`) ; <br>• empêcher la mise en veille pendant un calcul ; <br>• chemins absolus, forme étendue `\\?\` pour les appels directs, clé de chemin (avec le partage réseau derrière une lettre de lecteur). |
| `batch` | Les lots : une action sur une liste de fichiers. Un lecteur par volume, plusieurs volumes en parallèle, ordre de la liste, pause, annulation, volume ou base perdus, doublons par identité ; chaque fichier finit avec un résultat ([lots.md](lots.md)). |
| `discovery` | La sélection : des fichiers et dossiers choisis à la liste des fichiers, en arrière-plan et par paquets. Ordre de l'Explorateur, exclusions avec leur raison, filtre d'extension, doublons, liens jamais suivis dans un dossier ([selection.md](selection.md)). |
| `sidecar` | Fichiers de hash : écriture stricte et atomique, lecture tolérante ([formats-hash.md](formats-hash.md)). |
| `store` | Bases SQLite : création, références, vérifications. `store::schema` : ouverture verrouillée, repli en lecture seule, contrôles (identifiant, version, `quick_check`), migrations avec copie préalable ([modele-donnees.md](modele-donnees.md)). |
| `store::browse` | Lire la base dans son ensemble : recherche des références (nom, chemin, début de hash), historique d'un élément, emplacements courants ; renommer la base ; réaffecter beaucoup d'emplacements en une transaction. |
| `store::files` | Le fichier de la base : sauvegarde contrôlée, examen sans modification, déplacement (renommage ou copie contrôlée), restauration après une copie de sécurité. |
| `settings` | `settings.json` : langue, bases récentes, base à rouvrir au démarrage. Un fichier absent ou abîmé donne les réglages par défaut. |
| `ops` | Les actions sur un fichier : `create_reference`, `verify_file` (règles du cadrage n° 2 à 4 et 8–9, rapprochement R1 à R5). Sous-modules : <br>• `manifest` : fichiers listés dans un fichier de hash ouvert (comme `sha256sum -c`) : `list` en fait une liste pour un lot, chaque fichier avec sa ligne ; <br>• `content` : recherche d'un contenu dans la base ; <br>• `search` : l'action Rechercher, qui n'écrit rien ; <br>• `decide` : rattacher et accepter, décisions explicites et tracées ; <br>• `follow` : le suivi automatique des emplacements, quand l'utilisateur l'a choisi ([rapprochement.md](rapprochement.md#suivi-automatique-des-emplacements)) ; <br>• `volumes` : les disques de la base et où ils sont aujourd'hui, réaffectation d'un dossier ou d'un disque ([bases.md](bases.md)). |
| `matching` | Les règles de rapprochement ([rapprochement.md](rapprochement.md)), fonctions pures sans accès disque ni base. |
| `report` | Le rapport CSV ([rapport-csv.md](rapport-csv.md)). |
| `progress` | Progression pondérée par les octets, débit sur 5 s glissantes, temps restant, limitation à 10 événements/s. |
| `problem` | Les erreurs sous une forme que l'interface sait expliquer : un `kind` stable (verrouillé, disque débranché, réseau perdu, secteur illisible…) et un détail technique. Les codes d'erreur Windows sont classés avant les catégories génériques de Rust. |
| `faults` | Injection de pannes, compilée seulement en test : écriture ou publication refusée, erreur système précise en pleine lecture, fichier modifié pendant la lecture. |

## L'interface (`ui/`)

Une seule fenêtre, décrite dans [interface.md](interface.md).

| Fichier | Rôle |
|---|---|
| `App.svelte` | Assemble la fenêtre : action, réglages, liste, lancement, progression, résultats, détail ; raccourcis clavier, glisser-déposer, décisions. |
| `lib/list.ts` | La liste en données pures : tri (ordre de l'Explorateur), déplacement, retrait, filtres, statistiques par extension, dossier commun. Testée par Vitest. |
| `lib/results.ts` | Statut d'un résultat, et résultats concernés par chaque décision pour tous (R2, R5, nouvelles versions). Testé par Vitest. |
| `lib/selection.svelte.ts` | Remplit la liste : parcours des dossiers par le moteur, fichiers de hash développés, doublons, écartés et problèmes. |
| `lib/run.svelte.ts` | Un lot vu de la page : résultats par ligne, progression, bilan, décisions prises. |
| `lib/components/VirtualGrid.svelte` | Grille ARIA virtualisée : seules les lignes visibles sont dessinées ; clavier complet. |
| `lib/components/DatabasesScreen.svelte` | L'écran Bases : `DatabaseCard` (base active), `DisksPanel` (disques, réaffectation), `ReferencesPanel` (recherche, historique). |
| `lib/components/*` | Réglages, liste, progression, résultats, détail ; les composants de résultat d'un fichier (création, vérification, recherche) sont réutilisés dans le détail. |
| `lib/api.ts` | Les appels aux commandes Rust, typés par les types générés. |
| `lib/i18n/` | Textes en anglais (référence) et en français ; chaque dictionnaire met en forme ses nombres. |

## Fils d'exécution

- Les commandes Tauri sont toutes `async` : une commande synchrone tournerait sur le fil principal et gèlerait la fenêtre.
- Un travail long (calcul) tourne sur un fil bloquant (`spawn_blocking`). Il envoie sa progression par un `Channel`, au plus 10 fois par seconde.
- Une seule opération à la fois. La base active est un `Mutex<Option<Store>>` : l'opération en cours la garde pour toute sa durée.
- **Lots** : le fil bloquant lance un lecteur par volume (`std::thread::scope`, 1 à 4 fils) et rassemble lui-même les résultats, reçus par un canal, pour les envoyer à l'interface au plus 10 fois par seconde. Les lecteurs partagent la base par un `SharedStore` (`Mutex<&mut Store>`), verrouillé le temps d'une requête ou d'une écriture, jamais pendant une lecture de fichier.

  Le plan prévoyait un fil unique propriétaire de la base, à qui les lecteurs auraient envoyé leurs demandes. Le verrou bref donne la même garantie, une seule écriture à la fois, avec moins de code : les opérations interrogent la base à plusieurs moments, entre deux accès au fichier. Un fil propriétaire aurait demandé un aller-retour de messages à chaque fois.
- Annulation et pause : `Control`, partagé, consulté entre deux blocs de lecture (`checkpoint`), avant chaque fichier d'un lot, et entre deux éléments pendant un parcours de dossier. Une pause attend sur une variable de condition, réveillée par la reprise ou l'annulation.
- Veille : chaque calcul garde le système éveillé (`platform::keep_awake`) le temps de son exécution, sur le fil qui fait la demande.
- Fermeture : pendant une opération, la demande de fermeture de la fenêtre est retenue (`CloseRequested`), et la page pose la question. `close_confirmed` annule l'opération, attend sa fin (5 s au plus), ferme la base et quitte.
- **Sortie de l'application** (`RunEvent::Exit`) : la base active est fermée explicitement. Tauri termine le processus sans libérer son état ; sans cela, le journal `-wal` de SQLite restait à côté de la base jusqu'à l'ouverture suivante, et une copie du seul fichier `.fileseal` aurait été incomplète.

## Mode portable

`src-tauri/src/portable.rs`. Un fichier `portable.flag` à côté du programme est cherché au démarrage, avant la création de la fenêtre :
- **réglages, journaux et données WebView2** vont dans `data\` à côté du programme. La fenêtre est donc créée par le code (`create: false` dans `tauri.conf.json`), pour recevoir ce dossier de données (`data_directory`) ;
- **les chemins des bases** situées sur le disque du programme sont écrits sans leur lettre (`Settings::without_drive`), puis la reprennent à la lecture (`with_drive`) : une clé USB qui change de lettre retrouve ses bases ;
- **une nouvelle base** est proposée à côté du programme (`AppInfo::database_folder`) ;
- **un dossier où l'on ne peut pas écrire** (support en lecture seule) : FileSeal démarre comme s'il était installé, et le journal le signale.

Sans ce fichier, les emplacements sont ceux de Tauri pour l'identifiant `org.fileseal.desktop` : réglages dans `%APPDATA%`, journaux et WebView2 dans `%LOCALAPPDATA%`.
- **Ordre des messages** : la réponse d'une commande peut arriver avant les derniers messages de son `Channel` (les gros messages passent par un autre chemin). Quand le dernier message compte, comme pour la découverte, il porte lui-même une marque de fin, que l'interface attend.


## Déroulement d'une création de référence

1. Chercher les références existantes : les fichiers de hash présents (demandés ou non), puis l'entrée de la base pour ce chemin.
2. Lire le fichier une seule fois, avec les algorithmes demandés et ceux des références existantes.
3. Comparer les références existantes au contenu lu. Si l'une est contredite ou illisible, **rien n'est écrit** (statut « Conflit »).
4. Sinon, écrire ce qui manque : fichiers de hash par publication atomique, base en une transaction.
5. Statut final : un échec sur une seule destination donne « Réussite partielle », jamais « Créé ».

## Déroulement d'une vérification

1. Rassembler les références : fichiers de hash à côté du fichier, entrée de la base par chemin.
2. Sans aucune référence utilisable, conclure sans lire le fichier (« Aucune référence », « Fichier de hash invalide »).
3. Lire le fichier en entier avec tous les algorithmes nécessaires, puis comparer chaque source.
4. Si toutes correspondent : « Conforme ». Si aucune ne correspond : « Différent ». Si certaines correspondent et d'autres non : « Références contradictoires ».
5. Enregistrer la vérification dans l'historique, uniquement pour un élément connu de la base. L'empreinte observée est conservée.
