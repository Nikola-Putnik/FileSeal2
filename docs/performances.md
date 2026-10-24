# Performances

Mesures à refaire à chaque changement du moteur de lecture. Outil : `cargo run -p fileseal-core --release --example throughput -- <taille en Mio>`. Il hache un fichier déjà en cache : le processeur est alors la seule limite, et le disque n'intervient pas.

## 2026-10-22 — AMD Ryzen AI 7 350, 32 Go, Windows 11, fichier de 2 Gio en cache

| Mesure | Débit |
|---|---|
| Lecture seule, blocs de 1 Mio | 8 733 Mio/s |
| SHA-256 | 1 831 Mio/s |
| MD5 | 701 Mio/s |
| SHA-256 + MD5, une seule lecture | 539 Mio/s |

**Lecture** :
- SHA-256 profite des instructions SHA du processeur (SHA-NI, détectées automatiquement par la crate `sha2`). Il est plus rapide que MD5, qui n'a pas d'accélération matérielle.
- Avec les deux algorithmes, les deux calculs s'enchaînent sur le même cœur : 1 / (1/1 831 + 1/701) ≈ 507 Mio/s en théorie, 539 Mio/s mesurés.
- Dans l'application (`tauri dev`), le fichier de 10 Gio en cache a été haché en SHA-256 à environ 1,7 Go/s.

**Comparaison avec les supports** (débits séquentiels habituels) :

| Support | Débit typique | Facteur limitant avec SHA-256 | … avec SHA-256 + MD5 |
|---|---|---|---|
| Disque dur externe USB | 120 à 250 Mo/s | le disque | le disque |
| Partage réseau en Gigabit | ~110 Mo/s | le réseau | le réseau |
| SSD SATA | ~550 Mo/s | le disque | les deux à égalité |
| SSD NVMe | 2 à 7 Go/s | le processeur | le processeur (~540 Mio/s) |

**Décision (étape 2)** : pas de parallélisation pour l'instant. Pour les usages principaux (disques durs, disques externes, NAS), le support reste la limite, même avec deux algorithmes. Le gain ne concernerait que les SSD NVMe avec SHA-256 + MD5 :
- calculer les deux algorithmes sur deux cœurs donnerait au plus ~700 Mio/s au lieu de 540, soit +30 % ;
- superposer la lecture et le calcul n'aurait d'effet que sur un SSD NVMe.

Ces optimisations sont reportées à l'étape 10 (mesures sur de vrais supports), si elles s'avèrent utiles.

## 2026-10-22 — Découverte (étape 6), même machine

Fichiers vides ou petits, dossiers dans le cache du système (deuxième parcours), SSD NVMe.

| Mesure | Résultat |
|---|---|
| 50 000 fichiers dans 100 dossiers, moteur seul (`--release`) | premier paquet après 2 ms, tout en 85 ms |
| 20 000 fichiers dans 40 dossiers, dans l'application (`tauri dev`, à travers le canal) | premier paquet après 23 ms, tout reçu en 300 ms |
| Annulation demandée à réception du premier paquet, dans l'application | parcours arrêté 8 ms après la demande |

Sous Windows, la liste d'un dossier fournit déjà la taille et les attributs de chaque fichier : la découverte n'ouvre aucun fichier. Sur un disque dur ou un partage réseau, le premier parcours d'un dossier non mis en cache sera plus lent ; à mesurer à l'étape 10.

## 2026-10-22 — Lots (étape 7), même machine

| Mesure | Résultat |
|---|---|
| 1 000 fichiers de quelques octets, création en base seule (CLI `--release`) | 2,4 s, soit 2,4 ms par fichier |
| Les mêmes, vérification en base seule | 0,9 s, soit 0,9 ms par fichier |
| 300 fichiers de 20 Ko sur deux dossiers, création avec fichiers de hash et base, dans l'application (`tauri dev`) | 1,2 s |
| Les mêmes, vérification | 0,36 s |
| Annulation pendant la lecture d'un fichier de 1,5 Gio, dans l'application | effective en 5 ms |

**Constat** : pour les petits fichiers, le coût vient des écritures en base, pas de la lecture. Chaque fichier fait sa propre transaction, synchronisée sur le disque (`synchronous=FULL`) : de l'ordre d'une milliseconde par transaction sur ce SSD, bien plus sur un disque dur.

**Piste pour l'étape 10** : regrouper les écritures d'un lot en une transaction par paquet (par exemple toutes les secondes). Le gain attendu est d'un facteur 10 ou plus sur les petits fichiers. En contrepartie, un arrêt brutal ferait perdre au plus le dernier paquet : les fichiers de hash correspondants seraient déjà écrits, et une nouvelle création compléterait la base (« complété »). *Étape 10 : non fait, `synchronous=NORMAL` pendant les lots a suffi (voir plus bas).*

## 2026-10-22 — Interface (étape 8), même machine, `tauri dev`

| Mesure | Résultat |
|---|---|
| Dossier de 50 000 fichiers déposé : premières lignes affichées | 45 ms |
| … liste complète (50 000 lignes) | 0,8 s |
| Défilement par sauts de 13 000 px, 120 images | 16,6 ms par image en médiane (60 images/s), pire 21 ms |
| Lignes réellement présentes dans la page | 28 |
| Vérification des 50 000 (sans référence : aucune lecture), résultats affichés au fil de l'eau | 53 s ; image la plus longue pendant le lot : 41 ms |
| Mémoire de la page avec 50 000 résultats | 82 Mo |
| 301 fichiers (1,5 Go) : création, fichiers de hash et base, par l'interface | 2,1 s |

Les 53 s de la vérification viennent du moteur, pas de l'interface : environ 1 ms par fichier de recherches en base et d'ouvertures, même sans lire le contenu. À reprendre à l'étape 10, avec le regroupement des écritures.

## 2026-10-22 — Cas réels et optimisations (étape 10), même machine

Outils :
- `cargo run -p fileseal-core --release --example disk_read -- <dossier> 4096` : fichiers écrits sans passer par le cache de Windows, puis lus ;
- `--example costs` : coût des appels système par fichier.

### Le disque, hors cache

SSD NVMe SK hynix, 1 To, fichiers de 4 Gio :

| Mesure | Débit |
|---|---|
| Écriture sans cache | 2 500 Mio/s |
| Lecture sans cache (le disque seul) | 4 879 Mio/s |
| FileSeal SHA-256, fichier hors cache | 2 034 Mio/s |
| FileSeal SHA-256, fichier en cache | 2 024 Mio/s |
| FileSeal MD5, hors cache | 735 Mio/s |
| FileSeal SHA-256 + MD5, hors cache | 560 Mio/s |
| Par SMB (`\\localhost\C$`, lecteur réseau monté), SHA-256 | ≈ 1 700 Mio/s (4 Gio en 2,4 s) |

**Conclusions** :
- **Hors cache, le débit est le même qu'en cache.** La lecture anticipée de Windows (`FILE_FLAG_SEQUENTIAL_SCAN`) recouvre déjà le calcul, et le processeur est la seule limite. Une lecture en parallèle du calcul, dans FileSeal, n'apporterait rien : **non faite**.
- **MD5 et SHA-256 sur deux cœurs** donneraient environ 735 Mio/s au lieu de 560 (+30 %), seulement quand les deux sont cochés, et seulement sur un SSD rapide : **non fait**. Le gain ne justifie pas un fil par algorithme.
- **Le vrai gisement sur SSD NVMe** : lire deux fichiers à la fois sur le même disque. Le disque débite 4,9 Gio/s, un cœur hache 2 Gio/s. C'est l'évolution 9 du cadrage (plusieurs lecteurs par volume pour les SSD), qui demande de reconnaître un SSD d'un disque dur. Laissé pour plus tard.
- **Pas de disque dur ni de vrai NAS ici.** Sur ces supports, le disque ou le réseau limite (environ 110 à 250 Mo/s), bien en dessous de ce que hache FileSeal. Mesures à faire sur ces supports ([tests-build-livraison.md](tests-build-livraison.md)).

### Le coût de chaque fichier (petits fichiers)

2 000 fichiers de quelques octets dans un dossier à 12 niveaux de profondeur ; ligne de commande `--release`, base seule (sans fichier de hash) :

| Lot | Avant | Après | |
|---|---|---|---|
| Vérifier des fichiers inconnus de la base | 1 808 ms | 264 ms | ×6,8 |
| Créer les références | 4 948 ms | 1 055 ms | ×4,7 |
| Vérifier des fichiers connus | 1 928 ms | 644 ms | ×3 |

Ce qui coûtait, et ce qui a été fait :
1. **`GetVolumePathNameW`** : 660 µs par appel pour un dossier profond. Windows remonte le chemin à la recherche de points de montage. FileSeal l'appelait pour chaque fichier (place sur le disque, présence du disque). **La racine est désormais retenue par dossier** pendant 30 s (`platform::volume_root_of_file`), et un fichier ne coûte plus que 0,5 µs.
2. **Une synchronisation du disque par transaction** (`synchronous=FULL`), deux par fichier vérifié. **Pendant un lot, la base passe en `synchronous=NORMAL`.** En mode WAL, chaque transaction est écrite dans le journal sans attendre le disque. À la fin du lot, tout est forcé sur le disque (`wal_checkpoint(FULL)`), et `FULL` revient. Une écriture qui échoue (disque plein, base perdue) échoue toujours aussitôt.
3. **La préparation des requêtes SQL** les plus fréquentes est désormais gardée en cache (`prepare_cached`) : environ 10 % de gain.

**Ce qui reste** : environ 0,13 ms par fichier inconnu, 0,5 ms par fichier créé, 0,3 ms par fichier vérifié. Avec des fichiers de hash, il faut ajouter environ 0,7 ms par fichier écrit. Ce coût est voulu : un fichier de hash est forcé sur le disque avant d'être publié.

### 50 000 fichiers

Deux jeux de 50 000 petits fichiers : 100 dossiers de 500 fichiers, avec les mêmes noms (`file 0.jpg`…) dans chaque dossier. Ligne de commande `--release`, base seule :

| Lot | Contenus tous différents | Contenus tous identiques |
|---|---|---|
| Vérifier, base vide (rien à lire) | 5,3 s | 5,3 s |
| Créer les références | 48,6 s | 43,2 s (234 s avant la correction 4) |
| Vérifier les fichiers connus | 15,0 s | 15,0 s |

L'interface mettait 53 s pour la première ligne avant ces changements.

Ce que cette mesure a fait corriger :

4. **La recherche par contenu** listait d'abord toutes les références ayant la même empreinte (`r.id IN (SELECT …)`), avant de garder les 20 premières. Avec 50 000 fichiers identiques, chaque nouveau fichier coûtait 4,7 ms, de plus en plus à mesure que la base grandissait. C'est désormais une jointure qui part de l'index des empreintes et s'arrête à la 20ᵉ. Un test vérifie que chaque recherche faite pour un fichier passe par un index (`per_file_lookups_use_an_index`).
5. Déjà pendant cette étape : les recherches ne sont plus triées (un tri rassemble d'abord toutes les correspondances), elles s'arrêtent à 20, et un fichier vide n'est jamais rapproché par son contenu (tous les fichiers vides ont le même).

**Ce qui reste** : 1 ms par fichier créé.
- **Les indices par nom (R7)** en prennent 21 s sur 48,6. Chaque nouveau fichier dont le nom est déjà dans la base (ici, chaque nom existe dans 100 dossiers) donne jusqu'à 20 contrôles de présence sur le disque, de 16 µs chacun. Sans indices, la création prend 27,8 s. C'est le cas d'une photothèque (`IMG_0001.JPG` dans chaque dossier). **Gardé** : l'indice sert à retrouver un fichier déplacé puis modifié, et son coût est borné (0,4 ms par fichier, sans augmenter avec la base). Sur ce SSD, une photo de 5 Mo se lit en 2,5 ms ; sur un disque dur, en 30 ms.
- **Essayé sans gain** : un contrôle de présence par les seuls attributs (`GetFileAttributesW`) au lieu de `Path::exists` (15,6 µs au lieu de 16,7) ; un cache SQLite de 64 Mo au lieu de 2 (48,9 s). Non retenus.
- **Regrouper les écritures par paquets** (piste de l'étape 7) : **non fait**. Depuis `synchronous=NORMAL`, une transaction n'attend plus le disque. Il resterait au plus 0,3 ms par fichier à gagner (écart entre créer sans indices, 0,56 ms, et vérifier, 0,30 ms). En échange, la règle deviendrait plus fragile : un fichier ne pourrait être annoncé « créé » qu'après la validation de tout son paquet.

## 2026-10-24 — Base : taille et tri de l'écran Bases, même machine

### Taille d'une base

Ligne de commande `--release`, 10 000 fichiers, SHA-256 dans la base seule, chemins de 205 caractères :

| | Taille |
|---|---|
| Base vide | 86 Ko |
| 10 000 références | 18,0 Mo, soit 1,8 Ko par référence |
| Chaque vérification des 10 000 fichiers | +4,5 Mo, soit 450 octets par fichier vérifié |

Avec des noms plus longs de 100 caractères : 9 octets de plus par caractère et par référence, 1,4 par vérification. Pour un chemin d'environ 80 caractères, on peut compter environ 1 Ko par référence et 300 octets par vérification. **C'est l'historique des vérifications qui fait grossir une base** : 10 000 fichiers vérifiés chaque mois pendant 10 ans donnent environ 370 Mo. Une référence retirée garde sa place (environ 1 Ko et ses vérifications), sans en prendre davantage.

### Trier les références (écran Bases)

Le tri est fait par la base sur toutes les références trouvées, puis les 2 000 premières sont envoyées. La requête tourne hors du fil de la fenêtre : un tri long retarderait l'affichage de la liste, sans jamais figer l'application.

Base de 100 000 références avec 5 vérifications chacune (une sur dix jamais vérifiée), test `listing_a_large_database_in_every_order` en `--release`, 2 000 lignes :

| Ordre | Toute la base | Recherche « season 3 » (20 000 trouvées) |
|---|---|---|
| Dossier (chemin), croissant ou décroissant | 29 ms | 41 ms |
| Nom | 75 à 128 ms | 76 à 120 ms |
| Taille | 101 ms | 88 ms |
| Enregistrée le | 112 ms | 99 ms |
| Dernière vérification | 113 à 118 ms | 96 à 98 ms |

Le tri par chemin suit l'index des emplacements actuels, sans trier le reste. Les autres ordres trient toutes les références trouvées, ce qui coûte environ 1 µs par référence.

Dans la fenêtre (`tauri dev`), sur une base réelle de 41 036 références : 50 à 200 ms entre le clic sur un en-tête et la liste triée, transfert des 2 000 lignes compris.
