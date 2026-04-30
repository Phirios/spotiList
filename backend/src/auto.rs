//! Auto-clustered playlists — group the user's liked songs into themed
//! playlists by clustering their embeddings, then label each cluster from
//! its dominant Last.fm tags. Persisted in `auto_playlists` /
//! `auto_playlist_tracks` so re-renders are cheap.

use std::collections::HashMap;

use axum::extract::{Path, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use axum_extra::extract::cookie::PrivateCookieJar;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::Row;
use uuid::Uuid;

use crate::auth::{current_user, ensure_fresh_token};
use crate::error::{AppError, AppResult};
use crate::library::{ensure_user_library, TrackLite};
use crate::user::User;
use crate::AppState;

const SPOTIFY_API: &str = "https://api.spotify.com/v1";

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/auto-playlists", get(list))
        .route("/auto-playlists/regenerate", post(regenerate))
        .route("/auto-playlists/:id", get(get_one))
        .route("/auto-playlists/:id/save", post(save_to_spotify))
}

#[derive(Debug, Serialize)]
pub struct AutoPlaylistSummary {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub track_count: i32,
    pub spotify_playlist_id: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// Up to 4 sample tracks for card display.
    pub sample: Vec<TrackOut>,
}

#[derive(Debug, Serialize)]
pub struct AutoPlaylistFull {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub track_count: i32,
    pub spotify_playlist_id: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub tracks: Vec<TrackOut>,
}

#[derive(Debug, Serialize)]
pub struct TrackOut {
    pub id: String,
    pub name: String,
    pub artists: Vec<String>,
    pub album: String,
    pub image_url: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct RegenerateResponse {
    pub considered: usize,
    pub k: usize,
    pub playlists: Vec<AutoPlaylistSummary>,
}

async fn regenerate(
    State(state): State<AppState>,
    jar: PrivateCookieJar,
) -> AppResult<Json<RegenerateResponse>> {
    let mut user = current_user(&state, &jar).await?;
    let lib = ensure_user_library(&state, &mut user).await?;
    let considered = lib.tracks.len();

    if considered < 6 {
        return Err(AppError::Upstream(format!(
            "need at least 6 liked tracks to cluster, have {considered}"
        )));
    }

    // 1. Build the embedding matrix in a deterministic order
    let ordered: Vec<(String, &TrackLite, &Vec<f32>)> = lib
        .tracks
        .iter()
        .filter_map(|t| {
            let id = t.id.as_ref()?;
            let emb = lib.embeddings.get(id)?;
            Some((id.clone(), t, emb))
        })
        .collect();

    if ordered.len() < 6 {
        return Err(AppError::Upstream(format!(
            "only {} tracks have embeddings; try again",
            ordered.len()
        )));
    }

    let matrix: Vec<Vec<f32>> = ordered.iter().map(|(_, _, e)| (*e).clone()).collect();

    // 2. Cluster via the embedder service
    let cluster_resp: ClusterResponse = state
        .http
        .post(format!("{}/cluster", &state.embedder.base_url()))
        .json(&json!({ "embeddings": matrix, "target_per_cluster": 25 }))
        .send()
        .await?
        .json()
        .await?;

    if cluster_resp.labels.len() != ordered.len() {
        return Err(AppError::Upstream(format!(
            "cluster label count mismatch: {} vs {}",
            cluster_resp.labels.len(),
            ordered.len()
        )));
    }

    // 3. Group track indices by cluster
    let mut groups: HashMap<i32, Vec<usize>> = HashMap::new();
    for (idx, label) in cluster_resp.labels.iter().enumerate() {
        groups.entry(*label).or_default().push(idx);
    }

    // 4. Wipe existing auto-playlists for this user
    sqlx::query("DELETE FROM auto_playlists WHERE user_id = $1")
        .bind(user.id)
        .execute(&state.db)
        .await
        .map_err(|e| AppError::Internal(e.into()))?;

    // 5. Build, name, and persist new playlists
    let mut summaries: Vec<AutoPlaylistSummary> = Vec::with_capacity(groups.len());

    let mut sorted_labels: Vec<i32> = groups.keys().copied().collect();
    sorted_labels.sort();

    for label in sorted_labels {
        let indices = &groups[&label];
        let cluster_tracks: Vec<&TrackLite> =
            indices.iter().map(|i| ordered[*i].1).collect();
        let cluster_ids: Vec<String> =
            indices.iter().map(|i| ordered[*i].0.clone()).collect();

        let top_tags = top_tags_in_cluster(&cluster_ids, &lib.tags, 3);
        let name = if top_tags.is_empty() {
            format!("Cluster {}", label + 1)
        } else {
            top_tags.join(" · ")
        };
        let description = format!(
            "{} tracks · auto-clustered from your liked songs",
            cluster_tracks.len()
        );

        let inserted: (Uuid,) = sqlx::query_as(
            "INSERT INTO auto_playlists (user_id, name, description, cluster_index, track_count) \
             VALUES ($1, $2, $3, $4, $5) RETURNING id",
        )
        .bind(user.id)
        .bind(&name)
        .bind(&description)
        .bind(label)
        .bind(cluster_tracks.len() as i32)
        .fetch_one(&state.db)
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
        let pid = inserted.0;

        // Insert tracks in the original order
        let mut tx = state
            .db
            .begin()
            .await
            .map_err(|e| AppError::Internal(e.into()))?;
        for (pos, t) in cluster_tracks.iter().enumerate() {
            let id = t.id.clone().unwrap_or_default();
            let artists: Vec<String> = t.artists.iter().map(|a| a.name.clone()).collect();
            let image_url = t.album.images.first().map(|i| i.url.clone());
            sqlx::query(
                "INSERT INTO auto_playlist_tracks \
                    (auto_playlist_id, position, spotify_track_id, name, artists, album, image_url) \
                 VALUES ($1, $2, $3, $4, $5, $6, $7)",
            )
            .bind(pid)
            .bind(pos as i32)
            .bind(&id)
            .bind(&t.name)
            .bind(&artists)
            .bind(&t.album.name)
            .bind(image_url)
            .execute(&mut *tx)
            .await
            .map_err(|e| AppError::Internal(e.into()))?;
        }
        tx.commit()
            .await
            .map_err(|e| AppError::Internal(e.into()))?;

        let sample: Vec<TrackOut> = cluster_tracks
            .iter()
            .take(4)
            .map(|t| TrackOut {
                id: t.id.clone().unwrap_or_default(),
                name: t.name.clone(),
                artists: t.artists.iter().map(|a| a.name.clone()).collect(),
                album: t.album.name.clone(),
                image_url: t.album.images.first().map(|i| i.url.clone()),
            })
            .collect();

        summaries.push(AutoPlaylistSummary {
            id: pid,
            name,
            description: Some(description),
            track_count: cluster_tracks.len() as i32,
            spotify_playlist_id: None,
            created_at: chrono::Utc::now(),
            sample,
        });
    }

    Ok(Json(RegenerateResponse {
        considered,
        k: cluster_resp.k as usize,
        playlists: summaries,
    }))
}

async fn list(
    State(state): State<AppState>,
    jar: PrivateCookieJar,
) -> AppResult<Json<Vec<AutoPlaylistSummary>>> {
    let user = current_user(&state, &jar).await?;
    let summaries = list_for_user(&state, &user).await?;
    Ok(Json(summaries))
}

async fn list_for_user(
    state: &AppState,
    user: &User,
) -> AppResult<Vec<AutoPlaylistSummary>> {
    let rows = sqlx::query(
        "SELECT id, name, description, track_count, spotify_playlist_id, created_at \
         FROM auto_playlists WHERE user_id = $1 ORDER BY created_at DESC",
    )
    .bind(user.id)
    .fetch_all(&state.db)
    .await
    .map_err(|e| AppError::Internal(e.into()))?;

    let mut out = Vec::with_capacity(rows.len());
    for r in rows {
        let id: Uuid = r.try_get("id").map_err(|e| AppError::Internal(e.into()))?;
        let sample = sample_tracks(state, id, 4).await?;
        out.push(AutoPlaylistSummary {
            id,
            name: r
                .try_get("name")
                .map_err(|e| AppError::Internal(e.into()))?,
            description: r
                .try_get("description")
                .map_err(|e| AppError::Internal(e.into()))?,
            track_count: r
                .try_get("track_count")
                .map_err(|e| AppError::Internal(e.into()))?,
            spotify_playlist_id: r
                .try_get("spotify_playlist_id")
                .map_err(|e| AppError::Internal(e.into()))?,
            created_at: r
                .try_get("created_at")
                .map_err(|e| AppError::Internal(e.into()))?,
            sample,
        });
    }
    Ok(out)
}

async fn get_one(
    State(state): State<AppState>,
    jar: PrivateCookieJar,
    Path(id): Path<Uuid>,
) -> AppResult<Json<AutoPlaylistFull>> {
    let user = current_user(&state, &jar).await?;

    let row = sqlx::query(
        "SELECT id, name, description, track_count, spotify_playlist_id, created_at \
         FROM auto_playlists WHERE id = $1 AND user_id = $2",
    )
    .bind(id)
    .bind(user.id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| AppError::Internal(e.into()))?
    .ok_or_else(|| AppError::NotFound(format!("auto-playlist {id}")))?;

    let tracks = all_tracks(&state, id).await?;

    Ok(Json(AutoPlaylistFull {
        id,
        name: row
            .try_get("name")
            .map_err(|e| AppError::Internal(e.into()))?,
        description: row
            .try_get("description")
            .map_err(|e| AppError::Internal(e.into()))?,
        track_count: row
            .try_get("track_count")
            .map_err(|e| AppError::Internal(e.into()))?,
        spotify_playlist_id: row
            .try_get("spotify_playlist_id")
            .map_err(|e| AppError::Internal(e.into()))?,
        created_at: row
            .try_get("created_at")
            .map_err(|e| AppError::Internal(e.into()))?,
        tracks,
    }))
}

#[derive(Debug, Serialize)]
pub struct SaveAutoResponse {
    pub url: String,
    pub spotify_playlist_id: String,
}

async fn save_to_spotify(
    State(state): State<AppState>,
    jar: PrivateCookieJar,
    Path(id): Path<Uuid>,
) -> AppResult<Json<SaveAutoResponse>> {
    let mut user = current_user(&state, &jar).await?;

    let row = sqlx::query(
        "SELECT name, description FROM auto_playlists WHERE id = $1 AND user_id = $2",
    )
    .bind(id)
    .bind(user.id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| AppError::Internal(e.into()))?
    .ok_or_else(|| AppError::NotFound(format!("auto-playlist {id}")))?;
    let name: String = row
        .try_get("name")
        .map_err(|e| AppError::Internal(e.into()))?;
    let description: Option<String> = row
        .try_get("description")
        .map_err(|e| AppError::Internal(e.into()))?;

    let track_ids: Vec<String> = sqlx::query_scalar(
        "SELECT spotify_track_id FROM auto_playlist_tracks \
         WHERE auto_playlist_id = $1 ORDER BY position",
    )
    .bind(id)
    .fetch_all(&state.db)
    .await
    .map_err(|e| AppError::Internal(e.into()))?;

    if track_ids.is_empty() {
        return Err(AppError::Upstream("playlist has no tracks".into()));
    }

    let token = ensure_fresh_token(&state, &mut user).await?;

    let create_url = format!("{SPOTIFY_API}/users/{}/playlists", user.spotify_id);
    let resp = state
        .http
        .post(&create_url)
        .bearer_auth(&token)
        .json(&json!({
            "name": name,
            "description": description.unwrap_or_else(|| "Auto-clustered from your liked songs".into()),
            "public": false,
        }))
        .send()
        .await?;
    if !resp.status().is_success() {
        let s = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(AppError::Upstream(format!(
            "create playlist failed: {s}: {body}"
        )));
    }
    #[derive(Deserialize)]
    struct CreatedPlaylist {
        id: String,
        #[serde(default)]
        external_urls: Option<ExternalUrls>,
    }
    #[derive(Deserialize)]
    struct ExternalUrls {
        #[serde(default)]
        spotify: Option<String>,
    }
    let created: CreatedPlaylist = resp.json().await?;

    let uris: Vec<String> = track_ids
        .iter()
        .map(|tid| format!("spotify:track:{tid}"))
        .collect();
    let add_url = format!("{SPOTIFY_API}/playlists/{}/tracks", created.id);
    for chunk in uris.chunks(100) {
        let resp = state
            .http
            .post(&add_url)
            .bearer_auth(&token)
            .json(&json!({ "uris": chunk }))
            .send()
            .await?;
        if !resp.status().is_success() {
            let s = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(AppError::Upstream(format!(
                "add tracks failed: {s}: {body}"
            )));
        }
    }

    sqlx::query("UPDATE auto_playlists SET spotify_playlist_id = $1 WHERE id = $2")
        .bind(&created.id)
        .bind(id)
        .execute(&state.db)
        .await
        .map_err(|e| AppError::Internal(e.into()))?;

    let url = created
        .external_urls
        .and_then(|u| u.spotify)
        .unwrap_or_else(|| format!("https://open.spotify.com/playlist/{}", created.id));

    Ok(Json(SaveAutoResponse {
        url,
        spotify_playlist_id: created.id,
    }))
}

async fn sample_tracks(
    state: &AppState,
    playlist_id: Uuid,
    limit: i32,
) -> AppResult<Vec<TrackOut>> {
    let rows = sqlx::query(
        "SELECT spotify_track_id, name, artists, album, image_url \
         FROM auto_playlist_tracks WHERE auto_playlist_id = $1 \
         ORDER BY position LIMIT $2",
    )
    .bind(playlist_id)
    .bind(limit)
    .fetch_all(&state.db)
    .await
    .map_err(|e| AppError::Internal(e.into()))?;
    Ok(rows
        .into_iter()
        .map(row_to_track)
        .collect::<Result<Vec<_>, _>>()?)
}

async fn all_tracks(state: &AppState, playlist_id: Uuid) -> AppResult<Vec<TrackOut>> {
    let rows = sqlx::query(
        "SELECT spotify_track_id, name, artists, album, image_url \
         FROM auto_playlist_tracks WHERE auto_playlist_id = $1 ORDER BY position",
    )
    .bind(playlist_id)
    .fetch_all(&state.db)
    .await
    .map_err(|e| AppError::Internal(e.into()))?;
    Ok(rows
        .into_iter()
        .map(row_to_track)
        .collect::<Result<Vec<_>, _>>()?)
}

fn row_to_track(r: sqlx::postgres::PgRow) -> AppResult<TrackOut> {
    Ok(TrackOut {
        id: r
            .try_get("spotify_track_id")
            .map_err(|e| AppError::Internal(e.into()))?,
        name: r
            .try_get("name")
            .map_err(|e| AppError::Internal(e.into()))?,
        artists: r
            .try_get("artists")
            .map_err(|e| AppError::Internal(e.into()))?,
        album: r
            .try_get("album")
            .map_err(|e| AppError::Internal(e.into()))?,
        image_url: r
            .try_get("image_url")
            .map_err(|e| AppError::Internal(e.into()))?,
    })
}

fn top_tags_in_cluster(
    track_ids: &[String],
    tags: &HashMap<String, Vec<String>>,
    limit: usize,
) -> Vec<String> {
    let mut counts: HashMap<String, u32> = HashMap::new();
    for id in track_ids {
        if let Some(tag_list) = tags.get(id) {
            for t in tag_list {
                *counts.entry(t.to_lowercase()).or_insert(0) += 1;
            }
        }
    }
    let mut sorted: Vec<(String, u32)> = counts.into_iter().collect();
    sorted.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    sorted.into_iter().take(limit).map(|(t, _)| t).collect()
}

#[derive(Debug, Deserialize)]
struct ClusterResponse {
    k: i32,
    labels: Vec<i32>,
    #[allow(dead_code)]
    centroids: Vec<Vec<f32>>,
}
