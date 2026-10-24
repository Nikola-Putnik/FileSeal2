# ADR 0002 — Interface en anglais par défaut, français en option

**Statut** : accepté — 2026-10-22

## Contexte

Le cadrage prévoyait une interface en français, avec des textes centralisés pour permettre une traduction plus tard. Il a ensuite été décidé de mettre l'application en anglais par défaut, avec une traduction française optionnelle.

## Décision

- **Dictionnaires**, un fichier par langue dans `ui/src/lib/i18n/` :
  - `en.ts` est la **référence** ;
  - `fr.ts` doit avoir exactement les mêmes clés : le type `Messages` (déduit de `en.ts`) le vérifie à la compilation, et `i18n.test.ts` à l'exécution ;
  - le même test échoue si un texte français est resté identique à l'anglais, sauf liste explicite (mots identiques dans les deux langues).
- **Phrases avec des valeurs** : ce sont des fonctions (`createSummary(algorithms, name, size, where)`). Chaque langue garde ainsi l'ordre des mots et sa typographie : guillemets « » en français et “ ” en anglais, espace avant les deux-points et le signe % en français.
- **Langue courante** : un état réactif de Svelte (`i18n/index.svelte.ts`). Les composants lisent `t()` et se mettent à jour quand la langue change. Les messages et les erreurs sont gardés sous forme de clés, pas de phrases, pour suivre un changement de langue.
- **Nombres, tailles et dates** : formatés selon la langue (`1.5 GB` / `1,5 Go`, dates `en-GB` ou `fr-FR`).
- **Choix de la langue** : une liste dans l'en-tête. Le choix est enregistré dans `settings.json` (dossier de configuration de l'application, ou `FILESEAL_CONFIG_DIR` pour les instances de test).
- **Ce qui reste en anglais quelle que soit la langue** : le code, les commentaires, les journaux, la ligne de commande de développement, et les détails techniques des erreurs (souvent le message du système, dans la langue de Windows).
- **Documentation** (`docs/`) : reste en français.

## Conséquences

- Tout nouveau texte d'interface va dans `en.ts`, puis dans `fr.ts`, jamais directement dans un composant.
- Ajouter une langue : créer `xx.ts` de type `Messages`, l'ajouter à `LOCALES` et à `INTL` dans `index.svelte.ts`, puis à la liste de l'en-tête.
