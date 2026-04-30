use axum::extract::{Query, State};
use axum::routing::get;
use axum::{Json, Router};
use axum_extra::extract::cookie::PrivateCookieJar;
use serde::Deserialize;

use crate::auth::{current_user, ensure_fresh_token};
use crate::error::{AppError, AppResult};
use crate::AppState;

const SPOTIFY_API: &str = "https://api.spotify.com/v1";

pub fn router() -> Router<AppState> {
    Router::new().route("/me/liked", get(liked))
}

#[derive(Debug, Deserialize)]
pub struct LikedQuery {
    #[serde(default)]
    pub limit: Option<u32>,
    #[serde(default)]
    pub offset: Option<u32>,
}

async fn liked(
    State(state): State<AppState>,
    jar: PrivateCookieJar,
    Query(q): Query<LikedQuery>,
) -> AppResult<Json<serde_json::Value>> {
    let mut user = current_user(&state, &jar).await?;
    let token = ensure_fresh_token(&state, &mut user).await?;

    let limit = q.limit.unwrap_or(50).min(50);
    let offset = q.offset.unwrap_or(0);

    let resp = state
        .http
        .get(format!("{SPOTIFY_API}/me/tracks"))
        .bearer_auth(token)
        .query(&[("limit", limit.to_string()), ("offset", offset.to_string())])
        .send()
        .await?;

    if !resp.status().is_success() {
        let s = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(AppError::Upstream(format!(
            "spotify /me/tracks failed: {s}: {body}"
        )));
    }

    let body: serde_json::Value = resp.json().await?;
    Ok(Json(body))
}
