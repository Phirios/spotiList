CREATE TABLE IF NOT EXISTS track_youtube (
    spotify_track_id TEXT PRIMARY KEY,
    video_id         TEXT,
    video_title      TEXT,
    video_channel    TEXT,
    match_score      REAL,
    matched_at       TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS track_comments (
    video_id     TEXT PRIMARY KEY,
    comments     JSONB NOT NULL,
    fetched_at   TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS track_emotion (
    spotify_track_id TEXT PRIMARY KEY,
    joy              REAL NOT NULL,
    sadness          REAL NOT NULL,
    anger            REAL NOT NULL,
    fear             REAL NOT NULL,
    surprise         REAL NOT NULL,
    disgust          REAL NOT NULL,
    neutral          REAL NOT NULL,
    comment_count    INTEGER NOT NULL,
    computed_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);
