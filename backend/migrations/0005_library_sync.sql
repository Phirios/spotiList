-- Central per-track metadata cache. All other per-track tables now reference
-- this row by spotify_track_id. We populate it from the Spotify /me/tracks
-- response during a sync; later sync runs upsert in case anything changed.
CREATE TABLE IF NOT EXISTS tracks (
    spotify_track_id TEXT PRIMARY KEY,
    name             TEXT NOT NULL,
    artists          TEXT[] NOT NULL,
    album            TEXT NOT NULL,
    image_url        TEXT,
    duration_ms      INT,
    isrc             TEXT,
    fetched_at       TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Many-to-many of users → liked tracks. The cache backing for the user's
-- library so we don't re-paginate Spotify on every request.
CREATE TABLE IF NOT EXISTS user_liked_tracks (
    user_id          UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    spotify_track_id TEXT NOT NULL REFERENCES tracks(spotify_track_id),
    added_at         TIMESTAMPTZ,
    cached_at        TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (user_id, spotify_track_id)
);

CREATE INDEX IF NOT EXISTS user_liked_tracks_user_idx
    ON user_liked_tracks (user_id);

-- One row per user representing the current (or most recent) sync job state.
-- The server upserts on start, updates progress/stage as it works, and
-- finalizes status to 'done' or 'failed'.
CREATE TABLE IF NOT EXISTS sync_jobs (
    user_id     UUID PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    status      TEXT NOT NULL,
    stage       TEXT,
    progress    INT  NOT NULL DEFAULT 0,
    total       INT  NOT NULL DEFAULT 0,
    started_at  TIMESTAMPTZ,
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    finished_at TIMESTAMPTZ,
    error       TEXT
);

-- Lyrics cache so click-to-detail is instant after the first hit.
CREATE TABLE IF NOT EXISTS track_lyrics (
    spotify_track_id TEXT PRIMARY KEY REFERENCES tracks(spotify_track_id) ON DELETE CASCADE,
    plain            TEXT,
    synced           TEXT,
    instrumental     BOOLEAN NOT NULL DEFAULT false,
    looked_up_at     TIMESTAMPTZ NOT NULL DEFAULT now()
);
