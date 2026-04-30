use serde::Deserialize;

const API_BASE: &str = "https://ws.audioscrobbler.com/2.0/";

pub struct LastFmClient {
    http: reqwest::Client,
    api_key: Option<String>,
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
    pub fn new(http: reqwest::Client, api_key: Option<String>) -> Self {
        Self { http, api_key }
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
