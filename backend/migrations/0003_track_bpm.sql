CREATE TABLE IF NOT EXISTS track_bpm (
    spotify_track_id TEXT PRIMARY KEY,
    tempo            REAL,
    looked_up_at     TIMESTAMPTZ NOT NULL DEFAULT now()
);
