# Nouveautés

## À venir

- **Écran Bases : trier les références** en cliquant sur les en-têtes (nom, dossier, taille, date d'enregistrement, dernière vérification). Le tri porte sur toute la base, pas seulement sur les 2 000 lignes affichées, et prend moins d'un quart de seconde pour 100 000 références.
- **« Vider la liste »** ne demande plus de confirmation : rien n'est supprimé du disque.

## 2.4.0 — 2026-10-24

- **Thèmes** (⚙) : comme Windows (par défaut), clair, sombre, ou sombre OLED (noir pur). Contrastes vérifiés pour chacun ; listes déroulantes, cases et champs suivent le thème, la barre de titre aussi.
- **Suivre les fichiers déplacés automatiquement** (Réglages, pour Créer et Vérifier) : un fichier déplacé, renommé ou retrouvé sous une autre lettre de lecteur est rattaché à sa référence sans confirmation, puis vérifié ou complété. La base garde son dernier emplacement connu, l'ancien reste dans l'historique. Une copie n'est jamais rattachée. Désactivé par défaut.
- **« Exporter le rapport… »** est à côté du bilan qu'il exporte, et non plus collé au bouton de lancement. Un trait sépare les résultats de la ligne de lancement.
- **« Vider la liste »** est à côté de « Retirer », au lieu du menu ⋯. Il ne demande confirmation que si des résultats seraient perdus.
- **Détail moins envahissant** : dans une fenêtre étroite, où il s'ouvre sous la liste, un clic ne fait plus que choisir une ligne. Le détail s'ouvre sur demande (double-clic, Entrée, bouton « Détail »). De même pour l'historique de l'écran Bases.
- **Puces « Afficher : »** : elles comptent les lignes encore dans la liste, en direct (pendant un lot, après un retrait, après une décision), et disparaissent quand plus aucune ligne n'a de résultat.

## 2.3.0 — 2026-10-23

- **Nouvelle mise en page** : la fenêtre ne défile plus. La liste prend toute la hauteur, le détail s'ouvre à côté (en dessous si la fenêtre est étroite), et une barre fixe en bas sert à lancer, suivre et conclure un lot.
- **Réglages** repliés derrière un bouton qui les résume, et gardés d'un lancement à l'autre.
- **En-tête** : la base active en un bouton, l'aide (raccourcis clavier) et les préférences (langue). Les actions rares de la liste sont dans un menu ⋯, le filtre d'extensions dans un panneau.
- **Écran Bases** : sous-onglets Références et Disques, historique dans un volet, menu « Fichier de la base ».
- **Correction** : en contraste élevé, le texte de la ligne sélectionnée était invisible.

## 2.2.0 — 2026-10-23

- **Retirer des références d'une base** : une sélection, ou tous les résultats d'une recherche. Rien n'est effacé : l'historique reste, et une vue « Retirées » permet de les remettre.

## 2.1.0 — 2026-10-22

- **Créer ne relit plus les fichiers déjà référencés** : au choix, s'ils ont tous les algorithmes cochés, ou une référence quelle qu'elle soit. Leurs fichiers de hash peuvent être ajoutés à la base sans relecture.
- **« Seulement ces extensions »** : une liste tapée, majuscules ou non.
- **Lisibilité** : noms longs coupés au milieu, page en pleine largeur, liste plus haute.

## 2.0.0 — 2026-10-22

Première version :

- créer, vérifier et rechercher des références ; accepter une nouvelle version ; rattacher un fichier déplacé ;
- fichiers de hash compatibles GNU (`sha256sum`, `md5sum`), et lecture de l'ancien format `MD5: …` ;
- bases SQLite multiples, avec historique ;
- lots sur plusieurs disques, avec pause, annulation et rapport CSV ;
- écran Bases : disques, changement de lettre de lecteur, réaffectation, sauvegarde, restauration ;
- installateur par utilisateur, sans droits administrateur, et version portable.
