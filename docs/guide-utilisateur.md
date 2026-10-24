# Guide utilisateur

FileSeal 2. Ce guide suit l'interface en français : bouton **⚙** en haut à droite, **Language** → **Français**. Le choix est gardé pour les fois suivantes.

FileSeal enregistre l'**empreinte** (hash) du contenu de vos fichiers, puis vérifie plus tard que ce contenu n'a pas changé. Il sert à surveiller une collection (films, photos), des sauvegardes, des disques externes ou un partage réseau.

Ce qu'il faut savoir d'emblée :
- **FileSeal ne modifie jamais vos fichiers.** Il les lit, sans jamais les ouvrir en écriture.
- **« Différent » veut dire que le contenu a changé**, pas forcément qu'il est abîmé : un fichier retouché exprès est aussi « différent ». C'est à vous de dire si le changement est voulu.
- **Une vérification ne remplace jamais une référence.** Seule l'action « Accepter comme nouvelle version » le fait, et l'ancienne reste dans l'historique.
- FileSeal ne répare rien et ne sauvegarde pas vos fichiers. Il vous dit lesquels ont changé, pour que vous les repreniez dans une sauvegarde.

## Installer

Windows 11, 64 bits. Sous Windows 10, le runtime « Microsoft Edge WebView2 » est nécessaire. Il est en général déjà présent ; sinon, l'installateur le télécharge.

### Avec l'installateur

1. Lancez `FileSeal_2.3.0_x64-setup.exe`. Aucun droit administrateur n'est demandé : FileSeal s'installe pour vous seul, dans `%LOCALAPPDATA%\FileSeal`.
2. Le programme n'étant pas signé, Windows peut afficher « Windows a protégé votre ordinateur » (SmartScreen). Choisissez **Informations complémentaires**, puis **Exécuter quand même**.
3. FileSeal apparaît dans le menu Démarrer. La dernière page de l'installateur propose aussi un raccourci sur le Bureau.

L'installateur s'affiche en français ou en anglais, selon la langue de Windows.

### Version portable (clé USB)

1. Décompressez `FileSeal_2.3.0_x64-portable.zip` où vous voulez, par exemple sur une clé USB. Vous obtenez un dossier `FileSeal` avec `FileSeal.exe`, `portable.flag` et `README.txt`.
2. Lancez `FileSeal.exe`. « portable » s'affiche à côté du numéro de version.

En version portable :
- **rien n'est écrit dans votre profil Windows** : réglages, journaux et données de l'interface vont dans un dossier `data`, à côté de `FileSeal.exe` ;
- **une nouvelle base est proposée à côté de `FileSeal.exe`**, donc sur la clé ;
- **une base sur la clé est retrouvée même si la clé change de lettre** d'un PC à l'autre (E: ici, F: ailleurs).

Supprimer `portable.flag` fait fonctionner ce même programme comme la version installée, avec les réglages dans votre profil.

### Vérifier le fichier téléchargé

Chaque fichier livré est accompagné de son fichier de hash (`FileSeal_2.3.0_x64-setup.exe.sha256`), écrit par FileSeal lui-même. Pour contrôler le téléchargement, mettez les deux fichiers dans le même dossier, puis au choix :
- avec un FileSeal déjà installé : action **Vérifier**, puis ajoutez le fichier `.sha256` à la liste ;
- avec Git for Windows : `sha256sum -c FileSeal_2.3.0_x64-setup.exe.sha256` ;
- sans outil : `certutil -hashfile FileSeal_2.3.0_x64-setup.exe SHA256`, à comparer au contenu du fichier `.sha256`.

## Mettre à jour

Lancez l'installateur de la nouvelle version : il remplace l'ancienne. Vos bases et vos réglages sont conservés.

Si la nouvelle version a fait évoluer le format des bases, chaque base est mise à jour à sa première ouverture. **Une copie de la base dans son état précédent est faite avant**, à côté d'elle, et un message vous indique son nom. Une base mise à jour ne s'ouvre plus avec l'ancienne version de FileSeal, d'où cette copie.

En version portable, remplacez `FileSeal.exe` par le nouveau et gardez le dossier `data`.

## Désinstaller

Paramètres de Windows > Applications > Applications installées > FileSeal > **Désinstaller**.

- **Vos bases ne sont jamais supprimées** : ce sont vos fichiers, rangés là où vous les avez créés.
- **Vos fichiers de hash restent** à côté de vos fichiers.
- **Les réglages sont conservés**, sauf si vous cochez « Supprimer les données de l'application » pendant la désinstallation. Ils servent à une réinstallation.

Pour la version portable, supprimez simplement son dossier.

## Deux façons de garder les références

| | Fichier de hash à côté du fichier | Base FileSeal |
|---|---|---|
| Où | `film.mkv.sha256`, dans le dossier du fichier | un fichier `.fileseal` de votre choix |
| Voyage avec le fichier | oui, s'il est copié avec | non |
| Lisible par d'autres outils | oui (`sha256sum -c`, `md5sum -c`) | non |
| Historique des vérifications et des versions | non | oui |
| Retrouver un fichier déplacé, renommé ou copié | non | oui |
| Disques qui changent de lettre | sans objet | reconnus |

Les deux se cumulent, et c'est le choix par défaut quand une base est ouverte. Sans base, FileSeal fonctionne avec les seuls fichiers de hash.

**Une base** se crée avec **Créer une base…** dans la barre du haut. Le nom proposé est celui de la collection (« Films », « Photos 2024 »). Plusieurs bases peuvent coexister : une par collection ou par disque, comme vous préférez. **Ouvrir une base…** et la liste **Récentes** permettent de passer de l'une à l'autre. La dernière base utilisée est rouverte au démarrage.

## La fenêtre

- **En haut** :
  - les écrans **Fichiers** et **Bases** ;
  - la **base active** : un clic ouvre la base actuelle, les récentes, créer, ouvrir, fermer ;
  - **?** : les raccourcis clavier ;
  - **⚙** : la langue.
- **L'action** (Créer, Vérifier, Rechercher) et le bouton **Réglages**, qui résume les réglages en cours et les déplie au besoin. Vos réglages sont gardés d'un lancement à l'autre.
- **La liste des fichiers** prend toute la hauteur. Le **Détail** du fichier choisi s'ouvre à côté. Si la fenêtre est étroite, il s'ouvre en dessous, et seulement sur demande : double-clic, Entrée ou bouton **Détail**. ✕ le ferme, Échap ramène à la liste.
- **En bas, toujours visible** :
  - avant un lot : la phrase qui dit ce qui va être fait, et le bouton de lancement ;
  - pendant : la progression ;
  - après : le bilan, les décisions pour tous les résultats et l'export du rapport.

## Premiers pas

### 1. Créer les références

1. **Action** : **Créer une référence**.
2. **Réglages** (le bouton les déplie) :
   - algorithmes : **SHA-256** (recommandé) ; MD5 seulement pour la compatibilité avec d'anciens outils ;
   - destinations : fichier de hash à côté de chaque fichier, base active, ou les deux.
3. Ajoutez des fichiers ou des dossiers : **Ajouter des fichiers…**, **Ajouter un dossier…**, ou glissez-les dans la fenêtre.
4. Relisez la phrase au-dessus du bouton, par exemple « Créer les références SHA-256 de 1 234 fichiers (456 Go) dans des fichiers de hash à côté d'eux et la base « Films ». », puis cliquez sur **Créer les références** (ou Ctrl+Entrée).

**Fichiers déjà référencés** : par défaut, un fichier qui a déjà une référence pour chaque algorithme coché, dans chaque destination cochée, **n'est pas relu**. Il ressort « Déjà référencé (non relu) ». Relancer Créer sur une collection ne lit donc que les nouveaux fichiers. Deux autres choix :
- **les sauter s'ils ont une référence, quelle qu'elle soit** : un ancien `.md5` suffit, même avec SHA-256 coché. Rien n'est ajouté à ces fichiers ;
- **les relire** : leurs références sont contrôlées contre leur contenu, puis complétées (un algorithme ou une destination en plus). Un fichier dont le contenu a changé ressort « Conflit avec une référence existante », et rien n'est écrasé.

**Ajouter leurs fichiers de hash à la base active, sans relire** (cochée par défaut) : un fichier sauté que la base ne connaît pas y est enregistré à partir de ses fichiers de hash existants. Il ressort « Ajouté à la base (non relu) ». C'est le moyen de faire entrer dans une base une collection déjà équipée de fichiers `.md5` ou `.sha256`, y compris ceux de l'ancien FileSeal, sans tout relire. La base reçoit la valeur du fichier de hash : si le fichier a changé depuis, la prochaine vérification le dira.

Un fichier sauté n'est pas contrôlé : vérifier, c'est le rôle de **Vérifier**. Un fichier est toujours relu si l'une de ses références pose un problème : fichier de hash illisible, désaccord entre le fichier de hash et la base, fichier qui semble déplacé.

### 2. Vérifier, plus tard

1. **Action** : **Vérifier**.
2. **Références à consulter** : les fichiers de hash, la base active, ou les deux.
3. Ajoutez les mêmes dossiers, puis **Vérifier**.

Pour vérifier les fichiers listés dans un fichier de hash (`.sha256`, `.md5`, `SHA256SUMS`, ou ceux de l'ancien FileSeal au format `MD5: …`), ajoutez ce fichier de hash à la liste en mode Vérifier : il est remplacé par les fichiers qu'il nomme.

### 3. Rechercher un contenu

**Action** : **Rechercher**, avec une base ouverte. Pour chaque fichier, FileSeal dit si la base connaît déjà ce contenu, et où : connu ici, probablement déplacé ici, copie d'un fichier connu… C'est utile pour un fichier retrouvé dont on ne sait plus d'où il vient, ou après avoir réorganisé des dossiers.

## La liste des fichiers

- **Les dossiers sont parcourus avec leurs sous-dossiers.** Ne sont pas retenus, avec leur raison dans l'onglet **Écartés** :
  - les fichiers cachés et système, sauf si vous les incluez ;
  - les fichiers de hash et les bases FileSeal ;
  - les fichiers OneDrive « en ligne uniquement » (les lire les téléchargerait) ;
  - les liens et jonctions, qui ne sont pas suivis.

  Un fichier choisi directement est toujours retenu.
- **Extensions** (bouton au-dessus de la liste, qui indique le filtre en cours) :
  - **« Seulement ces extensions »** : tapez par exemple `mkv iso mp4`, puis **Appliquer**. Majuscules et minuscules sont confondues (`.MKV`, `.Mkv`, `.mkv`). Toutes les autres extensions sont écartées, y compris dans les dossiers ajoutés ensuite ;
  - une **puce** par extension trouvée, avec son nombre de fichiers et sa taille. Cliquez-la pour l'écarter ou la reprendre ;
  - **Toutes** revient à toutes les extensions.
- **Noms longs** : un nom qui ne tient pas dans sa colonne est coupé au milieu, et sa fin (épisode, extension) reste visible. Le nom entier s'affiche au survol et dans le **Détail**. Agrandir la fenêtre donne plus de place à la colonne Nom.
- **Un même fichier n'est traité qu'une fois**, même ajouté deux fois, par un lien ou par un lecteur réseau et son chemin `\\serveur\partage`.
- **L'ordre de la liste est l'ordre de traitement.** Monter, Descendre et Trier, dans le menu **⋯**, le changent. Un clic sur un en-tête de colonne trie aussi.
- **Clavier** : la liste se pilote comme l'Explorateur. Le détail des touches est dans [interface.md](interface.md).

## Pendant un lot

- La progression donne le fichier en cours sur chaque disque, le volume lu, le débit et le temps restant.
- **Plusieurs disques sont lus en même temps** (2 par défaut, réglable de 1 à 4), un fichier à la fois par disque.
- **Pause** suspend la lecture, **Reprendre** la poursuit là où elle était. **Annuler** arrête : les fichiers terminés gardent leur résultat, les autres sont « Non traité ».
- **Le PC ne se met pas en veille pendant un lot.** Fermer la fenêtre demande confirmation.
- **Un disque débranché ou un partage perdu** : le fichier en cours de lecture est « Erreur de lecture », les suivants de ce disque « Non traité (disque indisponible) ». Les autres disques continuent.
- **Une base devenue inaccessible** (disque plein, disque débranché) arrête le lot proprement. Ce qui a été annoncé comme enregistré l'est vraiment.

## Lire les résultats

Des **puces** au-dessus de la liste comptent les résultats par statut, les problèmes en premier. Cliquer sur une puce filtre la liste. Le **Détail** du fichier choisi donne les empreintes attendues et observées, les références consultées et les décisions possibles. Chaque statut a une icône, un texte et une couleur : la couleur n'est jamais la seule indication.

### Vérification

| Statut | Ce que cela veut dire | Que faire |
|---|---|---|
| ✔ Conforme à la référence | Le contenu est celui enregistré. | Rien. |
| ✖ Différent de la référence | Le contenu a changé depuis l'enregistrement. « Taille modifiée » le précise s'il y a lieu. | Changement voulu : **Accepter comme nouvelle version**. Sinon : reprendre le fichier dans une sauvegarde. |
| ⚠ Références contradictoires | Le fichier de hash et la base ne sont pas d'accord. Le détail dit lequel correspond au contenu actuel. | Si le contenu actuel est le bon : **Accepter comme nouvelle version**, ce qui met la base et le fichier de hash d'accord. |
| ℹ Aucune référence | Ni fichier de hash, ni entrée dans la base. | Créer une référence. Avec une base, le détail indique si ce contenu y est connu ailleurs. |
| ℹ Aucune référence compatible | La référence utilise un algorithme que cette version ne sait pas calculer. | Créer une référence SHA-256 en plus. |
| ⚠ Fichier de hash invalide | Le fichier de hash est illisible ou mal formé. FileSeal ne le remplace pas de lui-même. | L'ouvrir pour voir. Le supprimer, puis créer la référence de nouveau. |
| ✖ Fichier attendu introuvable | Rien à l'emplacement attendu : fichier supprimé, déplacé ou renommé. | Le chercher. Avec une base, **Rechercher** à son nouvel emplacement, puis **Rattacher**. |
| ✖ Erreur de lecture | Le fichier n'a pas pu être lu en entier : droits, disque débranché, réseau, secteur défectueux. | Voir la cause dans le détail, puis vérifier de nouveau. |
| ⚠ Fichier verrouillé ou en cours d'utilisation | Un autre programme écrit dans le fichier. | Fermer ce programme, puis vérifier de nouveau. |
| ⚠ Modifié pendant le calcul | Le fichier a changé pendant sa lecture ; le résultat est écarté. | Vérifier de nouveau. |
| ⏹ Annulé, – Non traité | Le lot a été arrêté, ou un disque ou la base a disparu. | Relancer. |
| ✖ Échec | La vérification n'a pas pu être menée. | Voir le détail. |

Un fichier vérifié avec une base est ajouté à son **historique**. Ce n'est pas le cas si la base est en lecture seule : le détail le signale.

### Création

| Statut | Ce que cela veut dire |
|---|---|
| Référence créée | Le contenu actuel est enregistré. |
| Référence complétée | La référence existante a été vérifiée identique, puis complétée (par exemple, MD5 ajouté à côté de SHA-256). |
| Déjà référencé | Une référence identique existait, contrôlée en relisant le fichier : rien n'a été réécrit. |
| Déjà référencé (non relu) | Il avait déjà ses références : il n'a pas été lu, et elles n'ont donc pas été contrôlées. |
| Ajouté à la base (non relu) | Ses fichiers de hash existants ont été enregistrés dans la base, sans relire le fichier. |
| Conflit avec une référence existante | Le contenu ne correspond plus à la référence existante. Rien n'est écrasé : vérifiez, puis acceptez la nouvelle version si le changement est voulu. |
| Décision requise | La base connaît déjà ce contenu, à un emplacement où il n'est plus : est-ce le même fichier déplacé (**Rattacher**) ou une autre copie (**créer une nouvelle référence**) ? |
| Réussite partielle | Une destination a échoué, l'autre a réussi. Le détail dit laquelle et pourquoi (dossier protégé en écriture, disque plein…). |
| Échec, Annulé | Rien n'a été enregistré. |

### Recherche

| Statut | Ce que cela veut dire |
|---|---|
| Connu ici | Enregistré à cet emplacement, avec ce contenu. |
| Modifié depuis l'enregistrement | Enregistré ici, mais le contenu a changé. |
| Reconnu (lettre de lecteur changée) | Le même fichier, sur le même disque, qui a changé de lettre. |
| Probablement déplacé ici | Un fichier connu avec ce contenu a disparu de son emplacement : c'est sans doute lui. |
| Copie d'un fichier connu | Ce contenu existe aussi ailleurs, toujours présent. |
| Plusieurs origines possibles | Plusieurs fichiers connus ont ce contenu et ont disparu : à vous de choisir. |
| Contenu inconnu | Aucune référence de la base n'a ce contenu. Les fichiers de même nom sont montrés comme indices, jamais associés d'office. |

## Décider

Les décisions se prennent pour un fichier, dans son **Détail**, ou **pour tous les résultats** concernés à la fois. Chacune demande une confirmation dans la page, et l'historique de la base en garde la trace.

- **Accepter comme nouvelle version** : le contenu observé pendant la vérification devient la référence. Le fichier n'est pas relu. Dans la base, la référence précédente reste dans l'historique ; les fichiers de hash existants à côté du fichier sont remplacés.
- **Rattacher** : ce fichier est celui de la base qui a disparu de son emplacement (déplacé, renommé). Il reprend son historique. Un cas « plusieurs origines possibles » n'est jamais rattaché par lot.
- **Rattacher malgré un contenu différent** (un seul fichier à la fois) : pour un fichier renommé **et** modifié, retrouvé grâce à un indice de nom.
- **C'est une autre copie** : une nouvelle référence, indépendante.
- **Mettre à jour les emplacements** : fichiers retrouvés sur le même disque sous une autre lettre.

## Rapport

**Exporter le rapport…** écrit les résultats du dernier lot dans un fichier CSV, dans l'ordre de la liste. Excel l'ouvre directement. Colonnes : [rapport-csv.md](rapport-csv.md).

## L'écran Bases

Onglet **Bases**, en haut de la fenêtre. La liste et les résultats de l'écran Fichiers sont gardés pendant ce temps. Détail : [bases.md](bases.md).

- **La base active** : renommer, afficher dans le dossier, déplacer ou renommer le fichier, **sauvegarder** (copie contrôlée), **restaurer une sauvegarde** (le contenu actuel est d'abord copié à côté).
- **Disques** : où sont aujourd'hui les fichiers de la base, disque par disque.
  - **Un disque revenu sous une autre lettre** se règle en trois clics : onglet **Bases**, **Mettre à jour vers F:\…**, **Confirmer**.
  - **Réaffecter…** : pour un dossier déplacé, copié ou renommé ailleurs. Un aperçu indique combien de fichiers sont retrouvés avant d'appliquer.
- **Références** : recherche par nom, dossier ou début de hash, avec la date et le résultat de la dernière vérification. L'**historique** de la ligne choisie montre ses versions, ses emplacements successifs et ses vérifications.
- **Retirer des références de la base** : pour un fichier supprimé exprès, un dossier ajouté par erreur, un disque dont vous vous êtes séparé.
  - Choisissez des lignes (Maj, Ctrl, Ctrl+A), puis **Retirer la sélection de la base**. Ou cherchez un dossier, puis **Retirer les N résultats de la base**.
  - Une référence retirée ne sert plus ni à vérifier ni à rechercher. Vos fichiers et leurs fichiers de hash ne sont pas touchés.
  - Rien n'est effacé : **Afficher : Retirées** liste les références retirées, avec leur historique, et **Remettre** les replace là où elles étaient.

Conseils :
- **Sauvegardez vos bases** comme vos autres données : **Sauvegarder…** produit une copie cohérente, même pendant une utilisation.
- **Gardez de préférence une base sur un disque local.** Sur un partage réseau, elle fonctionne, mais le verrouillage de SQLite y est moins sûr, et un avertissement le rappelle.
- **Une base ne s'ouvre que dans un seul FileSeal à la fois.** Un deuxième obtient « base déjà utilisée ». Fermez-la dans FileSeal avant de la copier avec un autre outil.

## Où FileSeal range ses données

| | Version installée | Version portable |
|---|---|---|
| Programme | `%LOCALAPPDATA%\FileSeal` | le dossier décompressé |
| Réglages (langue, bases récentes, choix des lots) | `%APPDATA%\org.fileseal.desktop\settings.json` | `data\settings.json` |
| Journaux (14 jours) | `%LOCALAPPDATA%\org.fileseal.desktop\logs` | `data\logs` |
| Données de l'interface (WebView2) | `%LOCALAPPDATA%\org.fileseal.desktop\EBWebView` | `data\webview` |
| Bases | là où vous les créez (proposé : Documents) | là où vous les créez (proposé : à côté de `FileSeal.exe`) |

Les journaux contiennent les opérations, les chemins et les erreurs, jamais le contenu des fichiers. Joignez-les à un signalement de problème.

## Questions fréquentes

**« Différent » : mon fichier est-il abîmé ?** Pas forcément. Le contenu a changé depuis l'enregistrement : une modification voulue (métadonnées d'une photo, sous-titres intégrés à un film) donne le même résultat qu'une altération. Si vous n'avez rien modifié, comparez avec une sauvegarde.

**Pourquoi FileSeal n'a-t-il pas remplacé mon fichier de hash ?** Il ne remplace jamais une référence sans votre accord explicite. Un fichier de hash existant et différent donne un conflit ; « Accepter comme nouvelle version » le remplace.

**MD5 ou SHA-256 ?** SHA-256. MD5 détecte aussi les erreurs accidentelles, mais il ne protège pas contre une falsification volontaire. Il ne sert qu'à rester compatible avec d'anciens fichiers `.md5`.

**Mes anciens fichiers `.md5` de FileSeal 1 ?** Ils sont lus tels quels, au format `MD5: …`, en UTF-8 ou dans l'ancien encodage de Windows.

**Un fichier OneDrive n'est pas vérifié.** S'il est « en ligne uniquement », il est écarté lors du parcours d'un dossier, pour ne pas le télécharger. Choisissez-le directement pour le vérifier quand même : il sera alors téléchargé.

**La vérification est lente.** Le disque est la limite : environ 100 à 250 Mo/s pour un disque dur ou un NAS, plusieurs Go/s pour un SSD. Lisez plusieurs disques en même temps plutôt que l'un après l'autre. Mesures : [performances.md](performances.md).

**Deux FileSeal ouverts ?** Relancer FileSeal ramène la fenêtre déjà ouverte au premier plan.

## Limites connues

Les principales ; la liste complète est dans [limites.md](limites.md).
- Windows seulement pour l'instant.
- Pas de reprise d'un lot après fermeture ou plantage : relancé, il reconnaît ce qui est déjà fait (« Déjà référencé »).
- Les flux de données alternatifs (ADS) de NTFS ne sont pas vérifiés, seulement le contenu principal.
- Une coupure de courant pendant un lot peut faire perdre les derniers résultats enregistrés en base, sans jamais l'abîmer. Les fichiers de hash, eux, sont toujours écrits complètement.
- Un seul fichier lu à la fois par disque, même sur un SSD très rapide.
- La version installée et une version portable ne peuvent pas être ouvertes en même temps : la seconde ramène la fenêtre de la première.
- Le programme n'est pas signé : Windows peut avertir au premier lancement.
