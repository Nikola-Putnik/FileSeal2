# Format du rapport CSV

Le bouton « Export report… » des résultats écrit le rapport de la **dernière opération** : une ligne par fichier. Pour un lot, les lignes suivent l'ordre de la liste. Code : `crates/fileseal-core/src/report.rs`.

## Fichier

- UTF-8 **avec BOM** : Excel lit ainsi correctement les accents.
- Séparateur : **`,`** quand l'interface est en anglais, **`;`** quand elle est en français. C'est ce qu'Excel attend selon la langue de Windows.
- Fins de ligne CRLF ; champs entre guillemets s'ils contiennent le séparateur, un guillemet ou un saut de ligne, avec les guillemets doublés (RFC 4180).
- Écrit de façon atomique : un fichier existant n'est remplacé qu'une fois le nouveau complet.

## Colonnes

| Colonne | Contenu |
|---|---|
| `action` | `create`, `verify` ou `search` (celle du lot pour un fichier non traité) |
| `path` | Chemin absolu du fichier |
| `status` | Code du statut (liste ci-dessous) |
| `size` | Taille en octets, si connue |
| `sha256`, `md5` | Hash calculés (vide si non calculé) |
| `expected_sha256`, `expected_md5` | **Vérification** : hash attendus, ceux de la base d'abord, puis ceux des fichiers de hash. **Création** : hash de la référence existante en base. |
| `detail` | Détails séparés par ` \| ` : résultat de chaque destination (création), écarts de taille, problème rencontré, emplacement enregistré sous une autre lettre de lecteur, résultat de la recherche par contenu (`content: …`, `same content: <chemin> (<présence>)`, `same name: <chemin>`) |

Les codes sont en anglais, stables, et identiques à ceux de l'historique de la base.

## Codes de statut

- **create** : `created`, `completed`, `alreadyReferenced`, `skipped` (déjà référencé, non relu), `imported` (ajouté à la base depuis ses fichiers de hash, non relu), `conflict`, `partialSuccess`, `needsDecision`, `failed`, `cancelled`. Dans `detail`, une destination laissée sans lecture vaut `notRead`, ou `imported` pour la base.
- **verify** : `match`, `mismatch`, `noReference`, `noCompatibleReference`, `conflict`, `invalidHashFile`, `notFound`, `readError`, `locked`, `modifiedDuringRead`, `cancelled`, `notProcessed`, `failed`.
- **search** : `unknown`, `knownHere`, `changedHere`, `locationRecognized`, `probablyMoved`, `copyOfKnown`, `ambiguous`, `notFound`, `readError`, `locked`, `modifiedDuringRead`, `cancelled`, `failed`.
- **Fichiers d'un lot non lus** (toutes actions) : `notProcessed` (raison dans `detail` : `cancelled`, `volumeUnavailable`, `databaseUnavailable`, avec la cause) et `duplicate` (`detail` : `same file as <chemin>`). Voir [lots.md](lots.md).
- **Présence d'une copie** (dans `detail`) : `present`, `missing`, `volumeUnavailable`.
- **Issue d'une recherche par contenu** (dans `detail`) : `unknown`, `knownHere`, `probablyMoved`, `copyOfKnown`, `ambiguous`.
