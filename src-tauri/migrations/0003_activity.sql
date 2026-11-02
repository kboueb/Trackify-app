-- Suivi d'activité : fenêtre au premier plan, échantillonnée toutes les ~10 s
-- puis fusionnée en segments. Purgé au-delà de 30 jours ; jamais exporté.
CREATE TABLE activity_segments (
    id         TEXT PRIMARY KEY,
    app        TEXT NOT NULL,
    title      TEXT NOT NULL DEFAULT '',
    started_at INTEGER NOT NULL,
    ended_at   INTEGER NOT NULL,
    CHECK (ended_at >= started_at)
);

CREATE INDEX ix_activity_started ON activity_segments(started_at);
