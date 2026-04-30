//! User-library helpers shared by the vibe-generator and auto-clusterer.
//!
//! Builds and caches everything needed to do retrieval over a user's liked
//! songs: the track list itself, the per-track Last.fm tag cache, and the
//! per-track sentence-transformer embeddings. Both caches are keyed by
//! Spotify track id and persisted in Postgres so re-running is cheap.

use std::collections::HashMap;

use futures::stream::{self, StreamExt};
use serde::Deserialize;
use sqlx::Row;

use crate::auth::ensure_fresh_token;
use crate::embeddings;
use crate::error::{AppError, AppResult};
use crate::user::User;
use crate::AppState;

const SPOTIFY_API: &str = "https://api.spotify.com/v1";
const TAG_FETCH_CONCURRENCY: usize = 8;

/// Bumped whenever `text_for` changes — appended to the embedder model id
/// before caching so older entries get refreshed lazily.
pub const TEXT_VERSION: &str = "v2-tags";

#[derive(Debug, Clone, Deserialize)]
pub struct TrackLite {
    pub id: Option<String>,
    pub name: String,
    pub artists: Vec<ArtistLite>,
    pub album: AlbumLite,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ArtistLite {
    pub name: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AlbumLite {
    pub name: String,
    #[serde(default)]
    pub images: Vec<ImageLite>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ImageLite {
    pub url: String,
}

#[derive(Debug, Deserialize)]
struct SavedTracksPage {
    items: Vec<SavedItem>,
    next: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SavedItem {
    track: TrackLite,
}

pub struct UserLibrary {
    pub tracks: Vec<TrackLite>,
    pub embeddings: HashMap<String, Vec<f32>>,
    pub tags: HashMap<String, Vec<String>>,
    pub stored_model: String,
}

/// Fetch the user's liked songs, ensure tags + embeddings are cached for all
/// of them, return everything together.
pub async fn ensure_user_library(state: &AppState, user: &mut User) -> AppResult<UserLibrary> {
    let token = ensure_fresh_token(state, user).await?;
    let stored_model = format!("{}#{}", state.embedder_model, TEXT_VERSION);

    let liked = fetch_all_liked(state, &token).await?;
    let liked_with_id: Vec<(String, TrackLite)> = liked
        .iter()
        .filter_map(|t| t.id.as_ref().map(|id| (id.clone(), t.clone())))
        .collect();

    let ids: Vec<String> = liked_with_id.iter().map(|(id, _)| id.clone()).collect();

    let cached_tags = load_cached_tags(&state.db, &ids)
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    let missing_tag_tracks: Vec<(String, TrackLite)> = liked_with_id
        .iter()
        .filter(|(id, _)| !cached_tags.contains_key(id))
        .cloned()
        .collect();

    let mut tags = cached_tags;
    if !missing_tag_tracks.is_empty() {
        let new_tags = fetch_tags_concurrent(state, &missing_tag_tracks).await;
        store_tags(&state.db, &new_tags)
            .await
            .map_err(|e| AppError::Internal(e.into()))?;
        for (id, t) in new_tags {
            tags.insert(id, t);
        }
    }

    let cached_emb = embeddings::fetch_cached(&state.db, &ids, &stored_model)
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    let missing_emb_tracks: Vec<(String, TrackLite)> = liked_with_id
        .iter()
        .filter(|(id, _)| !cached_emb.contains_key(id))
        .cloned()
        .collect();

    let mut all_emb = cached_emb;
    if !missing_emb_tracks.is_empty() {
        let enriched: Vec<(String, String)> = missing_emb_tracks
            .iter()
            .map(|(id, t)| {
                let track_tags = tags.get(id).cloned().unwrap_or_default();
                (id.clone(), text_for(t, &track_tags))
            })
            .collect();
        for chunk in enriched.chunks(128) {
            let batch_texts: Vec<String> =
                chunk.iter().map(|(_, t)| t.clone()).collect();
            let resp = state.embedder.embed(&batch_texts).await?;
            let to_upsert: Vec<(String, String, Vec<f32>)> = chunk
                .iter()
                .zip(resp.embeddings.iter())
                .map(|((id, text), emb)| (id.clone(), text.clone(), emb.clone()))
                .collect();
            embeddings::upsert_many(&state.db, &stored_model, &to_upsert)
                .await
                .map_err(|e| AppError::Internal(e.into()))?;
            for (id, _, emb) in to_upsert {
                all_emb.insert(id, emb);
            }
        }
    }

    Ok(UserLibrary {
        tracks: liked,
        embeddings: all_emb,
        tags,
        stored_model,
    })
}

pub fn text_for(t: &TrackLite, tags: &[String]) -> String {
    let artists = t
        .artists
        .iter()
        .map(|a| a.name.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    let mut s = format!("{} by {} from album {}", t.name, artists, t.album.name);
    if !tags.is_empty() {
        s.push_str(". Tags: ");
        s.push_str(&tags.join(", "));
    }
    s
}

pub async fn fetch_all_liked(state: &AppState, token: &str) -> AppResult<Vec<TrackLite>> {
    let mut out: Vec<TrackLite> = Vec::new();
    let mut next: Option<String> =
        Some(format!("{SPOTIFY_API}/me/tracks?limit=50&offset=0"));
    while let Some(url) = next.take() {
        let resp = state.http.get(&url).bearer_auth(token).send().await?;
        if !resp.status().is_success() {
            let s = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(AppError::Upstream(format!(
                "spotify /me/tracks failed: {s}: {body}"
            )));
        }
        let page: SavedTracksPage = resp.json().await?;
        for item in page.items {
            out.push(item.track);
        }
        next = page.next;
    }
    Ok(out)
}

async fn fetch_tags_concurrent(
    state: &AppState,
    tracks: &[(String, TrackLite)],
) -> HashMap<String, Vec<String>> {
    let lastfm = state.lastfm.clone();
    stream::iter(tracks.iter().cloned())
        .map(|(id, t)| {
            let lastfm = lastfm.clone();
            async move {
                let primary_artist = t
                    .artists
                    .first()
                    .map(|a| a.name.clone())
                    .unwrap_or_default();
                let tags = lastfm.clean_tags(&primary_artist, &t.name).await;
                (id, tags)
            }
        })
        .buffer_unordered(TAG_FETCH_CONCURRENCY)
        .collect::<Vec<_>>()
        .await
        .into_iter()
        .collect()
}

async fn load_cached_tags(
    pool: &crate::db::Pool,
    ids: &[String],
) -> sqlx::Result<HashMap<String, Vec<String>>> {
    if ids.is_empty() {
        return Ok(HashMap::new());
    }
    let rows = sqlx::query("SELECT spotify_track_id, tags FROM track_tags WHERE spotify_track_id = ANY($1)")
        .bind(ids)
        .fetch_all(pool)
        .await?;
    let mut out = HashMap::with_capacity(rows.len());
    for r in rows {
        let id: String = r.try_get("spotify_track_id")?;
        let tags: Vec<String> = r.try_get("tags")?;
        out.insert(id, tags);
    }
    Ok(out)
}

async fn store_tags(
    pool: &crate::db::Pool,
    items: &HashMap<String, Vec<String>>,
) -> sqlx::Result<()> {
    if items.is_empty() {
        return Ok(());
    }
    let mut tx = pool.begin().await?;
    for (id, tags) in items {
        sqlx::query(
            "INSERT INTO track_tags (spotify_track_id, tags, looked_up_at) \
             VALUES ($1, $2, now()) \
             ON CONFLICT (spotify_track_id) DO UPDATE SET \
                tags = EXCLUDED.tags, \
                looked_up_at = now()",
        )
        .bind(id)
        .bind(tags)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(())
}
