-- Horodatages : millisecondes UTC (INTEGER). Identifiants : UUID v7 (TEXT).
-- created_at / updated_at / deleted_at préparent une future synchronisation.

CREATE TABLE projects (
    id           TEXT PRIMARY KEY NOT NULL,
    name         TEXT NOT NULL,
    client       TEXT NOT NULL DEFAULT '',
    color        TEXT NOT NULL,
    archived     INTEGER NOT NULL DEFAULT 0,
    last_used_at INTEGER,
    created_at   INTEGER NOT NULL,
    updated_at   INTEGER NOT NULL,
    deleted_at   INTEGER
);

CREATE UNIQUE INDEX ux_projects_client_name ON projects(client, name) WHERE deleted_at IS NULL;

CREATE TABLE time_entries (
    id         TEXT PRIMARY KEY NOT NULL,
    project_id TEXT NOT NULL REFERENCES projects(id),
    started_at INTEGER NOT NULL,
    ended_at   INTEGER,                 -- NULL = timer en cours
    note       TEXT NOT NULL DEFAULT '',
    source     TEXT NOT NULL,           -- timer | manual | activity | idle
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    deleted_at INTEGER,
    CHECK (ended_at IS NULL OR ended_at > started_at)
);

-- Un seul timer actif, garanti par la base : toutes les lignes indexées valent 1.
CREATE UNIQUE INDEX ux_one_running ON time_entries((ended_at IS NULL))
    WHERE ended_at IS NULL AND deleted_at IS NULL;
CREATE INDEX ix_entries_started_at ON time_entries(started_at);
CREATE INDEX ix_entries_project ON time_entries(project_id);

CREATE TABLE settings (
    key   TEXT PRIMARY KEY NOT NULL,
    value TEXT NOT NULL                 -- JSON
);
