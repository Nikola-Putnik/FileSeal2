# FileSeal2 — Cadrage et plan de réalisation

> Cadrage initial du 2026-10-22. Ce document fixe le cadre de départ ; les décisions prises ensuite sont dans les ADR de [adr/](adr/), les évolutions dans [CHANGELOG.md](../CHANGELOG.md).

## Contexte

Application desktop de calcul et vérification de hash pour contrôler l'intégrité de fichiers dans le temps (collections de films, sauvegardes, disques externes, partages réseau). Windows 11 d'abord, portage macOS/Linux préparé mais non annoncé. Le dossier `C:\MyPC\Projects\FileSeal2` est vide : on part de zéro. L'ancien FileSeal (Python) n'est pas une base, mais ses fichiers `.md5` existants (`MD5: <hash>  <nom>`, UTF-8 ou cp1252, CRLF) doivent rester lisibles.

**Stack décidée** : Rust (moteur) + Tauri 2 (coque) + Svelte 5 / TypeScript (interface). Rust 1.99, VS Build Tools 2022 et Node 24 sont installés. Git for Windows fournit `sha256sum`/`md5sum` (coreutils 8.32), qui servira d'outil indépendant pour les tests de compatibilité.

Les points techniques dont dépend la conception ont été vérifiés : renommage sans remplacement, jonctions, chemins longs, identité de fichier, format GNU, SQLite, Tauri.

---

## 1. Reformulation

FileSeal enregistre l'empreinte d'un contenu (**référence**), puis contrôle plus tard que ce contenu n'a pas changé. Il y a deux supports de référence, indépendants et cumulables :
- **fichier de hash** à côté du fichier (`film.mkv.sha256`), portable, compatible GNU ;
- **base SQLite** (plusieurs bases possibles), qui ajoute l'historique, la recherche par contenu et le suivi des déplacements.

Quatre actions, jamais confondues :
1. **Créer** une référence.
2. **Vérifier** un fichier contre la référence attendue.
3. **Rechercher** si un contenu est déjà connu d'une base (fichier déplacé, renommé, copie).
4. **Accepter** explicitement une nouvelle version comme référence. C'est la seule façon de remplacer une référence.

Invariants :
- un fichier source n'est jamais ouvert en écriture ;
- une vérification ne modifie jamais une référence ;
- un hash différent veut dire « contenu modifié », pas « corrompu » ;
- aucune empreinte partielle n'est publiée ;
- une réussite partielle n'est jamais présentée comme une réussite.

---

## 2. Ambiguïtés relevées et comportements par défaut proposés

| # | Sujet | Comportement par défaut proposé |
|---|---|---|
| 1 | Qu'est-ce qu'un fichier suivi en base ? | Un **élément** est une copie physique suivie dans le temps. Son identité est un identifiant interne, jamais le chemin. Deux copies identiques (par exemple une sauvegarde sur un autre disque) sont deux éléments distincts, reliés par leur contenu (vue « exemplaires »). Un déplacement change l'emplacement d'un élément, pas son identité. Deux éléments actifs ne peuvent pas partager le même emplacement courant. |
| 2 | Créer une référence pour un fichier déjà référencé | Contenu identique : « Déjà référencé », rien n'est réécrit. Contenu différent : « Conflit avec la référence existante », rien n'est écrasé, et l'action **Accepter comme nouvelle version** est proposée. Contenu déjà connu à un autre emplacement : la référence est créée, avec la mention « contenu déjà connu » et l'action « Rattacher (déplacement) ». |
| 3 | Fichier de hash déjà présent | Identique : conservé tel quel, même s'il est au format ancien. Différent : conflit, rien n'est écrasé. Invalide ou illisible : signalé, rien n'est écrasé ; le remplacement passe par une action explicite. |
| 4 | Plusieurs algorithmes alors qu'une seule référence existe (par exemple `.md5` présent, `.sha256` absent) | Le fichier est lu une seule fois. Un nouvel algorithme est ajouté **seulement si la référence existante a été vérifiée conforme pendant cette même lecture** (statut « Complété »). Si elle est contredite ou invalide, **rien n'est écrit** : on ne valide pas en silence un contenu modifié. |
| 5 | Accepter une nouvelle version | On accepte l'empreinte **observée lors de la vérification**, avec sa date, conservée en base, sans relire le fichier. La référence précédente reste dans l'historique et la nouvelle porte le lien vers la vérification d'origine. Le fichier de hash est alors remplacé de façon atomique. Une option permet de relire le fichier avant d'accepter. |
| 6 | Vérifier depuis une base un fichier dont l'emplacement est inconnu | Pas de « Conforme » automatique. Une recherche par contenu affiche « Déplacement probable » ou « Copie d'un contenu connu », avec l'action **Rattacher** à confirmer (confirmation possible pour tout un lot). |
| 7 | Disque externe dont la lettre change (E: devient F:) | L'emplacement est aussi enregistré comme numéro de série du volume + chemin relatif au volume. Si les deux correspondent, l'emplacement est reconnu (« lecteur E: → F: ») et la mise à jour des chemins est proposée pour tout le lot. L'action « Réaffecter une racine » couvre les autres cas. |
| 8 | Sources de référence pendant une vérification | Au choix : base active, fichiers de hash, ou les deux (par défaut les deux si une base est active). Le résultat est détaillé par source. Si les sources se contredisent : statut **Conflit**, avec les deux valeurs affichées. |
| 9 | Algorithmes utilisés pour vérifier | Tous ceux présents dans la référence et pris en charge, calculés en une seule lecture. Sans algorithme commun : « Aucune référence compatible ». Deux algorithmes différents ne sont jamais comparés. |
| 10 | Exclusions par défaut | Elles s'appliquent **seulement au parcours d'un dossier**. Un fichier sélectionné directement est toujours retenu, et un fichier de hash sélectionné directement mène à la vérification. Sont exclus, avec leur raison et leur nombre : <br>• fichiers cachés et système (une case permet de les inclure) ; <br>• `$RECYCLE.BIN` et `System Volume Information` (toujours exclus) ; <br>• fichiers de hash ; <br>• bases `.fileseal` et leurs annexes ; <br>• fichiers temporaires de FileSeal ; <br>• fichiers « en ligne uniquement » (OneDrive) : les lire déclencherait leur téléchargement. |
| 11 | Liens symboliques et jonctions | Un lien sélectionné directement est suivi. Pendant le parcours d'un dossier, les liens et jonctions **ne sont pas suivis** et apparaissent dans les exclusions, ce qui rend les boucles impossibles. |
| 12 | Doublons de sélection | Un même fichier physique n'apparaît qu'une fois, avec la mention « doublon ignoré ». Cela couvre un même chemin, un lien physique, ou un lecteur mappé et son chemin UNC. Comparaison : clé de chemin d'abord, puis identifiant de fichier (volume + numéro de fichier ; un identifiant 0 est traité comme inconnu). Des fichiers distincts au contenu identique restent des entrées distinctes. |
| 13 | Extension | Partie après le dernier point, sauf si ce point est en tête du nom (`.bashrc` n'a pas d'extension, `a.tar.gz` a l'extension `gz`). Le filtre ignore la casse (Unicode) et propose l'entrée « (sans extension) ». Sans filtre, tous les fichiers éligibles sont retenus. |
| 14 | Ordre et concurrence | Un lecteur par volume, et des volumes différents en parallèle (2 au plus par défaut, réglable de 1 à 4). L'ordre de la liste est l'ordre de démarrage, et les résultats sont affichés dans cet ordre. Limite : deux partitions d'un même disque comptent pour deux volumes. |
| 15 | Lecture des fichiers | Ouverture en lecture seule qui **interdit l'écriture aux autres programmes pendant la lecture**. La détection des modifications concurrentes devient fiable. Un fichier déjà ouvert en écriture ailleurs donne « Verrouillé / en cours d'utilisation ». |
| 16 | Volume ou partage perdu en cours de lot | Le fichier en cours passe en « Erreur de lecture ». Les fichiers restants de ce volume passent en « Non traité (volume indisponible) ». Les autres volumes continuent. |
| 17 | Base indisponible en cours de lot (disque plein, perte) | Le lot s'arrête proprement. Les résultats déjà obtenus restent affichés et exportables, et les fichiers restants passent en « Non traité ». Un échec d'écriture d'un fichier de hash ne concerne que ce fichier (réussite partielle). |
| 18 | Vérification à partir des seuls fichiers de hash | Rien n'est écrit en base. Si une base est active, la vérification est historisée pour les éléments qu'elle reconnaît. |
| 19 | Rapport | CSV en UTF-8 avec BOM et séparateur `;` (Excel en français l'ouvre directement), colonnes documentées. JSON plus tard. |
| 20 | Dates et noms Unicode | Dates stockées en UTC, affichées en heure locale. Aucune normalisation Unicode pour l'identité. Les recherches par nom utilisent une clé NFC sans casse. Un nom non représentable en UTF-8 (cas très rare) donne une erreur pour ce fichier. |
| 21 | Clé de chemin | Elle sert seulement à retrouver un emplacement, jamais à établir l'identité. Insensible à la casse sous Windows et macOS, sensible sous Linux. Chemins absolus sans préfixe `\\?\` à l'affichage. |
| 22 | Pendant un lot | Le PC est empêché de se mettre en veille, et fermer la fenêtre demande une confirmation. |
| 23 | Langues et nom du produit | Interface en français avec des textes centralisés (traduction possible plus tard). Code et commentaires en anglais, documentation en français. Produit « FileSeal » version 2. Les bases ont l'extension `.fileseal` (fichier SQLite). |

**Choix les plus structurants** (ce sont eux qui changent le plus le fonctionnement) :
- n° 1, une copie = un élément ;
- n° 4, pas de nouvelle référence sur un contenu contredit ;
- n° 5, accepter l'empreinte déjà observée sans relire ;
- n° 10, fichiers cachés exclus pendant le parcours ;
- n° 15, interdire l'écriture aux autres programmes pendant la lecture ;
- la distribution (§11).

---

## 3. Architecture

```
FileSeal2/
  Cargo.toml                  workspace (edition 2024), lints clippy, unsafe interdit hors platform/
  crates/fileseal-core/       métier, sans Tauri ni UI, testable par cargo test
    src/algorithm.rs          enum Algorithm + métadonnées ; petit trait Hasher maison (update/finish)
    src/reader.rs             lecture en flux, annulation, pause, instantané avant/après par le handle
    src/platform/             identité de fichier, attributs, clé de chemin, volume, réseau, veille (windows.rs, unix.rs)
    src/discovery/            parcours en arrière-plan, filtres, exclusions, dédoublonnage
    src/sidecar/              format GNU : lecture tolérante, écriture stricte et atomique
    src/store/                SQLite (rusqlite bundled), migrations/*.sql, dépôts
    src/matching.rs           règles de rapprochement, pures et sans E/S
    src/engine/               lots, ordonnanceur par volume, progression, événements, résultats par destination
    src/report.rs             export CSV
  crates/fileseal-cli/        petite ligne de commande de développement et de test (create, verify, search)
  src-tauri/                  coque mince : commandes async, état, relais des événements
  ui/                         Svelte 5 + TypeScript + Vite
  docs/                       documentation en français + adr/
  scripts/                    .cmd (vérifications, build), car les .ps1 sont bloqués sur ce PC
```

Principes :
- **Moteur sur fils natifs**, sans async :
  - un fil de découverte par sélection ;
  - un lecteur par volume ;
  - **un seul fil propriétaire de la connexion SQLite**, qui reçoit les résultats et écrit en transactions courtes.
- **Tauri** :
  - toutes les commandes sont `async` (une commande synchrone tourne sur le fil principal et gèle l'interface) ;
  - les travaux longs tournent sur un `std::thread` ;
  - la progression passe par `tauri::ipc::Channel`, limitée à environ 10 mises à jour par seconde ;
  - plugins `single-instance` et `dialog` ;
  - permissions minimales et CSP stricte.
- **Types partagés** : types TypeScript générés par `ts-rs` (stable ; `tauri-specta` n'est encore qu'en version candidate), plus un `api.ts` écrit à la main. Une vérification échoue si les types générés ne sont plus à jour.
- **Extensibilité** : ajouter un algorithme = une variante de `Algorithm` + une crate. Les fichiers de hash, la base et l'interface lisent les métadonnées de l'enum.
- **Portage** : tout ce qui dépend du système vit dans `platform/`.
- **Testabilité** : des points d'injection de pannes, compilés seulement en test, sur l'écriture, le renommage et la lecture. Ils permettent de tester disque plein, débranchement et erreur réseau sans matériel.
- **Dépendances** :
  - `sha2` 0.11, `md-5` 0.11 ;
  - `rusqlite` 0.40 (bundled, backup) ;
  - `tempfile` (`persist_noclobber`), `file-id`, `windows-sys` ;
  - `encoding_rs` (cp1252), `unicode-normalization` ;
  - `serde`, `thiserror`, `tracing`, `ts-rs`.

---

## 4. Formats des fichiers de hash

**Nom** : `<nom du fichier source>.<algo>` en minuscules (`.md5`, `.sha256`), un fichier par algorithme, dans le même dossier que le fichier.

**Écriture** (stricte) :
- UTF-8 sans BOM ;
- une seule ligne `<hex minuscule><2 espaces><nom>`, terminée par **LF** (coreutils 8.32, la version livrée avec Git for Windows, refuse le CRLF) ;
- nom de base seul, relatif au fichier de hash, pour que le fichier et son fichier de hash puissent être déplacés ensemble ;
- échappement GNU : si le nom contient `\`, LF ou CR, la ligne commence par `\` et ces caractères sont échappés en `\\`, `\n` et `\r`. Ce cas est impossible avec un nom Windows, mais il est géré pour le portage et pour les fichiers venus de Linux ;
- publication atomique :
  - écriture dans un fichier temporaire du même dossier (`.<nom>.<algo>.fileseal-tmp`), puis `sync_all` ;
  - renommage **sans remplacement** (`persist_noclobber`, c'est-à-dire `MoveFileExW` sans remplacement, valable aussi sur exFAT et sur un partage SMB) ;
  - en cas d'échec, le temporaire est supprimé ;
  - le remplacement d'un fichier existant n'a lieu que lors d'une acceptation explicite.

**Lecture** (tolérante). Formes acceptées :
- BOM UTF-8 ;
- fins de ligne CRLF, LF ou CR ;
- hex en majuscules ;
- marqueur binaire `*` ;
- lignes échappées par `\` ;
- forme BSD `SHA256 (nom) = hash` ;
- ancien préfixe `MD5:` / `SHA256:` ;
- absence de saut de ligne final ;
- lignes vides et lignes `#` ignorées.

Traitements particuliers :
- **encodage** : un contenu qui n'est pas de l'UTF-8 valide est décodé en cp1252 et signalé « encodage hérité » ;
- **algorithme** : déduit de l'extension, contrôlé par la longueur du hash ; une incohérence rend le fichier « invalide » ;
- **nom interne** : s'il diffère du nom du fichier, avertissement « nom interne différent », mais le hash est quand même utilisé. Si seule la casse diffère, sur un système insensible à la casse, il n'y a qu'une note.

Ouvrir volontairement un `.sha256` vérifie le ou les fichiers qu'il nomme, relativement à son dossier, comme le fait `sha256sum -c`. Un fichier nommé mais absent donne « Fichier attendu introuvable ».

Les tests de compatibilité comparent hash et nom avec la sortie de `sha256sum`, pas les octets : l'outil de Git écrit `*` par défaut. Détail complet et exemples : `docs/formats-hash.md`.

---

## 5. Modèle de données (SQLite, un fichier `.fileseal` par base)

```
meta(key PK, value)              nom affiché, uuid de la base, dates, version de l'application
item(id PK, uuid UNIQUE, created_at, retired_at NULL, note)
        un élément = une copie physique suivie dans le temps
reference(id PK, item_id FK, size, created_at, origin,      -- origin : created | accepted | imported
          accepted_from_verification_id FK NULL, superseded_at NULL, comment)
        index unique partiel : une seule référence courante par élément ; index sur size
        historique des changements acceptés = chaîne des références d'un élément
reference_hash(reference_id FK, algorithm, hex CHECK(minuscule, longueur), PK(reference_id, algorithm))
        index (algorithm, hex) pour la recherche par contenu
location(id PK, item_id FK, path, path_key, file_name, volume_serial, volume_relative_path, volume_label,
         first_seen_at, last_seen_at, missing_since NULL, is_current)
        emplacements observés : des indices, jamais une preuve
        index unique partiel sur path_key des emplacements courants
operation(id PK, kind, started_at, finished_at, app_version, options_json, summary_json, status)
        kind : create | verify | search | accept | attach
verification(id PK, operation_id FK, item_id FK NULL, reference_id FK NULL, observed_path,
             observed_size, observed_mtime, started_at, finished_at, outcome, detail)
verification_hash(verification_id FK, algorithm, hex)
        l'empreinte observée est conservée, ce qui permet d'accepter plus tard sans relire
```

- **Identification** :
  - `PRAGMA application_id` identifie un fichier FileSeal ;
  - `PRAGMA user_version` donne le nombre de migrations appliquées ;
  - la dernière vérification d'un élément vient d'une requête indexée sur `verification(item_id, finished_at)`.
- **Connexion** :
  - WAL + `locking_mode=EXCLUSIVE` : une seule connexion, et une deuxième instance reçoit tout de suite « base déjà utilisée » ;
  - `synchronous=FULL` ;
  - limite documentée : aucun autre outil ne peut lire la base pendant qu'elle est ouverte.
- **Migrations** :
  - fichiers SQL numérotés et embarqués, chacun dans sa transaction ;
  - **copie de sécurité avant migration** (`VACUUM INTO <base>.avant-vN.fileseal`, après contrôle de l'espace libre) ;
  - une base plus récente que l'application est refusée, sans aucune écriture ;
  - une migration publiée n'est jamais modifiée ;
  - une base de test figée par version de schéma, dans `tests/fixtures/`.
- Le schéma reste libre jusqu'à l'étape 4, puis il est **figé en v1**.

---

## 6. Règles de rapprochement (`matching.rs`, chacune liée à ses tests)

- **R1** : vérifier par emplacement. Clé de chemin, puis emplacement courant, puis élément, puis référence courante, puis comparaison des algorithmes communs.
- **R2** : chemin inconnu, mais même série de volume et même chemin relatif. L'emplacement est reconnu (lettre de lecteur changée), la vérification porte sur cet élément, et la mise à jour du chemin est proposée.
- **R3** : une taille différente n'empêche jamais la comparaison avec une référence choisie. Le résultat est « Différent (taille modifiée) ».
- **R4** : un hash présent dans un **autre** élément ne rend jamais conforme un fichier comparé à une référence attendue différente. Le résultat est « Différent », avec la mention « identique à l'élément X ».
- **R5** : emplacement inconnu, donc recherche par contenu :
  - taille inconnue de la base : « Inconnu », sans calcul ;
  - aucun hash correspondant : « Inconnu » ;
  - un élément dont l'emplacement courant n'existe plus : « Déplacement probable » ;
  - un élément dont le fichier existe encore : « Copie d'un contenu connu » ;
  - plusieurs éléments : « Ambigu », avec un choix explicite.
- **R6** : un même nom ou une même taille est un indice affiché, jamais une association automatique. Deux fichiers de même nom au contenu différent ne sont jamais fusionnés.
- **R7** : renommé et modifié. FileSeal ne prétend pas retrouver l'origine : il propose des indices (nom proche, même dossier, taille), et l'association reste manuelle.
- **R8** : rattacher, accepter ou réaffecter une racine est toujours explicite et tracé (`operation` + `verification`).
- **R9** : la recherche par contenu se limite aux algorithmes présents des deux côtés.

Description complète : `docs/rapprochement.md`.

---

## 7. Statuts

| Opération | Statuts |
|---|---|
| Vérification | Conforme · Différent · Aucune référence · Aucune référence compatible · Ambigu · Conflit entre références · Fichier de hash invalide · Fichier attendu introuvable · Erreur de lecture · Verrouillé / en cours d'utilisation · Modifié pendant le calcul · Annulé · Non traité |
| Création | Créé · Complété (algorithme ajouté) · Déjà référencé · Conflit avec une référence existante · **Réussite partielle** (quelle destination a échoué, et pourquoi) · Échec |
| Recherche | Inconnu · Connu (même emplacement) · Emplacement reconnu (lecteur changé) · Déplacement probable · Copie d'un contenu connu · Ambigu |
| Sélection | Retenu · Exclu (avec sa raison) · Doublon ignoré · Erreur de parcours |

Avertissements possibles sur chaque ligne : nom interne différent, encodage hérité, taille modifiée.

Chaque statut a une icône, un libellé texte et une couleur. La couleur seule ne porte jamais l'information.

---

## 8. Exécution, progression, interruptions

- **Lecture** :
  - en flux, par blocs de 1 à 4 Mio, avec un nombre de tampons borné : la mémoire reste constante quelle que soit la taille du fichier ;
  - `FILE_FLAG_SEQUENTIAL_SCAN` ;
  - taille et dates relues depuis le handle après l'ouverture, car les données du parcours peuvent être périmées (cache SMB).
- **Modification pendant le calcul** :
  - instantané par le handle, avant et après la lecture : taille, LastWriteTime, **ChangeTime**, identifiant du fichier ;
  - le nombre d'octets lus doit égaler la taille ;
  - au moindre écart : « Modifié pendant le calcul », et rien n'est publié ;
  - **limite documentée** : ce n'est pas un instantané cohérent du fichier. L'interdiction d'écrire pendant la lecture rend toutefois ce cas rare.
- **Ce qui n'est pas lu** : les flux de données alternatifs (ADS) ne sont pas hachés, ce qui est documenté. Windows peut mettre à jour la date de dernier accès, mais le contenu n'est jamais touché.
- **Progression** :
  - globale, pondérée par les octets ;
  - débit = moyenne glissante sur environ 5 s ;
  - temps restant = octets restants divisés par le débit lissé ;
  - « estimation en cours » tant qu'il y a moins de 5 s ou de 64 Mio de mesure.
- **Annulation** : contrôlée entre deux blocs. Le fichier en cours passe en « Annulé » sans rien publier, les résultats terminés sont conservés, le reste passe en « Non traité ».
- **Pause** pendant la session : en V1 (attente entre deux blocs). La **reprise après fermeture ou plantage** est une évolution qui s'appuiera sur `operation` ; le fichier interrompu sera recalculé depuis le début.
- **Erreurs** : une erreur isolée laisse le lot continuer. Une erreur réseau en cours de lecture devient une erreur de lecture, sans nouvel essai silencieux en V1. Erreurs bloquantes : règles 16 et 17 du §2.
- **Journaux** (`tracing`, fichiers tournants dans `%LOCALAPPDATA%\FileSeal\logs`) : opérations, chemins, erreurs, durées. Jamais de contenu de fichier.

---

## 9. Interface

Une seule fenêtre :
1. **Barre d'action** : Créer · Vérifier · Rechercher, et le sélecteur de **base active** (ou « aucune »).
2. **Réglages toujours visibles** :
   - algorithmes : SHA-256 coché par défaut ; MD5 avec la mention « compatibilité, ne protège pas contre une falsification » ;
   - destinations ;
   - sources de vérification ;
   - récursivité, fichiers cachés ;
   - filtres d'extension : des puces avec le nombre de fichiers et la taille par extension détectée.
3. **Liste préparatoire** (grille virtualisée accessible : `role="grid"`, `aria-rowcount`/`aria-rowindex`, navigation au clavier ; pages demandées au moteur, jamais 50 000 lignes d'un coup) :
   - colonnes : nom, dossier, taille, état ;
   - ajout par « Ajouter des fichiers » et « Ajouter un dossier » (Windows ne permet pas les deux dans un même dialogue) et par glisser-déposer ;
   - Retirer, Monter/Descendre, Trier ;
   - nombre de fichiers et taille totale ;
   - onglets Exclusions et Erreurs de parcours.
4. **Phrase de confirmation** avant de démarrer, par exemple : « Vérifier 1 234 fichiers (456 Go) en SHA-256 contre la base « Films » et les fichiers de hash ».
5. **Progression** : fichier en cours et son pourcentage, progression globale, fichiers terminés et restants, volume lu et total, débit, temps écoulé et restant. Boutons Pause et Annuler.
6. **Résultats** :
   - récapitulatif par statut et tableau filtrable ;
   - actions par ligne et par lot : Accepter comme nouvelle version, Rattacher, Mettre à jour les emplacements, Ouvrir le dossier ;
   - export CSV.
7. **Écran Bases** :
   - créer, ouvrir, bases récentes, afficher l'emplacement ;
   - renommer, déplacer, sauvegarder, restaurer ;
   - réaffecter une racine ;
   - parcourir et rechercher les références (nom, chemin, hash), historique d'un élément.

Accessibilité : HTML sémantique, focus visible, raccourcis clavier, contrôle avec le Narrateur et NVDA. Les messages d'erreur disent quoi, pourquoi et que faire.

---

## 10. Bases : gestion et cas d'erreur

- **Création** : un nom et un dossier (par défaut `Documents\FileSeal`). Le nom affiché est stocké dans `meta`.
- **Bases récentes** : liste et réglages dans `%APPDATA%\FileSeal\settings.json`. Une base récente introuvable reste listée, marquée « introuvable », avec Localiser et Retirer.
- **Ouverture**, contrôles dans l'ordre :
  - `application_id` (un fichier SQLite étranger est refusé) ;
  - version du schéma ;
  - `quick_check`.

  Cas particuliers :
  - **lecture seule** : consultation seulement, raison affichée ;
  - **déjà utilisée** : message clair ;
  - **sur un partage réseau** : avertissement, car SQLite y est peu fiable.
- **Renommer ou déplacer** : point de contrôle WAL, fermeture, copie, contrôle d'intégrité et des comptes, mise à jour des bases récentes, puis suppression de l'original seulement après succès.
- **Sauvegarder** : copie cohérente (API backup de SQLite), après contrôle de l'espace libre.
- **Restaurer** : validation du fichier, copie de sécurité de la base actuelle, puis remplacement.

---

## 11. Périmètre

**V1 Windows, complète et utilisable** :
- SHA-256 et MD5 calculés en une seule lecture ;
- les quatre actions : Créer, Vérifier, Rechercher, Accepter ; plus Rattacher et Réaffecter une racine ;
- fichiers de hash : écriture GNU et lecture tolérante (y compris l'ancien format) ;
- bases multiples avec gestion complète ;
- sélection mixte, glisser-déposer, liste préparatoire, filtres ;
- lots avec progression, pause, annulation et rapport CSV ;
- règles de rapprochement R1 à R9 ;
- documentation et tests ;
- installateur et version portable.

**Évolutions séparées**, chacune évaluée à part :
1. Import de fichiers de hash existants dans une base, et de l'ancienne base FileSeal.
2. Écriture de manifestes multi-fichiers (`SHA256SUMS`). La lecture d'un fichier multi-lignes est déjà couverte.
3. Audit complet d'une collection : manquants, nouveaux, modifiés.
4. Reprise persistante après fermeture ou plantage.
5. Recherche et historique avancés, statistiques, export JSON.
6. Suivi des liens avec détection des boucles ; autres algorithmes (SHA-512, BLAKE3, SHA-1 en lecture).
7. Ligne de commande publique, à partir de `fileseal-cli`.
8. Portage macOS et Linux, annoncé seulement après validation sur ces plateformes.
9. Mises à jour intégrées (plugin updater de Tauri et signature) ; plusieurs lecteurs par volume pour les SSD NVMe.

**Distribution** :
- **Installateur NSIS par utilisateur**, sans droits administrateur ; WebView2 inclus pour une installation hors ligne.
- **Version portable** : un zip avec l'exe et un fichier `portable.flag`. Réglages et données WebView2 sont alors gardés à côté de l'exe.
- **Mises à jour en V1** : manuelles, par un nouvel installateur par-dessus l'ancien ; les bases ne sont pas touchées.
- **Compatibilité des bases entre versions** : migrations, plus copie de sécurité avant chaque migration.
- **SmartScreen** : sans signature de code, Windows affichera un avertissement, documenté dans le guide d'installation.

---

## 12. Plan par étapes

Chaque étape se termine par un bilan : ce qui fonctionne, ce qui a été testé (avec la sortie des tests), les limites restantes, la suite prévue. Les tests n'écrivent que dans des dossiers temporaires ; de vrais fichiers ne sont jamais lus qu'en lecture seule.

| # | Livrable | Tests clés | Critère d'acceptation |
|---|---|---|---|
| 0 | **Socle**<br>• workspace : `core`, `cli`, `src-tauri`, `ui`<br>• Svelte 5 + Vite + TS, lint, format, vérification des types<br>• export ts-rs, journaux<br>• scripts `.cmd`<br>• `git init`<br>• documentation : architecture, ADR 0001 (stack), journal | `cargo test`, `cargo clippy -D warnings`, `npm run check` | `npm run tauri dev` ouvre la fenêtre ; toutes les vérifications passent |
| 1 | **Tranche verticale : un fichier, SHA-256**<br>• lecture en flux avec annulation<br>• fichier de hash GNU, écrit de façon atomique et sans écraser<br>• base minimale (schéma provisoire)<br>• Créer, puis Vérifier par chemin<br>• commande async, fil dédié, progression par Channel<br>• UI : choisir un fichier, cases « fichier de hash » et « base », Créer, Vérifier, résultat<br>• même parcours en CLI | • hash connus : fichier vide, `abc`, motif traversant plusieurs blocs, fichier de plus de 4 Gio<br>• `sha256sum -c` de Git valide nos fichiers<br>• un fichier de hash existant n'est pas écrasé<br>• un octet modifié donne « Différent »<br>• une annulation ne laisse rien<br>• dossier non inscriptible donne « Réussite partielle » | Créer puis Vérifier donne « Conforme » par l'UI comme par la CLI ; l'UI reste réactive pendant un fichier de 10 Go |
| 2 | **Lecture robuste et multi-algorithme**<br>• MD5 et SHA-256 en une lecture<br>• partage qui interdit l'écriture, lecture séquentielle<br>• instantané avant/après par le handle<br>• erreurs système traduites en statuts<br>• progression limitée en fréquence | • fichier verrouillé par le test donne « Verrouillé »<br>• taille ou ChangeTime modifiés (injection) donne « Modifié pendant le calcul »<br>• une seule lecture pour deux algorithmes<br>• hash MD5 connus | Aucune empreinte partielle ne peut être publiée (tests d'injection) |
| 3 | **Fichiers de hash complets**<br>• lecture tolérante (ancien format, cp1252, CRLF, BOM, BSD, échappements)<br>• table de décision pour un fichier existant (identique, différent, invalide, nom interne), y compris avec plusieurs algorithmes<br>• vérification à partir d'un `.sha256` ouvert directement | • `md5sum -c` et `sha256sum -c` sur des noms avec Unicode, espaces, `-` initial, sans extension, chemin de plus de 260 caractères<br>• panne injectée entre écriture et renommage : rien n'est publié et le temporaire est supprimé | Comportement conforme à la table de `docs/formats-hash.md` |
| 4 | **Base v1**<br>• schéma figé<br>• créer, nommer, ouvrir, bases récentes<br>• `application_id`, verrouillage exclusif<br>• lecture seule, base absente, base plus récente, fichier étranger<br>• cadre de migration et copie avant migration | • deuxième processus : « base déjà utilisée »<br>• migration de test sur une base figée, avec sa copie<br>• base plus récente refusée<br>• SQLite étranger refusé | Un message clair pour chaque cas d'échec |
| 5 | **Opérations et rapprochement**<br>• Créer, Vérifier, Rechercher, Accepter, Rattacher<br>• sources base et fichiers de hash, conflits entre elles<br>• règles R1 à R9 (dont lecteur changé et filtre par taille)<br>• réussites partielles<br>• rapport CSV | Un scénario par règle :<br>• déplacement, renommage<br>• même nom, contenu différent<br>• même contenu à plusieurs endroits<br>• référence absente ou contradictoire<br>• références existantes protégées<br>• E: devenu F: | Une vérification n'écrit jamais de référence (test dédié) |
| 6 | **Découverte et sélection** dans le moteur<br>• plusieurs fichiers et dossiers<br>• parcours en arrière-plan, envoyé par paquets, annulable<br>• récursivité, filtres<br>• exclusions avec leur raison, dédoublonnage<br>• chemins longs, Unicode | • jonction (`mklink /J`)<br>• lien symbolique (ignoré si le droit manque)<br>• lien physique donne « doublon »<br>• noms réservés et points finaux<br>• 50 000 fichiers (test long, désactivé par défaut) | Premiers fichiers affichés en moins d'une seconde ; annulation immédiate |
| 7 | **Orchestrateur de lots**<br>• un lecteur par volume, plafond global<br>• ordre de démarrage<br>• progression pondérée, débit, temps restant<br>• pause et annulation<br>• poursuite après une erreur ; arrêt si la base est perdue ; volume disparu<br>• veille empêchée, confirmation à la fermeture | • erreurs injectées<br>• annulation en plein lot<br>• volume retiré (simulé) | Terminés + échoués + non traités = total, exactement |
| 8 | **Interface complète**<br>• liste préparatoire virtualisée<br>• glisser-déposer et deux sélecteurs<br>• bandeau action, algorithmes, destinations, base, filtres<br>• phrase de confirmation, progression<br>• résultats filtrables et actions par lot | • parcours entier au clavier seul<br>• contrôle avec le Narrateur et NVDA<br>• 50 000 lignes fluides | Chaque statut est lisible sans couleur |
| 9 | **Écran Bases**<br>• renommer et déplacer (opération gérée)<br>• sauvegarder et restaurer<br>• parcourir et rechercher les références, historique<br>• rattachement par lot, réaffecter une racine | • aller-retour sauvegarde puis restauration à l'identique<br>• scénario E: devenu F: | Un changement de lettre de lecteur se règle en 3 clics au plus |
| 10 | **Cas difficiles et performances**<br>• partage SMB, clé USB débranchée, disque plein (petit disque virtuel ou clé)<br>• fichier OneDrive en ligne uniquement, base en lecture seule<br>• mesures de débit (disque dur, SSD, réseau) et optimisations éventuelles (lecture et calcul en parallèle) | • procédures manuelles écrites et rejouées<br>• mesures consignées dans `docs/performances.md` | Chaque statut a été observé en conditions réelles ; limites documentées |
| 11 | **Livraison Windows**<br>• installateur NSIS par utilisateur et zip portable<br>• icônes, informations de version, instance unique<br>• guide utilisateur finalisé, limites connues, procédure de publication | Installation et désinstallation sur une session Windows 11 sans droits administrateur ; version portable lancée depuis une clé USB | Bases et réglages conservés à la désinstallation ; mise à jour par-dessus sans perte |

Les étapes 1 à 5 tiennent dans le moteur et la CLI avec une interface minimale. L'interface complète (étapes 8 et 9) arrive une fois le moteur stabilisé, pour éviter de la refaire.

**Documentation produite au fil des étapes** (`docs/`) :
- `guide-utilisateur.md`, `installation-developpement.md`, `architecture.md` ;
- `modele-donnees.md` (avec la stratégie de migration), `formats-hash.md`, `rapprochement.md` ;
- `tests-build-livraison.md`, `limites.md`, `performances.md` ;
- `adr/NNNN-*.md` (décisions structurantes).

---

## 13. Critères d'acceptation de la V1

1. Aucun fichier source n'est jamais ouvert en écriture (test : comparaison des octets et de la date de modification avant et après chaque scénario).
2. Les fichiers de hash produits sont validés par `sha256sum -c` / `md5sum -c` de Git for Windows, y compris pour les noms avec accents et espaces.
3. Les anciens fichiers `MD5: <hash>  <nom>` (UTF-8 ou cp1252, CRLF) sont lus correctement.
4. Une vérification ne crée ni ne remplace jamais une référence. Seule l'action « Accepter » le fait, et l'historique est conservé.
5. Aucune empreinte partielle n'est publiée après une annulation, une panne injectée ou un arrêt brutal.
6. Avec deux destinations, l'échec de l'une est affiché en « Réussite partielle », avec la cause.
7. Les fichiers déplacés, renommés ou changés de lecteur sont retrouvés selon R1 à R9. Les ambiguïtés sont signalées, jamais résolues en silence.
8. Un lot de 50 000 fichiers reste fluide, une sélection de dossier volumineux est annulable, et un fichier de plus de 100 Go est traité avec une mémoire constante.
9. Toutes les fonctions principales sont utilisables au clavier, et aucun statut ne dépend de la couleur seule.
10. Une base créée en v1 s'ouvre dans toute version ultérieure après migration, avec une copie de sécurité préalable.
11. L'application fonctionne sans connexion Internet ni compte.

---

## 14. Vérification de bout en bout

- **Automatique** (script `scripts\verify.cmd`) :
  - `cargo fmt --check`, `cargo clippy --workspace --all-targets -D warnings`, `cargo test --workspace` ;
  - `npm run check` (svelte-check et TypeScript) et `npm run test` (vitest) ;
  - vérification que les types générés sont à jour.
- **Compatibilité** : un test d'intégration appelle `C:\Program Files\Git\usr\bin\sha256sum.exe -c` et `md5sum.exe -c` sur des fichiers produits par FileSeal. Il est ignoré si l'outil est absent.
- **CLI** : `cargo run -p fileseal-cli -- create|verify|search <chemins> --db <base>` déroule les scénarios sans interface.
- **Application** : `npm run tauri dev` avec une base et un dossier de test dans un répertoire temporaire, jamais sur des données réelles sans accord. Scénarios manuels :
  - créer, vérifier, modifier une copie, vérifier, accepter ;
  - déplacer, rechercher, rattacher ;
  - annuler en plein lot ;
  - base déjà ouverte par une autre instance.
- **Cas matériels** (étape 10) : procédures écrites dans `docs/tests-build-livraison.md`, rejouées avant chaque version.
