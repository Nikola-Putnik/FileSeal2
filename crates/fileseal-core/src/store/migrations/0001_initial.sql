-- FileSeal database, schema v1. Frozen on 2026-10-22: later changes go in
-- new migration files, never in this one.
-- Model and rules: docs/modele-donnees.md.

CREATE TABLE meta (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
) STRICT;

-- One tracked physical copy of a file, followed over time. Its identity is
-- this row, never its path: a moved file keeps its item.
CREATE TABLE item (
    id         INTEGER PRIMARY KEY,
    uuid       TEXT NOT NULL UNIQUE,
    created_at TEXT NOT NULL,
    retired_at TEXT,
    note       TEXT
) STRICT;

-- One run of an action (create, verify…), for history and reports.
CREATE TABLE operation (
    id           INTEGER PRIMARY KEY,
    kind         TEXT NOT NULL CHECK (kind IN ('create', 'verify', 'search', 'accept', 'attach')),
    started_at   TEXT NOT NULL,
    finished_at  TEXT,
    app_version  TEXT NOT NULL,
    options_json TEXT,
    summary_json TEXT,
    status       TEXT NOT NULL DEFAULT 'running'
                 CHECK (status IN ('running', 'completed', 'cancelled', 'failed'))
) STRICT;

-- Expected content of an item. Exactly one current reference per item; an
-- accepted new version supersedes the previous one, which stays as history.
CREATE TABLE reference (
    id                            INTEGER PRIMARY KEY,
    item_id                       INTEGER NOT NULL REFERENCES item (id),
    size                          INTEGER NOT NULL CHECK (size >= 0),
    created_at                    TEXT NOT NULL,
    origin                        TEXT NOT NULL CHECK (origin IN ('created', 'accepted', 'imported')),
    accepted_from_verification_id INTEGER REFERENCES verification (id),
    superseded_at                 TEXT,
    comment                       TEXT
) STRICT;
CREATE UNIQUE INDEX reference_current ON reference (item_id) WHERE superseded_at IS NULL;
CREATE INDEX reference_size ON reference (size) WHERE superseded_at IS NULL;

-- No CHECK on the algorithm name: a newer version may add algorithms
-- without rebuilding this table.
CREATE TABLE reference_hash (
    reference_id INTEGER NOT NULL REFERENCES reference (id),
    algorithm    TEXT NOT NULL,
    hex          TEXT NOT NULL CHECK (length(hex) BETWEEN 32 AND 128 AND hex NOT GLOB '*[^0-9a-f]*'),
    PRIMARY KEY (reference_id, algorithm)
) STRICT, WITHOUT ROWID;
CREATE INDEX reference_hash_value ON reference_hash (algorithm, hex);

-- Places where an item was seen: hints to find it again, never proof of
-- identity. At most one current location per item, and one current item
-- per path.
CREATE TABLE location (
    id                   INTEGER PRIMARY KEY,
    item_id              INTEGER NOT NULL REFERENCES item (id),
    path                 TEXT NOT NULL,
    path_key             TEXT NOT NULL,
    file_name            TEXT NOT NULL,
    volume_serial        TEXT,
    volume_relative_path TEXT,
    volume_label         TEXT,
    -- Size and last write time when last seen there: hints to recognise a
    -- file again (never proof: only a hash is).
    last_size            INTEGER,
    last_mtime           TEXT,
    first_seen_at        TEXT NOT NULL,
    last_seen_at         TEXT NOT NULL,
    missing_since        TEXT,
    is_current           INTEGER NOT NULL DEFAULT 1 CHECK (is_current IN (0, 1))
) STRICT;
CREATE UNIQUE INDEX location_current_item ON location (item_id) WHERE is_current = 1;
CREATE UNIQUE INDEX location_current_path ON location (path_key) WHERE is_current = 1;
CREATE INDEX location_volume ON location (volume_serial, volume_relative_path) WHERE is_current = 1;
CREATE INDEX location_file_name ON location (file_name COLLATE NOCASE);

-- Every verification, with the hashes actually observed: a "different"
-- result can later be accepted as the new reference without reading again.
CREATE TABLE verification (
    id             INTEGER PRIMARY KEY,
    operation_id   INTEGER NOT NULL REFERENCES operation (id),
    item_id        INTEGER REFERENCES item (id),
    reference_id   INTEGER REFERENCES reference (id),
    observed_path  TEXT NOT NULL,
    observed_size  INTEGER,
    observed_mtime TEXT,
    started_at     TEXT NOT NULL,
    finished_at    TEXT NOT NULL,
    outcome        TEXT NOT NULL,
    detail         TEXT
) STRICT;
CREATE INDEX verification_item ON verification (item_id, finished_at);

CREATE TABLE verification_hash (
    verification_id INTEGER NOT NULL REFERENCES verification (id),
    algorithm       TEXT NOT NULL,
    hex             TEXT NOT NULL,
    PRIMARY KEY (verification_id, algorithm)
) STRICT, WITHOUT ROWID;
