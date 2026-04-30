use axum::extract::State;
use axum::routing::post;
use axum::{Json, Router};
use axum_extra::extract::cookie::PrivateCookieJar;
use serde::{Deserialize, Serialize};

use crate::auth::{current_user, ensure_fresh_token};
use crate::embeddings;
use crate::error::{AppError, AppResult};
use crate::AppState;

const SPOTIFY_API: &str = "https://api.spotify.com/v1";

pub fn router() -> Router<AppState> {
    Router::new().route("/playlists/generate", post(generate))
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

#[derive(Debug, Deserialize)]
struct TrackLite {
    id: Option<String>,
    name: String,
    artists: Vec<ArtistLite>,
    album: AlbumLite,
}

#[derive(Debug, Deserialize)]
struct ArtistLite {
    name: String,
}

#[derive(Debug, Deserialize)]
struct AlbumLite {
    name: String,
    #[serde(default)]
    images: Vec<ImageLite>,
}

#[derive(Debug, Deserialize)]
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

    // 1. Fetch all liked tracks (paginated)
    let liked = fetch_all_liked(&state, &token).await?;
    let considered = liked.len();
    if liked.is_empty() {
        return Ok(Json(GeneratedPlaylist {
            vibe: req.vibe,
            model: state.embedder_model.clone(),
            considered: 0,
            items: vec![],
        }));
    }

    // 2. Build text representations
    let texts: Vec<(String, String)> = liked
        .iter()
        .filter_map(|t| t.id.as_ref().map(|id| (id.clone(), text_for(t))))
        .collect();

    // 3. Look up cached embeddings; embed misses
    let ids: Vec<String> = texts.iter().map(|(id, _)| id.clone()).collect();
    let cached = embeddings::fetch_cached(&state.db, &ids, &state.embedder_model)
        .await
        .map_err(|e| AppError::Internal(e.into()))?;

    let missing: Vec<(String, String)> = texts
        .iter()
        .filter(|(id, _)| !cached.contains_key(id))
        .cloned()
        .collect();

    let mut all_embeddings = cached;

    if !missing.is_empty() {
        // Embed in batches of 128 to keep request size reasonable
        for chunk in missing.chunks(128) {
            let batch_texts: Vec<String> =
                chunk.iter().map(|(_, t)| t.clone()).collect();
            let resp = state.embedder.embed(&batch_texts).await?;
            let to_upsert: Vec<(String, String, Vec<f32>)> = chunk
                .iter()
                .zip(resp.embeddings.iter())
                .map(|((id, text), emb)| (id.clone(), text.clone(), emb.clone()))
                .collect();
            embeddings::upsert_many(&state.db, &state.embedder_model, &to_upsert)
                .await
                .map_err(|e| AppError::Internal(e.into()))?;
            for (id, _, emb) in to_upsert {
                all_embeddings.insert(id, emb);
            }
        }
    }

    // 4. Embed the vibe prompt
    let prompt_resp = state
        .embedder
        .embed(&[req.vibe.clone()])
        .await?;
    let prompt_vec = prompt_resp
        .embeddings
        .into_iter()
        .next()
        .ok_or_else(|| AppError::Upstream("empty prompt embedding".into()))?;

    // 5. Score and rank
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

    scored.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    scored.truncate(req.limit.clamp(1, 100));

    Ok(Json(GeneratedPlaylist {
        vibe: req.vibe,
        model: state.embedder_model.clone(),
        considered,
        items: scored,
    }))
}

fn text_for(t: &TrackLite) -> String {
    let artists = t
        .artists
        .iter()
        .map(|a| a.name.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    format!("{} by {} from album {}", t.name, artists, t.album.name)
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
