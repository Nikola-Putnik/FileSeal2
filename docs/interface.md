# L'interface

Code : `ui/src/App.svelte` et `ui/src/lib/`. Cadrage, §9.

Deux écrans, choisis par les onglets du haut : **Fichiers** (décrit ici) et **Bases** ([bases.md](bases.md)). L'écran Fichiers garde sa liste et ses résultats quand on passe à l'autre.

**La fenêtre ne défile pas comme une page** (version 2.3) :
- un en-tête ;
- la ligne de commande ;
- la liste, qui prend toute la hauteur restante, avec le détail à côté ;
- une barre fixe en bas.

Seules les zones longues défilent d'elles-mêmes : la liste, le détail, les réglages dépliés. Ce qui sert de temps en temps est derrière un bouton qui en résume l'état (réglages, extensions, base active), ou dans un menu (⋯, ⚙, ?). Les panneaux qui s'ouvrent se referment par leur bouton, par Échap ou par un clic ailleurs (`Popover.svelte`).

L'écran Fichiers, de haut en bas :

1. **En-tête** :
   - le nom et la version ;
   - les écrans Fichiers et Bases ;
   - la **base active**, en un bouton. Il ouvre la base actuelle, les récentes, créer, ouvrir et fermer. « lecture seule » et « partage réseau » restent affichés à côté ;
   - **?** : les raccourcis clavier ; **⚙** : la langue et le [thème](#thèmes).
2. **Action** (Créer une référence · Vérifier · Rechercher) et le bouton **Réglages**. Il résume les réglages en cours (« SHA-256 · fichiers de hash + base · référencés sautés ») et les déplie en un panneau teinté, au-dessus de la liste, limité à 40 % de la hauteur. **Les réglages et l'état du panneau sont gardés** d'un lancement à l'autre (`settings.json`, `run`) ; une valeur absente ou incohérente reprend sa valeur par défaut. Les réglages :
   - Créer : algorithmes (SHA-256 coché ; MD5 avec la mention « compatibilité ») et destinations (fichier de hash à côté de chaque fichier, base active). **Fichiers déjà référencés** : les sauter sans les relire s'ils ont tous les algorithmes cochés (par défaut), ou s'ils ont une référence quelle qu'elle soit, ou les relire ; et, pour ceux qui sont sautés, ajouter leurs fichiers de hash à la base active ([formats-hash.md](formats-hash.md)).
   - Vérifier : références à consulter (fichiers de hash, base active).
   - Créer et Vérifier : **Fichiers déplacés : les suivre automatiquement** (décoché par défaut ; grisé sans base active modifiable). Le bouton le résume par « déplacements suivis ». Règles : [rapprochement.md](rapprochement.md#suivi-automatique-des-emplacements).
   - Rechercher : la base active.
   - Sélection : sous-dossiers, fichiers cachés et système, nombre de disques lus en même temps (1 à 4).
3. **Barre de la liste** :
   - ajouter des fichiers, ajouter un dossier ;
   - **Extensions : toutes ▾** (ou « seulement .mkv, .iso », « 2 écartées »). Le bouton ouvre « Seulement ces extensions », une liste tapée (`mkv iso mp4`, casse ignorée) qui écarte toute autre extension, y compris dans les dossiers ajoutés ensuite. Il ouvre aussi une puce par extension trouvée dans les dossiers, avec son nombre de fichiers et sa taille ; une puce décochée écarte l'extension. L'état est dit par la case ☑/☐, par le texte barré et par `aria-pressed`, pas par la couleur seule ;
   - filtre par nom, **Retirer** ;
   - **Vider la liste** ;
   - **⋯** : Monter, Descendre, Trier par.
4. **Onglets** Fichiers, Écartés, Problèmes, suivis du compte (fichiers, taille, dossier commun, sélection), puis la **liste des fichiers**. Elle devient le tableau des résultats après un lot (voir plus bas).
5. **Détail** du fichier choisi dans la liste, dans un volet à droite. Il passe sous la liste quand la fenêtre fait moins de 1 400 px de large (pixels CSS) : la liste garde alors toute sa largeur, plus utile aux noms longs que la hauteur.
   - Contenu : le chemin, « Afficher dans le dossier » (ouvre l'Explorateur, fichier sélectionné), le résultat complet et les décisions possibles pour ce fichier.
   - Entrée sur une ligne amène le focus dans le volet, Échap le ramène à la liste ; ✕ le ferme.
   - **Sous la liste** (fenêtre étroite), il prend de la place à la liste : un clic ou les flèches ne font que choisir une ligne, et le détail s'ouvre seulement sur demande (double-clic, Entrée, ou le bouton **Détail** au bout de la ligne des onglets). Ouvert, il suit la ligne choisie jusqu'à ce qu'on le ferme.
6. **Barre du bas**, toujours visible :
   - **avant un lot** : la phrase de confirmation, puis le bouton de lancement. Par exemple : « Vérifier 1 234 fichiers (456 Go) avec leurs fichiers de hash et la base « Films ». » Tant qu'il manque quelque chose, la phrase dit quoi (« Ajoutez d'abord des fichiers à la liste », « Cochez au moins un algorithme »…) ;
   - **pendant un lot** : la progression d'ensemble (pondérée par les octets), les boutons **Pause / Reprendre** et **Annuler**. Une ligne donne les fichiers terminés, le volume lu, le débit, le temps écoulé et restant ; une autre, le fichier lu sur chaque disque avec son pourcentage ;
   - **après un lot** : le bilan suivi de « Exporter le rapport… », puis les **décisions pour tous les résultats**. Un trait les sépare de la phrase et du bouton de lancement, en dessous, pour qu'on ne confonde pas les deux boutons. Les causes d'un arrêt (base perdue, disques disparus, arrêt demandé) s'y affichent aussi.

## La liste

- **Ajouter** :
  - « Ajouter des fichiers… » et « Ajouter un dossier… » : Windows ne permet pas les deux dans un même dialogue ;
  - glisser-déposer des fichiers et des dossiers dans la fenêtre.

  Les dossiers sont parcourus par le moteur selon les règles de la [sélection](selection.md). Les lignes arrivent par paquets, et on peut arrêter le parcours.
- **Un fichier de hash ajouté en mode Vérifier** (`.sha256`, `.md5`, `SHA256SUMS`…) est remplacé par les fichiers qu'il liste. Chacun sera vérifié avec sa ligne, et seulement elle, comme le fait `sha256sum -c`. Les lignes inutilisables vont dans l'onglet Problèmes. Dans un autre mode, le fichier de hash est ajouté comme un fichier ordinaire, avec un message qui l'explique.
- **Retirer** la sélection ; **Vider la liste**, avec confirmation dans la page seulement si des résultats seraient perdus ; dans le menu **⋯** : **Monter / Descendre** la sélection, **Trier** par nom, dossier, taille ou extension (un clic sur un en-tête de colonne trie aussi).
- **L'ordre de la liste est l'ordre de démarrage** du lot ([lots.md](lots.md)).
- **Filtrer par nom** : une partie du nom ou du dossier.
- **Onglets** :
  - Fichiers ;
  - Écartés : chaque fichier ou dossier écarté, avec sa raison ;
  - Problèmes : dossiers illisibles, chemins introuvables, lignes de fichiers de hash inutilisables.
- **Colonnes** : nom, dossier, taille, état. La page prend toute la largeur de la fenêtre, et le nom a la colonne la plus large.
  - **Un nom trop long est coupé au milieu**, pas à la fin : le début (titre, épisode) et la fin (extension) restent visibles, et deux noms qui ne diffèrent qu'à la fin ne se confondent plus. Le nom entier est dans l'info-bulle et dans le détail ; un lecteur d'écran le lit en entier.
  - Une ligne sur deux est légèrement teintée.
  - Le dossier commun à toute la liste est indiqué une seule fois au-dessus, et la colonne n'affiche que la partie qui diffère.
  - L'état vaut « À traiter » avant le lot (avec la ligne du fichier de hash le cas échéant), puis « Lecture… 45 % » ou « En attente » pendant le lot, puis le statut.
- **Changer « sous-dossiers » ou « fichiers cachés »** parcourt de nouveau les dossiers ajoutés. Les retraits et déplacements faits à la main sont alors annulés, et un message le dit.
- Un même fichier n'est ajouté qu'une fois : les suivants sont « Déjà dans la liste », dans l'onglet Écartés.

### Clavier

| Touche | Effet |
|---|---|
| Tab | Entre dans la liste et en sort : la liste est un seul arrêt de tabulation. |
| ↑ ↓, Page↑ Page↓, Ctrl+Début / Fin | Change de ligne ; la sélection suit, comme dans l'Explorateur. |
| Maj + ces touches | Étend la sélection. |
| Ctrl + ces touches | Change de ligne sans toucher à la sélection. |
| Espace | Ajoute la ligne à la sélection, ou l'en retire. |
| Ctrl+A | Sélectionne toute la liste (filtrée). |
| ← →, Début, Fin | Change de colonne (pour la lecture par un lecteur d'écran). |
| Suppr | Retire la sélection de la liste ; la ligne suivante devient active. |
| Alt+↑ / Alt+↓ | Monte ou descend la sélection. |
| Entrée (ou double-clic) | Affiche le détail du fichier et y place le focus ; Échap ramène à la liste. |

Raccourcis de la fenêtre :
- **Ctrl+O** : ajouter des fichiers ;
- **Ctrl+Maj+O** : ajouter un dossier ;
- **Ctrl+Entrée** : lancer.

Dans les onglets, ← et → passent d'un onglet à l'autre. Le bouton **?** de l'en-tête rappelle tous ces raccourcis ; pour les lecteurs d'écran, l'aide clavier de la liste est aussi sa description (`aria-describedby`).

### Pour les lecteurs d'écran

- La liste est une grille ARIA (`role="grid"`, `aria-multiselectable`) qui annonce son nombre total de lignes (`aria-rowcount`) et la position de chacune (`aria-rowindex`), même si seules les lignes visibles existent dans la page.
- Le focus reste sur la grille ; la cellule active est désignée par `aria-activedescendant`. Ainsi le focus ne se perd pas quand des lignes sont recyclées au défilement ou ajoutées pendant le parcours.
- Une ligne est annoncée en entier (« Photos 0.jpg Photos 19,5 Ko Conforme à la référence ») et dit si elle est sélectionnée (`aria-selected`).
- Les messages (parcours, bilan, décisions) sont des régions `role="status"` ; une base perdue est une alerte (`role="alert"`).
- La question posée à la fermeture pendant une opération est une `alertdialog`, avec le focus sur « Continuer ».

## Résultats et décisions

- **Puces par statut** au-dessus de la liste, les problèmes en premier (« ✖ Différent de la référence (3) », « – Non traité (12) », « ✔ Conforme (1 200) »…). Chacune filtre la liste. Elles comptent ce que montrent les lignes **encore dans la liste**, en direct : pendant le lot, quand des lignes sont retirées, quand une décision est prise (« ✔ Nouvelle version acceptée (2) »). Sans résultat dans la liste, elles disparaissent ; un filtre que plus aucune ligne ne montre revient à « Tous ».
- **Décisions pour tous les résultats**, chacune avec son nombre et une confirmation dans la page :
  - « Mettre à jour N emplacements » : fichiers reconnus sur le même disque sous une autre lettre (R2), avec le contenu enregistré ;
  - « Rattacher N fichiers déplacés » : contenu d'un seul fichier connu, disparu de son emplacement (R5). Un cas ambigu n'est jamais rattaché par lot ;
  - « Accepter N nouvelles versions » : fichiers vérifiés différents de leur référence.

  Les décisions sont prises une par une, avec un compteur. Chaque ligne affiche ensuite l'issue (« Rattaché », « Nouvelle version acceptée », « Échec de la décision » et sa cause dans le détail).
- **Décisions pour un fichier**, dans le détail : les mêmes, plus « rattacher malgré un contenu différent » (renommé et modifié, R7) et « c'est une autre copie : créer une nouvelle référence ».
- **Rapport** : « Exporter le rapport… » écrit le CSV du dernier lot, dans l'ordre de la liste ([rapport-csv.md](rapport-csv.md)).

## Statuts lisibles sans couleur

Chaque statut associe une icône de forme différente (✔, ✖, ⚠, ℹ, –, =, ⏹), un texte et une couleur. Vérifié sur une capture en niveaux de gris : les statuts restent distincts.
- La ligne active est marquée par une barre à gauche ; la cellule active, par un contour quand la liste a le focus.
- L'onglet choisi est en gras et souligné.
- Une puce de filtre active est soulignée et encadrée.
- En mode « couleurs forcées » (contraste élevé de Windows), la sélection prend les couleurs système `Highlight` et `HighlightText`.

## Thèmes

Choisis dans **⚙** (préférences), enregistrés dans `settings.json` (`theme`) :

| Thème | Pour qui | Fond / texte |
|---|---|---|
| **Comme Windows** (par défaut, `theme: null`) | Suit le mode clair ou sombre de Windows, y compris quand il change. | Clair ou Sombre |
| **Clair** | | `#f6f7f9` / `#1c2430` |
| **Sombre** | Sombre mais doux : gris bleutés, texte légèrement en dessous du blanc. | `#171b21` / `#dfe5ec` |
| **Sombre OLED** | Noir pur pour les écrans OLED : gris neutres, zones séparées par leurs bordures plutôt que par un fond plus clair, texte `#e2e2e2` (un blanc pur bave sur du noir). | `#000000` / `#e2e2e2` |

- **Jetons** (`ui/src/app.css`) : chaque couleur donne sa valeur claire et sa valeur sombre (`light-dark()`). Le schéma de couleurs (`color-scheme`) choisit l'une ou l'autre : celui de Windows par défaut, celui du thème choisi sinon (`data-theme` sur la racine). Le thème OLED redéfinit les jetons. Cases à cocher, listes déroulantes et champs de saisie suivent le thème.
- **Contrastes** (WCAG), vérifiés pour les trois palettes : texte et texte secondaire sur chaque fond (fond, surface, panneau, ligne sélectionnée) **≥ 4,5:1** ; couleurs des statuts employées comme texte ≥ 4,5:1 ; texte du bouton principal ≥ 4,5:1 ; icônes des statuts et contour du focus ≥ 3:1. Le plus faible : texte secondaire sur ligne sélectionnée, 4,6:1 (Sombre).
- **Barre de titre** : la fenêtre prend le thème clair ou sombre correspondant (`set_theme`), dès le démarrage.
- **Au démarrage**, le thème de la session précédente est appliqué avant la lecture des réglages (copie dans le stockage de la page), pour ne pas changer de couleurs sous les yeux.
- **Contraste élevé de Windows** : il l'emporte sur tous les thèmes.

## Mémoire et fluidité

La liste vit dans la page : environ 80 Mo pour 50 000 fichiers avec leurs résultats complets. Seules les lignes visibles, et quelques-unes autour, sont dessinées (une trentaine). Mesures dans [performances.md](performances.md).

Le cadrage prévoyait de demander les lignes au moteur page par page. Les garder dans la page est bien plus simple, et suffit largement pour 50 000 fichiers. Au-delà de quelques centaines de milliers, la mémoire deviendrait le facteur limitant ; ce serait alors à revoir.

## Contrôles manuels

Lecteurs d'écran (Narrateur, NVDA) et contraste élevé réel : procédure dans [tests-build-livraison.md](tests-build-livraison.md).
