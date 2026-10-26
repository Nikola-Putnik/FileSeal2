# Lots : une action sur une liste de fichiers

Code : `crates/fileseal-core/src/batch.rs`. Règles du [cadrage](cadrage.md) §2, n° 14, 16, 17 et 22, et §8.

Un lot applique une action (Créer, Vérifier ou Rechercher) à une liste de fichiers, en général celle que donne la [sélection](selection.md). Chaque fichier passe par les mêmes opérations que lorsqu'il est traité seul : les règles de création, de vérification et de [rapprochement](rapprochement.md) sont identiques.

## Ordre et parallélisme

- **Un lecteur par volume** (disque, partage réseau, point de montage). Deux lectures simultanées sur un même disque dur feraient aller et venir sa tête de lecture : ce serait plus lent qu'une lecture après l'autre.
- **Plusieurs volumes en même temps** : 2 par défaut, réglable de 1 à 4. Un disque externe et un NAS sont lus ensemble.
- **Ordre de démarrage** : celui de la liste, autant que la règle précédente le permet. Un lecteur libre prend le premier fichier de la liste dont le volume n'est pas déjà en cours de lecture.
- Chaque résultat porte sa **position dans la liste** : l'interface l'affiche à sa place, quel que soit l'ordre d'arrivée.

Le volume d'un fichier est déterminé une fois par dossier, et non pour chaque fichier.

**Limite** : deux partitions d'un même disque comptent pour deux volumes, et seront lues en même temps.

## La base pendant un lot

Les lecteurs partagent la base active. Chaque recherche ou écriture la verrouille un court instant (quelques millisecondes), jamais pendant la lecture d'un fichier : un lecteur sur un disque n'attend jamais celui d'un autre disque.

Un lot de création ou de vérification est enregistré comme **une seule opération** dans la base, avec son récapitulatif. Une recherche n'écrit rien.

## Chaque fichier finit avec exactement un résultat

Quoi qu'il arrive, chaque fichier de la liste reçoit un et un seul résultat, classé dans une catégorie :

| Catégorie | Contenu |
|---|---|
| **Traité** | L'action a abouti à une conclusion, bonne ou mauvaise : créé, déjà référencé, conforme, **différent**, conflit, aucune référence, inconnu… Un fichier « différent » est un résultat, pas un échec. |
| **Échoué** | Le fichier n'a pas pu être lu, ou l'action n'a pas pu se faire : introuvable, erreur de lecture, verrouillé, modifié pendant le calcul, échec. |
| **Annulé** | Le fichier était en cours de lecture au moment de l'annulation. |
| **Non traité** | Le fichier n'a pas été commencé : lot annulé, volume disparu, base devenue inutilisable. La raison est donnée. |
| **Doublon** | Le même fichier physique qu'un fichier précédent de la liste (lien physique, ou le même chemin deux fois). Il n'est pas relu ; le résultat indique lequel. |

**Traités + échoués + annulés + non traités + doublons = total**, exactement. C'est le critère d'acceptation de l'étape 7, et chaque test de lot le vérifie.

**Listes par dossier** (`SHA256SUMS`) : un fichier dont la ligne attend la liste de son dossier reçoit d'abord un résultat provisoire, « En attente de son dossier », puis son résultat définitif quand la liste est écrite, c'est-à-dire quand tous les fichiers de ce dossier dans le lot ont leur résultat. Il n'est compté qu'une fois, à son résultat définitif ; le rapport CSV ne garde que celui-ci ([formats-hash.md](formats-hash.md)).

## Pause et annulation

- **Pause** : les lecteurs s'arrêtent au bloc suivant (1 Mio), en gardant leur fichier ouvert, et ne commencent aucun nouveau fichier. Pendant la pause, le débit n'est pas mesuré ; à la reprise, la mesure repart de zéro, et le temps restant affiche « estimation en cours » pendant quelques secondes.
- **Annulation** : contrôlée entre deux blocs, et aussi pendant une pause. Le fichier en cours de lecture devient « Annulé », et rien n'en est publié (ni fichier de hash, ni base). Les fichiers terminés gardent leur résultat. Les autres deviennent « Non traité (annulé) ».

## Erreurs

- **Erreur sur un fichier** (verrouillé, illisible, introuvable…) : le fichier est en échec, et le lot continue.
- **Volume disparu** (disque débranché, partage perdu) : après l'échec d'une lecture, FileSeal vérifie si la racine du volume existe encore. Si elle a disparu :
  - le fichier en cours est en échec (erreur de lecture, ou introuvable s'il n'avait pas encore été ouvert) ;
  - les autres fichiers de ce volume deviennent aussitôt « Non traité (volume indisponible) » ;
  - les autres volumes continuent ;
  - le récapitulatif liste les volumes perdus.

  Un fichier déjà ouvert peut se terminer normalement même si la lettre de lecteur disparaît : c'est le comportement de Windows, et la lecture reste vérifiée de bout en bout.
- **Base inutilisable** (disque de la base débranché ou plein, base endommagée, support devenu protégé en écriture) : le lot s'arrête proprement.
  - Le fichier concerné garde son résultat. Avec un fichier de hash écrit, c'est une **réussite partielle**.
  - Les lecteurs des autres volumes terminent leur fichier en cours.
  - Tout le reste devient « Non traité (base indisponible) », avec la cause.
  - Les résultats obtenus restent affichés et exportables.
- **Base ouverte en lecture seule** : elle refuse toute écriture, c'est attendu. Une vérification continue sans historique, et chaque résultat le signale.
- **Échec d'écriture d'un fichier de hash** : il ne concerne que ce fichier (réussite partielle). Le lot continue.

## Mise en veille et fermeture

- Pendant un lot, et pendant tout calcul sur un seul fichier, **Windows ne se met pas en veille** de lui-même. L'écran peut s'éteindre. FileSeal utilise `SetThreadExecutionState`, sans droits particuliers. La demande est retirée à la fin de l'opération.
- **Fermer la fenêtre pendant une opération** affiche une question dans la page : « Continuer » (choix proposé par défaut) ou « Arrêter et quitter ». Avant de quitter, l'opération est annulée, puis la base est fermée proprement. Rien de ce qui est terminé n'est perdu, et rien n'est laissé à moitié écrit.

## Progression

Elle est envoyée au plus 10 fois par seconde, avec les résultats arrivés entre-temps :
- fichiers terminés et total ;
- octets traités et total, pondérés par la taille. Un fichier terminé compte pour toute sa taille, même s'il a échoué dès l'ouverture : la barre atteint toujours 100 % ;
- débit sur les 5 dernières secondes, et temps restant ;
- le fichier en cours de lecture sur chaque volume, avec son avancement ;
- l'état « en pause ».

Le dernier envoi porte le récapitulatif : nombres par catégorie et par statut, annulation par l'utilisateur, base perdue, volumes perdus, durée. Comme pour la sélection, l'interface attend ce dernier envoi avant de considérer la liste complète.

## Fichiers listés par un fichier de hash

Un fichier de la liste peut porter une **ligne de fichier de hash** (`listed` : le fichier de hash, le numéro de ligne, le hash attendu). En vérification, il est alors comparé à cette ligne seulement, et pas à ses propres fichiers de hash, comme le fait `sha256sum -c`. La base active est consultée en plus. `manifest::list` (commande `expand_hash_file`) transforme un fichier de hash en une telle liste ; les lignes inutilisables sont rendues à part.

## Interfaces

- **Moteur** : `batch::run(fichiers, options, base, control, sink)`.
- **Application** : commande `run_batch` (envois par `Channel`), `pause_job`, `resume_job`, `cancel_job`, `close_confirmed`. Côté interface : `api.runBatch` dans `ui/src/lib/api.ts`, utilisé par la fenêtre ([interface.md](interface.md)).
- **Ligne de commande** : `create`, `verify` et `search` acceptent des fichiers et des dossiers. Elles appliquent la sélection, puis traitent le tout en un lot :
  `cargo run -p fileseal-cli -- verify "D:\Films" --db films.fileseal [--parallel 2] [--flat] [--hidden] [--ext mkv]`.
