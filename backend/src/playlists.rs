use std::collections::HashMap;

use axum::extract::State;
use axum::routing::post;
use axum::{Json, Router};
use axum_extra::extract::cookie::PrivateCookieJar;
use futures::stream::{self, StreamExt};
use serde::{Deserialize, Serialize};

use crate::auth::{current_user, ensure_fresh_token};
use crate::embeddings;
use crate::error::{AppError, AppResult};
use crate::AppState;

const SPOTIFY_API: &str = "https://api.spotify.com/v1";

/// Bumped whenever the `text_for(...)` format changes — appended to the
/// embedder model id when caching, so old embeddings get refreshed lazily.
const TEXT_VERSION: &str = "v2-tags";

const TAG_FETCH_CONCURRENCY: usize = 8;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/playlists/generate", post(generate))
        .route("/playlists/save", post(save))
}

#[derive(Debug, Deserialize)]
pub struct GenerateRequest {
    pub vibe: String,
    #[serde(default = "default_limit")]
    pub limit: usize,
}

fn default_limit() -> usize {
    20
}

#[derive(Debug, Serialize)]
pub struct GeneratedPlaylist {
    pub vibe: String,
    pub model: String,
    pub considered: usize,
    pub items: Vec<RankedTrack>,
}

#[derive(Debug, Serialize)]
pub struct RankedTrack {
    pub id: String,
    pub name: String,
    pub artists: Vec<String>,
    pub album: String,
    pub image_url: Option<String>,
    pub score: f32,
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

#[derive(Debug, Deserialize, Clone)]
struct TrackLite {
    id: Option<String>,
    name: String,
    artists: Vec<ArtistLite>,
    album: AlbumLite,
}

#[derive(Debug, Deserialize, Clone)]
struct ArtistLite {
    name: String,
}

#[derive(Debug, Deserialize, Clone)]
struct AlbumLite {
    name: String,
    #[serde(default)]
    images: Vec<ImageLite>,
}

#[derive(Debug, Deserialize, Clone)]
struct ImageLite {
    url: String,
}

async fn generate(
    State(state): State<AppState>,
    jar: PrivateCookieJar,
    Json(req): Json<GenerateRequest>,
) -> AppResult<Json<GeneratedPlaylist>> {
    if req.vibe.trim().is_empty() {
        return Err(AppError::Upstream("vibe is empty".into()));
    }

    let mut user = current_user(&state, &jar).await?;
    let token = ensure_fresh_token(&state, &mut user).await?;
    let stored_model = format!("{}#{}", state.embedder_model, TEXT_VERSION);

    let liked = fetch_all_liked(&state, &token).await?;
    let considered = liked.len();
    if liked.is_empty() {
        return Ok(Json(GeneratedPlaylist {
            vibe: req.vibe,
            model: stored_model,
            considered: 0,
            items: vec![],
        }));
    }

    let liked_with_id: Vec<(String, TrackLite)> = liked
        .iter()
        .filter_map(|t| t.id.as_ref().map(|id| (id.clone(), t.clone())))
        .collect();

    let ids: Vec<String> = liked_with_id.iter().map(|(id, _)| id.clone()).collect();
    let cached = embeddings::fetch_cached(&state.db, &ids, &stored_model)
        .await
        .map_err(|e| AppError::Internal(e.into()))?;

    let missing: Vec<(String, TrackLite)> = liked_with_id
        .iter()
        .filter(|(id, _)| !cached.contains_key(id))
        .cloned()
        .collect();

    let mut all_embeddings = cached;

    if !missing.is_empty() {
        // Fetch Last.fm tags concurrently for the missing tracks.
        let tag_map = fetch_tags_concurrent(&state, &missing).await;

        // Build enriched text per missing track.
        let enriched: Vec<(String, String)> = missing
            .iter()
            .map(|(id, t)| {
                let tags = tag_map.get(id).cloned().unwrap_or_default();
                (id.clone(), text_for(t, &tags))
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
                all_embeddings.insert(id, emb);
            }
        }
    }

    let prompt_resp = state.embedder.embed(&[req.vibe.clone()]).await?;
    let prompt_vec = prompt_resp
        .embeddings
        .into_iter()
        .next()
        .ok_or_else(|| AppError::Upstream("empty prompt embedding".into()))?;

    let mut scored: Vec<RankedTrack> = liked
        .iter()
        .filter_map(|t| {
            let id = t.id.as_ref()?;
            let emb = all_embeddings.get(id)?;
            let score = embeddings::cosine(&prompt_vec, emb);
            Some(RankedTrack {
                id: id.clone(),
                name: t.name.clone(),
                artists: t.artists.iter().map(|a| a.name.clone()).collect(),
                album: t.album.name.clone(),
                image_url: t.album.images.first().map(|i| i.url.clone()),
                score,
            })
        })
        .collect();

    scored.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    scored.truncate(req.limit.clamp(1, 100));

    Ok(Json(GeneratedPlaylist {
        vibe: req.vibe,
        model: stored_model,
        considered,
        items: scored,
    }))
}

async fn fetch_tags_concurrent(
    state: &AppState,
    tracks: &[(String, TrackLite)],
) -> HashMap<String, Vec<String>> {
    let lastfm = state.lastfm.clone();
    let results = stream::iter(tracks.iter().cloned())
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
        .await;
    results.into_iter().collect()
}

fn text_for(t: &TrackLite, tags: &[String]) -> String {
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

#[derive(Debug, Deserialize)]
pub struct SaveRequest {
    pub name: String,
    pub vibe: Option<String>,
    pub track_ids: Vec<String>,
    #[serde(default)]
    pub public: bool,
}

#[derive(Debug, Serialize)]
pub struct SaveResponse {
    pub playlist_id: String,
    pub url: String,
    pub name: String,
    pub track_count: usize,
}

#[derive(Debug, Deserialize)]
struct CreatedPlaylist {
    id: String,
    #[serde(default)]
    external_urls: Option<ExternalUrls>,
}

#[derive(Debug, Deserialize)]
struct ExternalUrls {
    #[serde(default)]
    spotify: Option<String>,
}

async fn save(
    State(state): State<AppState>,
    jar: PrivateCookieJar,
    Json(req): Json<SaveRequest>,
) -> AppResult<Json<SaveResponse>> {
    if req.name.trim().is_empty() {
        return Err(AppError::Upstream("name is empty".into()));
    }
    if req.track_ids.is_empty() {
        return Err(AppError::Upstream("no tracks to save".into()));
    }

    let mut user = current_user(&state, &jar).await?;
    let token = ensure_fresh_token(&state, &mut user).await?;

    let description = match req.vibe.as_deref() {
        Some(v) if !v.trim().is_empty() => format!(
            "Generated by spoti.phirios.com — vibe: {}",
            v.trim().chars().take(180).collect::<String>()
        ),
        _ => "Generated by spoti.phirios.com".into(),
    };

    // 1. Create the playlist
    let create_url = format!("{SPOTIFY_API}/users/{}/playlists", user.spotify_id);
    let create_body = serde_json::json!({
        "name": req.name,
        "description": description,
        "public": req.public,
    });
    let resp = state
        .http
        .post(&create_url)
        .bearer_auth(&token)
        .json(&create_body)
        .send()
        .await?;
    if !resp.status().is_success() {
        let s = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(AppError::Upstream(format!(
            "create playlist failed: {s}: {body}"
        )));
    }
    let created: CreatedPlaylist = resp.json().await?;

    // 2. Add tracks in chunks of 100 (Spotify limit)
    let uris: Vec<String> = req
        .track_ids
        .iter()
        .map(|id| format!("spotify:track:{id}"))
        .collect();
    let add_url = format!("{SPOTIFY_API}/playlists/{}/tracks", created.id);
    for chunk in uris.chunks(100) {
        let body = serde_json::json!({ "uris": chunk });
        let resp = state
            .http
            .post(&add_url)
            .bearer_auth(&token)
            .json(&body)
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

    let url = created
        .external_urls
        .and_then(|u| u.spotify)
        .unwrap_or_else(|| format!("https://open.spotify.com/playlist/{}", created.id));

    Ok(Json(SaveResponse {
        playlist_id: created.id,
        url,
        name: req.name,
        track_count: uris.len(),
    }))
}

async fn fetch_all_liked(state: &AppState, token: &str) -> AppResult<Vec<TrackLite>> {
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
