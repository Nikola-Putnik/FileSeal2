# Nouveautés

## À venir

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
