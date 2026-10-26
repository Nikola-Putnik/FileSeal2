# Changelog

## Unreleased

- **One hash list per folder** (`SHA256SUMS`, `MD5SUMS`): a new destination when creating references. Each folder gets one list naming its files, checkable with `sha256sum -c SHA256SUMS` from that folder. Lines already there are never changed; new ones are added. Verifying also reads the folder's list, and accepting a new version updates its line.
- **Fix**: after a run, the number of references shown for the active database sometimes stayed as it was before the run.
- **Third-party notices**: the licenses of the open-source code FileSeal contains come with the program (THIRD-PARTY-NOTICES.txt, installed beside it and in the portable zip), with its own MIT license (LICENSE.txt).

## 2.5.0 — 2026-10-24

- **Sort the references of a database**: click a column header on the Databases screen (name, folder, size, date recorded, last verification); click again to reverse. The sort applies to the whole database, not only to the 2,000 rows shown, and takes less than a quarter of a second for 100,000 references.
- **"Clear the list"** no longer asks for a confirmation: nothing is deleted from the disk.

## 2.4.0 — 2026-10-24

- **Themes** (⚙): as Windows (the default), light, dark, or dark OLED (pure black). Contrast checked for each; lists, check boxes, fields and the title bar follow the theme.
- **Follow moved files automatically** (Settings, for Create and Verify): a file moved, renamed or found under another drive letter is attached to its reference without asking, then verified or completed. The database keeps its last known location, and the former one stays in the history. A copy is never attached. Off by default.
- **"Export the report…"** sits beside the summary it exports, away from the start button. A line sets the results apart from the start line.
- **"Clear the list"** sits beside "Remove", instead of the ⋯ menu.
- **A quieter detail pane**: on a narrow window, where it opens below the list, a click only selects a row. The detail opens when asked for (double-click, Enter, or the "Details" button). The same for the history on the Databases screen.
- **"Show:" chips** count the rows still in the list, live (during a run, after a removal, after a decision), and disappear when no row has a result any more.

## 2.3.0 — 2026-10-23

- **New layout**: the window no longer scrolls. The list takes the whole height, the detail opens beside it (below on a narrow window), and a bar at the bottom starts, follows and concludes a run.
- **Settings** folded behind a button that sums them up, and kept from one session to the next.
- **Header**: the active database in one button, help (keyboard shortcuts) and preferences (language). Rare list actions are in a ⋯ menu, the extension filter in a panel.
- **Databases screen**: References and Disks tabs, history in a side pane, "Database file" menu.
- **Fix**: in high contrast mode, the text of the selected row was invisible.

## 2.2.0 — 2026-10-23

- **Remove references from a database**: a selection, or every result of a search. Nothing is erased: the history stays, and a "Removed" view puts them back.

## 2.1.0 — 2026-10-22

- **Create no longer reads files that already have references**: either when they have every algorithm checked, or any reference at all. Their hash files can be added to the database without reading them.
- **"Only these extensions"**: a typed list, in any case.
- **Readability**: long names cut in the middle, full-width page, taller list.

## 2.0.0 — 2026-10-22

First version:

- create, verify and search references; accept a new version; attach a moved file;
- hash files compatible with GNU `sha256sum` and `md5sum`, and reading of the older `MD5: …` format;
- several SQLite databases, with history;
- runs over several disks, with pause, cancel and a CSV report;
- Databases screen: disks, drive letter changes, reassigning a folder, backup and restore;
- per-user installer without administrator rights, and a portable version.
