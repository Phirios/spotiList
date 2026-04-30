use serde::Deserialize;

const API_BASE: &str = "https://api.getsongbpm.com";

pub struct GetSongBpmClient {
    http: reqwest::Client,
    api_key: Option<String>,
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
    #[default]
    Empty,
}

#[derive(Debug, Deserialize)]
struct SearchHit {
    #[serde(default)]
    tempo: Option<serde_json::Value>,
}

impl GetSongBpmClient {
    pub fn new(http: reqwest::Client, api_key: Option<String>) -> Self {
        Self { http, api_key }
    }

    /// Look up tempo by artist + title. Returns None if no key is configured
    /// or no result is found. Errors from the upstream are swallowed and
    /// reported as None so the wider track lookup still succeeds.
    pub async fn lookup(&self, artist: &str, title: &str) -> Option<f64> {
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
            .await
            .ok()?;

        if !resp.status().is_success() {
            tracing::warn!(status = %resp.status(), "getsongbpm search failed");
            return None;
        }

        let parsed: SearchResponse = resp.json().await.ok()?;
        let hits = match parsed.search {
            SearchPayload::Hits(h) => h,
            SearchPayload::Empty => return None,
        };
        for hit in hits {
            if let Some(t) = hit.tempo {
                if let Some(n) = parse_tempo(&t) {
                    return Some(n);
                }
            }
        }
        None
    }
}

fn parse_tempo(v: &serde_json::Value) -> Option<f64> {
    match v {
        serde_json::Value::Number(n) => n.as_f64(),
        serde_json::Value::String(s) => s.trim().parse::<f64>().ok(),
        _ => None,
    }
}
