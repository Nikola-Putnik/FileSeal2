# Tests manuels, build et livraison

Les tests automatiques sont décrits dans [installation-developpement.md](installation-developpement.md). Ce document rassemble les **contrôles manuels**, à rejouer avant chaque version, sur des fichiers de test (jamais sur des données réelles), puis la **construction et la publication** d'une version (fin du document).

## Préparer des données de test

Dans un dossier temporaire :
- un dossier `Photos` d'une centaine de petits fichiers ;
- un dossier `Video` avec un fichier d'au moins 1 Go (pour avoir le temps de mettre en pause ou d'annuler) ;
- une base de test créée depuis l'application.

## Accessibilité

### Clavier seul (sans souris)

1. Depuis le haut de la fenêtre, parcourir avec Tab. Attendu :
   - l'ordre est : écrans, base active, aide (?), préférences (⚙), action (un seul arrêt), Réglages (puis leurs cases s'ils sont dépliés), ajout de fichiers et de dossier, Extensions, filtre, Retirer, ⋯, onglets (un seul arrêt), liste, puis le lancement dans la barre du bas ;
   - le focus est toujours visible.
2. Ajouter un dossier avec Ctrl+Maj+O, choisir le dossier de test au clavier dans la boîte de dialogue de Windows.
3. Dans la liste :
   - ↓ ↓ sélectionne une ligne ;
   - Maj+↓ étend la sélection ;
   - Alt+↑ la monte ;
   - Suppr la retire ;
   - Entrée amène le focus dans le détail, Échap le ramène à la liste.
4. Lancer avec Ctrl+Entrée, puis atteindre Pause, Reprendre et Annuler avec Tab.
5. Après le lot :
   - atteindre une puce de statut et l'activer avec Espace ;
   - prendre une décision pour tous les résultats ;
   - confirmer dans la page.
6. Fermer la fenêtre (Alt+F4) pendant un lot. Attendu : la question s'affiche avec le focus sur « Continuer ».

### Narrateur (intégré à Windows) et NVDA

Démarrer le Narrateur avec Win+Ctrl+Entrée. Installer NVDA depuis nvaccess.org (version portable possible). Avec chacun, vérifier :

- **Liste** : en entrant dans la liste, elle est annoncée comme une grille, avec son nom (« Fichiers à traiter ») et son nombre de lignes. En descendant, chaque ligne est annoncée avec son nom, son dossier, sa taille et son état, ainsi que « sélectionné » le cas échéant.
- **Défilement** : sur une liste de plusieurs milliers de fichiers, Page↓ puis Ctrl+Fin. L'annonce suit, et la position (ligne n sur N) est cohérente.
- **Statuts** : chaque statut est annoncé en mots (« Différent de la référence »), jamais seulement par une couleur.
- **Messages** : en fin de parcours et en fin de lot, le bilan est annoncé sans avoir à le chercher.
- **Erreurs** : une base perdue en cours de lot est annoncée comme une alerte.
- **Fermeture** : la question de fermeture est annoncée comme une boîte de dialogue d'alerte.

À consigner : version de Windows, du Narrateur et de NVDA, et ce qui a été entendu, dans le journal de l'étape.

### Contraste élevé

Paramètres de Windows > Accessibilité > Thèmes de contraste : choisir « Désert » puis « Nuit aquatique ». Attendu :
- les icônes de statut et leurs textes restent visibles ;
- la ligne sélectionnée se distingue (couleurs système de sélection) ;
- le focus est visible partout.

## Changement de lettre de lecteur (critère de l'étape 9)

1. `subst X: <dossier de test>`, puis créer dans FileSeal les références (base) des fichiers de `X:\…`.
2. `subst X: /D`, puis `subst W: <même dossier>`.
3. Dans FileSeal, onglet **Bases** (clic 1). Attendu : le disque `X:\` est « Maintenant en W:\ ».
4. « Mettre à jour vers W:\… » (clic 2), puis **Confirmer** (clic 3). Attendu : « N emplacements mis à jour », le disque est « Branché ».
5. Vérifier `W:\…` depuis l'écran Fichiers : tout est conforme. L'historique d'un fichier montre l'ancien emplacement en `X:`.
6. `subst W: /D` pour finir.

Avec un vrai disque USB : le brancher, créer les références, le débrancher, changer sa lettre dans « Gestion des disques » de Windows, le rebrancher, puis les étapes 3 et 4.

## Conditions réelles, rejouables sur ce PC

Le script `tools/manual/real-cases.ps1` produit chaque statut par une situation réelle, sans injection de panne et sans droits administrateur. Il fonctionne sur des fichiers de test, dans un dossier à lui, supprimé à la fin :

```
cargo build --release -p fileseal-cli
powershell -ExecutionPolicy Bypass -File tools\manual\real-cases.ps1 [-Folder <dossier sur C:>]
```

Les cas réseau montent le partage administratif local `\\localhost\C$` sur une lettre libre, le déconnectent en plein lot, puis le démontent.

| Situation réelle | Statut attendu (observé le 2026-10-22) |
|---|---|
| Fichier inchangé / modifié / supprimé après son fichier de hash / jamais référencé | conforme / différent / fichier attendu introuvable / aucune référence |
| Fichier ouvert en écriture par un autre programme | verrouillé (erreur Windows 32) |
| Lecture interdite par les droits du fichier | erreur de lecture (accès refusé) |
| Fichier de hash et base en désaccord | conflit (fichier de hash conforme, base différente) |
| Fichier de hash qui n'en est pas un | fichier de hash invalide |
| Dossier interdit en écriture, base disponible | réussite partielle (fichier de hash refusé, base écrite) |
| Base protégée en écriture | vérifier : conforme, « non ajouté à l'historique » ; créer : refusé avant de commencer |
| Créer de nouveau : même contenu / MD5 en plus / autre contenu / fichier déplacé ici | déjà référencé / complété / conflit / décision requise |
| Rechercher : connu ici, modifié ici, déplacé ici, copie, deux origines possibles, inconnu | connu ici, modifié depuis l'enregistrement, déplacement probable, copie d'un fichier connu, plusieurs origines possibles, contenu inconnu |
| Le même fichier par un lien physique | doublon |
| Fichier marqué « hors ligne » (`attrib +O`, l'un des attributs des fichiers « en ligne uniquement ») | écarté « en ligne uniquement » quand son dossier est parcouru ; retenu s'il est choisi directement |
| Base ouverte par un autre FileSeal | base déjà utilisée |
| Un dossier par une lettre réseau et par son chemin UNC | doublon (même dossier) |
| Partage perdu pendant la lecture d'un fichier | ce fichier : erreur de lecture (erreur réseau, Windows 59) ; les suivants : non traité (disque indisponible) ; volume signalé perdu |
| Base sur le partage, partage perdu pendant un lot | un fichier en échec, lot arrêté « base indisponible », le reste non traité ; après reconnexion, chaque fichier annoncé créé est bien dans la base |

Observés aussi en conditions réelles aux étapes précédentes :
- annulé et non traité (annulation et pause, étapes 7 et 8) ;
- disque disparu (lecteur `subst` retiré, étape 7) ;
- changement de lettre (étape 9) ;
- noms trop longs, invalides, réservés ou non représentables (étapes 3 et 6, sur NTFS).

## Avec du matériel réel

Ces cas demandent un geste physique ou un matériel particulier : ils ne sont pas rejoués automatiquement.

### Clé USB débranchée en pleine lecture

1. Sur une clé USB, un dossier avec un gros fichier (plusieurs Go) et quelques petits.
2. Dans FileSeal, créer leurs références (fichiers de hash), puis lancer une vérification du dossier.
3. Débrancher la clé pendant la lecture du gros fichier.
4. Attendu :
   - ce fichier : « erreur de lecture » (disque débranché) ;
   - les autres fichiers de la clé : « non traité (disque indisponible) » ;
   - la clé signalée perdue ;
   - les fichiers d'autres disques du même lot : traités.

### Disque plein

1. Une clé USB presque pleine (la remplir avec un gros fichier jusqu'à quelques Mo libres).
2. Créer les références (fichiers de hash) d'un dossier de cette clé. Attendu : « échec » ou « réussite partielle » (fichier de hash : « espace disque insuffisant ») ; aucun fichier `.fileseal-*.tmp` laissé.
3. Avec une base sur cette clé : « base indisponible » (disque plein), le lot s'arrête proprement.

### OneDrive « en ligne uniquement »

1. Dans un dossier OneDrive de test, un fichier marqué « Libérer de l'espace » (il n'est plus sur le PC).
2. Ajouter le dossier dans FileSeal. Attendu : le fichier est dans « Écartés » (« En ligne uniquement »), et il n'est pas téléchargé.
3. Ajouter le fichier lui-même (choix direct). Il est alors lu, ce qui le télécharge si OneDrive est lancé. OneDrive arrêté, il donne « erreur de lecture ».

### Disque dur et NAS

`cargo run -p fileseal-core --release --example disk_read -- <dossier sur le disque> 4096`, puis noter les débits dans [performances.md](performances.md). Sur un NAS, faire de même avec un fichier de plusieurs Go copié sur le partage, et une vérification par FileSeal.

### Version portable sur une vraie clé USB

1. Décompresser le zip portable sur une clé USB, lancer `FileSeal.exe`. Attendu : « portable » dans l'en-tête, un dossier `data` créé à côté du programme.
2. Créer une base proposée à côté du programme, quelques références, puis fermer.
3. Brancher la clé sur un autre PC, ou lui donner une autre lettre dans « Gestion des disques ». Relancer : la base est rouverte sous la nouvelle lettre.
4. Sur ce second PC, rien ne doit apparaître dans `%APPDATA%\org.fileseal.desktop`.

## Construire une version

```
scripts\build.cmd
```

Produit dans `target\release\bundle\` :

| Fichier | Contenu |
|---|---|
| `nsis\FileSeal_<version>_x64-setup.exe` | Installateur NSIS par utilisateur, en anglais et en français selon la langue de Windows. Sans droits administrateur. Installe dans `%LOCALAPPDATA%\FileSeal`, avec un raccourci dans le menu Démarrer et sur le Bureau, et une entrée dans « Applications installées ». La licence et les licences des autres projets sont installées à côté du programme (`LICENSE.txt`, `THIRD-PARTY-NOTICES.txt`, ressources de `tauri.conf.json`). |
| `portable\FileSeal_<version>_x64-portable.zip` | Un dossier `FileSeal` : `FileSeal.exe` (le même programme), `portable.flag` et `README.txt` (anglais et français), venus de `scripts\portable\`, `LICENSE.txt` (licence MIT, copie de `LICENSE`) et `THIRD-PARTY-NOTICES.txt`. |
| `….sha256` à côté de chacun | Leur empreinte, écrite par `fileseal-cli create` : un fichier de hash GNU, vérifiable par FileSeal, `sha256sum -c` ou `certutil`. |

- **La version** est celle de `[workspace.package]` dans `Cargo.toml` : elle vaut pour le moteur, l'application, l'installateur et les informations de version de Windows. `package.json` doit dire la même (test `the_interface_package_has_the_same_version`).
- **Icône** : `assets\icon.svg` est la source. `npx tauri icon assets\icon.svg -o <dossier temporaire>` la décline dans toutes les tailles ; copier dans `src-tauri\icons\` les seuls fichiers listés dans `tauri.conf.json`, les autres servent à d'autres plateformes.
- **Licences des autres projets** : `build.cmd` régénère d'abord `THIRD-PARTY-NOTICES.txt` (`node tools/third-party-notices.mjs`) à partir de ce qui est réellement construit : les crates compilées dans `fileseal.exe` pour Windows (sans les scripts de construction ni les macros procédurales, qui ne laissent rien dans le programme) et les paquets npm présents dans l'interface (lus dans les cartes de sources d'une construction faite pour l'occasion). Pour une crate à plusieurs licences au choix, celle retenue est nommée (MIT d'abord) ; les textes sont ceux des crates, ou le texte standard au nom de leurs auteurs quand une crate n'en fournit pas. Si le fichier change, le committer avec la version.
- **Aucun chemin de l'ordinateur de construction** dans les programmes publiés : Rust y garde les chemins des sources des dépendances (dans le profil Windows, avec le nom de session) pour ses messages d'erreur internes. `build.cmd` les rend relatifs (`--remap-path-prefix`, calculé depuis `%USERPROFILE%` au moment de la construction), puis refuse la livraison si le chemin du profil figure encore dans `fileseal.exe` ou `fileseal-cli.exe`. L'option `trim-paths` de Cargo le ferait aussi, mais n'est pas encore stable.
- Le premier build télécharge les outils NSIS de Tauri (accès Internet).
- Pas de signature de code : voir [limites.md](limites.md).

## Publier une version

1. **Version** : changer `version` dans `Cargo.toml` (`[workspace.package]`) et dans `package.json`, en suivant le versionnage sémantique : correctif `2.0.1`, fonction nouvelle `2.1.0`, base ou fichiers de hash incompatibles `3.0.0`.
2. **Vérifications** : `scripts\verify.cmd` sans erreur.
3. **Contrôles manuels** de ce document : au minimum les cas réels (`real-cases.ps1`) et, si l'interface a changé, le clavier, le Narrateur et le contraste élevé.
4. **Construire** : `scripts\build.cmd`.
5. **Installer et désinstaller** (procédure ci-dessous), sur une session sans droits administrateur.
6. **Nouveautés** : une entrée dans [CHANGELOG.md](../CHANGELOG.md), en anglais comme le README (la face publique du dépôt), avec ce qui change pour l'utilisateur.
7. **Commit et étiquette** : `git tag v<version>`.
8. **Publication** sur https://github.com/Nikola-Putnik/FileSeal2 : `git push origin main v<version>`, puis une « release » à l'étiquette (page Releases du dépôt), avec l'installateur, le zip et leurs deux fichiers `.sha256`, et les nouveautés de `CHANGELOG.md` en texte. Sans `gh` : `node tools/github-release.mjs v<version> <notes.md> <les quatre fichiers>`, qui passe par l'API REST de GitHub avec l'identifiant que Git utilise déjà pour pousser (jamais affiché), et refuse si l'étiquette a déjà sa release. Les notes : celles du CHANGELOG, en anglais, avec les empreintes SHA-256 des deux fichiers.

## Captures d'écran du README

Dans `docs/images/`, faites sur une collection de démonstration, pour qu'aucun chemin ni nom de fichier personnel n'apparaisse :

1. `node tools/demo-collection.cjs <dossier vide>` : cinq films en fichiers épars (de 29 à 59 Go affichés, sans place prise sur le disque, lus à la vitesse du processeur), l'album « Photos\Crète 2026 », une bande originale.
2. `subst D: <dossier>` : les chemins affichés commencent par `D:\`.
3. Le programme publié, lancé à part (réglages et données WebView2 temporaires, port de débogage), en anglais et dans le thème de Windows. Base `D:\Collection.fileseal`, puis **Créer** sur `D:\Movies`, `D:\Photos`, `D:\Music`.
4. `node tools/demo-collection.cjs <dossier> --alter` (une photo change d'un octet, une nouvelle arrive), puis **Vérifier** les trois dossiers.
5. Captures à 1 920 × 1 040 (`cdp.mjs shot … 1920x1040`) : pendant la vérification (`verify-progress.png`), après avec la ligne de Blade Runner choisie (`verify-results.png`), et l'écran Bases trié par taille, avec l'historique de Blade Runner (`databases.png`).
6. Arrêter le programme, `subst D: /d`, supprimer le dossier.

## Installer, mettre à jour, désinstaller (contrôle de livraison)

Sur une session Windows 11 **sans droits administrateur**. Les commandes en mode silencieux (`/S`) évitent les clics ; faire au moins une fois le parcours avec les fenêtres de l'installateur.

1. **État de départ** : aucun dossier `%LOCALAPPDATA%\FileSeal`, aucune entrée FileSeal dans « Applications installées ».
2. **Installer** : `FileSeal_<version>_x64-setup.exe /S`. Attendu :
   - `%LOCALAPPDATA%\FileSeal\fileseal.exe` et `uninstall.exe` ;
   - l'entrée « FileSeal » avec la bonne version dans « Applications installées » ;
   - les raccourcis du menu Démarrer et du Bureau ;
   - les propriétés de `fileseal.exe` (onglet Détails) donnent la version.
3. **Lancer** : l'en-tête affiche le numéro de version. Choisir le français (⚙) et créer une base de test.
4. **Instance unique** : relancer FileSeal. Attendu : la fenêtre ouverte revient au premier plan, aucun second processus ne reste.
5. **Mettre à jour** : fermer, puis installer une version plus récente par-dessus. Attendu : la nouvelle version s'affiche, la langue et la base sont conservées, la base est rouverte.
6. **Désinstaller** : `%LOCALAPPDATA%\FileSeal\uninstall.exe /S`, ou par « Applications installées ». Attendu :
   - le dossier du programme, l'entrée et les raccourcis ont disparu ;
   - `%APPDATA%\org.fileseal.desktop\settings.json` est conservé, inchangé ;
   - la base de test est intacte.
7. **Version portable** : décompresser le zip dans un dossier de test et lancer `FileSeal.exe`. Attendu : « portable » dans l'en-tête ; `data\settings.json`, `data\logs` et `data\webview` créés à côté du programme ; rien dans `%APPDATA%\org.fileseal.desktop`.
8. **Changement de lettre** : `subst P: <dossier de test>`, lancer `P:\FileSeal\FileSeal.exe`, créer `P:\FileSeal\Test.fileseal`, fermer. Puis `subst P: /D`, `subst Q: <même dossier>`, et lancer depuis `Q:`. Attendu : la base est rouverte en `Q:\FileSeal\Test.fileseal`. `data\settings.json` l'enregistre sans lettre (`\FileSeal\Test.fileseal`).
9. **Fermeture propre** : après la fermeture de la fenêtre, aucun fichier `Test.fileseal-wal` ne reste à côté de la base.
10. **Dossier non inscriptible** : interdire la création de fichiers dans le dossier du programme (`icacls <dossier>\FileSeal /deny *S-1-1-0:(AD,WD)`), supprimer `data`, lancer. Attendu : FileSeal démarre sans « portable », et son journal (dans le profil) dit « portable, but not writable ». Retirer ensuite l'interdiction (`icacls … /remove:d *S-1-1-0`).

Résultats du 2026-10-22 (version 2.0.0, et une 2.0.1 construite pour l'occasion) : tous les points sont conformes, sauf le point 9, qui a révélé un défaut corrigé avant la livraison ([CHANGELOG.md](../CHANGELOG.md)). Le parcours avec les fenêtres de l'installateur et une vraie clé USB restent à faire.
