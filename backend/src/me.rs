use axum::extract::{Query, State};
use axum::routing::get;
use axum::{Json, Router};
use axum_extra::extract::cookie::PrivateCookieJar;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::Row;

use crate::auth::current_user;
use crate::error::{AppError, AppResult};
use crate::AppState;

pub fn router() -> Router<AppState> {
    Router::new().route("/me/library", get(library))
}

#[derive(Debug, Deserialize)]
pub struct LibraryQuery {
    #[serde(default)]
    pub q: Option<String>,
    #[serde(default)]
    pub limit: Option<u32>,
    #[serde(default)]
    pub offset: Option<u32>,
}

#[derive(Debug, Serialize)]
pub struct LibraryItem {
    pub id: String,
    pub name: String,
    pub artists: Vec<String>,
    pub album: String,
    pub image_url: Option<String>,
    pub added_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize)]
pub struct LibraryResponse {
    pub total: i64,
    pub items: Vec<LibraryItem>,
    pub limit: u32,
    pub offset: u32,
    pub q: Option<String>,
}

async fn library(
    State(state): State<AppState>,
    jar: PrivateCookieJar,
    Query(q): Query<LibraryQuery>,
) -> AppResult<Json<LibraryResponse>> {
    let user = current_user(&state, &jar).await?;
    let limit = q.limit.unwrap_or(50).clamp(1, 200);
    let offset = q.offset.unwrap_or(0);
    let search = q
        .q
        .as_ref()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    let pattern = search.as_ref().map(|s| format!("%{}%", s));

    // Total count for pagination UI
    let total: i64 = if let Some(pat) = pattern.as_ref() {
        sqlx::query_scalar(
            "SELECT COUNT(*) FROM user_liked_tracks ul \
             JOIN tracks t ON t.spotify_track_id = ul.spotify_track_id \
             WHERE ul.user_id = $1 \
               AND (t.name ILIKE $2 OR t.album ILIKE $2 \
                    OR EXISTS (SELECT 1 FROM unnest(t.artists) a WHERE a ILIKE $2))",
        )
        .bind(user.id)
        .bind(pat)
        .fetch_one(&state.db)
        .await
        .map_err(|e| AppError::Internal(e.into()))?
    } else {
        sqlx::query_scalar("SELECT COUNT(*) FROM user_liked_tracks WHERE user_id = $1")
            .bind(user.id)
            .fetch_one(&state.db)
            .await
            .map_err(|e| AppError::Internal(e.into()))?
    };

    let rows = if let Some(pat) = pattern.as_ref() {
        sqlx::query(
            "SELECT t.spotify_track_id, t.name, t.artists, t.album, t.image_url, ul.added_at \
             FROM user_liked_tracks ul \
             JOIN tracks t ON t.spotify_track_id = ul.spotify_track_id \
             WHERE ul.user_id = $1 \
               AND (t.name ILIKE $2 OR t.album ILIKE $2 \
                    OR EXISTS (SELECT 1 FROM unnest(t.artists) a WHERE a ILIKE $2)) \
             ORDER BY ul.added_at DESC NULLS LAST \
             LIMIT $3 OFFSET $4",
        )
        .bind(user.id)
        .bind(pat)
        .bind(limit as i64)
        .bind(offset as i64)
        .fetch_all(&state.db)
        .await
    } else {
        sqlx::query(
            "SELECT t.spotify_track_id, t.name, t.artists, t.album, t.image_url, ul.added_at \
             FROM user_liked_tracks ul \
             JOIN tracks t ON t.spotify_track_id = ul.spotify_track_id \
             WHERE ul.user_id = $1 \
             ORDER BY ul.added_at DESC NULLS LAST \
             LIMIT $2 OFFSET $3",
        )
        .bind(user.id)
        .bind(limit as i64)
        .bind(offset as i64)
        .fetch_all(&state.db)
        .await
    }
    .map_err(|e| AppError::Internal(e.into()))?;

    let items = rows
        .into_iter()
        .map(|r| LibraryItem {
            id: r.try_get("spotify_track_id").unwrap_or_default(),
            name: r.try_get("name").unwrap_or_default(),
            artists: r.try_get("artists").unwrap_or_default(),
            album: r.try_get("album").unwrap_or_default(),
            image_url: r.try_get("image_url").ok().flatten(),
            added_at: r.try_get("added_at").ok().flatten(),
        })
        .collect();

    Ok(Json(LibraryResponse {
        total,
        items,
        limit,
        offset,
        q: search,
    }))
}
