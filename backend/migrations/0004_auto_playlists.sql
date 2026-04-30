CREATE TABLE IF NOT EXISTS auto_playlists (
    id                    UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id               UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    name                  TEXT NOT NULL,
    description           TEXT,
    cluster_index         INT NOT NULL,
    track_count           INT NOT NULL,
    spotify_playlist_id   TEXT,
    created_at            TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS auto_playlists_user_idx
    ON auto_playlists (user_id, created_at DESC);

CREATE TABLE IF NOT EXISTS auto_playlist_tracks (
    auto_playlist_id  UUID NOT NULL REFERENCES auto_playlists(id) ON DELETE CASCADE,
    position          INT NOT NULL,
    spotify_track_id  TEXT NOT NULL,
    name              TEXT NOT NULL,
    artists           TEXT[] NOT NULL,
    album             TEXT NOT NULL,
    image_url         TEXT,
    PRIMARY KEY (auto_playlist_id, position)
);

CREATE INDEX IF NOT EXISTS auto_playlist_tracks_track_idx
    ON auto_playlist_tracks (spotify_track_id);

-- Track-level tag cache so renaming/regen can read tags without refetching.
CREATE TABLE IF NOT EXISTS track_tags (
    spotify_track_id  TEXT PRIMARY KEY,
    tags              TEXT[] NOT NULL,
    looked_up_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);
