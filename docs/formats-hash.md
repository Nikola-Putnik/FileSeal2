# Formats des fichiers de hash

Code : `crates/fileseal-core/src/sidecar.rs`. Tests de compatibilité avec GNU coreutils : `crates/fileseal-core/tests/gnu_compat.rs`.

## Nom

`<nom du fichier source>.<algo>`, extension en minuscules, un fichier par algorithme, dans le même dossier.

| Fichier source | Fichiers de hash |
|---|---|
| `Blade Runner.mkv` | `Blade Runner.mkv.sha256`, `Blade Runner.mkv.md5` |
| `notes` (sans extension) | `notes.sha256` |

## Écriture : ce que FileSeal produit

- Encodage UTF-8 **sans BOM**.
- Une ligne, terminée par **LF** : `<hash en hexadécimal minuscule><espace><espace><nom>`.
  - Exemple : `ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad  Blade Runner.mkv`.
  - Pas de CRLF : `sha256sum` 8.32, la version livrée avec Git for Windows, refuse les lignes en CRLF.
- Le nom est celui du fichier seul, sans dossier. Le fichier et son fichier de hash peuvent donc être déplacés ensemble.
- Espaces, accents et autres caractères Unicode sont écrits tels quels, en UTF-8.
- **Échappement GNU** : si le nom contient `\`, un saut de ligne (LF) ou un retour chariot (CR), la ligne commence par `\` et ces caractères deviennent `\\`, `\n` et `\r`. C'est impossible avec un nom Windows ; c'est prévu pour les noms venus de Linux et pour le portage.
- Un nom qui n'est pas de l'Unicode valide (UTF-16 mal formé, très rare sous Windows) est refusé avec une erreur, plutôt qu'écrit déformé.

### Publication atomique, jamais d'écrasement

1. Le contenu est écrit dans un fichier temporaire du même dossier, au nom court `.fileseal-XXXXXX.tmp`. Un nom court reste sous la limite de 255 caractères même quand le nom source est très long.
2. Le contenu est forcé sur le disque (`sync_all`).
3. Le fichier temporaire est renommé en fichier de hash **sans remplacement** (`MoveFileExW` sans `MOVEFILE_REPLACE_EXISTING`, aussi valable sur exFAT et sur un partage SMB). Si un fichier de hash existe déjà, le renommage échoue et l'existant reste intact.
4. En cas d'échec à n'importe quelle étape, le fichier temporaire est supprimé.

Après un plantage ou un disque plein, il y a donc un fichier de hash complet ou aucun, jamais un fichier tronqué. Le remplacement d'un fichier de hash n'aura lieu que lors d'une acceptation explicite d'une nouvelle version (étape 5 du plan).

## Lecture : ce que FileSeal accepte

| Forme | Exemple | Remarque affichée |
|---|---|---|
| GNU, mode texte | `<hash>  nom` | — |
| GNU, mode binaire (Git for Windows, Windows en général) | `<hash> *nom` | — |
| GNU échappé | `\<hash>  a\\b` | — |
| BSD / `--tag` | `SHA256 (nom) = <hash>` | format non standard |
| Ancien FileSeal | `MD5: <hash>  nom` | format non standard |
| Hash seul, sans nom | `<hash>` | format non standard |
| Une seule espace | `<hash> nom` | — |

Tolérances générales :
- BOM UTF-8 accepté ;
- fins de ligne LF, CRLF ou CR ;
- hash en majuscules ou en minuscules ;
- absence de saut de ligne final ;
- lignes vides et lignes commençant par `#` ignorées ;
- espaces en début de ligne ignorées.

Les espaces en fin de ligne font partie du nom, comme pour GNU.

Un hash n'est reconnu que s'il a la longueur d'une vraie empreinte : 32, 40, 56, 64, 96 ou 128 chiffres hexadécimaux (MD5, SHA-1, SHA-224, SHA-256, SHA-384, SHA-512). Une ligne comme « bad line » ou « cafe menu » n'est donc pas prise pour un hash suivi d'un nom.

Contrôles :
- **Encodage** : un contenu qui n'est pas de l'UTF-8 valide est lu en Windows-1252 (anciens fichiers FileSeal), avec la remarque « encodage ancien ».
- **Algorithme** : il vient de l'extension du fichier de hash. Il est contrôlé par la longueur du hash (64 caractères pour SHA-256, 32 pour MD5) et par l'étiquette éventuelle (`MD5 (…)`, `MD5:`). Une incohérence rend le fichier invalide.
- **Nom inscrit** : la ligne qui porte exactement le nom du fichier est retenue. À défaut, sous Windows, une ligne dont seule la casse diffère (remarque). À défaut, la ligne unique d'un fichier qui n'en contient qu'une : la remarque « le nom inscrit n'est pas celui du fichier » s'affiche, mais le hash est utilisé, car le fichier et son fichier de hash ont pu être renommés ensemble.
- **Fichier invalide** : vide, illisible, sans ligne valide, plusieurs lignes dont aucune pour ce fichier, mauvais algorithme, ou plus de 16 Mio.

## Fichier de hash déjà présent lors d'une création

| Situation | Résultat | Le fichier existant |
|---|---|---|
| Même hash que le contenu lu | « déjà présent, identique » | conservé tel quel, même au format ancien |
| Hash différent | « Conflit » : rien n'est écrit, nulle part, pour ce fichier | conservé tel quel |
| Invalide | « Conflit » : rien n'est écrit | conservé tel quel |
| `.md5` existant et identique, `.sha256` demandé | le `.md5` est vérifié pendant la même lecture, puis le `.sha256` est écrit | conservé |

Un fichier **relu** voit ses fichiers de hash existants vérifiés, même pour un algorithme non demandé. Une référence n'est jamais posée sur un contenu qui en contredit une autre.

### Sauter les fichiers déjà référencés

Réglage « Fichiers déjà référencés » de l'action Créer (`CreateOptions::existing`). Par défaut dans l'interface, un fichier qui a déjà ce qui est demandé **n'est pas relu** : relancer Créer sur une collection ne lit que les nouveaux fichiers.

| Choix | Un fichier est sauté, sans être lu, quand… |
|---|---|
| Les sauter s'ils ont tous les algorithmes cochés (par défaut) | chaque destination cochée a déjà une référence pour chaque algorithme coché |
| Les sauter s'ils ont une référence, quelle qu'elle soit | chaque destination cochée a déjà une référence, de n'importe quel algorithme : un ancien `.md5` suffit, même avec SHA-256 coché, et rien n'est ajouté |
| Les relire | jamais : contrôle et complément, comme dans les tableaux ci-dessus |

- **Un fichier sauté** ressort « Déjà référencé (non relu) ». Ses références ne sont pas contrôlées : c'est le rôle de Vérifier.
- **Ajouter leurs fichiers de hash à la base** (case à cocher, avec la base active comme destination) : un fichier sauté que la base ne connaît pas y est enregistré à partir de ses fichiers de hash, avec tous leurs algorithmes, sans être lu. Il ressort « Ajouté à la base (non relu) », et son historique dit « importée ». La base reçoit la valeur du fichier de hash, pas celle du contenu actuel : si le fichier a changé depuis, la prochaine vérification le dira.
- **Un fichier est toujours relu** quand une référence aurait quelque chose à signaler :
  - un fichier de hash invalide ;
  - un fichier de hash et la base en désaccord ;
  - un fichier reconnu sous une autre lettre de lecteur ;
  - un contenu connu de la base qui semble avoir été déplacé ici. La décision habituelle (rattacher, ou autre copie) est alors demandée.

| Situation particulière | Résultat |
|---|---|
| Ancien format (`MD5: hash  nom`, CRLF) et identique | conservé **octet pour octet**, pas réécrit au nouveau format |
| Fichier et fichier de hash renommés ensemble (le nom inscrit est l'ancien), hash identique | « déjà présent, identique », remarque « le nom inscrit n'est pas celui du fichier », fichier de hash laissé tel quel |
| Chemin de plus de 260 caractères | écrit normalement (chemins Windows étendus `\\?\`) |

## Vérifier à partir d'un fichier de hash

Choisir ou glisser un fichier de hash dans FileSeal fait passer en « Vérifier ». FileSeal vérifie alors **les fichiers qu'il liste**, comme `sha256sum -c`. C'est le cas pour un fichier nommé `*.sha256` ou `*.md5`, ou `SHA256SUMS`, `MD5SUMS`, `sha256sums.txt`… Aucune base n'est nécessaire ; la base active peut être consultée en plus.

**Algorithme**, dans l'ordre :
1. l'extension (`.sha256`, `.md5`) ;
2. le nom (`SHA256SUMS`, `md5sums.txt`) ;
3. les étiquettes des lignes (`SHA256 (…) = …`), ou la longueur des hash, si toutes les lignes s'accordent.

Une ligne étiquetée peut avoir son propre algorithme (`--tag` permet de mélanger). Une étiquette qui contredit l'extension (`MD5 (…)` dans un `.sha256`) rend la ligne invalide.

**Fichier visé par chaque ligne** :
- Le nom est lu **relativement au dossier du fichier de hash**. Les sous-dossiers sont acceptés (`Saison 1/Épisode 1.mkv`, avec `/` ou `\`), et un chemin absolu est pris tel quel.
- Un fichier de hash **dédié** (`film.mkv.sha256`) qui ne contient qu'une ligne :
  - si le nom inscrit n'existe plus mais que `film.mkv` existe, la ligne vise `film.mkv`, avec la remarque « nom inscrit différent » (fichier et fichier de hash renommés ensemble) ;
  - si la ligne n'a pas de nom, elle vise aussi `film.mkv`.
- Une ligne sans nom dans un fichier qui en contient plusieurs ne vise rien : « Fichier de hash invalide ».

**Résultats** : un par ligne, dans l'ordre du fichier.

| Situation | Statut |
|---|---|
| Hash identique | Conforme à la référence |
| Hash différent | Différent de la référence (empreintes attendue et observée affichées) |
| Fichier absent | Fichier attendu introuvable |
| Ligne inutilisable (mauvaise longueur, algorithme inconnu ou contredit) | Fichier de hash invalide |
| Lecture impossible | Verrouillé, Erreur de lecture, Modifié pendant le calcul… |
| Annulation | « Annulé » pour le fichier en cours, « Non traité » pour les suivants |

Les lignes qui ne sont ni un hash, ni un commentaire, ni vides sont signalées par leur numéro. Elles n'empêchent pas de vérifier les autres.

## Compatibilité vérifiée automatiquement

`tests/gnu_compat.rs` crée des fichiers de hash avec FileSeal, puis les fait valider par `sha256sum -c --strict` et `md5sum -c --strict` de Git for Windows (coreutils 8.32). Noms testés :
- espaces et crochets ;
- accents et tiret cadratin ;
- `-` en tête de nom ;
- sans extension ;
- japonais.

Le test vérifie aussi l'inverse :
- FileSeal relit le fichier de hash que `sha256sum` écrit pour un fichier (`hash *nom`) ;
- il vérifie un `SHA256SUMS` écrit par `sha256sum` pour plusieurs fichiers, avec un sous-dossier, des accents et un `-` en tête de nom.
