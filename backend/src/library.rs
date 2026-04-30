//! Library reads. Backed by the `tracks` × `user_liked_tracks` cache that
//! the sync engine populates. If the user has never synced (or sync is in
//! progress and has zero rows yet) callers should treat the empty library
//! as "not ready" and prompt the client to sync.

use std::collections::HashMap;

use serde::Deserialize;
use sqlx::Row;

use crate::error::{AppError, AppResult};
use crate::user::User;
use crate::AppState;

/// Bumped whenever the embedding text format changes — appended to the
/// embedder model id before caching so older entries get refreshed lazily.
pub const TEXT_VERSION: &str = "v3-cache";

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

pub struct UserLibrary {
    pub tracks: Vec<TrackLite>,
    pub embeddings: HashMap<String, Vec<f32>>,
    pub tags: HashMap<String, Vec<String>>,
    pub stored_model: String,
}

/// Load the user's library from the local cache. Returns an empty library
/// if the user has never synced; callers should check `tracks.len()` and
/// surface a "sync first" message rather than treating that as success.
pub async fn ensure_user_library(state: &AppState, user: &User) -> AppResult<UserLibrary> {
    let stored_model = format!("{}#{}", state.embedder_model, TEXT_VERSION);

    let track_rows = sqlx::query(
        "SELECT t.spotify_track_id, t.name, t.artists, t.album, t.image_url \
         FROM user_liked_tracks ul \
         JOIN tracks t ON t.spotify_track_id = ul.spotify_track_id \
         WHERE ul.user_id = $1 \
         ORDER BY ul.added_at DESC NULLS LAST",
    )
    .bind(user.id)
    .fetch_all(&state.db)
    .await
    .map_err(|e| AppError::Internal(e.into()))?;

    let mut tracks: Vec<TrackLite> = Vec::with_capacity(track_rows.len());
    let mut ids: Vec<String> = Vec::with_capacity(track_rows.len());
    for r in track_rows {
        let id: String = r
            .try_get("spotify_track_id")
            .map_err(|e| AppError::Internal(e.into()))?;
        let name: String = r
            .try_get("name")
            .map_err(|e| AppError::Internal(e.into()))?;
        let artists_v: Vec<String> = r
            .try_get("artists")
            .map_err(|e| AppError::Internal(e.into()))?;
        let album: String = r
            .try_get("album")
            .map_err(|e| AppError::Internal(e.into()))?;
        let image_url: Option<String> = r
            .try_get("image_url")
            .map_err(|e| AppError::Internal(e.into()))?;

        ids.push(id.clone());
        tracks.push(TrackLite {
            id: Some(id),
            name,
            artists: artists_v
                .into_iter()
                .map(|n| ArtistLite { name: n })
                .collect(),
            album: AlbumLite {
                name: album,
                images: image_url
                    .map(|url| vec![ImageLite { url }])
                    .unwrap_or_default(),
            },
        });
    }

    let embeddings = if ids.is_empty() {
        HashMap::new()
    } else {
        crate::embeddings::fetch_cached(&state.db, &ids, &stored_model)
            .await
            .map_err(|e| AppError::Internal(e.into()))?
    };

    let tags = if ids.is_empty() {
        HashMap::new()
    } else {
        load_tags(&state.db, &ids)
            .await
            .map_err(|e| AppError::Internal(e.into()))?
    };

    Ok(UserLibrary {
        tracks,
        embeddings,
        tags,
        stored_model,
    })
}

async fn load_tags(
    pool: &crate::db::Pool,
    ids: &[String],
) -> sqlx::Result<HashMap<String, Vec<String>>> {
    let rows = sqlx::query(
        "SELECT spotify_track_id, tags FROM track_tags WHERE spotify_track_id = ANY($1)",
    )
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
