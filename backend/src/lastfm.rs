use serde::Deserialize;
use sqlx::Row;

use crate::db::Pool;

const API_BASE: &str = "https://ws.audioscrobbler.com/2.0/";

pub struct LastFmClient {
    http: reqwest::Client,
    api_key: Option<String>,
    db: Pool,
}

#[derive(Debug, Deserialize)]
struct TopTagsResponse {
    #[serde(default)]
    toptags: Option<TopTags>,
}

#[derive(Debug, Deserialize)]
struct TopTags {
    #[serde(default)]
    tag: Vec<Tag>,
}

#[derive(Debug, Deserialize)]
struct Tag {
    name: String,
    #[serde(default)]
    count: serde_json::Value,
}

impl LastFmClient {
    pub fn new(http: reqwest::Client, api_key: Option<String>, db: Pool) -> Self {
        Self { http, api_key, db }
    }

    /// Cache-first variant of `clean_tags` keyed by Spotify track id.
    /// Reads from `track_tags` if present, otherwise fetches and stores.
    pub async fn clean_tags_cached(
        &self,
        spotify_track_id: &str,
        artist: &str,
        track: &str,
    ) -> Vec<String> {
        if let Ok(Some(cached)) = self.cached(spotify_track_id).await {
            return cached;
        }
        let fresh = self.clean_tags(artist, track).await;
        let _ = self.store(spotify_track_id, &fresh).await;
        fresh
    }

    async fn cached(&self, id: &str) -> sqlx::Result<Option<Vec<String>>> {
        let row = sqlx::query("SELECT tags FROM track_tags WHERE spotify_track_id = $1")
            .bind(id)
            .fetch_optional(&self.db)
            .await?;
        Ok(row.and_then(|r| r.try_get::<Vec<String>, _>("tags").ok()))
    }

    async fn store(&self, id: &str, tags: &[String]) -> sqlx::Result<()> {
        sqlx::query(
            "INSERT INTO track_tags (spotify_track_id, tags, looked_up_at) \
             VALUES ($1, $2, now()) \
             ON CONFLICT (spotify_track_id) DO UPDATE SET \
                tags = EXCLUDED.tags, \
                looked_up_at = now()",
        )
        .bind(id)
        .bind(tags)
        .execute(&self.db)
        .await?;
        Ok(())
    }

    /// Returns top tags filtered to those with non-zero count, capped to 10.
    /// Returns empty Vec when no key is set or upstream errors out.
    pub async fn top_tags(&self, artist: &str, track: &str) -> Vec<String> {
        let Some(key) = self.api_key.as_ref() else {
            return vec![];
        };

        let resp = self
            .http
            .get(API_BASE)
            .query(&[
                ("method", "track.gettoptags"),
                ("artist", artist),
                ("track", track),
                ("api_key", key.as_str()),
                ("autocorrect", "1"),
                ("format", "json"),
            ])
            .send()
            .await;

        let resp = match resp {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!(error = %e, "lastfm request failed");
                return vec![];
            }
        };

        if !resp.status().is_success() {
            return vec![];
        }

        let parsed: TopTagsResponse = match resp.json().await {
            Ok(p) => p,
            Err(_) => return vec![],
        };

        let Some(top) = parsed.toptags else {
            return vec![];
        };
        top.tag
            .into_iter()
            .filter(|t| count_value(&t.count) > 0)
            .take(10)
            .map(|t| t.name)
            .collect()
    }

    /// Same as `top_tags` but with noise filtered: year-only tags (e.g. "2017"),
    /// decade tags ("2010s", "90s"), tags case-insensitively equal to the
    /// artist or track name. Useful for building stable embedding inputs.
    pub async fn clean_tags(&self, artist: &str, track: &str) -> Vec<String> {
        let raw = self.top_tags(artist, track).await;
        let artist_l = artist.to_lowercase();
        let track_l = track.to_lowercase();
        raw.into_iter()
            .filter(|t| !is_noise(t, &artist_l, &track_l))
            .collect()
    }
}

fn is_noise(tag: &str, artist_l: &str, track_l: &str) -> bool {
    let t = tag.trim().to_lowercase();
    if t.is_empty() {
        return true;
    }
    if t == *artist_l || t == *track_l {
        return true;
    }
    // 4-digit year, e.g. "2017"
    if t.len() == 4 && t.chars().all(|c| c.is_ascii_digit()) {
        return true;
    }
    // decade tags: "2010s", "90s", "1970s"
    if (t.ends_with('s')
        && t[..t.len() - 1].chars().all(|c| c.is_ascii_digit())
        && (t.len() == 3 || t.len() == 5))
        || t == "favourites"
        || t == "favorites"
        || t == "seen live"
    {
        return true;
    }
    false
}

fn count_value(v: &serde_json::Value) -> u64 {
    match v {
        serde_json::Value::Number(n) => n.as_u64().unwrap_or(0),
        serde_json::Value::String(s) => s.parse().unwrap_or(0),
        _ => 0,
    }
}
