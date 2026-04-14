CREATE TABLE IF NOT EXISTS downloads (
    id          TEXT    PRIMARY KEY,
    url         TEXT    NOT NULL,
    file_path   TEXT    NOT NULL,
    total_bytes INTEGER,
    status      TEXT    NOT NULL DEFAULT 'queued', -- queued | active | paused | complete | failed
    created_at  INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS segments (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    download_id TEXT    NOT NULL REFERENCES downloads(id) ON DELETE CASCADE,
    start_byte  INTEGER NOT NULL,
    end_byte    INTEGER NOT NULL,
    completed   INTEGER NOT NULL DEFAULT 0
);
