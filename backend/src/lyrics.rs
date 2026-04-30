use serde::Deserialize;
use sqlx::Row;

use crate::db::Pool;
use crate::model::Lyrics;

const API_BASE: &str = "https://lrclib.net/api";

pub struct LrcLibClient {
    http: reqwest::Client,
    db: Pool,
}

#[derive(Debug, Deserialize)]
struct LrcResponse {
    #[serde(default)]
    instrumental: bool,
    #[serde(rename = "plainLyrics", default)]
    plain_lyrics: Option<String>,
    #[serde(rename = "syncedLyrics", default)]
    synced_lyrics: Option<String>,
}

impl LrcLibClient {
    pub fn new(http: reqwest::Client, db: Pool) -> Self {
        Self { http, db }
    }

    /// Cache-first lookup keyed by Spotify track id. Returns the cached row
    /// if present, otherwise falls back to LRCLIB and writes through.
    /// We store both hits and misses (a row with all-null content represents
    /// "we tried, no lyrics") so we don't keep retrying.
    pub async fn lookup_cached(
        &self,
        spotify_track_id: &str,
        artist: &str,
        track: &str,
        album: Option<&str>,
        duration_secs: u64,
    ) -> Option<Lyrics> {
        if let Ok(Some(cached)) = self.cached(spotify_track_id).await {
            return cached;
        }
        let fresh = self.lookup_upstream(artist, track, album, duration_secs).await;
        let _ = self.store(spotify_track_id, fresh.as_ref()).await;
        fresh
    }

    /// Direct LRCLIB lookup with no caching. Useful if there's no Spotify
    /// id to key by.
    pub async fn lookup_upstream(
        &self,
        artist: &str,
        track: &str,
        album: Option<&str>,
        duration_secs: u64,
    ) -> Option<Lyrics> {
        if let Some(l) = self.exact(artist, track, album, duration_secs).await {
            return Some(l);
        }
        self.search(artist, track).await
    }

    async fn exact(
        &self,
        artist: &str,
        track: &str,
        album: Option<&str>,
        duration_secs: u64,
    ) -> Option<Lyrics> {
        let mut params = vec![
            ("artist_name", artist.to_string()),
            ("track_name", track.to_string()),
            ("duration", duration_secs.to_string()),
        ];
        if let Some(a) = album {
            params.push(("album_name", a.to_string()));
        }

        let resp = self
            .http
            .get(format!("{API_BASE}/get"))
            .query(&params)
            .send()
            .await
            .ok()?;

        if !resp.status().is_success() {
            return None;
        }
        let parsed: LrcResponse = resp.json().await.ok()?;
        Some(into_lyrics(parsed))
    }

    async fn search(&self, artist: &str, track: &str) -> Option<Lyrics> {
        let resp = self
            .http
            .get(format!("{API_BASE}/search"))
            .query(&[("artist_name", artist), ("track_name", track)])
            .send()
            .await
            .ok()?;

        if !resp.status().is_success() {
            return None;
        }
        let mut hits: Vec<LrcResponse> = resp.json().await.ok()?;
        if hits.is_empty() {
            return None;
        }
        Some(into_lyrics(hits.remove(0)))
    }

    async fn cached(&self, id: &str) -> sqlx::Result<Option<Option<Lyrics>>> {
        let row = sqlx::query(
            "SELECT plain, synced, instrumental FROM track_lyrics WHERE spotify_track_id = $1",
        )
        .bind(id)
        .fetch_optional(&self.db)
        .await?;
        Ok(row.map(|r| {
            let plain: Option<String> = r.try_get("plain").ok().flatten();
            let synced: Option<String> = r.try_get("synced").ok().flatten();
            let instrumental: bool = r.try_get("instrumental").unwrap_or(false);
            if plain.is_none() && synced.is_none() && !instrumental {
                None
            } else {
                Some(Lyrics {
                    plain,
                    synced,
                    instrumental,
                    source: "lrclib",
                })
            }
        }))
    }

    async fn store(&self, id: &str, lyrics: Option<&Lyrics>) -> sqlx::Result<()> {
        let (plain, synced, instrumental) = match lyrics {
            Some(l) => (l.plain.as_deref(), l.synced.as_deref(), l.instrumental),
            None => (None, None, false),
        };
        sqlx::query(
            "INSERT INTO track_lyrics (spotify_track_id, plain, synced, instrumental, looked_up_at) \
             VALUES ($1, $2, $3, $4, now()) \
             ON CONFLICT (spotify_track_id) DO UPDATE SET \
                plain = EXCLUDED.plain, \
                synced = EXCLUDED.synced, \
                instrumental = EXCLUDED.instrumental, \
                looked_up_at = now()",
        )
        .bind(id)
        .bind(plain)
        .bind(synced)
        .bind(instrumental)
        .execute(&self.db)
        .await?;
        Ok(())
    }
}

fn into_lyrics(r: LrcResponse) -> Lyrics {
    Lyrics {
        plain: r.plain_lyrics.filter(|s| !s.is_empty()),
        synced: r.synced_lyrics.filter(|s| !s.is_empty()),
        instrumental: r.instrumental,
        source: "lrclib",
    }
}
