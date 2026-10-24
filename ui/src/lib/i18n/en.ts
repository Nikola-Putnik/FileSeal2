// English: the reference dictionary. Every other language must have the
// same keys (checked by TypeScript through the `Messages` type, and by
// i18n.test.ts). Sentences with values are functions.

/** A count with this language's digit grouping. */
const n = (value: number) => value.toLocaleString('en-GB');

export const en = {
  language: { label: 'Language', en: 'English', fr: 'Français' },
  theme: {
    label: 'Theme',
    system: 'As Windows (light or dark)',
    light: 'Light',
    dark: 'Dark',
    oled: 'Dark OLED (pure black)',
  },

  header: { preferences: 'Preferences', help: 'Help: keyboard shortcuts' },

  about: {
    version: (version: string) => `Version ${version}`,
    portable: 'portable',
    portableHint: 'Portable: settings and data are kept next to the program, nothing on this PC.',
  },

  database: {
    region: 'Active database',
    current: 'Database:',
    references: (count: number) => `${n(count)} reference${count === 1 ? '' : 's'}`,
    none: 'No database open',
    create: 'Create a database…',
    open: 'Open a database…',
    close: 'Close',
    createTitle: 'Create a FileSeal database',
    openTitle: 'Open a FileSeal database',
    fileFilter: 'FileSeal database',
    defaultFileName: 'My collection.fileseal',
    recent: 'Recent',
    noRecent: 'No recent database',
    notFound: '(not found)',
    locate: 'Locate…',
    forget: (name: string) => `Remove “${name}” from the list (the file is not deleted)`,
    readOnly: 'read-only',
    readOnlyHint:
      'This database cannot be written (read-only file, folder or disk): it can be consulted and used to verify, but nothing is recorded.',
    network: 'network share',
    networkHint:
      'This database is on a network share, where SQLite’s locking is not reliable. Keep databases on a local disk when possible.',
    openFailed: (name: string, reason: string) => `Could not open “${name}”: ${reason}`,
    reopenFailed: (name: string, reason: string) =>
      `The last database, “${name}”, could not be reopened: ${reason}`,
    migrated: (copy: string) =>
      `This database was updated for this version of FileSeal. A copy of it as it was before is kept: ${copy}`,
  },

  nav: { label: 'Screens', files: 'Files', databases: 'Databases' },

  dbScreen: {
    none: 'No database is open: create or open one with the buttons at the top.',
    active: 'Active database',
    rename: 'Rename…',
    renameLabel: 'Name shown',
    renamed: (name: string) => `The database is now called “${name}”.`,
    save: 'Save',
    cancel: 'Cancel',
    facts: (references: number, version: number) =>
      `${n(references)} reference${references === 1 ? '' : 's'} · schema version ${version}`,
    readOnly: 'Opened read-only: it can be consulted, not changed.',
    showInFolder: 'Show in folder',
    move: 'Move or rename the file…',
    moveTitle: 'Move or rename the database file',
    moved: (path: string) => `The database is now ${path}.`,
    backup: 'Back up…',
    backupTitle: 'Back up the database',
    backupName: (name: string, date: string) => `${name} backup ${date}.fileseal`,
    backedUp: (path: string, size: string) => `Backup written and checked: ${path} (${size}).`,
    restore: 'Restore from a backup…',
    restoreTitle: 'Choose a backup to restore',
    restoreConfirm: (backup: string, references: number, current: string) =>
      `Replace the content of “${current}” with this backup (“${backup}”, ${n(references)} reference${references === 1 ? '' : 's'})? What it holds now is kept as a copy next to it.`,
    restoreYes: 'Restore',
    restored: (copy: string) =>
      `Backup restored. What the database held before is kept in ${copy}.`,

    disks: 'Disks',
    disksHint:
      'Where the files of this database are now. A disk back under another drive letter is recognised without reading its files.',
    refresh: 'Refresh',
    disksEmpty: 'No file recorded yet.',
    disksGrid: 'Disks of the database',
    columns: {
      root: 'Recorded under',
      files: 'Files',
      state: 'Now',
      name: 'Name',
      folder: 'Folder',
      size: 'Size',
      recorded: 'Recorded',
      lastCheck: 'Last verification',
      retired: 'Removed on',
    },
    volume: {
      connected: 'Connected',
      movedTo: (root: string) => `Now at ${root}`,
      notConnected: 'Not connected',
      unidentified: 'Disk not identified',
      unidentifiedPresent: 'Disk not identified; the folder is there',
    },
    update: (root: string) => `Update to ${root}…`,
    confirmUpdate: (files: number, from: string, to: string) =>
      `Record the new place of ${n(files)} file${files === 1 ? '' : 's'}: ${from} → ${to}? Nothing is read; their former places stay in the history.`,
    updated: (moved: number, to: string) =>
      `${n(moved)} location${moved === 1 ? '' : 's'} updated to ${to}.`,
    reassign: 'Reassign…',
    reassignTitle: 'Reassign a folder',
    reassignOpen: 'Reassign a folder…',
    fileMenu: 'Database file',
    sections: 'Parts of the database',
    reassignHint:
      'For files moved or copied elsewhere together (another disk, a share, a renamed folder): their references follow them.',
    from: 'Files recorded under',
    to: 'are now under',
    choose: 'Choose…',
    chooseTitle: 'Where the files are now',
    includeMissing: 'Also those not found there (a disk not plugged in now)',
    preview: 'Preview',
    previewed: (files: number, found: number, missing: number) =>
      `${n(files)} file${files === 1 ? '' : 's'} recorded there: ${n(found)} found at the new place, ${n(missing)} not found.`,
    notFoundThere: 'not found there',
    apply: 'Apply…',
    confirmApply: (count: number, from: string, to: string) =>
      `Record the new place of ${n(count)} file${count === 1 ? '' : 's'}: ${from} → ${to}? Their former places stay in the history.`,
    reassigned: (moved: number, left: number, taken: number) =>
      `${n(moved)} moved${left > 0 ? `, ${n(left)} left where they were (not found)` : ''}${taken > 0 ? `, ${n(taken)} not moved: another file of the database is recorded there` : ''}.`,

    references: 'References',
    search: 'Search',
    searchHint: 'Part of a name or folder, or the start of a hash',
    shown: (shown: number, total: number) =>
      shown < total
        ? `${n(shown)} of ${n(total)} references shown: narrow the search.`
        : `${n(total)} reference${total === 1 ? '' : 's'}`,
    referencesGrid: 'References of the database',
    noReference: 'No reference matches.',
    never: 'never',
    listed: { label: 'Show', active: 'In the database', retired: 'Removed' },
    removeSelected: (count: number) => `Remove the selection from the database (${n(count)})…`,
    removeMatching: (count: number) =>
      count === 1
        ? 'Remove the result from the database…'
        : `Remove the ${n(count)} results from the database…`,
    confirmRemove: (count: number, name: string) =>
      `Remove ${n(count)} reference${count === 1 ? '' : 's'} from “${name}”? The files and their hash files are not touched. ${count === 1 ? 'It is' : 'They are'} no longer used to verify or to search, and can be put back from “Removed”.`,
    removed: (count: number) =>
      `${n(count)} reference${count === 1 ? '' : 's'} removed from the database.`,
    putBackSelected: (count: number) => `Put the selection back in the database (${n(count)})…`,
    putBackMatching: (count: number) =>
      count === 1
        ? 'Put the result back in the database…'
        : `Put the ${n(count)} results back in the database…`,
    confirmPutBack: (count: number) =>
      `Put ${n(count)} reference${count === 1 ? '' : 's'} back in the database, where ${count === 1 ? 'it was' : 'they were'}?`,
    putBack: (count: number, taken: number) =>
      `${n(count)} reference${count === 1 ? '' : 's'} put back in the database.${taken > 0 ? ` ${n(taken)} left removed: another file of the database is recorded at ${taken === 1 ? 'its place' : 'their places'}.` : ''}`,
    retiredOn: (date: string) => `Removed from the database on ${date}`,
    noRetired: 'No removed reference.',
    selectHint:
      'Several references: Shift or Ctrl with a click or the arrows, Ctrl+A for all those listed.',

    history: 'History',
    itemSince: (date: string) => `Followed since ${date}`,
    versions: 'Versions',
    origin: { created: 'created', accepted: 'accepted as a new version', imported: 'imported' },
    current: 'current',
    replacedOn: (date: string) => `replaced on ${date}`,
    locations: 'Locations',
    seen: (first: string, last: string) => `seen from ${first} to ${last}`,
    missingSince: (date: string) => `left on ${date}`,
    checks: 'Verifications',
    noChecks: 'Never verified.',
    moreChecks: 'Older verifications are not shown.',
  },

  action: { label: 'Action', create: 'Create a reference', verify: 'Verify', search: 'Search' },

  list: {
    title: 'Files',
    addFiles: 'Add files…',
    addFilesTitle: 'Add files',
    addFolder: 'Add a folder…',
    addFolderTitle: 'Add a folder',
    remove: 'Remove',
    moveUp: 'Move up',
    moveDown: 'Move down',
    sortBy: 'Sort by',
    sortKeys: { name: 'name', folder: 'folder', size: 'size', extension: 'extension' },
    clear: 'Clear the list',
    more: 'More actions',
    extensionsButton: (summary: string) => `Extensions: ${summary}`,
    extensionsAll: 'all',
    extensionsOnly: (list: string) => `only ${list}`,
    extensionsLeftOut: (count: number) => `${n(count)} left out`,
    stop: 'Stop',
    scanning: (files: number, folders: number) =>
      `Looking through the folders… ${n(files)} file${files === 1 ? '' : 's'} found in ${n(folders)} folder${folders === 1 ? '' : 's'}`,
    count: (files: number, size: string) =>
      `${n(files)} file${files === 1 ? '' : 's'} to process, ${size}`,
    leftOutByExtension: (files: number) => `${n(files)} more left out by the extension filter`,
    selected: (count: number) => `${n(count)} selected`,
    inFolder: (folder: string) => `in ${folder}`,
    filter: 'Filter by name',
    filterHint: 'Part of a name or folder',
    show: 'Show',
    showing: 'Show:',
    showAll: (count: number) => `All (${n(count)})`,
    tabs: {
      files: (count: number) => `Files (${n(count)})`,
      excluded: (count: number) => `Left out (${n(count)})`,
      problems: (count: number) => `Problems (${n(count)})`,
    },
    columns: {
      name: 'Name',
      folder: 'Folder',
      size: 'Size',
      state: 'State',
      reason: 'Reason',
      path: 'Path',
      problem: 'Problem',
    },
    grid: 'Files to process',
    excludedGrid: 'Files and folders left out',
    problemsGrid: 'Selection problems',
    empty: 'No file yet. Add files or folders, or drop them into the window.',
    emptyFiltered: 'No file matches the filters.',
    emptyExcluded: 'Nothing was left out.',
    emptyProblems: 'No problem.',
    ready: 'To process',
    next: 'Next',
    reading: (percent: number) => `Reading… ${percent}%`,
    listedFrom: (line: number, name: string) => `line ${line} of “${name}”`,
    folder: '(folder, not looked into)',
    lineProblem: (line: number, name: string, reason: string) =>
      `Line ${line} of “${name}”: ${reason}`,
    keyboard:
      'In the list: arrows to select, Shift+arrows to extend, Ctrl+arrows to move without selecting, Space to add or take out a file, Ctrl+A for all, Delete to remove, Alt+↑/↓ to move the selection up or down, Enter for the details.',
    notices: {
      expanded: (count: number, name: string) =>
        `${n(count)} file${count === 1 ? '' : 's'} listed in “${name}” added: each will be verified against its line in that file.`,
      hashFileAsFile: (name: string) =>
        `“${name}” is a hash file, added as an ordinary file. To verify the files it lists, choose Verify first, then add it.`,
      stopped: 'Looking through the folders was stopped: the list is incomplete.',
      rescanned:
        'The folders were looked through again with the new options: changes made by hand to the list were reset.',
    },
  },

  exclusion: {
    hidden: 'Hidden',
    system: 'System file',
    systemFolder: 'Windows system folder',
    hashFile: 'Hash file',
    fileSealFile: 'FileSeal database',
    temporaryFile: 'Temporary file',
    onlineOnly: 'Online only (not on this computer)',
    link: 'Link or junction (not followed)',
    special: 'Device, pipe or socket',
    duplicate: 'Already in the list',
  },

  selection: {
    title: 'Selection',
    subfolders: 'Include subfolders',
    hidden: 'Include hidden and system files',
    optionsHint: 'They apply to folders; changing them looks through the folders again.',
    parallel: 'Disks read at the same time',
    parallelHint: 'One reader per disk; several disks are read together.',
    extensions: 'Extensions',
    noExtension: '(none)',
    extension: (name: string, files: number, size: string) =>
      `${name} — ${n(files)} file${files === 1 ? '' : 's'}, ${size}`,
    allExtensions: 'All',
    only: 'Only these extensions',
    apply: 'Apply',
    onlyHint: 'e.g. mkv iso mp4; capitals or not',
    onlyActive: (list: string) =>
      `Only ${list}: the other extensions are left out, also in folders added afterwards.`,
  },

  settings: {
    title: 'Settings',
    algorithms: 'Algorithms',
    recommended: '(recommended)',
    md5Note: '(compatibility: detects accidental errors, does not protect against tampering)',
    destinations: 'Destinations',
    sidecarNextToFile: 'Hash file next to each file',
    activeDatabase: 'Active database',
    noDatabase: '(no database open)',
    readOnlyDatabase: '(read-only database)',
    referencesToUse: 'References to use',
    sidecarsNextToFile: 'Hash files next to each file',
    listedReferences: 'Files added from a hash file are verified against their line in it only.',
    summary: {
      none: 'none',
      hashFiles: 'hash files',
      database: 'database',
      existing: {
        skipComplete: 'referenced files skipped',
        skipAny: 'files with any reference skipped',
        read: 'referenced files read again',
      },
      references: (sources: string) => `references: ${sources}`,
      noSubfolders: 'without subfolders',
      hidden: 'hidden files included',
      followMoves: 'moves followed',
    },
    existing: {
      title: 'Files already referenced',
      skipComplete: 'Skip them, without reading, if they have every checked algorithm',
      skipAny: 'Skip them, without reading, if they have any reference (an old .md5 is enough)',
      read: 'Read them again: check their references and complete them',
      importHashFiles: 'Add their hash files to the active database, without reading',
      hint: 'A skipped file is not checked: that is what Verify does.',
    },
    follow: {
      title: 'Moved files',
      label: 'Follow them automatically',
      hint: 'A file found elsewhere than recorded (moved, renamed, drive letter changed) is attached to its reference without asking: the database keeps its last known location. A copy is never attached.',
    },
  },

  launch: {
    skipping: 'Files already referenced are not read.',
    importing:
      'Files already referenced are not read; the hash files of those not yet in the database are added to it.',
    region: 'Start',
    needFiles: 'Add files to the list first.',
    busyScanning: 'Wait until the folders are looked through.',
    needAlgorithm: 'Select at least one algorithm.',
    needDestination: 'Select at least one destination.',
    needSource: 'Select at least one source of references.',
    whereSidecar: 'hash files next to them',
    whereSidecars: 'their hash files',
    whereLists: 'the lines of the hash files added',
    whereDatabase: (name: string) => `the “${name}” database`,
    and: ' and ',
    files: (count: number) => `${n(count)} file${count === 1 ? '' : 's'}`,
    createSummary: (algorithms: string, files: string, size: string, where: string) =>
      `Create the ${algorithms} references of ${files} (${size}) in ${where}.`,
    verifySummary: (files: string, size: string, where: string) =>
      `Verify ${files} (${size}) against ${where}.`,
    searchSummary: (files: string, size: string, database: string) =>
      `Look ${files} (${size}) up in the “${database}” database.`,
    needDatabase: 'Open a database to search it.',
    search: 'Search',
    create: 'Create the references',
    verify: 'Verify',
    shortcut: 'Ctrl+Enter',
    pause: 'Pause',
    resume: 'Resume',
    cancel: 'Cancel',
    cancelling: 'Cancelling…',
  },

  progress: {
    title: 'Progress',
    paused: 'Paused: reading stops between two blocks and resumes where it was.',
    reading: 'Being read',
    readingList: (files: string) => `Reading: ${files}`,
    currentFileBar: 'Progress of the current file',
    overall: 'Overall:',
    overallBar: 'Overall progress',
    files: 'Files',
    filesValue: (done: number, total: number) => `${n(done)} of ${n(total)} done`,
    volume: 'Read',
    volumeValue: (done: string, total: string) => `${done} of ${total}`,
    rate: 'Throughput',
    measuring: 'measuring',
    elapsed: 'Elapsed',
    remaining: 'Remaining',
    estimating: 'estimating',
    percent: (value: number) => `${value}%`,
  },

  result: {
    title: 'Result',
    destinations: 'Destinations',
    hashFile: (algorithm: string) => `${algorithm} hash file:`,
    existingChecked: '(already there, checked)',
    database: (name: string) => `“${name}” database:`,
    existingValue: 'Existing value:',
    invalidDetail: (detail: string) => `Unreadable or malformed. Technical detail: ${detail}`,
    algorithmAdded: (algorithms: string) => `Algorithm added: ${algorithms}`,
    recordedReference: (size: string) => `Recorded reference${size}:`,
    computedDigests: 'Computed hashes',
    noDigest: 'None: the file was not read completely.',
    skippedConflict: 'not written: another reference is in conflict',
    skippedUnread: 'not written: the file was not read completely',
    skippedReferenced: 'not written: already referenced, not read',
    referencesUsed: 'References used',
    referenceOf: (date: string) => `Reference of ${date}`,
    expected: (algorithm: string) => `Expected ${algorithm}:`,
    sizeChanged: (before: string, now: string) => `Size changed: ${before} recorded, ${now} now.`,
    observedDigests: 'Observed hashes',
    notRecorded: 'The verification could not be added to the database history. ',
    technical: (detail: string) => `Technical detail: ${detail}`,
    listedFrom: (line: number, path: string) => `Line ${line} of the hash file ${path}`,
  },

  results: {
    title: 'Results',
    counts: (done: number, total: number) => `${n(done)} of ${n(total)} files done`,
    categories: (processed: number, failed: number, cancelled: number, notProcessed: number) =>
      `${n(processed)} processed, ${n(failed)} failed, ${cancelled > 0 ? `${n(cancelled)} cancelled, ` : ''}${n(notProcessed)} not processed`,
    stopped: 'Stopped by you: the files not started are “not processed”.',
    databaseLost: (reason: string) =>
      `The database became unavailable, so the batch stopped: ${reason} The files not started are “not processed”.`,
    volumesLost: (volumes: string) =>
      `No longer available during the batch: ${volumes}. Its remaining files are “not processed”; the other disks went on.`,
    forAll: 'For all the results',
    updateLocations: (count: number) =>
      `Update ${n(count)} location${count === 1 ? '' : 's'} (drive letter changed)…`,
    confirmUpdate: (count: number) =>
      `Record the new location of ${n(count)} file${count === 1 ? '' : 's'} found at the same place of the same disk under another drive letter?`,
    attachMoved: (count: number) => `Attach ${n(count)} moved file${count === 1 ? '' : 's'}…`,
    confirmAttach: (count: number) =>
      `Attach ${n(count)} file${count === 1 ? '' : 's'} to the known file whose content they have and which is missing from its location? Their former location stays in the history.`,
    acceptVersions: (count: number) => `Accept ${n(count)} new version${count === 1 ? '' : 's'}…`,
    confirmAccept: (count: number) =>
      count === 1
        ? 'Accept the current content of 1 file as its new reference? The previous reference stays in the history, and hash files next to it are updated.'
        : `Accept the current content of ${n(count)} files as their new references? The previous references stay in the history, and hash files next to them are updated.`,
    working: (done: number, total: number) => `Working… ${n(done)} of ${n(total)}`,
    decisionsDone: (done: number, failed: number) =>
      failed === 0
        ? `Done for ${n(done)} file${done === 1 ? '' : 's'}.`
        : `Done for ${n(done)} file${done === 1 ? '' : 's'}; ${n(failed)} failed: see their details.`,
    decided: {
      attached: 'Attached',
      updated: 'Location updated',
      accepted: 'New version accepted',
      created: 'New reference created',
      failed: 'Decision failed',
    },
    details: 'Details',
    closeDetails: 'Close the details',
    noActive: 'Select a file in the list to see its details here.',
    notRun: 'Not processed yet.',
    reveal: 'Show in folder',
    sameAs: (path: string) => `Same file as ${path}: read once.`,
  },

  skip: {
    cancelled: {
      label: 'Not processed',
      explanation: 'The batch was stopped before this file.',
    },
    volumeUnavailable: {
      label: 'Not processed (disk unavailable)',
      explanation: 'Its disk or network share disappeared during the batch.',
    },
    databaseUnavailable: {
      label: 'Not processed (database unavailable)',
      explanation: 'The database could no longer be written, so the batch stopped.',
    },
    duplicate: {
      label: 'Duplicate',
      explanation: 'The same physical file as another one of the list: read only once.',
    },
  },

  shortcuts: {
    title: 'Keyboard shortcuts',
    list: 'Ctrl+O: add files · Ctrl+Shift+O: add a folder · Ctrl+Enter: start',
  },

  createStatus: {
    created: {
      label: 'Reference created',
      explanation: 'The current content of the file is recorded.',
    },
    completed: {
      label: 'Reference completed',
      explanation: 'The existing reference was checked identical, then completed.',
    },
    alreadyReferenced: {
      label: 'Already referenced',
      explanation: 'An identical reference already existed; nothing was rewritten.',
    },
    skipped: {
      label: 'Already referenced (not read)',
      explanation:
        'It already had its references: it was not read, so they were not checked. Verify tells whether it changed.',
    },
    imported: {
      label: 'Added to the database (not read)',
      explanation:
        'Its existing hash files were recorded in the database, without reading the file. Verify tells whether it still matches them.',
    },
    conflict: {
      label: 'Conflict with an existing reference',
      explanation:
        'An existing reference does not match the current content, or cannot be read. Nothing was written or replaced.',
    },
    needsDecision: {
      label: 'Decision needed',
      explanation:
        'This content is a known file that seems to have moved here. Nothing was written to the database: say whether it moved, or is another copy.',
    },
    partialSuccess: {
      label: 'Partial success',
      explanation: 'At least one destination failed: see the details below.',
    },
    failed: { label: 'Failed', explanation: 'No reference could be recorded.' },
    cancelled: { label: 'Cancelled', explanation: 'Nothing was recorded.' },
  },

  verifyStatus: {
    match: {
      label: 'Matches the reference',
      explanation: 'The content is identical to the recorded one.',
    },
    mismatch: {
      label: 'Differs from the reference',
      explanation:
        'The content changed since the reference was created. This shows a modification, not necessarily a corruption.',
    },
    noReference: {
      label: 'No reference',
      explanation: 'No hash file and no database entry for this file.',
    },
    noCompatibleReference: {
      label: 'No compatible reference',
      explanation: 'The references found use an algorithm FileSeal cannot compute.',
    },
    conflict: {
      label: 'Contradictory references',
      explanation:
        'Some references match the file and others do not. FileSeal does not pick one: compare them below.',
    },
    invalidHashFile: {
      label: 'Invalid hash file',
      explanation:
        'The hash file, or its line for this file, is unreadable or malformed: nothing could be compared.',
    },
    notFound: {
      label: 'Expected file not found',
      explanation: 'No file at this location: deleted, moved or renamed?',
    },
    readError: { label: 'Read error', explanation: 'The file could not be read completely.' },
    locked: {
      label: 'File locked or in use',
      explanation: 'Another program is writing to this file. Try again once it is closed.',
    },
    modifiedDuringRead: {
      label: 'Modified while being read',
      explanation: 'The file changed while it was read; the result was discarded.',
    },
    cancelled: { label: 'Cancelled', explanation: 'The verification was interrupted.' },
    notProcessed: {
      label: 'Not processed',
      explanation: 'The operation was cancelled before this file.',
    },
    failed: { label: 'Failed', explanation: 'The verification could not be carried out.' },
  },

  writeOutcome: {
    written: 'written',
    completed: 'completed',
    alreadyPresent: 'already there, identical',
    conflict: 'different value — left as it was',
    invalid: 'invalid — left as it was',
    skipped: 'not written',
    needsDecision: 'not written: decision needed',
    notRead: 'already there, not read',
    imported: 'added from its hash files, not read',
    failed: 'failed',
  },

  sourceOutcome: {
    match: 'matches',
    mismatch: 'differs',
    invalid: 'invalid',
    notCompared: 'not compared',
  },

  problem: {
    notFound: 'File or folder not found.',
    permissionDenied: 'Access denied: insufficient permissions.',
    locked: 'The file is used by another program. Close that program, then try again.',
    modifiedDuringRead:
      'The file changed while it was read. Try again once it is no longer being modified.',
    cancelled: 'Operation cancelled.',
    diskFull: 'Not enough disk space.',
    alreadyExists: 'A file with this name already exists; it was not replaced.',
    nameNotRepresentable:
      'This file name contains invalid characters: it cannot be recorded faithfully.',
    nameTooLong: 'Name or path too long.',
    invalidName: 'Invalid name or path (forbidden characters or wrong syntax).',
    notAFile: 'This is a folder, not a file.',
    deviceUnavailable:
      'The disk can no longer be reached (unplugged, switched off or ejected?). Plug it back in, then try again.',
    networkError:
      'The connection to the network share was lost. Check the network, then try again.',
    mediaError:
      'The disk could not read some data (unreadable sector). This may be a sign of a failing disk: back up its data and check its health.',
    databaseInUse: 'The database is already open in another FileSeal or another program.',
    databaseReadOnly: 'The database is read-only.',
    databaseTooNew:
      'This database comes from a newer version of FileSeal. Update FileSeal to open it.',
    notAFileSealDatabase: 'This file is not a FileSeal database.',
    databaseCorrupt:
      'This database is damaged. Restore a backup copy (FileSeal keeps one before each update).',
    databaseUnavailable:
      'The database can no longer be read or written: its disk or network share is gone, or failing.',
    invalidHashFile: 'This hash file is unreadable or has no valid line.',
    locationTaken: 'Another file is already recorded at this location in the database.',
    staleVerification:
      'The file or its reference changed since it was checked. Verify it again, then decide.',
    contentDiffers: 'This file’s content is not the one recorded for that item.',
    database: 'Database error.',
    busy: 'An operation is already running.',
    io: 'Input/output error.',
  },

  searchStatus: {
    unknown: {
      label: 'Unknown content',
      explanation: 'No reference in the database has this content.',
    },
    knownHere: {
      label: 'Known here',
      explanation: 'This file is recorded at this location, with this content.',
    },
    changedHere: {
      label: 'Changed since recorded',
      explanation:
        'This file is recorded at this location, but its content changed. Verify it to accept the new version.',
    },
    locationRecognized: {
      label: 'Recognised (drive letter changed)',
      explanation:
        'This file is recorded at the same place on the same disk, under another drive letter.',
    },
    probablyMoved: {
      label: 'Probably moved here',
      explanation: 'A known file with this content is missing from its recorded location.',
    },
    copyOfKnown: {
      label: 'Copy of a known file',
      explanation: 'Known files with this content are where they should be: this is another copy.',
    },
    ambiguous: {
      label: 'Several possible origins',
      explanation: 'Several known files with this content are missing: which one moved here?',
    },
    notFound: { label: 'File not found', explanation: 'No file at this location.' },
    readError: { label: 'Read error', explanation: 'The file could not be read completely.' },
    locked: {
      label: 'File locked or in use',
      explanation: 'Another program is writing to this file. Try again once it is closed.',
    },
    modifiedDuringRead: {
      label: 'Modified while being read',
      explanation: 'The file changed while it was read; the result was discarded.',
    },
    cancelled: { label: 'Cancelled', explanation: 'The search was interrupted.' },
    failed: { label: 'Failed', explanation: 'The search could not be carried out.' },
  },

  finding: {
    sameContent: 'Same content in the database',
    knownElsewhere:
      'No reference at this location, but the database knows this content: see below.',
    sameName: 'Same name, different content (a hint only)',
    present: 'in place',
    missing: 'missing from its location',
    volumeUnavailable: 'its disk is not connected',
    attachMoved: 'It moved here: attach',
    attachThis: 'This one moved here',
    attachDifferent: 'Attach anyway (renamed and modified)',
    confirmAttachDifferent: (path: string) =>
      `Attach this file to the one recorded at “${path}”, although their contents differ? The new content can then be accepted as its new version.`,
    updateLocation: (from: string) => `Recorded at “${from}”: update the location`,
    createAnyway: 'It is another copy: create a new reference',
    attached: (from: string) => `Attached: this file is now the one recorded at “${from}”.`,
    followed: (from: string) => `Location followed: it was recorded at “${from}”.`,
  },

  accept: {
    button: 'Accept as the new version',
    confirm: (name: string) =>
      `Accept the current content of “${name}” as its new reference? The previous one stays in the history, and hash files next to it are updated.`,
    done: 'New version accepted.',
    partly: 'New version accepted, but not everywhere: see below.',
  },

  confirm: { yes: 'Confirm', no: 'Cancel' },

  close: {
    dismiss: 'Dismiss',
    question:
      'An operation is running. Quit anyway? It is stopped first: what is finished is kept, nothing half-written is left behind.',
    quit: 'Stop and quit',
    stay: 'Keep running',
  },

  report: {
    export: 'Export report…',
    title: 'Export the report',
    filter: 'CSV file',
    fileName: 'fileseal-report.csv',
    exported: (count: number, path: string) =>
      `Report exported (${n(count)} line${count === 1 ? '' : 's'}): ${path}`,
  },

  note: {
    legacyEncoding: 'Old encoding (Windows-1252).',
    nonStandardFormat: 'Non-standard format (older FileSeal or another tool).',
    nameDiffers: (written: string) =>
      `The name written in the hash file (“${written}”) is not the file's name.`,
    nameCaseDiffers: (written: string) =>
      `The name written (“${written}”) differs only in upper/lower case.`,
  },

  units: {
    bytes: (count: number, formatted: string) => `${formatted} ${count === 1 ? 'byte' : 'bytes'}`,
    multiples: ['KB', 'MB', 'GB', 'TB'],
    perSecond: '/s',
  },
};

export type Messages = typeof en;
