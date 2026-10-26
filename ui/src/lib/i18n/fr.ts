// Français. Mêmes clés que en.ts (vérifié par le type `Messages`).

import type { Messages } from './en';

/** A count with this language's digit grouping. */
const n = (value: number) => value.toLocaleString('fr-FR');

export const fr: Messages = {
  language: { label: 'Langue', en: 'English', fr: 'Français' },
  updates: {
    available: (version: string) => `FileSeal ${version} est disponible.`,
    current: (version: string) => `(vous avez la ${version})`,
    notes: 'Nouveautés',
    install: 'Installer et redémarrer',
    download: 'Télécharger',
    later: 'Plus tard',
    busy: 'Possible une fois le lot en cours terminé : l’installation ferme FileSeal.',
    downloading: (amount: string) => `Téléchargement de la mise à jour… ${amount}`,
    installing: 'Installation : FileSeal se ferme, puis redémarre.',
    failed: (reason: string) => `La mise à jour n’a pas pu être installée : ${reason}`,
    checkAtStart: 'Rechercher une nouvelle version au démarrage',
    checkNow: 'Rechercher maintenant',
    checking: 'Recherche d’une nouvelle version…',
    upToDate: (version: string) => `La ${version} est la dernière version.`,
    checkFailed: (reason: string) => `Recherche impossible : ${reason}`,
  },
  theme: {
    label: 'Thème',
    system: 'Comme Windows (clair ou sombre)',
    light: 'Clair',
    dark: 'Sombre',
    oled: 'Sombre OLED (noir pur)',
  },

  header: { preferences: 'Préférences', help: 'Aide : raccourcis clavier' },

  about: {
    version: (version: string) => `Version ${version}`,
    portable: 'portable',
    portableHint: 'Portable : réglages et données sont gardés à côté du programme, rien sur ce PC.',
  },

  database: {
    region: 'Base de données active',
    current: 'Base :',
    references: (count: number) => `${n(count)} référence${count > 1 ? 's' : ''}`,
    none: 'Aucune base ouverte',
    create: 'Créer une base…',
    open: 'Ouvrir une base…',
    close: 'Fermer',
    createTitle: 'Créer une base FileSeal',
    openTitle: 'Ouvrir une base FileSeal',
    fileFilter: 'Base FileSeal',
    defaultFileName: 'Ma collection.fileseal',
    recent: 'Récentes',
    noRecent: 'Aucune base récente',
    notFound: '(introuvable)',
    locate: 'Localiser…',
    forget: (name: string) => `Retirer « ${name} » de la liste (le fichier n’est pas supprimé)`,
    readOnly: 'lecture seule',
    readOnlyHint:
      'Cette base ne peut pas être modifiée (fichier, dossier ou disque en lecture seule) : on peut la consulter et vérifier avec, mais rien n’est enregistré.',
    network: 'partage réseau',
    networkHint:
      'Cette base est sur un partage réseau, où le verrouillage de SQLite n’est pas fiable. Gardez de préférence les bases sur un disque local.',
    openFailed: (name: string, reason: string) => `Impossible d’ouvrir « ${name} » : ${reason}`,
    reopenFailed: (name: string, reason: string) =>
      `La dernière base, « ${name} », n’a pas pu être rouverte : ${reason}`,
    migrated: (copy: string) =>
      `Cette base a été mise à jour pour cette version de FileSeal. Une copie de son état précédent est conservée : ${copy}`,
  },

  nav: { label: 'Écrans', files: 'Fichiers', databases: 'Bases' },

  dbScreen: {
    none: 'Aucune base ouverte : créez-en une ou ouvrez-en une avec les boutons du haut.',
    active: 'Base active',
    rename: 'Renommer…',
    renameLabel: 'Nom affiché',
    renamed: (name: string) => `La base s’appelle maintenant « ${name} ».`,
    save: 'Enregistrer',
    cancel: 'Annuler',
    facts: (references: number, version: number) =>
      `${n(references)} référence${references > 1 ? 's' : ''} · schéma version ${version}`,
    readOnly: 'Ouverte en lecture seule : on peut la consulter, pas la modifier.',
    showInFolder: 'Afficher dans le dossier',
    move: 'Déplacer ou renommer le fichier…',
    moveTitle: 'Déplacer ou renommer le fichier de la base',
    moved: (path: string) => `La base est maintenant ${path}.`,
    backup: 'Sauvegarder…',
    backupTitle: 'Sauvegarder la base',
    backupName: (name: string, date: string) => `${name} sauvegarde ${date}.fileseal`,
    backedUp: (path: string, size: string) => `Sauvegarde écrite et contrôlée : ${path} (${size}).`,
    restore: 'Restaurer une sauvegarde…',
    restoreTitle: 'Choisir la sauvegarde à restaurer',
    restoreConfirm: (backup: string, references: number, current: string) =>
      `Remplacer le contenu de « ${current} » par cette sauvegarde (« ${backup} », ${n(references)} référence${references > 1 ? 's' : ''}) ? Ce qu’elle contient maintenant est gardé en copie à côté.`,
    restoreYes: 'Restaurer',
    restored: (copy: string) =>
      `Sauvegarde restaurée. Ce que la base contenait avant est gardé dans ${copy}.`,

    disks: 'Disques',
    disksHint:
      'Où sont aujourd’hui les fichiers de cette base. Un disque revenu sous une autre lettre est reconnu sans relire ses fichiers.',
    refresh: 'Actualiser',
    disksEmpty: 'Aucun fichier enregistré pour l’instant.',
    disksGrid: 'Disques de la base',
    columns: {
      root: 'Enregistrés sous',
      files: 'Fichiers',
      state: 'Aujourd’hui',
      name: 'Nom',
      folder: 'Dossier',
      size: 'Taille',
      recorded: 'Enregistrée le',
      lastCheck: 'Dernière vérification',
      retired: 'Retirée le',
    },
    volume: {
      connected: 'Branché',
      movedTo: (root: string) => `Maintenant en ${root}`,
      notConnected: 'Non branché',
      unidentified: 'Disque non identifié',
      unidentifiedPresent: 'Disque non identifié ; le dossier est là',
    },
    update: (root: string) => `Mettre à jour vers ${root}…`,
    confirmUpdate: (files: number, from: string, to: string) =>
      `Enregistrer le nouvel emplacement de ${n(files)} fichier${files > 1 ? 's' : ''} : ${from} → ${to} ? Rien n’est relu ; les anciens emplacements restent dans l’historique.`,
    updated: (moved: number, to: string) =>
      `${n(moved)} emplacement${moved > 1 ? 's' : ''} mis à jour vers ${to}.`,
    reassign: 'Réaffecter…',
    reassignTitle: 'Réaffecter un dossier',
    reassignOpen: 'Réaffecter un dossier…',
    fileMenu: 'Fichier de la base',
    sections: 'Parties de la base',
    reassignHint:
      'Pour des fichiers déplacés ou copiés ensemble ailleurs (autre disque, partage, dossier renommé) : leurs références les suivent.',
    from: 'Les fichiers enregistrés sous',
    to: 'sont maintenant sous',
    choose: 'Choisir…',
    chooseTitle: 'Où sont les fichiers maintenant',
    includeMissing: 'Aussi ceux qui n’y sont pas trouvés (disque non branché pour l’instant)',
    preview: 'Aperçu',
    previewed: (files: number, found: number, missing: number) =>
      `${n(files)} fichier${files > 1 ? 's' : ''} enregistré${files > 1 ? 's' : ''} là : ${n(found)} trouvé${found > 1 ? 's' : ''} au nouvel emplacement, ${n(missing)} non trouvé${missing > 1 ? 's' : ''}.`,
    notFoundThere: 'pas trouvé là',
    apply: 'Appliquer…',
    confirmApply: (count: number, from: string, to: string) =>
      `Enregistrer le nouvel emplacement de ${n(count)} fichier${count > 1 ? 's' : ''} : ${from} → ${to} ? Les anciens emplacements restent dans l’historique.`,
    reassigned: (moved: number, left: number, taken: number) =>
      `${n(moved)} déplacé${moved > 1 ? 's' : ''}${left > 0 ? `, ${n(left)} laissé${left > 1 ? 's' : ''} où ${left > 1 ? 'ils étaient' : 'il était'} (non trouvé${left > 1 ? 's' : ''})` : ''}${taken > 0 ? `, ${n(taken)} non déplacé${taken > 1 ? 's' : ''} : un autre fichier de la base y est enregistré` : ''}.`,

    references: 'Références',
    search: 'Rechercher',
    searchHint: 'Partie d’un nom ou d’un dossier, ou début d’un hash',
    shown: (shown: number, total: number) =>
      shown < total
        ? `${n(shown)} références affichées sur ${n(total)} : précisez la recherche.`
        : `${n(total)} référence${total > 1 ? 's' : ''}`,
    referencesGrid: 'Références de la base',
    noReference: 'Aucune référence ne correspond.',
    never: 'jamais',
    listed: { label: 'Afficher', active: 'Dans la base', retired: 'Retirées' },
    removeSelected: (count: number) => `Retirer la sélection de la base (${n(count)})…`,
    removeMatching: (count: number) =>
      count === 1
        ? 'Retirer le résultat de la base…'
        : `Retirer les ${n(count)} résultats de la base…`,
    confirmRemove: (count: number, name: string) =>
      `Retirer ${n(count)} référence${count > 1 ? 's' : ''} de « ${name} » ? Les fichiers et leurs fichiers de hash ne sont pas touchés. ${count > 1 ? 'Elles ne servent' : 'Elle ne sert'} plus à vérifier ni à rechercher, et ${count > 1 ? 'peuvent' : 'peut'} être remise${count > 1 ? 's' : ''} depuis « Retirées ».`,
    removed: (count: number) =>
      `${n(count)} référence${count > 1 ? 's' : ''} retirée${count > 1 ? 's' : ''} de la base.`,
    putBackSelected: (count: number) => `Remettre la sélection dans la base (${n(count)})…`,
    putBackMatching: (count: number) =>
      count === 1
        ? 'Remettre le résultat dans la base…'
        : `Remettre les ${n(count)} résultats dans la base…`,
    confirmPutBack: (count: number) =>
      `Remettre ${n(count)} référence${count > 1 ? 's' : ''} dans la base, là où ${count > 1 ? 'elles étaient' : 'elle était'} ?`,
    putBack: (count: number, taken: number) =>
      `${n(count)} référence${count > 1 ? 's' : ''} remise${count > 1 ? 's' : ''} dans la base.${taken > 0 ? ` ${n(taken)} laissée${taken > 1 ? 's' : ''} retirée${taken > 1 ? 's' : ''} : un autre fichier de la base est enregistré à ${taken > 1 ? 'leur place' : 'sa place'}.` : ''}`,
    retiredOn: (date: string) => `Retirée de la base le ${date}`,
    noRetired: 'Aucune référence retirée.',
    selectHint:
      'Plusieurs références : Maj ou Ctrl avec un clic ou les flèches, Ctrl+A pour toutes celles affichées.',

    history: 'Historique',
    itemSince: (date: string) => `Suivi depuis le ${date}`,
    versions: 'Versions',
    origin: { created: 'créée', accepted: 'acceptée comme nouvelle version', imported: 'importée' },
    current: 'actuelle',
    replacedOn: (date: string) => `remplacée le ${date}`,
    locations: 'Emplacements',
    seen: (first: string, last: string) => `vu du ${first} au ${last}`,
    missingSince: (date: string) => `quitté le ${date}`,
    checks: 'Vérifications',
    noChecks: 'Jamais vérifié.',
    moreChecks: 'Les vérifications plus anciennes ne sont pas affichées.',
  },

  action: {
    label: 'Action',
    create: 'Créer une référence',
    verify: 'Vérifier',
    search: 'Rechercher',
  },

  list: {
    title: 'Fichiers',
    addFiles: 'Ajouter des fichiers…',
    addFilesTitle: 'Ajouter des fichiers',
    addFolder: 'Ajouter un dossier…',
    addFolderTitle: 'Ajouter un dossier',
    remove: 'Retirer',
    moveUp: 'Monter',
    moveDown: 'Descendre',
    sortBy: 'Trier par',
    sortKeys: { name: 'nom', folder: 'dossier', size: 'taille', extension: 'extension' },
    clear: 'Vider la liste',
    more: 'Plus d’actions',
    extensionsButton: (summary: string) => `Extensions : ${summary}`,
    extensionsAll: 'toutes',
    extensionsOnly: (list: string) => `seulement ${list}`,
    extensionsLeftOut: (count: number) => `${n(count)} écartée${count > 1 ? 's' : ''}`,
    stop: 'Arrêter',
    scanning: (files: number, folders: number) =>
      `Parcours des dossiers… ${n(files)} fichier${files > 1 ? 's' : ''} trouvé${files > 1 ? 's' : ''} dans ${n(folders)} dossier${folders > 1 ? 's' : ''}`,
    count: (files: number, size: string) =>
      `${n(files)} fichier${files > 1 ? 's' : ''} à traiter, ${size}`,
    leftOutByExtension: (files: number) =>
      `${n(files)} autre${files > 1 ? 's' : ''} écarté${files > 1 ? 's' : ''} par le filtre d’extensions`,
    selected: (count: number) => `${n(count)} sélectionné${count > 1 ? 's' : ''}`,
    inFolder: (folder: string) => `dans ${folder}`,
    filter: 'Filtrer par nom',
    filterHint: 'Partie d’un nom ou d’un dossier',
    show: 'Afficher',
    showing: 'Afficher :',
    showAll: (count: number) => `Tous (${n(count)})`,
    tabs: {
      files: (count: number) => `Fichiers (${n(count)})`,
      excluded: (count: number) => `Écartés (${n(count)})`,
      problems: (count: number) => `Problèmes (${n(count)})`,
    },
    columns: {
      name: 'Nom',
      folder: 'Dossier',
      size: 'Taille',
      state: 'État',
      reason: 'Raison',
      path: 'Chemin',
      problem: 'Problème',
    },
    grid: 'Fichiers à traiter',
    excludedGrid: 'Fichiers et dossiers écartés',
    problemsGrid: 'Problèmes de sélection',
    empty:
      'Aucun fichier pour l’instant. Ajoutez des fichiers ou des dossiers, ou glissez-les dans la fenêtre.',
    emptyFiltered: 'Aucun fichier ne correspond aux filtres.',
    emptyExcluded: 'Rien n’a été écarté.',
    emptyProblems: 'Aucun problème.',
    ready: 'À traiter',
    next: 'En attente',
    reading: (percent: number) => `Lecture… ${percent} %`,
    listedFrom: (line: number, name: string) => `ligne ${line} de « ${name} »`,
    folder: '(dossier, non parcouru)',
    lineProblem: (line: number, name: string, reason: string) =>
      `Ligne ${line} de « ${name} » : ${reason}`,
    keyboard:
      'Dans la liste : flèches pour sélectionner, Maj+flèches pour étendre, Ctrl+flèches pour se déplacer sans sélectionner, Espace pour ajouter ou retirer un fichier de la sélection, Ctrl+A pour tout, Suppr pour retirer de la liste, Alt+↑/↓ pour monter ou descendre la sélection, Entrée pour le détail.',
    notices: {
      expanded: (count: number, name: string) =>
        `${n(count)} fichier${count > 1 ? 's' : ''} listé${count > 1 ? 's' : ''} dans « ${name} » ajouté${count > 1 ? 's' : ''} : chacun sera vérifié avec sa ligne dans ce fichier.`,
      hashFileAsFile: (name: string) =>
        `« ${name} » est un fichier de hash, ajouté comme un fichier ordinaire. Pour vérifier les fichiers qu’il liste, choisissez d’abord Vérifier, puis ajoutez-le.`,
      stopped: 'Le parcours des dossiers a été arrêté : la liste est incomplète.',
      rescanned:
        'Les dossiers ont été parcourus de nouveau avec les nouvelles options : les changements faits à la main dans la liste ont été annulés.',
    },
  },

  exclusion: {
    hidden: 'Caché',
    system: 'Fichier système',
    systemFolder: 'Dossier système de Windows',
    hashFile: 'Fichier de hash',
    fileSealFile: 'Base FileSeal',
    temporaryFile: 'Fichier temporaire',
    onlineOnly: 'En ligne uniquement (absent de cet ordinateur)',
    link: 'Lien ou jonction (non suivi)',
    special: 'Périphérique, tube ou socket',
    duplicate: 'Déjà dans la liste',
  },

  selection: {
    title: 'Sélection',
    subfolders: 'Inclure les sous-dossiers',
    hidden: 'Inclure les fichiers cachés et système',
    optionsHint: 'Elles s’appliquent aux dossiers ; les changer relance leur parcours.',
    parallel: 'Disques lus en même temps',
    parallelHint: 'Un lecteur par disque ; plusieurs disques sont lus ensemble.',
    extensions: 'Extensions',
    noExtension: '(aucune)',
    extension: (name: string, files: number, size: string) =>
      `${name} — ${n(files)} fichier${files > 1 ? 's' : ''}, ${size}`,
    allExtensions: 'Toutes',
    only: 'Seulement ces extensions',
    apply: 'Appliquer',
    onlyHint: 'par exemple mkv iso mp4 ; majuscules ou non',
    onlyActive: (list: string) =>
      `Seulement ${list} : les autres extensions sont écartées, y compris dans les dossiers ajoutés ensuite.`,
  },

  settings: {
    title: 'Réglages',
    algorithms: 'Algorithmes',
    recommended: '(recommandé)',
    md5Note:
      '(compatibilité : détecte les erreurs accidentelles, ne protège pas contre une falsification)',
    destinations: 'Destinations',
    sidecarNextToFile: 'Fichier de hash à côté de chaque fichier',
    listPerFolder: 'Une liste de hash par dossier (SHA256SUMS, MD5SUMS)',
    activeDatabase: 'Base active',
    noDatabase: '(aucune base ouverte)',
    readOnlyDatabase: '(base en lecture seule)',
    referencesToUse: 'Références à consulter',
    sidecarsNextToFile:
      'Fichiers de hash à côté de chaque fichier, et liste du dossier (SHA256SUMS)',
    summary: {
      none: 'aucune',
      hashFiles: 'fichiers de hash',
      hashLists: 'listes par dossier',
      database: 'base',
      existing: {
        skipComplete: 'référencés sautés',
        skipAny: 'fichiers ayant une référence sautés',
        read: 'référencés relus',
      },
      references: (sources: string) => `références : ${sources}`,
      noSubfolders: 'sans les sous-dossiers',
      hidden: 'fichiers cachés inclus',
      followMoves: 'déplacements suivis',
    },
    listedReferences:
      'Les fichiers ajoutés depuis un fichier de hash sont vérifiés avec leur ligne dans celui-ci, et seulement elle.',
    existing: {
      title: 'Fichiers déjà référencés',
      skipComplete: 'Les sauter, sans les relire, s’ils ont tous les algorithmes cochés',
      skipAny:
        'Les sauter, sans les relire, s’ils ont une référence, quelle qu’elle soit (un ancien .md5 suffit)',
      read: 'Les relire : contrôler leurs références et les compléter',
      importHashFiles: 'Ajouter leurs fichiers de hash à la base active, sans relire',
      hint: 'Un fichier sauté n’est pas contrôlé : c’est le rôle de Vérifier.',
    },
    follow: {
      title: 'Fichiers déplacés',
      label: 'Les suivre automatiquement',
      hint: 'Un fichier trouvé ailleurs que prévu (déplacé, renommé, lettre de lecteur changée) est rattaché à sa référence sans confirmation : la base garde son dernier emplacement connu. Une copie n’est jamais rattachée.',
    },
  },

  launch: {
    skipping: 'Les fichiers déjà référencés ne seront pas relus.',
    importing:
      'Les fichiers déjà référencés ne seront pas relus ; les fichiers de hash de ceux qui ne sont pas encore dans la base y seront ajoutés.',
    region: 'Lancement',
    needFiles: 'Ajoutez d’abord des fichiers à la liste.',
    busyScanning: 'Attendez la fin du parcours des dossiers.',
    needAlgorithm: 'Cochez au moins un algorithme.',
    needDestination: 'Choisissez au moins une destination.',
    needSource: 'Choisissez au moins une source de références.',
    whereSidecar: 'des fichiers de hash à côté d’eux',
    whereSums: 'une liste de hash dans chaque dossier',
    whereSidecars: 'leurs fichiers de hash',
    whereLists: 'les lignes des fichiers de hash ajoutés',
    whereDatabase: (name: string) => `la base « ${name} »`,
    and: ' et ',
    files: (count: number) => `${n(count)} fichier${count > 1 ? 's' : ''}`,
    createSummary: (algorithms: string, files: string, size: string, where: string) =>
      `Créer les références ${algorithms} de ${files} (${size}) dans ${where}.`,
    verifySummary: (files: string, size: string, where: string) =>
      `Vérifier ${files} (${size}) avec ${where}.`,
    searchSummary: (files: string, size: string, database: string) =>
      `Chercher ${files} (${size}) dans la base « ${database} ».`,
    needDatabase: 'Ouvrez une base pour y chercher.',
    search: 'Rechercher',
    create: 'Créer les références',
    verify: 'Vérifier',
    shortcut: 'Ctrl+Entrée',
    pause: 'Pause',
    resume: 'Reprendre',
    cancel: 'Annuler',
    cancelling: 'Annulation…',
  },

  progress: {
    title: 'Progression',
    paused: 'En pause : la lecture s’arrête entre deux blocs et reprendra où elle en était.',
    reading: 'En cours de lecture',
    readingList: (files: string) => `En lecture : ${files}`,
    currentFileBar: 'Progression du fichier en cours',
    overall: 'Ensemble :',
    overallBar: 'Progression globale',
    files: 'Fichiers',
    filesValue: (done: number, total: number) => `${n(done)} terminé(s) sur ${n(total)}`,
    volume: 'Volume lu',
    volumeValue: (done: string, total: string) => `${done} sur ${total}`,
    rate: 'Débit',
    measuring: 'mesure en cours',
    elapsed: 'Temps écoulé',
    remaining: 'Temps restant',
    estimating: 'estimation en cours',
    percent: (value: number) => `${value} %`,
  },

  result: {
    title: 'Résultat',
    destinations: 'Destinations',
    hashFile: (algorithm: string) => `Fichier de hash ${algorithm} :`,
    hashList: (algorithm: string) => `Liste ${algorithm} du dossier :`,
    existingChecked: '(déjà présent, vérifié)',
    database: (name: string) => `Base « ${name} » :`,
    existingValue: 'Valeur existante :',
    invalidDetail: (detail: string) => `Illisible ou mal formé. Détail technique : ${detail}`,
    algorithmAdded: (algorithms: string) => `Algorithme ajouté : ${algorithms}`,
    recordedReference: (size: string) => `Référence enregistrée${size} :`,
    computedDigests: 'Empreintes calculées',
    noDigest: 'Aucune : le fichier n’a pas été lu entièrement.',
    skippedConflict: 'non écrit : une autre référence est en conflit',
    skippedUnread: 'non écrit : le fichier n’a pas été lu entièrement',
    skippedReferenced: 'non écrit : déjà référencé, non relu',
    referencesUsed: 'Références consultées',
    referenceOf: (date: string) => `Référence du ${date}`,
    expected: (algorithm: string) => `Attendu ${algorithm} :`,
    sizeChanged: (before: string, now: string) =>
      `Taille modifiée : ${before} enregistrés, ${now} aujourd’hui.`,
    observedDigests: 'Empreintes observées',
    notRecorded: 'La vérification n’a pas pu être ajoutée à l’historique de la base. ',
    technical: (detail: string) => `Détail technique : ${detail}`,
    listedFrom: (line: number, path: string) => `Ligne ${line} du fichier de hash ${path}`,
  },

  results: {
    title: 'Résultats',
    counts: (done: number, total: number) =>
      `${n(done)} fichier${done > 1 ? 's' : ''} traité${done > 1 ? 's' : ''} sur ${n(total)}`,
    categories: (processed: number, failed: number, cancelled: number, notProcessed: number) =>
      `${n(processed)} abouti${processed > 1 ? 's' : ''}, ${n(failed)} en échec, ${cancelled > 0 ? `${n(cancelled)} annulé${cancelled > 1 ? 's' : ''}, ` : ''}${n(notProcessed)} non traité${notProcessed > 1 ? 's' : ''}`,
    stopped: 'Arrêté par vous : les fichiers non commencés sont « non traités ».',
    databaseLost: (reason: string) =>
      `La base est devenue indisponible, le lot s’est donc arrêté : ${reason} Les fichiers non commencés sont « non traités ».`,
    volumesLost: (volumes: string) =>
      `Plus disponible pendant le lot : ${volumes}. Ses fichiers restants sont « non traités » ; les autres disques ont continué.`,
    forAll: 'Pour tous les résultats',
    updateLocations: (count: number) =>
      `Mettre à jour ${n(count)} emplacement${count > 1 ? 's' : ''} (lettre de lecteur changée)…`,
    confirmUpdate: (count: number) =>
      `Enregistrer le nouvel emplacement de ${n(count)} fichier${count > 1 ? 's' : ''} retrouvé${count > 1 ? 's' : ''} au même endroit du même disque, sous une autre lettre de lecteur ?`,
    attachMoved: (count: number) =>
      `Rattacher ${n(count)} fichier${count > 1 ? 's' : ''} déplacé${count > 1 ? 's' : ''}…`,
    confirmAttach: (count: number) =>
      `Rattacher ${n(count)} fichier${count > 1 ? 's' : ''} au fichier connu dont il${count > 1 ? 's ont' : ' a'} le contenu et qui a disparu de son emplacement ? L’ancien emplacement reste dans l’historique.`,
    acceptVersions: (count: number) =>
      `Accepter ${n(count)} nouvelle${count > 1 ? 's' : ''} version${count > 1 ? 's' : ''}…`,
    confirmAccept: (count: number) =>
      `Accepter le contenu actuel de ${n(count)} fichier${count > 1 ? 's' : ''} comme nouvelle référence ? Les références précédentes restent dans l’historique, et les fichiers de hash à côté sont mis à jour.`,
    working: (done: number, total: number) => `En cours… ${n(done)} sur ${n(total)}`,
    decisionsDone: (done: number, failed: number) =>
      failed === 0
        ? `Fait pour ${n(done)} fichier${done > 1 ? 's' : ''}.`
        : `Fait pour ${n(done)} fichier${done > 1 ? 's' : ''} ; ${n(failed)} en échec : voir leur détail.`,
    decided: {
      attached: 'Rattaché',
      updated: 'Emplacement mis à jour',
      accepted: 'Nouvelle version acceptée',
      created: 'Nouvelle référence créée',
      failed: 'Échec de la décision',
    },
    details: 'Détail',
    closeDetails: 'Fermer le détail',
    noActive: 'Choisissez un fichier dans la liste pour voir son détail ici.',
    notRun: 'Pas encore traité.',
    reveal: 'Afficher dans le dossier',
    sameAs: (path: string) => `Même fichier que ${path} : lu une seule fois.`,
  },

  skip: {
    cancelled: {
      label: 'Non traité',
      explanation: 'Le lot a été arrêté avant ce fichier.',
    },
    volumeUnavailable: {
      label: 'Non traité (disque indisponible)',
      explanation: 'Son disque ou son partage réseau a disparu pendant le lot.',
    },
    databaseUnavailable: {
      label: 'Non traité (base indisponible)',
      explanation: 'La base ne pouvait plus être écrite : le lot s’est arrêté.',
    },
    duplicate: {
      label: 'Doublon',
      explanation: 'Le même fichier physique qu’un autre de la liste : lu une seule fois.',
    },
  },

  shortcuts: {
    title: 'Raccourcis clavier',
    list: 'Ctrl+O : ajouter des fichiers · Ctrl+Maj+O : ajouter un dossier · Ctrl+Entrée : lancer',
  },

  createStatus: {
    created: {
      label: 'Référence créée',
      explanation: 'Le contenu actuel du fichier est enregistré.',
    },
    completed: {
      label: 'Référence complétée',
      explanation: 'La référence existante a été vérifiée identique, puis complétée.',
    },
    alreadyReferenced: {
      label: 'Déjà référencé',
      explanation: 'Une référence identique existait déjà ; rien n’a été réécrit.',
    },
    skipped: {
      label: 'Déjà référencé (non relu)',
      explanation:
        'Il avait déjà ses références : il n’a pas été lu, elles n’ont donc pas été contrôlées. Vérifier dira s’il a changé.',
    },
    imported: {
      label: 'Ajouté à la base (non relu)',
      explanation:
        'Ses fichiers de hash existants ont été enregistrés dans la base, sans relire le fichier. Vérifier dira s’il leur correspond toujours.',
    },
    conflict: {
      label: 'Conflit avec une référence existante',
      explanation:
        'Une référence existante ne correspond pas au contenu actuel, ou ne peut pas être lue. Rien n’a été écrit ni remplacé.',
    },
    pending: {
      label: 'En attente de son dossier',
      explanation:
        'Lu et enregistré. Sa ligne ira dans la liste de hash du dossier quand les autres fichiers du dossier seront traités.',
    },
    needsDecision: {
      label: 'Décision requise',
      explanation:
        'Ce contenu est un fichier connu qui semble avoir été déplacé ici. Rien n’a été écrit dans la base : dites s’il a été déplacé, ou s’il s’agit d’une autre copie.',
    },
    partialSuccess: {
      label: 'Réussite partielle',
      explanation: 'Au moins une destination a échoué : voir le détail ci-dessous.',
    },
    failed: { label: 'Échec', explanation: 'Aucune référence n’a pu être enregistrée.' },
    cancelled: { label: 'Annulé', explanation: 'Rien n’a été enregistré.' },
  },

  verifyStatus: {
    match: {
      label: 'Conforme à la référence',
      explanation: 'Le contenu est identique à celui enregistré.',
    },
    mismatch: {
      label: 'Différent de la référence',
      explanation:
        'Le contenu a changé depuis la création de la référence. Cela indique une modification, pas forcément une corruption.',
    },
    noReference: {
      label: 'Aucune référence',
      explanation: 'Aucun fichier de hash ni aucune entrée de la base pour ce fichier.',
    },
    noCompatibleReference: {
      label: 'Aucune référence compatible',
      explanation:
        'Les références trouvées utilisent un algorithme que FileSeal ne sait pas calculer.',
    },
    conflict: {
      label: 'Références contradictoires',
      explanation:
        'Certaines références correspondent au fichier et d’autres non. FileSeal ne choisit pas : comparez-les ci-dessous.',
    },
    invalidHashFile: {
      label: 'Fichier de hash invalide',
      explanation:
        'Le fichier de hash, ou sa ligne pour ce fichier, est illisible ou mal formé : rien n’a pu être comparé.',
    },
    notFound: {
      label: 'Fichier attendu introuvable',
      explanation: 'Aucun fichier à cet emplacement : supprimé, déplacé ou renommé ?',
    },
    readError: {
      label: 'Erreur de lecture',
      explanation: 'Le fichier n’a pas pu être lu entièrement.',
    },
    locked: {
      label: 'Fichier verrouillé ou en cours d’utilisation',
      explanation: 'Un autre programme écrit dans ce fichier. Réessayez quand il sera fermé.',
    },
    modifiedDuringRead: {
      label: 'Modifié pendant le calcul',
      explanation: 'Le fichier a changé pendant sa lecture ; le résultat a été écarté.',
    },
    cancelled: { label: 'Annulé', explanation: 'La vérification a été interrompue.' },
    notProcessed: {
      label: 'Non traité',
      explanation: 'L’opération a été annulée avant ce fichier.',
    },
    failed: { label: 'Échec', explanation: 'La vérification n’a pas pu être menée.' },
  },

  writeOutcome: {
    written: 'écrit',
    completed: 'complété',
    alreadyPresent: 'déjà présent, identique',
    conflict: 'valeur différente — conservé tel quel',
    invalid: 'invalide — conservé tel quel',
    skipped: 'non écrit',
    needsDecision: 'non écrit : décision requise',
    notRead: 'déjà présent, non relu',
    imported: 'ajouté depuis ses fichiers de hash, non relu',
    pending: 'en attente des autres fichiers du dossier',
    failed: 'échec',
  },

  sourceOutcome: {
    match: 'conforme',
    mismatch: 'différent',
    invalid: 'invalide',
    notCompared: 'non comparé',
  },

  problem: {
    notFound: 'Fichier ou dossier introuvable.',
    permissionDenied: 'Accès refusé : droits insuffisants.',
    locked: 'Le fichier est utilisé par un autre programme. Fermez ce programme, puis recommencez.',
    modifiedDuringRead:
      'Le fichier a changé pendant sa lecture. Recommencez quand il ne sera plus modifié.',
    cancelled: 'Opération annulée.',
    diskFull: 'Espace disque insuffisant.',
    alreadyExists: 'Un fichier de ce nom existe déjà ; il n’a pas été remplacé.',
    nameNotRepresentable:
      'Ce nom de fichier contient des caractères invalides : il ne peut pas être enregistré fidèlement.',
    nameTooLong: 'Nom ou chemin trop long.',
    invalidName: 'Nom ou chemin invalide (caractères interdits ou syntaxe incorrecte).',
    notAFile: 'C’est un dossier, pas un fichier.',
    deviceUnavailable:
      'Le disque n’est plus accessible (débranché, éteint ou éjecté ?). Rebranchez-le, puis recommencez.',
    networkError:
      'La connexion au partage réseau a été perdue. Vérifiez le réseau, puis recommencez.',
    mediaError:
      'Le disque n’a pas pu lire certaines données (secteur illisible). C’est un signe possible de défaillance du support : sauvegardez ses données et vérifiez son état.',
    databaseInUse: 'La base est déjà ouverte par un autre FileSeal ou un autre programme.',
    databaseReadOnly: 'La base est en lecture seule.',
    databaseTooNew:
      'Cette base vient d’une version plus récente de FileSeal. Mettez FileSeal à jour pour l’ouvrir.',
    notAFileSealDatabase: 'Ce fichier n’est pas une base FileSeal.',
    databaseCorrupt:
      'Cette base est endommagée. Restaurez une copie de sauvegarde (FileSeal en garde une avant chaque mise à jour).',
    databaseUnavailable:
      'La base ne peut plus être lue ni écrite : son disque ou son partage réseau a disparu, ou présente des erreurs.',
    invalidHashFile: 'Ce fichier de hash est illisible ou ne contient aucune ligne valide.',
    locationTaken: 'Un autre fichier est déjà enregistré à cet emplacement dans la base.',
    staleVerification:
      'Le fichier ou sa référence a changé depuis la vérification. Vérifiez-le de nouveau, puis décidez.',
    contentDiffers: 'Le contenu de ce fichier n’est pas celui enregistré pour cet élément.',
    database: 'Erreur de la base de données.',
    busy: 'Une opération est déjà en cours.',
    io: 'Erreur d’entrée-sortie.',
  },

  searchStatus: {
    unknown: {
      label: 'Contenu inconnu',
      explanation: 'Aucune référence de la base n’a ce contenu.',
    },
    knownHere: {
      label: 'Connu ici',
      explanation: 'Ce fichier est enregistré à cet emplacement, avec ce contenu.',
    },
    changedHere: {
      label: 'Modifié depuis l’enregistrement',
      explanation:
        'Ce fichier est enregistré à cet emplacement, mais son contenu a changé. Vérifiez-le pour accepter la nouvelle version.',
    },
    locationRecognized: {
      label: 'Reconnu (lettre de lecteur changée)',
      explanation:
        'Ce fichier est enregistré au même endroit du même disque, sous une autre lettre de lecteur.',
    },
    probablyMoved: {
      label: 'Probablement déplacé ici',
      explanation: 'Un fichier connu avec ce contenu a disparu de son emplacement enregistré.',
    },
    copyOfKnown: {
      label: 'Copie d’un fichier connu',
      explanation:
        'Les fichiers connus avec ce contenu sont à leur place : celui-ci est une autre copie.',
    },
    ambiguous: {
      label: 'Plusieurs origines possibles',
      explanation:
        'Plusieurs fichiers connus avec ce contenu ont disparu : lequel a été déplacé ici ?',
    },
    notFound: { label: 'Fichier introuvable', explanation: 'Aucun fichier à cet emplacement.' },
    readError: {
      label: 'Erreur de lecture',
      explanation: 'Le fichier n’a pas pu être lu entièrement.',
    },
    locked: {
      label: 'Fichier verrouillé ou en cours d’utilisation',
      explanation: 'Un autre programme écrit dans ce fichier. Réessayez quand il sera fermé.',
    },
    modifiedDuringRead: {
      label: 'Modifié pendant le calcul',
      explanation: 'Le fichier a changé pendant sa lecture ; le résultat a été écarté.',
    },
    cancelled: { label: 'Annulé', explanation: 'La recherche a été interrompue.' },
    failed: { label: 'Échec', explanation: 'La recherche n’a pas pu être menée.' },
  },

  finding: {
    sameContent: 'Même contenu dans la base',
    knownElsewhere:
      'Aucune référence à cet emplacement, mais la base connaît ce contenu : voir ci-dessous.',
    sameName: 'Même nom, contenu différent (simple indice)',
    present: 'à sa place',
    missing: 'absent de son emplacement',
    volumeUnavailable: 'son disque n’est pas branché',
    attachMoved: 'Il a été déplacé ici : rattacher',
    attachThis: 'C’est celui-ci qui a été déplacé ici',
    attachDifferent: 'Rattacher quand même (renommé et modifié)',
    confirmAttachDifferent: (path: string) =>
      `Rattacher ce fichier à celui enregistré à « ${path} », bien que leurs contenus diffèrent ? Le nouveau contenu pourra ensuite être accepté comme nouvelle version.`,
    updateLocation: (from: string) => `Enregistré à « ${from} » : mettre à jour l’emplacement`,
    createAnyway: 'C’est une autre copie : créer une nouvelle référence',
    attached: (from: string) =>
      `Rattaché : ce fichier est désormais celui enregistré à « ${from} ».`,
    followed: (from: string) => `Emplacement suivi : il était enregistré à « ${from} ».`,
  },

  accept: {
    button: 'Accepter comme nouvelle version',
    confirm: (name: string) =>
      `Accepter le contenu actuel de « ${name} » comme nouvelle référence ? L’ancienne reste dans l’historique, et les fichiers de hash à côté sont mis à jour.`,
    done: 'Nouvelle version acceptée.',
    partly: 'Nouvelle version acceptée, mais pas partout : voir ci-dessous.',
  },

  confirm: { yes: 'Confirmer', no: 'Annuler' },

  close: {
    dismiss: 'Masquer',
    question:
      'Une opération est en cours. Quitter quand même ? Elle est d’abord arrêtée : ce qui est terminé est conservé, rien n’est laissé à moitié écrit.',
    quit: 'Arrêter et quitter',
    stay: 'Continuer',
  },

  report: {
    export: 'Exporter le rapport…',
    title: 'Exporter le rapport',
    filter: 'Fichier CSV',
    fileName: 'rapport-fileseal.csv',
    exported: (count: number, path: string) =>
      `Rapport exporté (${n(count)} ligne${count > 1 ? 's' : ''}) : ${path}`,
  },

  note: {
    legacyEncoding: 'Encodage ancien (Windows-1252).',
    nonStandardFormat: 'Format non standard (ancien FileSeal ou autre outil).',
    nameDiffers: (written: string) =>
      `Le nom inscrit dans le fichier de hash (« ${written} ») n’est pas celui du fichier.`,
    nameCaseDiffers: (written: string) =>
      `Le nom inscrit (« ${written} ») ne diffère que par les majuscules.`,
  },

  units: {
    // En français, 0 et 1 sont au singulier.
    bytes: (count: number, formatted: string) => `${formatted} ${count < 2 ? 'octet' : 'octets'}`,
    multiples: ['Ko', 'Mo', 'Go', 'To'],
    perSecond: '/s',
  },
};
