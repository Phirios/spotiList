use serde::Deserialize;
use serde_json::json;

use crate::error::{AppError, AppResult};

const SESSION: &str = "spoti-gsbpm";
const WARMUP_URL: &str = "https://getsongbpm.com/";
const REQUEST_TIMEOUT_MS: u64 = 60_000;

pub struct FlareSolverrClient {
    http: reqwest::Client,
    base_url: String,
}

#[derive(Debug, Deserialize)]
struct CmdResponse {
    status: String,
    #[serde(default)]
    message: Option<String>,
    #[serde(default)]
    solution: Option<Solution>,
}

#[derive(Debug, Deserialize)]
struct Solution {
    status: u16,
    response: String,
}

impl FlareSolverrClient {
    pub fn new(http: reqwest::Client, base_url: String) -> Self {
        Self { http, base_url }
    }

    pub async fn ensure_session(&self) -> AppResult<()> {
        // sessions.create is idempotent enough — if it already exists FS just
        // returns an error we can safely ignore.
        let _ = self
            .http
            .post(format!("{}/v1", self.base_url))
            .json(&json!({
                "cmd": "sessions.create",
                "session": SESSION,
            }))
            .send()
            .await?;

        // Warmup hit on the public site so CF's clearance cookies land in
        // the headless browser's jar; without this the API endpoint times out.
        let resp = self
            .http
            .post(format!("{}/v1", self.base_url))
            .json(&json!({
                "cmd": "request.get",
                "url": WARMUP_URL,
                "session": SESSION,
                "maxTimeout": REQUEST_TIMEOUT_MS,
            }))
            .send()
            .await?;
        let cmd: CmdResponse = resp.json().await?;
        if cmd.status != "ok" {
            return Err(AppError::Upstream(format!(
                "flaresolverr warmup failed: {}",
                cmd.message.unwrap_or_default()
            )));
        }
        Ok(())
    }

    /// Fetch a URL through the warmed CF-bypassing session. Returns the
    /// response body. JSON endpoints viewed in a browser are wrapped in
    /// `<pre>...</pre>` — we strip that automatically when present.
    pub async fn get(&self, url: &str) -> AppResult<String> {
        let resp = self
            .http
            .post(format!("{}/v1", self.base_url))
            .json(&json!({
                "cmd": "request.get",
                "url": url,
                "session": SESSION,
                "maxTimeout": REQUEST_TIMEOUT_MS,
            }))
            .send()
            .await?;
        let cmd: CmdResponse = resp.json().await?;
        if cmd.status != "ok" {
            return Err(AppError::Upstream(format!(
                "flaresolverr: {}",
                cmd.message.unwrap_or_default()
            )));
        }
        let sol = cmd
            .solution
            .ok_or_else(|| AppError::Upstream("flaresolverr no solution".into()))?;
        if sol.status != 200 {
            return Err(AppError::Upstream(format!(
                "flaresolverr inner status: {}",
                sol.status
            )));
        }
        Ok(strip_pre_wrapper(&sol.response).unwrap_or(sol.response))
    }
}

fn strip_pre_wrapper(s: &str) -> Option<String> {
    let start = s.find("<pre>")?;
    let end = s.find("</pre>")?;
    let inner = &s[start + 5..end];
    Some(inner.to_string())
}
