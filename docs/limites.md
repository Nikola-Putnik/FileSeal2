# Limites connues

Mises à jour à chaque étape. Les limites prévues pour être levées dans une étape ultérieure sont signalées.

## Intégrité

- **« Modifié pendant le calcul » n'est pas un instantané cohérent.** Deux protections se complètent :
  - **Pendant la lecture**, les autres programmes ne peuvent ni écrire dans le fichier, ni le renommer, ni le supprimer (partage refusé). Un fichier déjà ouvert en écriture ailleurs donne « Verrouillé ».
  - **Avant et après la lecture**, FileSeal compare sur le fichier ouvert la taille, la date de modification, la date de changement (ChangeTime sous Windows, ctime sous Unix, que les programmes ordinaires ne peuvent pas remettre en arrière) et l'identifiant du fichier. Il vérifie aussi que le nombre d'octets lus égale la taille. Au moindre écart, l'empreinte est écartée.

  Ce qui reste possible :
  - un serveur de fichiers (NAS) qui écrit lui-même dans le fichier pendant la lecture, sans respecter le partage. Ce cas est détecté après coup par les dates, sauf si le serveur ne les met pas à jour ;
  - certains systèmes de fichiers réseau ne donnent ni date de changement ni identifiant. Seules la taille et la date de modification sont alors comparées ;
  - **fausse alerte** : la date de changement bouge aussi quand un autre programme modifie seulement les attributs du fichier pendant la lecture (logiciel de sauvegarde qui efface l'attribut « archive », par exemple). Le résultat « Modifié pendant le calcul » est alors prudent à tort : il suffit de recommencer.
- **MD5** détecte les modifications accidentelles, pas une falsification délibérée : deux contenus différents peuvent être fabriqués avec le même MD5. SHA-256 est l'algorithme par défaut.
- Les **flux de données alternatifs** NTFS (ADS) ne sont pas lus : seul le contenu principal du fichier est haché.
- Windows peut mettre à jour la **date de dernier accès** d'un fichier lu. Le contenu et la date de modification ne sont jamais touchés.

## Sélection (étape 6)

- Les **liens physiques** ne sont pas reconnus comme doublons pendant la découverte, seulement par le lot, à l'ouverture ; voir [selection.md](selection.md).
- Un dossier atteint par deux chemins différents dont l'un passe par un lien choisi directement (`D:\Lien` et `D:\Cible`) n'est pas reconnu comme doublon pendant la découverte ; ses fichiers le seront à la lecture, par leur identité.
- La reconnaissance d'un lecteur réseau et de son chemin UNC n'a pas de test automatique (il faudrait un partage). Elle a été observée à l'étape 10, sur le partage administratif de ce PC (`tools/manual/real-cases.ps1`).
- Les fichiers « en ligne uniquement » sont reconnus par leurs attributs. L'attribut « hors ligne » a été vérifié à l'étape 10 ; un vrai fichier OneDrive reste à observer ([tests-build-livraison.md](tests-build-livraison.md)).
- Hors Windows, « caché » veut dire « nom commençant par un point » ; l'attribut caché de macOS n'est pas lu (à faire lors du portage).
- La taille annoncée par la découverte est celle de la liste du dossier ; la lecture relit la taille sur le fichier ouvert.
- Le test des liens symboliques est sauté si Windows ne permet pas d'en créer (mode développeur désactivé, comme sur le PC de développement actuel) ; les jonctions, qui suivent le même chemin dans le code, sont testées.

## Lots (étape 7)

- **Durabilité pendant un lot** : chaque fichier fait sa propre transaction, mais sans attendre le disque à chaque fois (`synchronous=NORMAL`). Tout est forcé sur le disque à la fin du lot. **Une coupure de courant ou un plantage de Windows pendant un lot peut faire perdre les derniers résultats enregistrés en base**, jamais abîmer la base (WAL). Un plantage de FileSeal seul ne perd rien. Les fichiers de hash, eux, sont toujours forcés sur le disque avant d'être publiés. Relancer le lot complète la base (« complété », « déjà référencé »). Mesures : [performances.md](performances.md).
- **Racine des volumes retenue 30 s par dossier** : un volume monté dans un dossier pendant un lot ne serait vu qu'au plus 30 s après. Sans effet sur les lettres de lecteur.
- **Volumes** : deux partitions d'un même disque sont deux volumes, lus en même temps. Deux partages d'un même NAS aussi.
- Un **volume disparu** n'est détecté qu'après l'échec d'une lecture. Le premier fichier touché est donc en échec (« introuvable » ou « erreur de lecture »), pas « non traité ».
- **Pendant une pause**, les fichiers en cours restent ouverts, et les autres programmes ne peuvent pas les modifier.
- Le choix entre « la base est perdue » et « refus pour ce fichier seulement » repose sur le type d'erreur de SQLite : disque plein, erreur d'entrée-sortie ou base endommagée arrêtent le lot. Une contrainte non respectée, par exemple un emplacement déjà pris, ne concerne que le fichier.
- **Reprise après fermeture** : non. Un lot interrompu (fermeture, plantage) se relance en entier. Les fichiers déjà référencés donnent alors « déjà référencé » sans rien réécrire. C'est l'évolution 4 du cadrage.
- L'empêchement de la mise en veille n'existe que sous Windows (à faire lors du portage).

## Conditions réelles (étape 10)

- **Jamais observés en conditions réelles** (la logique est testée par injection de pannes) :
  - « modifié pendant le calcul » : impossible sur un disque local ou un partage Windows. Un autre programme ne peut pas écrire pendant la lecture, car le partage du fichier le lui interdit. Seul un serveur qui ne respecte pas ce partage (certains NAS) pourrait le provoquer ;
  - « secteur illisible » (erreur du support) : il faudrait un disque défectueux ;
  - « aucune référence compatible » : il faudrait une base écrite par une version future de FileSeal, avec un algorithme inconnu.
- **À observer avec du matériel réel** (procédures dans [tests-build-livraison.md](tests-build-livraison.md)) :
  - clé USB débranchée en pleine lecture ;
  - disque plein ;
  - fichiers OneDrive « en ligne uniquement » ;
  - disque dur et NAS réels.
- **Recherche par contenu et indices par nom limités à 20 éléments**, plus l'élément du fichier lui-même, toujours cherché en premier. Au-delà de 20 éléments de même contenu, le résultat ne tient compte que de 20 d'entre eux, sans ordre particulier. Si l'original déplacé n'en fait pas partie, le fichier est donné « copie d'un contenu connu » au lieu de « déplacement probable ». Seules des collections de fichiers identiques en grand nombre sont concernées, pas un fichier et ses quelques sauvegardes. Chaque élément trouvé est aussi cherché sur le disque : jusqu'à 0,4 ms de plus par fichier quand un nom revient souvent (`IMG_0001.JPG`).
- **Plusieurs lecteurs par disque** pour les SSD NVMe : non fait. Un seul fichier est lu à la fois par disque, à 2 Gio/s en SHA-256, alors que le disque en permet plus du double. C'est l'évolution 9 du cadrage.

## Écran Bases (étape 9)

- **Disques retrouvés par leur lettre seulement** : les volumes montés dans un dossier (et non sous une lettre) et les partages réseau non associés à une lettre ne sont pas parcourus. Un tel disque est vu « branché » s'il est à sa place enregistrée, sinon « non branché ». La réaffectation d'un dossier reste possible.
- **Le numéro de série d'un disque** change s'il est reformaté ; deux disques clonés peuvent avoir le même. Un disque n'est donné pour « déplacé » que si ses fichiers sont trouvés à leur place sur la nouvelle lettre, mais deux clones branchés en même temps restent indiscernables.
- **Réaffecter** relit la place sur le disque de chaque fichier trouvé (une ouverture par fichier) : quelques secondes pour des milliers de fichiers, plus sur un partage réseau. La mise à jour d'un disque revenu sous une autre lettre ne l'ouvre pas.
- **Restaurer** est fait pour les sauvegardes faites par FileSeal. Une copie faite à la main d'une base ouverte, avec son `-wal` à côté, peut être incomplète : FileSeal la lit sans son journal.
- **Noms horodatés** des copies (avant migration, avant restauration) : en heure UTC.
- **Liste des références** : 2 000 lignes au plus (la recherche les réduit) ; historique : 200 vérifications au plus.
- Sous macOS et Linux, la liste des disques présents reste à écrire (table des montages) ; le reste est portable.

## Interface (étape 8)

- **Lecteurs d'écran** : la structure a été vérifiée dans l'arbre d'accessibilité de WebView2 (rôles, noms, états), mais pas encore à l'oreille avec le Narrateur ni NVDA (absent de ce PC). Procédure dans [tests-build-livraison.md](tests-build-livraison.md).
- **Contraste élevé** : vérifié en niveaux de gris, pas encore avec un vrai thème de contraste de Windows (même procédure).
- **Réglages gardés** (2.3) : algorithmes, destinations, fichiers déjà référencés, suivi des fichiers déplacés, options de sélection, « Seulement ces extensions », nombre de disques et panneau déplié ou non. L'action (Créer, Vérifier, Rechercher) ne l'est pas : chaque lancement commence sur Créer. Le filtre par puces (extensions décochées une à une) ne l'est pas non plus, car il dépend des dossiers ajoutés.
- **Détail à côté de la liste** à partir de 1 400 px de large, en pixels CSS : une fenêtre de 1 920 pixels à l'échelle 125 % en fait 1 536. En dessous, le détail passe sous la liste et prend au plus 40 % de la hauteur.
- **Noms coupés au milieu** : les 14 derniers caractères au plus restent visibles. Dans une colonne très étroite, il reste peu du début. Le nom entier est dans l'info-bulle et le détail.
- **Changer « sous-dossiers » ou « fichiers cachés »** parcourt de nouveau les dossiers ajoutés et annule les retraits et déplacements faits à la main (un message le dit).
- **Doublons dans la liste** : reconnus par le chemin (sans casse). Un même fichier atteint par une lettre de lecteur et son chemin réseau, ou par un lien physique, est reconnu au moment du lot (« Doublon »).
- **Décisions pour tous les résultats** : prises une par une ; chacune est une opération dans la base. Pour des milliers de rattachements, cela prend quelques secondes.
- **Limites de la liste** : elle est gardée dans la page, avec environ 80 Mo pour 50 000 fichiers et leurs résultats. Au-delà de quelques centaines de milliers de fichiers, la mémoire deviendrait le facteur limitant.
- Un fichier de hash ne peut plus être ajouté comme fichier ordinaire en mode Vérifier : il est remplacé par les fichiers qu'il liste. Pour créer la référence d'un fichier de hash lui-même, il faut l'ajouter en mode Créer.
- « Afficher dans le dossier » n'existe que sous Windows pour l'instant.

## Fonctionnement (étape 5)

- R2 (lettre de lecteur changée) suppose que le numéro de série du volume ne change pas. Ce numéro change si le disque est reformaté ; deux disques clonés peuvent aussi partager le même. Le hash reste de toute façon le juge.
- Le rattachement et l'acceptation contrôlent la taille et la date de modification du fichier depuis sa lecture, pas son contenu. Un fichier modifié sans que sa taille ni sa date ne changent passerait ce contrôle ; seule une nouvelle vérification le verrait.
- La recherche par contenu ne connaît que les **références courantes**. Un fichier revenu à une ancienne version déjà remplacée n'est pas reconnu.
- Les détails techniques des erreurs sont affichés tels que le système ou le moteur les donnent, parfois en anglais. Le message principal est toujours en français.
- Une erreur réseau ou un disque débranché en pleine lecture ne sont pas réessayés automatiquement : le fichier est signalé en erreur de lecture.
- MD5 et SHA-256 sont calculés l'un après l'autre sur le même cœur : environ 540 Mio/s ensemble, contre 1 800 Mio/s pour SHA-256 seul. Cela ne compte que sur un SSD rapide ; voir [performances.md](performances.md).
- FileSeal **lit** les fichiers de hash qui listent plusieurs fichiers (`SHA256SUMS`), mais n'en **écrit** pas encore : il écrit un fichier de hash par fichier (évolution 2 du cadrage).
- Un fichier de hash est lu en entier en mémoire ; au-delà de 16 Mio (environ 150 000 lignes), il est refusé.
- **Suivi automatique des emplacements** :
  - un fichier déplacé **depuis un disque qui n'est plus branché** n'est pas suivi : rien ne le distingue d'une copie, et l'original est peut-être toujours sur ce disque. Il reste une copie distincte ; « Réaffecter » ou « Rattacher » le règlent ;
  - entre plusieurs éléments identiques disparus, le choix (même nom, sinon le plus ancien) peut ne pas être celui qu'aurait fait l'utilisateur. Le résultat des vérifications n'en dépend pas, seul l'historique ;
  - à la création, le rattachement n'ajoute pas de vérification à l'historique : seul l'ancien emplacement y reste. À la vérification, la vérification le mentionne.

## Base de données

- Tant qu'une base est ouverte dans FileSeal, aucun autre programme ne peut la lire (verrou exclusif voulu).
- Les bases sur un partage réseau sont déconseillées : le verrouillage de SQLite y est peu fiable. Elles s'ouvrent avec un avertissement.
- Une base en lecture seule s'ouvre en consultation (mode `immutable` si le dossier ne peut pas être écrit). SQLite suppose alors que personne ne modifie le fichier : si un autre programme l'écrivait quand même, les lectures pourraient être incohérentes.
- Le contrôle `quick_check` à l'ouverture lit toute la base : de l'ordre d'une seconde pour une base de plusieurs centaines de milliers de références.
- Le contrôle d'espace libre avant une migration n'existe que sous Windows (à faire lors du portage).

## Retirer des références (2.2)

- **Retirer n'efface rien** : la base garde les références retirées et leur historique, sa taille ne diminue pas. Il n'existe pas encore de suppression définitive (« purger les retirées ») ; ce serait une opération irréversible, à n'ajouter que sur demande.
- **Retirer se fait depuis l'écran Bases**, pas depuis les résultats d'un lot.
- **« Retirer les N résultats »** n'est proposé qu'avec un texte de recherche : vider toute une base passe par une nouvelle base.
- **Remettre** replace un élément à sa dernière place, sans vérifier que le fichier y est encore : une vérification le dira.

## Fichiers déjà référencés (2.1)

- **Un fichier sauté n'est pas contrôlé.** « Déjà référencé (non relu) » dit seulement que les références demandées existent ; « Vérifier » dit si le contenu leur correspond.
- **Ajouté à la base depuis un fichier de hash** : la base enregistre la valeur du fichier de hash telle quelle, avec la taille et la date actuelles du fichier. Si le contenu a changé depuis l'écriture du fichier de hash, la base ne le sait pas avant la prochaine vérification, qui le dira (« Différent »).
- **La base reconnaît un fichier déjà référencé par son chemin.** Un fichier retrouvé sous une autre lettre de lecteur est relu, comme sans l'option.
- **La ligne de commande** relit tout par défaut (`--existing read`), contrairement à l'interface.

## Livraison (étape 11)

- **Programme non signé** : Windows SmartScreen peut avertir au premier lancement de l'installateur ou de la version portable (« Informations complémentaires », puis « Exécuter quand même »). Un certificat de signature de code est payant ; à reconsidérer pour une diffusion large.
- **WebView2 n'est pas inclus dans l'installateur.** Il fait partie de Windows 11. Sur un Windows 10 qui ne l'aurait pas, l'installateur le télécharge, ce qui demande Internet ; la version portable ne démarre pas sans lui.
- **Mises à jour** (2.6.0) :
  - la recherche lit le `latest.json` de la dernière release GitHub : sans Internet, ou si GitHub ne répond pas, rien n'est proposé ;
  - les versions 2.5.0 et antérieures n'ont pas ce mécanisme : la 2.6.0 s'installe une fois à la main ;
  - la version portable ne se met pas à jour elle-même : elle annonce la version et ouvre sa page ;
  - la clé de signature des mises à jour est sur l'ordinateur de construction, sans mot de passe, protégée par la session Windows. Perdue, plus aucune mise à jour automatique n'est possible pour les installations existantes (une nouvelle clé exigerait une installation à la main) ; volée, elle permettrait de signer une fausse mise à jour. Elle doit être sauvegardée à l'abri ;
  - l'installateur lui-même n'est toujours pas signé pour Windows (SmartScreen, voir plus haut) : la signature des mises à jour ne concerne que leur vérification par FileSeal.
- **Une seule instance à la fois**, installée et portable confondues : lancer l'une pendant que l'autre est ouverte ramène la fenêtre de la première.
- **La désinstallation garde** les réglages et une clé de registre `HKCU\Software\FileSeal` (le dossier d'installation, pour une réinstallation), sauf si « Supprimer les données de l'application » est coché. Les bases et les fichiers de hash ne sont jamais touchés.
- **L'installation silencieuse** (`/S`) crée aussi un raccourci sur le Bureau.
- **Version portable** :
  - le dossier du programme doit pouvoir être écrit ; sinon FileSeal fonctionne comme s'il était installé, avec les réglages dans le profil ;
  - les données de WebView2 (quelques Mo de cache) sont écrites sur la clé ;
  - seules les bases situées sur le même disque que le programme suivent un changement de lettre ; une base sur un autre disque garde son chemin complet.
- **Testé sur ce PC seulement**, une session Windows 11 sans droits administrateur. La version portable a été lancée depuis des lettres de lecteur simulées (`subst`), pas depuis une vraie clé USB ([tests-build-livraison.md](tests-build-livraison.md)).
