use serde::Deserialize;

use crate::model::Lyrics;

const API_BASE: &str = "https://lrclib.net/api";

pub struct LrcLibClient {
    http: reqwest::Client,
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
    pub fn new(http: reqwest::Client) -> Self {
        Self { http }
    }

    /// Try the exact `/get` endpoint first (fastest, deterministic),
    /// then fall back to `/search` if no exact match is found.
    pub async fn lookup(
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
}

fn into_lyrics(r: LrcResponse) -> Lyrics {
    Lyrics {
        plain: r.plain_lyrics.filter(|s| !s.is_empty()),
        synced: r.synced_lyrics.filter(|s| !s.is_empty()),
        instrumental: r.instrumental,
        source: "lrclib",
    }
}
