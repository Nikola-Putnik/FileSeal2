# Règles de rapprochement

Comment FileSeal relie un fichier qu'il a en main à ce que la base connaît. Code :
- `crates/fileseal-core/src/matching.rs` : les règles, fonctions pures testées une à une ;
- `ops/content.rs` : la recherche dans la base ;
- `ops.rs`, `ops/search.rs`, `ops/decide.rs` : l'application des règles.

Scénarios testés : `ops/rules_tests.rs`, et `ops/follow_tests.rs` pour le suivi automatique.

## Principes

- **Un chemin ne prouve jamais l'identité ; un hash, si.** Le chemin (R1) ou la place sur le volume (R2) servent à trouver la référence *attendue*. C'est la comparaison des hash qui décide.
- **Une copie physique = un élément.** Deux fichiers au même contenu (une sauvegarde) sont deux éléments distincts, reliés par leur contenu.
- **Rien n'est rattaché ni accepté sans l'utilisateur.** FileSeal propose (« déplacement probable », « mettre à jour l'emplacement ») ; l'utilisateur décide. Chaque décision est enregistrée (R8). Seule exception, choisie d'avance dans les réglages : le [suivi automatique des emplacements](#suivi-automatique-des-emplacements).
- **Une vérification n'écrit jamais de référence**, et une recherche n'écrit rien du tout.

## Les règles

| Règle | Énoncé | Où |
|---|---|---|
| **R1** | La référence attendue d'un fichier est celle de l'élément enregistré à **son chemin** (clé insensible à la casse sous Windows). | `Store::current_by_path` |
| **R2** | Chemin inconnu, mais **même volume** (numéro de série) et **même chemin depuis la racine du volume** qu'un élément dont le chemin enregistré n'existe plus : c'est cet élément, sous une autre lettre de lecteur (E: devenu F:). La vérification se fait contre sa référence ; la mise à jour du chemin est **proposée**, pas faite. | `ops::recorded_reference`, `Store::current_by_volume` |
| **R3** | Une taille différente n'empêche jamais la comparaison : le résultat est « Différent », avec « taille modifiée ». Le hash est calculé quand même, pour pouvoir accepter la nouvelle version sans relire. | `ops::verify_against` |
| **R4** | Un contenu identique à celui d'un **autre** élément ne rend jamais conforme un fichier dont la référence attendue est différente. Le résultat reste « Différent », avec la mention « même contenu que … ». | `ops::verify_against` + `content::find` |
| **R5** | Fichier sans référence à son emplacement : son contenu est cherché parmi les références courantes, avec la même taille et les mêmes hash pour tous les algorithmes en commun. Le résultat est l'une de ces issues :<br>• **Inconnu** : aucune référence n'a ce contenu ;<br>• **Déplacement probable** : un seul élément de même contenu manque à sa place, sur un volume présent ;<br>• **Copie d'un fichier connu** : les éléments de même contenu sont à leur place, ou sur un disque non branché ;<br>• **Ambigu** : plusieurs éléments de même contenu manquent ; l'utilisateur choisit. | `matching::classify`, `content::find` |
| **R5 bis** | Si aucune référence courante n'a la **taille** du fichier, le contenu est inconnu **sans lire le fichier**. | `Store::has_reference_of_size` |
| **R6** | Même nom, contenu différent : jamais confondus. Le nom est un **indice** affiché, pas une association. Rattacher refuse un contenu différent, sauf si l'utilisateur le force (R7). | `matching::hints`, `decide::attach` |
| **R7** | Renommé **et** modifié : FileSeal ne prétend pas retrouver l'origine. Il montre les éléments de même nom ; l'utilisateur peut rattacher à la main malgré la différence (avec confirmation), puis accepter la nouvelle version. | `decide::attach(allow_different)` |
| **R8** | Rattacher, mettre à jour un emplacement et accepter une version sont des décisions **explicites et tracées**. L'ancien emplacement et l'ancienne référence restent dans l'historique ; une vérification enregistre la décision. | `decide`, `Store::relocate`, `Store::accept_version` |
| **R9** | La recherche par contenu se limite aux algorithmes présents des deux côtés. Deux algorithmes différents ne sont jamais comparés. | `content::agrees` |

### Copie sur un disque non branché

Un élément dont l'emplacement est **sur un volume absent** (disque débranché, partage hors ligne) n'est pas « manquant » : il est **hors d'atteinte**. Il ne fait jamais conclure à un déplacement. Sans cette règle, vérifier l'original sur le disque interne, pendant que la sauvegarde USB est rangée, proposerait de « rattacher » la sauvegarde au disque interne : ce serait une erreur.

### Créer une référence pour un contenu déjà connu

- Contenu d'un élément **présent ailleurs** : c'est une autre copie. La référence est créée, avec la mention « copie d'un fichier connu ».
- Contenu d'un élément **manquant** (déplacement probable) ou **ambigu** : **rien n'est écrit en base**, et le statut est « Décision requise ». L'utilisateur choisit « Il a été déplacé ici : rattacher » ou « C'est une autre copie : créer une nouvelle référence ». Les fichiers de hash à côté du fichier sont écrits normalement.

### Suivi automatique des emplacements

Réglage **Fichiers déplacés : les suivre automatiquement**, pour Créer et Vérifier, désactivé par défaut. Il s'adresse à qui veut seulement créer et vérifier des références, sans se soucier de l'endroit où sont les fichiers : la base garde le **dernier emplacement connu** de chacun. Code : `ops/follow.rs`, `matching::automatic_choice`, tests dans `ops/follow_tests.rs`.

Les règles qui trouvent l'élément ne changent pas. Seule la confirmation de R8 est remplacée par le choix fait d'avance :

| Cas | Sans le suivi | Avec le suivi |
|---|---|---|
| **R2**, lettre de lecteur changée | Mise à jour proposée | Chemin mis à jour, **avant** la lecture : avec « sauter les fichiers déjà référencés », le fichier n'est pas relu. |
| **R5**, déplacement probable (un seul élément de même contenu manque) | Rattacher, à confirmer ; à la création, « Décision requise » | Rattaché, puis comparé (à la vérification) ou complété (à la création). |
| **R5**, ambigu (plusieurs éléments de même contenu manquent) | L'utilisateur choisit | Rattaché à celui qui porte **le même nom**, sinon au **plus ancien**. Tous ont ce contenu : la vérification donne le même résultat quel que soit l'élément choisi ; seul l'historique en dépend. |
| Copie d'un fichier connu (l'élément est à sa place, ou sur un disque non branché) | Nouvel élément à la création | **Inchangé** : une copie n'est jamais rattachée. |
| Contenu inconnu, renommé **et** modifié (R7) | Indices de nom | **Inchangé** : rien n'est rattaché. |

- **Rechercher** n'écrit toujours rien, avec ou sans le suivi.
- **« C'est une autre copie »** (nouvel élément demandé) l'emporte sur le suivi.
- **Trace** : l'ancien emplacement reste dans l'historique de l'élément, avec ses dates. À la vérification, l'historique note aussi « location followed; previously at … ». Le résultat du fichier l'indique (« Emplacement suivi »), le rapport CSV aussi (`location followed from …`).
- **Plusieurs fichiers d'un même lot** pour un même élément (deux copies identiques déplacées ensemble) : le déplacement n'est enregistré que si l'élément est toujours à l'emplacement vu manquant. Sinon le contenu est cherché de nouveau, et le fichier va à un autre élément manquant, ou reste une copie.

## Les décisions

| Décision | Ce qui est vérifié avant | Ce qui est écrit |
|---|---|---|
| **Rattacher** (déplacement, renommage, lettre de lecteur) | Le fichier n'a pas changé depuis sa lecture (taille, date de modification). Son contenu est celui de l'élément, sauf rattachement forcé (R7). L'emplacement n'est pas déjà pris par un autre élément. | L'ancien emplacement devient de l'historique, le nouveau devient courant (avec volume et place sur le volume). Une opération `attach` et une vérification sont enregistrées. |
| **Accepter une nouvelle version** | Le fichier n'a pas changé depuis la vérification (taille, date de modification). La référence comparée est toujours la référence courante : la même vérification ne peut pas être acceptée deux fois. | Base : l'ancienne référence est marquée remplacée, la nouvelle (origine `accepted`) reprend les hash **observés** et le lien vers la vérification. Fichiers de hash existants à côté du fichier : remplacés de façon atomique par la valeur acceptée (aucun n'est créé). |

Ces contrôles de date et de taille ne prouvent pas que le contenu est identique ; seule une lecture le prouve. Ils attrapent un fichier modifié entre-temps, et le message invite alors à vérifier de nouveau.
