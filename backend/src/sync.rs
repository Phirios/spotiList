//! Background library sync engine.
//!
//! `start_sync` spawns a tokio task that walks Spotify pagination, populates
//! `tracks` + `user_liked_tracks`, fetches Last.fm tags concurrently, then
//! embeds anything missing. Progress is written to `sync_jobs` so the
//! frontend can poll for status / ETA.

use std::sync::Arc;

use axum::extract::State;
use axum::routing::post;
use axum::{Json, Router};
use axum_extra::extract::cookie::PrivateCookieJar;
use chrono::{DateTime, Utc};
use futures::stream::{self, StreamExt};
use serde::{Deserialize, Serialize};
use sqlx::Row;
use uuid::Uuid;

use crate::auth::{current_user, ensure_fresh_token};
use crate::db::Pool;
use crate::embeddings;
use crate::error::{AppError, AppResult};
use crate::library::TEXT_VERSION;
use crate::AppState;

const SPOTIFY_API: &str = "https://api.spotify.com/v1";
const TAG_FETCH_CONCURRENCY: usize = 8;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/library/sync", post(start).get(status))
}

#[derive(Debug, Serialize)]
pub struct SyncStatus {
    pub status: String,
    pub stage: Option<String>,
    pub progress: i32,
    pub total: i32,
    pub started_at: Option<DateTime<Utc>>,
    pub updated_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    pub error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct StartParams {
    #[serde(default)]
    force: bool,
}

async fn start(
    State(state): State<AppState>,
    jar: PrivateCookieJar,
    body: Option<Json<StartParams>>,
) -> AppResult<Json<SyncStatus>> {
    let user = current_user(&state, &jar).await?;
    let force = body.map(|b| b.force).unwrap_or(false);

    if !force && is_running(&state.db, user.id).await {
        return Ok(Json(read_status(&state.db, user.id).await?));
    }

    spawn_sync(state.clone(), user.id);
    // Brief moment so the spawned task can write the "running" row before
    // the caller polls.
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    Ok(Json(read_status(&state.db, user.id).await?))
}

async fn status(
    State(state): State<AppState>,
    jar: PrivateCookieJar,
) -> AppResult<Json<SyncStatus>> {
    let user = current_user(&state, &jar).await?;
    Ok(Json(read_status(&state.db, user.id).await?))
}

async fn is_running(pool: &Pool, user_id: Uuid) -> bool {
    sqlx::query_scalar::<_, String>(
        "SELECT status FROM sync_jobs WHERE user_id = $1",
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await
    .ok()
    .flatten()
    .map(|s| s == "running")
    .unwrap_or(false)
}

async fn read_status(pool: &Pool, user_id: Uuid) -> AppResult<SyncStatus> {
    let row = sqlx::query(
        "SELECT status, stage, progress, total, started_at, updated_at, finished_at, error \
         FROM sync_jobs WHERE user_id = $1",
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| AppError::Internal(e.into()))?;

    match row {
        Some(r) => Ok(SyncStatus {
            status: r.try_get("status").unwrap_or_else(|_| "idle".into()),
            stage: r.try_get("stage").ok().flatten(),
            progress: r.try_get("progress").unwrap_or(0),
            total: r.try_get("total").unwrap_or(0),
            started_at: r.try_get("started_at").ok().flatten(),
            updated_at: r.try_get("updated_at").unwrap_or(Utc::now()),
            finished_at: r.try_get("finished_at").ok().flatten(),
            error: r.try_get("error").ok().flatten(),
        }),
        None => Ok(SyncStatus {
            status: "idle".into(),
            stage: None,
            progress: 0,
            total: 0,
            started_at: None,
            updated_at: Utc::now(),
            finished_at: None,
            error: None,
        }),
    }
}

/// Spawn a sync task. Public so the OAuth callback can kick it off after a
/// fresh login without blocking the redirect.
pub fn spawn_sync(state: AppState, user_id: Uuid) {
    tokio::spawn(async move {
        if let Err(e) = run_sync(state, user_id).await {
            tracing::error!(error = %e, %user_id, "sync failed");
        }
    });
}

async fn run_sync(state: AppState, user_id: Uuid) -> anyhow::Result<()> {
    set_status(&state.db, user_id, "running", Some("starting"), 0, 0, None).await?;
    sqlx::query(
        "UPDATE sync_jobs SET started_at = now(), finished_at = NULL, error = NULL \
         WHERE user_id = $1",
    )
    .bind(user_id)
    .execute(&state.db)
    .await?;

    // Need a fresh user object for the access token.
    let mut user = match crate::user::find_by_id(&state.db, user_id).await? {
        Some(u) => u,
        None => {
            anyhow::bail!("user disappeared mid-sync");
        }
    };
    let token = ensure_fresh_token(&state, &mut user).await?;

    // Stage 1: fetch all liked from Spotify, upsert tracks + user_liked_tracks.
    let liked_ids = stage_fetch_library(&state, user_id, &token).await?;

    // Stage 2: fetch Last.fm tags for any tracks missing in cache.
    stage_fetch_tags(&state, user_id, &liked_ids).await?;

    // Stage 3: embed any tracks missing in embeddings cache.
    stage_embed(&state, user_id, &liked_ids).await?;

    // Done.
    sqlx::query(
        "UPDATE sync_jobs SET status='done', stage=NULL, finished_at=now(), updated_at=now() \
         WHERE user_id = $1",
    )
    .bind(user_id)
    .execute(&state.db)
    .await?;

    Ok(())
}

#[derive(Debug, Deserialize)]
struct SavedPage {
    items: Vec<SavedItem>,
    next: Option<String>,
    #[serde(default)]
    total: Option<i32>,
}

#[derive(Debug, Deserialize)]
struct SavedItem {
    added_at: Option<DateTime<Utc>>,
    track: SpotifyFullTrack,
}

#[derive(Debug, Deserialize)]
struct SpotifyFullTrack {
    id: Option<String>,
    name: String,
    duration_ms: Option<i32>,
    artists: Vec<TrackArtist>,
    album: TrackAlbum,
    #[serde(default)]
    external_ids: Option<TrackExternalIds>,
}

#[derive(Debug, Deserialize)]
struct TrackArtist {
    name: String,
}

#[derive(Debug, Deserialize)]
struct TrackAlbum {
    name: String,
    #[serde(default)]
    images: Vec<TrackImage>,
}

#[derive(Debug, Deserialize)]
struct TrackImage {
    url: String,
}

#[derive(Debug, Deserialize)]
struct TrackExternalIds {
    #[serde(default)]
    isrc: Option<String>,
}

async fn stage_fetch_library(
    state: &AppState,
    user_id: Uuid,
    token: &str,
) -> anyhow::Result<Vec<String>> {
    set_status(&state.db, user_id, "running", Some("fetching_library"), 0, 0, None).await?;
    let mut url = format!("{SPOTIFY_API}/me/tracks?limit=50&offset=0");
    let mut out_ids: Vec<String> = Vec::new();
    let mut total_known: Option<i32> = None;

    loop {
        let resp = state.http.get(&url).bearer_auth(token).send().await?;
        if !resp.status().is_success() {
            let s = resp.status();
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("spotify /me/tracks failed: {s}: {body}");
        }
        let page: SavedPage = resp.json().await?;
        if total_known.is_none() {
            total_known = page.total;
        }

        // Persist this page in a single transaction.
        let mut tx = state.db.begin().await?;
        // Wipe user_liked_tracks for this user only on the very first page
        // of a fresh sync, so we don't keep stale rows for unliked tracks.
        if out_ids.is_empty() {
            sqlx::query("DELETE FROM user_liked_tracks WHERE user_id = $1")
                .bind(user_id)
                .execute(&mut *tx)
                .await?;
        }

        for item in &page.items {
            let Some(id) = item.track.id.clone() else {
                continue;
            };
            let artists: Vec<String> =
                item.track.artists.iter().map(|a| a.name.clone()).collect();
            let image_url = item.track.album.images.first().map(|i| i.url.clone());
            let isrc = item
                .track
                .external_ids
                .as_ref()
                .and_then(|e| e.isrc.clone());

            sqlx::query(
                "INSERT INTO tracks (spotify_track_id, name, artists, album, image_url, duration_ms, isrc, fetched_at) \
                 VALUES ($1, $2, $3, $4, $5, $6, $7, now()) \
                 ON CONFLICT (spotify_track_id) DO UPDATE SET \
                    name = EXCLUDED.name, \
                    artists = EXCLUDED.artists, \
                    album = EXCLUDED.album, \
                    image_url = EXCLUDED.image_url, \
                    duration_ms = EXCLUDED.duration_ms, \
                    isrc = EXCLUDED.isrc, \
                    fetched_at = now()",
            )
            .bind(&id)
            .bind(&item.track.name)
            .bind(&artists)
            .bind(&item.track.album.name)
            .bind(image_url)
            .bind(item.track.duration_ms)
            .bind(isrc)
            .execute(&mut *tx)
            .await?;

            sqlx::query(
                "INSERT INTO user_liked_tracks (user_id, spotify_track_id, added_at) \
                 VALUES ($1, $2, $3) \
                 ON CONFLICT (user_id, spotify_track_id) DO UPDATE SET \
                    added_at = EXCLUDED.added_at",
            )
            .bind(user_id)
            .bind(&id)
            .bind(item.added_at)
            .execute(&mut *tx)
            .await?;

            out_ids.push(id);
        }
        tx.commit().await?;

        let total = total_known.unwrap_or(out_ids.len() as i32);
        update_progress(
            &state.db,
            user_id,
            "fetching_library",
            out_ids.len() as i32,
            total,
        )
        .await?;

        match page.next {
            Some(next) => url = next,
            None => break,
        }
    }
    Ok(out_ids)
}

async fn stage_fetch_tags(
    state: &AppState,
    user_id: Uuid,
    ids: &[String],
) -> anyhow::Result<()> {
    set_status(
        &state.db,
        user_id,
        "running",
        Some("fetching_tags"),
        0,
        ids.len() as i32,
        None,
    )
    .await?;

    // Find the IDs not yet in track_tags.
    let cached: Vec<String> = sqlx::query_scalar(
        "SELECT spotify_track_id FROM track_tags WHERE spotify_track_id = ANY($1)",
    )
    .bind(ids)
    .fetch_all(&state.db)
    .await?;
    let cached_set: std::collections::HashSet<String> = cached.into_iter().collect();
    let missing: Vec<String> = ids
        .iter()
        .filter(|id| !cached_set.contains(*id))
        .cloned()
        .collect();

    let already = (ids.len() - missing.len()) as i32;
    update_progress(
        &state.db,
        user_id,
        "fetching_tags",
        already,
        ids.len() as i32,
    )
    .await?;

    if missing.is_empty() {
        return Ok(());
    }

    let track_meta = load_track_meta(&state.db, &missing).await?;

    let lastfm = state.lastfm.clone();
    let pool = state.db.clone();
    let total = ids.len() as i32;
    let already_arc = Arc::new(std::sync::atomic::AtomicI32::new(already));

    let s = stream::iter(missing.into_iter())
        .map(|id| {
            let lastfm = lastfm.clone();
            let pool = pool.clone();
            let already_arc = already_arc.clone();
            let meta = track_meta.get(&id).cloned();
            async move {
                let (artist, name) = match meta {
                    Some(m) => m,
                    None => return Ok::<(), anyhow::Error>(()),
                };
                let tags = lastfm.clean_tags(&artist, &name).await;
                sqlx::query(
                    "INSERT INTO track_tags (spotify_track_id, tags, looked_up_at) \
                     VALUES ($1, $2, now()) \
                     ON CONFLICT (spotify_track_id) DO UPDATE SET \
                        tags = EXCLUDED.tags, looked_up_at = now()",
                )
                .bind(&id)
                .bind(&tags)
                .execute(&pool)
                .await?;
                let done = already_arc
                    .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
                    + 1;
                if done % 10 == 0 || done == total {
                    let _ = update_progress(
                        &pool,
                        user_id,
                        "fetching_tags",
                        done,
                        total,
                    )
                    .await;
                }
                Ok(())
            }
        })
        .buffer_unordered(TAG_FETCH_CONCURRENCY);

    futures::pin_mut!(s);
    while let Some(res) = s.next().await {
        if let Err(e) = res {
            tracing::warn!(error = %e, "tag fetch error (continuing)");
        }
    }

    let final_total = ids.len() as i32;
    update_progress(&state.db, user_id, "fetching_tags", final_total, final_total).await?;
    Ok(())
}

async fn stage_embed(
    state: &AppState,
    user_id: Uuid,
    ids: &[String],
) -> anyhow::Result<()> {
    let stored_model = format!("{}#{}", state.embedder_model, TEXT_VERSION);
    set_status(
        &state.db,
        user_id,
        "running",
        Some("embedding"),
        0,
        ids.len() as i32,
        None,
    )
    .await?;

    let cached: Vec<String> = sqlx::query_scalar(
        "SELECT spotify_track_id FROM track_embeddings \
         WHERE model = $1 AND spotify_track_id = ANY($2)",
    )
    .bind(&stored_model)
    .bind(ids)
    .fetch_all(&state.db)
    .await?;
    let cached_set: std::collections::HashSet<String> = cached.into_iter().collect();
    let missing: Vec<String> = ids
        .iter()
        .filter(|id| !cached_set.contains(*id))
        .cloned()
        .collect();

    let already = (ids.len() - missing.len()) as i32;
    update_progress(
        &state.db,
        user_id,
        "embedding",
        already,
        ids.len() as i32,
    )
    .await?;

    if missing.is_empty() {
        return Ok(());
    }

    // Build text per track from the cached metadata + tags.
    let track_meta = load_track_meta(&state.db, &missing).await?;
    let tag_map = load_tags_map(&state.db, &missing).await?;

    let texts: Vec<(String, String)> = missing
        .iter()
        .filter_map(|id| {
            let (artist, name) = track_meta.get(id)?.clone();
            let album_artist_text = format!("{} by {}", name, artist);
            let tags = tag_map.get(id).cloned().unwrap_or_default();
            let text = if tags.is_empty() {
                album_artist_text
            } else {
                format!("{}. Tags: {}", album_artist_text, tags.join(", "))
            };
            Some((id.clone(), text))
        })
        .collect();

    let mut done_count = already;
    for chunk in texts.chunks(128) {
        let batch: Vec<String> = chunk.iter().map(|(_, t)| t.clone()).collect();
        let resp = state.embedder.embed(&batch).await?;
        let to_upsert: Vec<(String, String, Vec<f32>)> = chunk
            .iter()
            .zip(resp.embeddings.iter())
            .map(|((id, text), emb)| (id.clone(), text.clone(), emb.clone()))
            .collect();
        embeddings::upsert_many(&state.db, &stored_model, &to_upsert).await?;
        done_count += chunk.len() as i32;
        update_progress(
            &state.db,
            user_id,
            "embedding",
            done_count,
            ids.len() as i32,
        )
        .await?;
    }

    Ok(())
}

async fn load_track_meta(
    pool: &Pool,
    ids: &[String],
) -> anyhow::Result<std::collections::HashMap<String, (String, String)>> {
    if ids.is_empty() {
        return Ok(std::collections::HashMap::new());
    }
    let rows = sqlx::query(
        "SELECT spotify_track_id, name, artists FROM tracks \
         WHERE spotify_track_id = ANY($1)",
    )
    .bind(ids)
    .fetch_all(pool)
    .await?;
    let mut out = std::collections::HashMap::with_capacity(rows.len());
    for r in rows {
        let id: String = r.try_get("spotify_track_id")?;
        let name: String = r.try_get("name")?;
        let artists: Vec<String> = r.try_get("artists")?;
        let primary = artists.first().cloned().unwrap_or_default();
        out.insert(id, (primary, name));
    }
    Ok(out)
}

async fn load_tags_map(
    pool: &Pool,
    ids: &[String],
) -> anyhow::Result<std::collections::HashMap<String, Vec<String>>> {
    if ids.is_empty() {
        return Ok(std::collections::HashMap::new());
    }
    let rows = sqlx::query(
        "SELECT spotify_track_id, tags FROM track_tags WHERE spotify_track_id = ANY($1)",
    )
    .bind(ids)
    .fetch_all(pool)
    .await?;
    let mut out = std::collections::HashMap::with_capacity(rows.len());
    for r in rows {
        let id: String = r.try_get("spotify_track_id")?;
        let tags: Vec<String> = r.try_get("tags")?;
        out.insert(id, tags);
    }
    Ok(out)
}

async fn set_status(
    pool: &Pool,
    user_id: Uuid,
    status: &str,
    stage: Option<&str>,
    progress: i32,
    total: i32,
    error: Option<&str>,
) -> anyhow::Result<()> {
    sqlx::query(
        "INSERT INTO sync_jobs (user_id, status, stage, progress, total, started_at, updated_at, error) \
         VALUES ($1, $2, $3, $4, $5, COALESCE((SELECT started_at FROM sync_jobs WHERE user_id = $1), now()), now(), $6) \
         ON CONFLICT (user_id) DO UPDATE SET \
            status = EXCLUDED.status, \
            stage = EXCLUDED.stage, \
            progress = EXCLUDED.progress, \
            total = EXCLUDED.total, \
            updated_at = now(), \
            error = EXCLUDED.error",
    )
    .bind(user_id)
    .bind(status)
    .bind(stage)
    .bind(progress)
    .bind(total)
    .bind(error)
    .execute(pool)
    .await?;
    Ok(())
}

async fn update_progress(
    pool: &Pool,
    user_id: Uuid,
    stage: &str,
    progress: i32,
    total: i32,
) -> anyhow::Result<()> {
    sqlx::query(
        "UPDATE sync_jobs SET stage = $2, progress = $3, total = $4, updated_at = now() \
         WHERE user_id = $1",
    )
    .bind(user_id)
    .bind(stage)
    .bind(progress)
    .bind(total)
    .execute(pool)
    .await?;
    Ok(())
}
