# Modèle de données et migrations

Code :
- `crates/fileseal-core/src/store.rs` : les données ;
- `store/schema.rs` : ouverture, contrôles, migrations ;
- `store/migrations/` : le schéma.

> **Schéma v1 figé le 2026-10-22** (étape 4). Tout changement de schéma passe désormais par une nouvelle migration ; `0001_initial.sql` n'est plus jamais modifié.

## Une base = un fichier

- Extension `.fileseal` (SQLite). On peut avoir autant de bases que l'on veut ; une seule est active par opération.
- `PRAGMA application_id = 0x4653454C` (« FSEL ») identifie une base FileSeal. Tout autre fichier SQLite est refusé **sans être modifié** : FileSeal lit l'identifiant avant tout autre réglage (testé octet pour octet).
- `PRAGMA user_version` = nombre de migrations appliquées.
- Connexion :
  - **WAL** : une coupure en pleine écriture ne corrompt pas la base ;
  - `synchronous=FULL`, sauf pendant un lot : `NORMAL`, puis tout est forcé sur le disque à la fin (`wal_checkpoint(FULL)`) et `FULL` revient ([performances.md](performances.md)) ;
  - clés étrangères actives ;
  - **`locking_mode=EXCLUSIVE`**, posé avant le premier accès. Il n'y a pas de fichier `-shm`, et la base reste verrouillée tant qu'elle est ouverte. Un second FileSeal, ou un autre processus, reçoit aussitôt « base déjà utilisée » (testé avec deux processus distincts). En contrepartie, aucun autre outil ne peut lire la base pendant qu'elle est ouverte dans FileSeal.
- Un fichier `-wal` peut rester à côté de la base après un arrêt brutal. SQLite le réintègre à la prochaine ouverture.

## Ouverture : contrôles dans l'ordre

1. Le fichier existe ; sinon « fichier introuvable ».
2. Connexion en lecture-écriture, avec repli en **lecture seule** (voir plus bas).
3. `application_id` : sinon « ce fichier n'est pas une base FileSeal », sans rien modifier.
4. Version du schéma : plus récente que l'application → « base d'une version plus récente », sans rien modifier.
5. `PRAGMA quick_check` : un défaut de structure (pages déchirées, index cassés) donne « base endommagée : restaurez une copie de sauvegarde ».
6. Réglages WAL, puis migrations éventuelles (précédées d'une copie).

### Bases en lecture seule

Une base qu'on ne peut pas écrire s'ouvre **en consultation** : fichier protégé en écriture, dossier, disque ou partage en lecture seule.
- **Comment** : un fichier protégé en écriture est ouvert directement en lecture seule. Sinon, si SQLite ne peut pas écrire (impossible de créer le fichier `-wal`), FileSeal rouvre en mode `immutable=1` : SQLite suppose alors que personne ne modifie le fichier et n'a besoin d'aucun fichier annexe.
- **Ce qui marche** : consulter, et vérifier avec ses références.
- **Ce qui est refusé, avec une raison claire** : écrire une référence. L'historique n'est pas enregistré (le résultat signale « non ajouté à l'historique : base en lecture seule »).
- Une base en lecture seule qui aurait besoin d'une migration ne peut pas être ouverte : la migration demande d'écrire.

### Base sur un partage réseau

Le chemin UNC ou le lecteur réseau est détecté, et un avertissement s'affiche. La base s'ouvre quand même. Le verrouillage de SQLite n'est pas fiable sur un partage : il vaut mieux garder les bases sur un disque local. Les fichiers vérifiés, eux, peuvent être n'importe où.

## Tables

| Table | Contenu |
|---|---|
| `meta` | Nom affiché de la base, uuid, date et version de création. |
| `item` | Un **élément** : une copie physique d'un fichier, suivie dans le temps. Son identité est cette ligne, jamais son chemin. Deux copies identiques font deux éléments. |
| `reference` | Contenu attendu d'un élément : taille, date, origine (`created`, `accepted`, `imported`). **Une seule référence courante par élément** (index unique partiel sur `superseded_at IS NULL`). Accepter une nouvelle version remplace la référence courante, et l'ancienne reste comme historique. Index sur la taille (recherche par contenu, étape 5). |
| `reference_hash` | Les hash d'une référence, un par algorithme, en hexadécimal minuscule (contrôlé par `CHECK`). Index `(algorithm, hex)` pour la recherche par contenu. Aucune contrainte sur le nom d'algorithme : une version future peut en ajouter sans reconstruire la table. |
| `location` | Les emplacements observés : chemin, clé de comparaison (`path_key`), nom, volume, et **taille et date de modification vues la dernière fois** (`last_size`, `last_mtime`). Ce sont des indices pour reconnaître un fichier, jamais une preuve d'identité. Au plus un emplacement courant par élément, et au plus un élément courant par chemin. Index sur le nom (sans casse) pour la recherche. |
| `operation` | Chaque exécution d'une action (création, vérification…), avec sa version d'application et son statut. Un lot est **une** opération ; son récapitulatif (nombres par catégorie et par statut, volumes perdus, cause d'un arrêt) est rangé en JSON dans `summary_json`, et son statut est `completed`, `cancelled`, ou `failed` si la base est devenue inutilisable. |
| `verification` | Chaque vérification, y compris celles qui ont échoué à la lecture (verrouillé, modifié pendant le calcul, erreur de lecture) : élément, référence comparée, chemin, taille et date observés, résultat, détail. |
| `verification_hash` | Les hash **observés** pendant une vérification. Un résultat « Différent » pourra être accepté plus tard sans relire le fichier. |

Dates : texte UTC `2026-10-22T09:15:03.250Z`, lisible et triable ; affichage en heure locale.

`path_key` : chemin absolu en minuscules sous Windows et macOS (systèmes insensibles à la casse par défaut), tel quel sous Linux. Il ne sert qu'à retrouver un emplacement connu.

## Ce qu'enregistrent les actions

- **Créer**, pour un chemin inconnu de la base : un `item`, sa `reference` (algorithmes demandés seulement), ses `reference_hash` et son `location` (avec taille et date vues), en une transaction.
- **Créer**, pour un chemin connu et un contenu identique : les hash des algorithmes manquants sont ajoutés à la référence existante, et l'emplacement est marqué « vu » avec sa taille et sa date.
- **Créer**, pour un chemin connu et un contenu différent : rien n'est écrit (conflit).
- **Vérifier** un élément connu : une `verification` et ses `verification_hash` ; l'emplacement est marqué « vu », sauf s'il a été reconnu sous une autre lettre de lecteur (R2 : la mise à jour est proposée). Une vérification d'un fichier inconnu de la base n'écrit rien. Une vérification annulée n'est pas enregistrée.
- **Rechercher** n'écrit rien.
- **Rattacher** : l'ancien emplacement passe en historique (`is_current = 0`, `missing_since`), et le nouveau est inséré avec son volume et sa place sur le volume. Une opération `attach` et une vérification (« attached; previously at … ») sont enregistrées.
- **Réaffecter un dossier ou un disque** (écran Bases) : pour chaque fichier concerné, comme « Rattacher », l'ancien emplacement passe en historique et le nouveau est inséré, le tout en une transaction. Une seule opération `attach` est enregistrée, avec `summary_json` = `{"reassign": {"from", "to", "serial"}, "moved", "left"}`. Aucune vérification n'est enregistrée : rien n'est lu.
- **Renommer une base** : la valeur `name` de `meta` change ; le fichier garde son nom.
- **Accepter une nouvelle version** : l'ancienne référence reçoit `superseded_at`, la nouvelle (`origin = accepted`) reprend la taille et les hash observés par la vérification indiquée (`accepted_from_verification_id`).

## Sauvegarde, restauration, déplacement

Détail dans [bases.md](bases.md).
- **Sauvegarde** : `VACUUM INTO` vers un nom provisoire `.partial`, renommé ensuite. La copie est contrôlée en la rouvrant en lecture seule et en comparant ses comptes.
- **Restauration** :
  - la sauvegarde est examinée en mode immuable, sans aucune écriture ;
  - le contenu actuel est copié en `<nom>.before-restore-AAAAMMJJ-HHMMSS.fileseal` ;
  - la sauvegarde remplace ensuite le fichier en un seul renommage, puis la base est rouverte (et migrée si besoin, avec sa copie).
- **Déplacement** : renommage sans remplacement sur le même disque. Sinon : copie, contrôle des comptes, puis suppression de l'original.

## Stratégie de migration

- **Migrations** : une migration est un fichier SQL numéroté (`0002_…sql`), embarqué dans l'exécutable et déclaré dans `MIGRATIONS` (`store/schema.rs`). Chacune s'applique dans sa propre transaction : une migration qui échoue est annulée, et la base reste à sa version précédente (testé). Une migration publiée n'est jamais modifiée.
- **Copie de sécurité avant toute migration** d'une base qui contient déjà des données :
  - fichier `<nom>.before-v<N>-<AAAAMMJJ>-<HHMMSS>.fileseal` à côté de la base, fait par `VACUUM INTO` ;
  - elle est d'abord écrite sous un nom provisoire `.partial`, puis renommée : une copie interrompue n'a jamais l'air complète ;
  - une copie précédente n'est jamais écrasée (celle d'une tentative ratée peut être la seule bonne) ;
  - l'espace libre est contrôlé d'abord (taille de la base et de son `-wal`, plus 16 Mio) ; s'il manque, la base n'est ni migrée ni ouverte, avec « espace disque insuffisant » ;
  - l'interface signale la mise à jour et l'emplacement de la copie.
- **Base trop récente** : une base dont la version dépasse celle de l'application est refusée, sans aucune écriture (« mettez FileSeal à jour »).
- **Bases de test figées** : une par version de schéma, dans `crates/fileseal-core/tests/fixtures/` (`schema-v1.fileseal`). Elles sont écrites une seule fois par `cargo run -p fileseal-core --example make_fixture -- crates/fileseal-core/tests/fixtures`, puis commitées et **jamais régénérées**. `tests/schema_fixtures.rs` vérifie que chacune s'ouvre dans la version courante avec ses données : références, algorithmes, historique, noms non ASCII.
- **Ajouter une migration** :
  1. écrire `000N_….sql` et l'ajouter à `MIGRATIONS` ;
  2. lancer `make_fixture` pour créer `schema-vN.fileseal` ;
  3. compléter `schema_fixtures.rs` (la base v1 doit toujours s'ouvrir) ;
  4. documenter ici.
