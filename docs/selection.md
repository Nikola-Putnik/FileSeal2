# Sélection : des fichiers et dossiers choisis à la liste des fichiers

Code : `crates/fileseal-core/src/discovery.rs`. Règles du [cadrage](cadrage.md) §2, n° 10 à 13.

L'utilisateur choisit des fichiers et des dossiers. La découverte en tire la liste des fichiers à traiter, en arrière-plan, et la transmet par paquets au fur et à mesure.

## Fichier choisi directement, fichier trouvé dans un dossier

| | Choisi directement | Trouvé en parcourant un dossier |
|---|---|---|
| Exclusions par défaut | jamais : il est toujours retenu | appliquées, avec leur raison |
| Filtre d'extension | non appliqué | appliqué |
| Lien symbolique ou jonction | suivi | jamais suivi, signalé « lien » |
| Fichier de hash | retenu (l'interface propose alors de vérifier les fichiers qu'il liste) | exclu |

Un dossier choisi directement est parcouru même s'il est caché, ou s'il s'agit de `$RECYCLE.BIN` : l'utilisateur l'a demandé.

## Ordre

Les éléments choisis sont traités dans l'ordre de la sélection. Dans un dossier : d'abord ses fichiers, puis ses sous-dossiers, chacun parcouru entièrement avant le suivant (parcours en profondeur).

Les noms sont triés comme dans l'Explorateur de Windows : sans tenir compte de la casse, et les nombres comparés comme des nombres (« Épisode 2 » avant « Épisode 10 »). À égalité, la comparaison exacte départage : l'ordre ne dépend jamais de celui du système.

## Exclusions (parcours d'un dossier seulement)

Chaque exclusion est listée avec son chemin et sa raison, et comptée par raison. La raison la plus précise l'emporte : une base FileSeal cachée est signalée comme base.

| Raison (code) | Ce qui est exclu |
|---|---|
| `fileSealFile` | Bases `.fileseal` et leurs fichiers annexes (`-wal`, `-shm`, `-journal`), copies avant migration, copies partielles `.fileseal.partial`. |
| `temporaryFile` | Fichiers temporaires de FileSeal (`.fileseal-XXXXXX.tmp`). |
| `hashFile` | Fichiers de hash : extensions `.md5`, `.sha1`, `.sha256`, `.sha512`, `.sfv`, `.md5sum`, `.sha256sum` ; listes `MD5SUMS`, `SHA1SUMS`, `SHA256SUMS`, `SHA512SUMS`, `B2SUMS` (avec ou sans `.txt`). Sans tenir compte de la casse. |
| `hidden` | Fichiers et dossiers cachés, sauf si l'option « inclure les fichiers cachés » est cochée. Un dossier caché n'est pas parcouru. Hors Windows : les noms qui commencent par un point. |
| `system` | Fichiers ayant l'attribut « système », sauf avec la même option. |
| `systemFolder` | `$RECYCLE.BIN` et `System Volume Information`, **toujours**, même avec l'option. |
| `onlineOnly` | Fichiers « en ligne uniquement » (OneDrive et autres services de synchronisation) : les lire déclencherait leur téléchargement, ou échouerait hors connexion. Reconnus par leurs attributs (`OFFLINE`, `RECALL_ON_OPEN`, `RECALL_ON_DATA_ACCESS`), sans les ouvrir. |
| `link` | Liens symboliques et jonctions, vers un fichier ou un dossier. Ils ne sont jamais suivis pendant un parcours : une boucle est impossible. |
| `special` | Périphérique, tube ou socket (hors Windows seulement). |
| `duplicate` | Fichier ou dossier déjà présent dans la sélection (voir plus bas). |

**Dossiers « système »** : l'attribut système seul n'exclut pas un dossier. Windows le pose sur des dossiers ordinaires dont l'icône ou le nom affiché a été personnalisé (`desktop.ini`) ; les exclure ferait disparaître des fichiers sans raison visible. Les vrais dossiers système de Windows sont aussi cachés, ou figurent dans la liste ci-dessus.

## Filtre d'extension

- L'extension est ce qui suit le dernier point, sauf si ce point commence le nom : `.bashrc` n'a pas d'extension, `archive.tar.gz` a l'extension `gz`, `nom.` n'en a pas.
- La comparaison ignore la casse (Unicode : `ÉTÉ.ÀVI` a l'extension `àvi`). Le point initial est accepté dans le filtre (`.mkv` = `mkv`).
- L'entrée vide (`""`) retient les fichiers sans extension. Un filtre vide retient tout.
- Les fichiers écartés par le filtre sont comptés (`filteredOut`), pas listés un par un. Un fichier écarté par le filtre puis choisi directement est retenu (et compté deux fois par extension, cas sans conséquence).
- Le récapitulatif compte les fichiers et les octets **par extension, avant le filtre** : l'interface peut proposer chaque extension trouvée, avec sa taille. Une découverte lancée sans filtre donne tous les fichiers avec leur extension : le filtre peut alors changer sans nouveau parcours.
- **Dans l'interface**, deux façons de filtrer, sans nouveau parcours :
  - **décocher des puces** : une puce par extension trouvée. Une extension découverte ensuite (autre dossier ajouté) est retenue ;
  - **« Seulement ces extensions »** : une liste tapée (`mkv iso mp4`, `.MKV, .Iso`, `*.mp4` : point, astérisque et casse ignorés). Toute autre extension est écartée, y compris dans les dossiers ajoutés ensuite. Les puces suivent la liste et la modifient.

  « Toutes » revient à toutes les extensions.

## Doublons

Un même fichier n'apparaît qu'une fois ; les suivants sont signalés `duplicate`. Sont reconnus :
- le même chemin donné deux fois, y compris avec une autre casse (Windows), des `.` ou des séparateurs doublés ;
- un fichier choisi directement et retrouvé dans un dossier choisi ;
- des dossiers qui se recouvrent (`D:\Films` et `D:\Films\SF`) : le second est signalé une fois, comme dossier, au lieu de chacun de ses fichiers ;
- un lecteur réseau et son chemin UNC (`Z:\Films` et `\\nas\films`) : la lettre est remplacée par le partage qu'elle désigne.

**Liens physiques** (deux noms pour le même fichier) : la découverte ne les reconnaît pas, car il faudrait ouvrir chaque fichier pour connaître son identité, ce qui serait très lent sur un partage réseau. Ils sont reconnus par le [lot](lots.md), qui ouvre le fichier de toute façon : le second est un « doublon », jamais relu.

Deux fichiers distincts au contenu identique restent deux entrées.

## Noms particuliers

- **Noms réservés et points finaux** : un fichier nommé `nul`, `con.txt`, `nom.` ou `nom ` (espace final) ne peut être créé que par un autre système ou par la forme étendue `\\?\`. FileSeal l'ouvre toujours par cette forme étendue : c'est bien ce fichier qui est lu, jamais le périphérique `NUL` ni un autre fichier que Windows aurait désigné en retirant le point.
- **Chemins longs** (plus de 260 caractères) : parcourus et lus normalement.
- **Noms non représentables** en Unicode (UTF-16 mal formé, très rare) : signalés en erreur de parcours (`nameNotRepresentable`), jamais enregistrés déformés.

## Erreurs de parcours

Un dossier qu'on ne peut pas lister (accès refusé, disque débranché…), un élément illisible ou un chemin choisi introuvable donnent une erreur de parcours, avec son chemin et le problème. Le parcours continue avec le reste.

## Paquets, progression, annulation

- Un paquet part dès qu'il contient 1 000 éléments, ou 100 ms après le précédent. Les premiers fichiers arrivent donc presque immédiatement, même dans un très grand dossier.
- Chaque paquet porte les totaux courants (fichiers, octets, dossiers parcourus, exclusions, erreurs) et le dossier en cours de lecture. Pendant la lecture d'un dossier énorme sur un partage lent, des paquets sans fichier font avancer les compteurs.
- Le **dernier paquet porte le récapitulatif**. Par un canal Tauri, la réponse de la commande peut arriver avant les derniers paquets (mesuré : 1 000 fichiers sur 20 000 encore en route) ; l'interface attend donc ce dernier paquet pour considérer la liste complète.
- L'annulation est contrôlée entre deux éléments : le parcours s'arrête aussitôt, et le récapitulatif indique `cancelled`. Les fichiers déjà transmis restent valables.

## Interfaces

- **Moteur** : `discovery::discover(racines, options, control, sink)`.
- **Application** : commande `discover_selection` (paquets par `Channel`, annulation par `cancel_job`), appelée par `api.discoverSelection` dans `ui/src/lib/api.ts`, qui remplit la liste de la fenêtre ([interface.md](interface.md)).
- **Ligne de commande** : `cargo run -p fileseal-cli -- scan <chemins…> [--flat] [--hidden] [--ext mkv --ext avi] [--quiet]`.
