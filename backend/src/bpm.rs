use std::sync::Arc;

use serde::Deserialize;

use crate::flaresolverr::FlareSolverrClient;

const API_BASE: &str = "https://api.getsongbpm.com";

pub struct GetSongBpmClient {
    api_key: Option<String>,
    flaresolverr: Option<Arc<FlareSolverrClient>>,
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
    pub fn new(
        api_key: Option<String>,
        flaresolverr: Option<Arc<FlareSolverrClient>>,
    ) -> Self {
        Self {
            api_key,
            flaresolverr,
        }
    }

    /// Look up tempo by artist + title. Returns None if no key is configured,
    /// no result is found, or an upstream error occurs (errors are logged but
    /// swallowed so a single missing BPM doesn't break the wider track lookup).
    pub async fn lookup(&self, artist: &str, title: &str) -> Option<f64> {
        let key = self.api_key.as_ref()?;
        let fs = self.flaresolverr.as_ref()?;

        let lookup = format!("song:{} artist:{}", title, artist);
        let url = format!(
            "{API_BASE}/search/?api_key={}&type=both&lookup={}",
            key,
            urlenc(&lookup),
        );

        let body = match fs.get(&url).await {
            Ok(b) => b,
            Err(e) => {
                tracing::warn!(error = %e, "getsongbpm via flaresolverr failed");
                return None;
            }
        };

        let parsed: SearchResponse = match serde_json::from_str(&body) {
            Ok(p) => p,
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    snippet = %body.chars().take(200).collect::<String>(),
                    "getsongbpm parse failed"
                );
                return None;
            }
        };

        let hits = match parsed.search {
            SearchPayload::Hits(h) => h,
            SearchPayload::Error { .. } | SearchPayload::Empty => return None,
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

fn urlenc(s: &str) -> String {
    url::form_urlencoded::byte_serialize(s.as_bytes()).collect()
}
