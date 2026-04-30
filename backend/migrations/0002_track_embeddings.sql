CREATE TABLE IF NOT EXISTS track_embeddings (
    spotify_track_id TEXT PRIMARY KEY,
    track_text       TEXT NOT NULL,
    embedding        REAL[] NOT NULL,
    model            TEXT NOT NULL,
    updated_at       TIMESTAMPTZ NOT NULL DEFAULT now()
);
