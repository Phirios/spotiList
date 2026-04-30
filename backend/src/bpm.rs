use serde::Deserialize;
use sqlx::Row;

use crate::db::Pool;

const API_BASE: &str = "https://api.getsong.co";

pub struct GetSongBpmClient {
    http: reqwest::Client,
    api_key: Option<String>,
    db: Pool,
}

#[derive(Debug, Deserialize)]
struct SearchResponse {
    #[serde(default)]
    search: SearchPayload,
}

#[derive(Debug, Default, Deserialize)]
#[serde(untagged)]
enum SearchPayload {
    Hits(Vec<SearchHit>),
    Error {
        #[allow(dead_code)]
        error: String,
    },
    #[default]
    Empty,
}

#[derive(Debug, Deserialize)]
struct SearchHit {
    #[serde(default)]
    tempo: Option<serde_json::Value>,
}

impl GetSongBpmClient {
    pub fn new(http: reqwest::Client, api_key: Option<String>, db: Pool) -> Self {
        Self { http, api_key, db }
    }

    /// Look up tempo for a Spotify track. Caches both hits and misses by
    /// `spotify_track_id`, so we never re-query the upstream for the same
    /// track (matters because the API has a 3000 req/hour limit).
    pub async fn lookup(
        &self,
        spotify_track_id: &str,
        artist: &str,
        title: &str,
    ) -> Option<f64> {
        if let Ok(Some(cached)) = self.cached(spotify_track_id).await {
            return cached;
        }
        let key = self.api_key.as_ref()?;
        let lookup = format!("song:{} artist:{}", title, artist);
        let resp = self
            .http
            .get(format!("{API_BASE}/search/"))
            .query(&[
                ("api_key", key.as_str()),
                ("type", "both"),
                ("lookup", lookup.as_str()),
            ])
            .send()
            .await;

        let resp = match resp {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!(error = %e, "getsongbpm request failed");
                return None;
            }
        };
        if !resp.status().is_success() {
            tracing::warn!(status = %resp.status(), "getsongbpm non-success status");
            return None;
        }

        let body = match resp.text().await {
            Ok(b) => b,
            Err(_) => return None,
        };
        let parsed: SearchResponse = match serde_json::from_str(&body) {
            Ok(p) => p,
            Err(e) => {
                tracing::warn!(error = %e, snippet = %body.chars().take(200).collect::<String>(), "getsongbpm parse failed");
                return None;
            }
        };

        let tempo = match parsed.search {
            SearchPayload::Hits(hits) => hits
                .into_iter()
                .find_map(|h| h.tempo.as_ref().and_then(parse_tempo)),
            SearchPayload::Error { .. } | SearchPayload::Empty => None,
        };

        // Cache the result (even None, so we don't keep retrying)
        let _ = self.store(spotify_track_id, tempo).await;
        tempo
    }

    async fn cached(&self, id: &str) -> sqlx::Result<Option<Option<f64>>> {
        let row = sqlx::query("SELECT tempo FROM track_bpm WHERE spotify_track_id = $1")
            .bind(id)
            .fetch_optional(&self.db)
            .await?;
        Ok(row.map(|r| {
            let v: Option<f32> = r.try_get("tempo").ok().flatten();
            v.map(|x| x as f64)
        }))
    }

    async fn store(&self, id: &str, tempo: Option<f64>) -> sqlx::Result<()> {
        sqlx::query(
            "INSERT INTO track_bpm (spotify_track_id, tempo, looked_up_at) \
             VALUES ($1, $2, now()) \
             ON CONFLICT (spotify_track_id) DO UPDATE SET \
                tempo = EXCLUDED.tempo, \
                looked_up_at = now()",
        )
        .bind(id)
        .bind(tempo.map(|t| t as f32))
        .execute(&self.db)
        .await?;
        Ok(())
    }
}

fn parse_tempo(v: &serde_json::Value) -> Option<f64> {
    match v {
        serde_json::Value::Number(n) => n.as_f64(),
        serde_json::Value::String(s) => s.trim().parse::<f64>().ok(),
        _ => None,
    }
}
