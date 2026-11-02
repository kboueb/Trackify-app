-- Absences détectées pendant qu'un timer tournait, en attente de décision.
CREATE TABLE idle_periods (
    id          TEXT PRIMARY KEY NOT NULL,
    entry_id    TEXT NOT NULL REFERENCES time_entries(id),
    kind        TEXT NOT NULL,          -- idle (inactivité, veille) | closed (app fermée)
    started_at  INTEGER NOT NULL,
    ended_at    INTEGER NOT NULL,
    resolution  TEXT,                   -- NULL = en attente | keep | discard | reassign
    created_at  INTEGER NOT NULL,
    resolved_at INTEGER,
    CHECK (ended_at > started_at)
);

CREATE INDEX ix_idle_pending ON idle_periods(started_at) WHERE resolution IS NULL;
