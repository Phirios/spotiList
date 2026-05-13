use axum::extract::{Path, Query, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use sqlx::Row;
use std::collections::BTreeSet;

use crate::error::{AppError, AppResult};
use crate::model::{Album, Artist, Bpm, TrackInfo};
use crate::AppState;

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/tracks/:id", get(get_track))
        .route("/tracks/:id/similar", get(similar_tracks))
        .route("/tracks/:id/emotion", post(track_emotion))
        .merge(crate::auth::router())
        .merge(crate::me::router())
        .merge(crate::playlists::router())
        .merge(crate::auto::router())
        .merge(crate::sync::router())
        .with_state(state)
}

async fn health() -> &'static str {
    "ok"
}

#[derive(Debug, Deserialize)]
pub struct TrackQuery {
    /// Skip lyrics fetch (e.g. for lightweight metadata calls).
    #[serde(default)]
    pub no_lyrics: bool,
    /// Skip BPM lookup.
    #[serde(default)]
    pub no_bpm: bool,
}

struct CachedTrackMeta {
    name: String,
    artists: Vec<String>,
    album: String,
    image_url: Option<String>,
    duration_ms: Option<i32>,
    isrc: Option<String>,
}

async fn load_cached_track(
    state: &AppState,
    id: &str,
) -> AppResult<Option<CachedTrackMeta>> {
    let row = sqlx::query(
        "SELECT name, artists, album, image_url, duration_ms, isrc \
         FROM tracks WHERE spotify_track_id = $1",
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| AppError::Internal(e.into()))?;
    Ok(row.map(|r| CachedTrackMeta {
        name: r.try_get("name").unwrap_or_default(),
        artists: r.try_get("artists").unwrap_or_default(),
        album: r.try_get("album").unwrap_or_default(),
        image_url: r.try_get("image_url").ok().flatten(),
        duration_ms: r.try_get("duration_ms").ok().flatten(),
        isrc: r.try_get("isrc").ok().flatten(),
    }))
}

async fn upstream_track_meta(state: &AppState, id: &str) -> AppResult<CachedTrackMeta> {
    let track = state.spotify.get_track(id).await?;
    // Persist to cache for next time so a track viewed once is fast forever.
    let artists: Vec<String> = track.artists.iter().map(|a| a.name.clone()).collect();
    let image_url = track
        .album
        .images
        .as_ref()
        .and_then(|imgs| imgs.first())
        .map(|i| i.url.clone());
    let isrc = track
        .external_ids
        .as_ref()
        .and_then(|e| e.isrc.clone());
    let _ = sqlx::query(
        "INSERT INTO tracks (spotify_track_id, name, artists, album, image_url, duration_ms, isrc, fetched_at) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, now()) \
         ON CONFLICT (spotify_track_id) DO UPDATE SET \
            name = EXCLUDED.name, artists = EXCLUDED.artists, album = EXCLUDED.album, \
            image_url = EXCLUDED.image_url, duration_ms = EXCLUDED.duration_ms, \
            isrc = EXCLUDED.isrc, fetched_at = now()",
    )
    .bind(id)
    .bind(&track.name)
    .bind(&artists)
    .bind(&track.album.name)
    .bind(image_url.clone())
    .bind(i32::try_from(track.duration_ms).ok())
    .bind(isrc.clone())
    .execute(&state.db)
    .await;

    Ok(CachedTrackMeta {
        name: track.name,
        artists,
        album: track.album.name,
        image_url,
        duration_ms: i32::try_from(track.duration_ms).ok(),
        isrc,
    })
}

async fn get_track(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(q): Query<TrackQuery>,
) -> AppResult<Json<TrackInfo>> {
    // 1. Track meta — DB cache first, Spotify fallback (and write-through).
    let meta = match load_cached_track(&state, &id).await? {
        Some(m) => m,
        None => upstream_track_meta(&state, &id).await?,
    };
    let primary_artist = meta.artists.first().cloned().unwrap_or_default();
    let title = meta.name.clone();
    let album_name = meta.album.clone();
    let duration_secs = (meta.duration_ms.unwrap_or(0) / 1000).max(0) as u64;

    // 2. Tags / BPM / lyrics — all cache-first via their respective clients.
    let lastfm_fut = state.lastfm.clean_tags_cached(&id, &primary_artist, &title);
    let bpm_fut = async {
        if q.no_bpm {
            None
        } else {
            state.bpm.lookup(&id, &primary_artist, &title).await
        }
    };
    let lyrics_fut = async {
        if q.no_lyrics {
            None
        } else {
            state
                .lyrics
                .lookup_cached(&id, &primary_artist, &title, Some(&album_name), duration_secs)
                .await
        }
    };

    let (lastfm_tags, bpm_val, lyrics) = tokio::join!(lastfm_fut, bpm_fut, lyrics_fut);

    let mut genre_set: BTreeSet<String> = BTreeSet::new();
    for g in lastfm_tags.into_iter() {
        let normalized = g.trim().to_lowercase();
        if !normalized.is_empty() {
            genre_set.insert(normalized);
        }
    }
    let genres: Vec<String> = genre_set.into_iter().collect();

    let info = TrackInfo {
        id: id.clone(),
        name: meta.name,
        artists: meta
            .artists
            .iter()
            .map(|n| Artist {
                id: String::new(),
                name: n.clone(),
            })
            .collect(),
        album: Album {
            id: String::new(),
            name: meta.album,
            release_date: None,
            image_url: meta.image_url,
        },
        duration_ms: meta.duration_ms.unwrap_or(0).max(0) as u64,
        explicit: false,
        popularity: None,
        isrc: meta.isrc,
        spotify_url: Some(format!("https://open.spotify.com/track/{id}")),
        genres,
        bpm: bpm_val.map(|t| Bpm {
            tempo: t,
            source: "getsongbpm",
        }),
        lyrics,
    };

    Ok(Json(info))
}

#[derive(Debug, Deserialize)]
pub struct SimilarQuery {
    #[serde(default = "default_similar_limit")]
    pub limit: usize,
}

fn default_similar_limit() -> usize {
    10
}

#[derive(Debug, serde::Serialize)]
pub struct SimilarTrack {
    pub id: String,
    pub name: String,
    pub artists: Vec<String>,
    pub album: String,
    pub image_url: Option<String>,
    pub score: f32,
}

async fn similar_tracks(
    State(state): State<AppState>,
    jar: axum_extra::extract::cookie::PrivateCookieJar,
    Path(id): Path<String>,
    Query(q): Query<SimilarQuery>,
) -> AppResult<Json<Vec<SimilarTrack>>> {
    let user = crate::auth::current_user(&state, &jar).await?;
    let stored_model = format!("{}#{}", state.embedder_model, crate::library::TEXT_VERSION);

    // 1. Get the target track's embedding.
    let target_row = sqlx::query(
        "SELECT embedding FROM track_embeddings \
         WHERE spotify_track_id = $1 AND model = $2",
    )
    .bind(&id)
    .bind(&stored_model)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| AppError::Internal(e.into()))?;
    let target_emb: Vec<f32> = match target_row {
        Some(r) => r.try_get("embedding").map_err(|e| AppError::Internal(e.into()))?,
        None => return Err(AppError::NotFound(format!("no embedding for {id}"))),
    };

    // 2. Pull all the user's library tracks + embeddings in one query.
    let rows = sqlx::query(
        "SELECT t.spotify_track_id, t.name, t.artists, t.album, t.image_url, e.embedding \
         FROM user_liked_tracks ul \
         JOIN tracks t ON t.spotify_track_id = ul.spotify_track_id \
         JOIN track_embeddings e ON e.spotify_track_id = ul.spotify_track_id \
         WHERE ul.user_id = $1 AND e.model = $2 AND ul.spotify_track_id <> $3",
    )
    .bind(user.id)
    .bind(&stored_model)
    .bind(&id)
    .fetch_all(&state.db)
    .await
    .map_err(|e| AppError::Internal(e.into()))?;

    let mut scored: Vec<SimilarTrack> = rows
        .into_iter()
        .filter_map(|r| {
            let emb: Vec<f32> = r.try_get("embedding").ok()?;
            let score = crate::embeddings::cosine(&target_emb, &emb);
            Some(SimilarTrack {
                id: r.try_get("spotify_track_id").ok()?,
                name: r.try_get("name").ok()?,
                artists: r.try_get("artists").ok()?,
                album: r.try_get("album").ok()?,
                image_url: r.try_get("image_url").ok().flatten(),
                score,
            })
        })
        .collect();

    scored.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    scored.truncate(q.limit.clamp(1, 50));

    Ok(Json(scored))
}

#[derive(Debug, Serialize)]
struct EmotionResponseDto {
    spotify_track_id: String,
    joy: f32,
    sadness: f32,
    anger: f32,
    fear: f32,
    surprise: f32,
    disgust: f32,
    neutral: f32,
    comment_count: i32,
}

async fn track_emotion(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<Json<EmotionResponseDto>> {
    let meta = match load_cached_track(&state, &id).await? {
        Some(m) => m,
        None => upstream_track_meta(&state, &id).await?,
    };
    let primary_artist = meta
        .artists
        .first()
        .cloned()
        .ok_or_else(|| AppError::NotFound(id.clone()))?;
    let duration_sec = meta.duration_ms.map(|ms| (ms / 1000).max(1));

    let vec = state
        .youtube
        .ensure_emotion(&id, &meta.name, &primary_artist, duration_sec)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("no youtube match for {id}")))?;

    Ok(Json(EmotionResponseDto {
        spotify_track_id: id,
        joy: vec.joy,
        sadness: vec.sadness,
        anger: vec.anger,
        fear: vec.fear,
        surprise: vec.surprise,
        disgust: vec.disgust,
        neutral: vec.neutral,
        comment_count: vec.comment_count,
    }))
}
