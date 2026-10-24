# L'écran Bases

Code :
- moteur : `crates/fileseal-core/src/store/browse.rs` (recherche, historique, emplacements), `store/files.rs` (fichier de la base) et `ops/volumes.rs` (disques, réaffectation) ;
- application : `src-tauri/src/databases.rs` ;
- interface : `ui/src/lib/components/DatabasesScreen.svelte` et ses trois parties.

Cadrage, §9 (point 7) et §10.

On y accède par l'onglet **Bases**, en haut de la fenêtre. L'écran Fichiers garde sa liste et ses résultats pendant ce temps.

Disposition (2.3) :
- une ligne pour la base active : nom, nombre de références, version du schéma, chemin, et le menu **Fichier de la base** (renommer, afficher dans le dossier, déplacer, sauvegarder, restaurer) ;
- deux sous-onglets : **Références** (par défaut) et **Disques** ;
- l'historique d'une référence s'ouvre dans un volet à droite de la liste. Si la fenêtre est étroite, il s'ouvre en dessous, et seulement sur demande (double-clic, Entrée, bouton **Historique**) ;
- le formulaire de réaffectation n'apparaît qu'avec **Réaffecter un dossier…**, ou **Réaffecter…** sur la ligne d'un disque.

## La base active

- **Renommer** : change le **nom affiché** (dans la base, table `meta`). Le fichier garde son nom ; la liste des bases récentes suit.
- **Afficher dans le dossier** : ouvre l'Explorateur sur le fichier de la base.
- **Déplacer ou renommer le fichier** : jamais par-dessus un autre fichier.
  1. La base est fermée, et son journal réintégré. Si un journal `-wal` reste, rien n'est déplacé.
  2. **Sur le même disque**, le fichier est renommé (`MoveFileExW` sans remplacement).
  3. **Vers un autre disque**, il est copié sous un nom provisoire, puis la copie est ouverte et ses comptes (éléments, références, emplacements, vérifications) comparés à ceux de la base. L'original n'est supprimé qu'ensuite. S'il ne peut pas l'être, les deux fichiers restent, et c'est la copie qui sert.
  4. En cas d'échec, la base d'origine est rouverte, telle qu'elle était.
- **Sauvegarder** : une copie cohérente et compacte (`VACUUM INTO` de SQLite).
  - Elle est écrite sous un nom provisoire (`.partial`), puis renommée : une copie interrompue n'a jamais l'air complète.
  - L'espace libre est contrôlé d'abord.
  - La copie est ensuite rouverte en lecture seule, et ses comptes comparés à ceux de la base.
  - La boîte de dialogue de Windows demande avant de remplacer un fichier existant ; une copie ne remplace jamais la base elle-même.
- **Restaurer une sauvegarde** :
  1. La sauvegarde choisie est **examinée sans être modifiée** (ouverture immuable) : identifiant FileSeal, version du schéma, `quick_check`, nom et nombre de références. La confirmation affiche ces informations.
  2. Le contenu actuel de la base est **sauvegardé à côté** : `<nom>.before-restore-AAAAMMJJ-HHMMSS.fileseal` (heure UTC).
  3. La base est fermée, la sauvegarde copiée sous un nom provisoire, puis mise à la place du fichier en un seul renommage.
  4. La base est rouverte. Une sauvegarde d'un schéma plus ancien est mise à jour à cette occasion, avec sa propre copie de sécurité ([modele-donnees.md](modele-donnees.md)).

  Une sauvegarde est refusée si ce n'est pas une base FileSeal, si elle vient d'une version plus récente, si elle est endommagée, ou si c'est la base elle-même.

Une base ouverte en lecture seule peut être consultée et sauvegardée, mais pas renommée, déplacée ni restaurée.

## Les disques

La liste regroupe les fichiers de la base par disque (numéro de série du volume, enregistré avec chaque emplacement), et dit où chaque disque est aujourd'hui.

| État | Signification |
|---|---|
| ✔ Branché | Le disque est là où il a été enregistré. |
| ⚠ Maintenant en F:\ | Le même disque est revenu sous une autre lettre. |
| – Non branché | Le disque n'est trouvé nulle part. |
| ℹ Disque non identifié | Fichiers enregistrés sans numéro de série (certains partages réseau) : seule la présence du dossier est indiquée. |

**Comment un disque est retrouvé** : FileSeal lit le numéro de série de chaque lettre de lecteur présente. Pour chaque lettre qui porte le bon numéro, il vérifie que quelques fichiers du disque (5 au plus) existent à leur place.

Le numéro seul ne suffit pas : un lecteur `subst` porte le numéro du disque qu'il montre. Seuls les noms des fichiers sont consultés ; aucun fichier n'est lu.

### Changement de lettre de lecteur : 3 clics

1. Onglet **Bases** ;
2. bouton **« Mettre à jour vers F:\… »** sur la ligne du disque ;
3. **Confirmer** dans la page.

Les emplacements des fichiers de ce disque (et seulement de lui) deviennent `F:\…`, à la même place sur le disque. Rien n'est relu. Les anciens emplacements restent dans l'historique de chaque fichier, et l'ensemble est enregistré comme une opération (`attach`, avec le détail dans `summary_json`).

C'est la règle n° 7 du cadrage, appliquée à tout un disque sans attendre une vérification. La mise à jour proposée après une vérification (R2, [rapprochement.md](rapprochement.md)) reste disponible dans les résultats.

### Réaffecter un dossier

Pour des fichiers déplacés ou copiés **ensemble** ailleurs : autre disque, partage réseau renommé, dossier renommé.
1. Indiquer où ils étaient enregistrés (le bouton « Réaffecter… » d'un disque remplit ce champ) et où ils sont maintenant (saisie ou « Choisir… »).
2. **Aperçu** : nombre de fichiers enregistrés sous l'ancien dossier, combien sont trouvés à la nouvelle place, et quelques exemples.
3. **Appliquer**, avec confirmation.

Règles :
- Le chemin sous l'ancien dossier est conservé tel quel : `D:\Films\SF\Alien.mkv` devient `E:\Archives\Films\SF\Alien.mkv` si l'on réaffecte `D:\Films` vers `E:\Archives\Films`.
- Seuls les fichiers trouvés à la nouvelle place sont déplacés. Les autres restent où ils étaient enregistrés, sauf si l'option « Aussi ceux qui n'y sont pas trouvés » est cochée (disque débranché pour l'instant).
- Pour un fichier trouvé, sa nouvelle place sur son disque est relue (numéro de série, chemin sur le volume), ainsi que sa taille et sa date. Son contenu n'est pas lu : une vérification dira s'il est conforme.
- Si un autre fichier de la base est déjà enregistré à la nouvelle place, rien n'est changé pour celui-ci, et c'est signalé.
- Tout se fait en une transaction et une opération. Les anciens emplacements restent dans l'historique.

## Les références

- **Recherche** : une partie du nom ou du dossier (sans tenir compte de la casse), ou le **début d'un hash** (8 chiffres hexadécimaux ou plus). Sans texte, tout est listé.
- **Liste** : nom, dossier, taille, date d'enregistrement, dernière vérification (date et résultat). 2 000 lignes au plus, triées par chemin ; au-delà, un message invite à préciser la recherche.
- **Historique** de la référence choisie (il suit la ligne active, au clic comme aux flèches) :
  - les **versions** : date, origine (créée, acceptée comme nouvelle version, importée), taille, empreintes, date de remplacement ;
  - les **emplacements** : l'actuel, puis les anciens, avec leurs dates (« vu du … au … », « quitté le … ») ;
  - les **vérifications** : date, résultat, chemin, détail. Les 200 dernières sont affichées.

### Retirer des références de la base

Pour un fichier supprimé exprès, un dossier ajouté par erreur, un disque donné ou détruit. Code : `ops/items.rs`, `store/browse.rs` (`retire_items`, `restore_items`).

- **Afficher : Dans la base / Retirées** choisit les références listées.
- **Retirer la sélection de la base (N)…** : plusieurs lignes se choisissent comme dans l'Explorateur (Maj, Ctrl, Ctrl+A pour toutes celles affichées).
- **Retirer les N résultats de la base…** : tous ceux que trouve la recherche, même au-delà des 2 000 affichés. Proposé seulement quand un texte est cherché, pour ne pas vider une base d'un clic.
- **Confirmation dans la page**, avec le nombre et le nom de la base.
- **Ce que fait un retrait** : en une transaction, l'élément est marqué retiré (`item.retired_at`, prévu dès le schéma v1), et son emplacement cesse d'être actuel.
  - L'élément ne sert plus à rien : vérifier (« Aucune référence »), rechercher (contenu inconnu), disques, compte des références.
  - Sa place est libre : le même fichier ajouté de nouveau devient un nouvel élément.
  - **Rien n'est effacé** : versions, emplacements et vérifications restent, et l'historique affiche « Retirée de la base le … ».
  - Les fichiers et leurs fichiers de hash ne sont jamais touchés.
- **Remettre** (vue « Retirées ») : l'élément retrouve sa dernière place. Si un autre élément l'occupe entre-temps, il reste retiré, et le message le dit.
- **Trace** : chaque retrait ou remise est une opération de la base. Le schéma v1, figé, n'en connaît que cinq sortes : comme la réaffectation, c'est un `attach`, avec le détail dans `summary_json` (`{"remove": N}`, `{"putBack": N, "taken": M}`).
- Une base en lecture seule refuse.
