use axum::extract::{Path, State, Query};
use axum::routing::get;
use axum::{Json, Router};
use serde::Deserialize;
use std::collections::BTreeSet;

use crate::error::AppResult;
use crate::model::{Album, Artist, Bpm, TrackInfo};
use crate::AppState;

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/tracks/:id", get(get_track))
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

async fn get_track(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(q): Query<TrackQuery>,
) -> AppResult<Json<TrackInfo>> {
    let track = state.spotify.get_track(&id).await?;

    let primary_artist = track
        .artists
        .first()
        .map(|a| a.name.clone())
        .unwrap_or_default();
    let artist_ids: Vec<String> = track.artists.iter().map(|a| a.id.clone()).collect();
    let title = track.name.clone();
    let album_name = track.album.name.clone();
    let duration_secs = track.duration_ms / 1000;

    let artists_fut = state.spotify.get_artists(&artist_ids);
    let lastfm_fut = state.lastfm.top_tags(&primary_artist, &title);
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
                .lookup(&primary_artist, &title, Some(&album_name), duration_secs)
                .await
        }
    };

    let (artists_res, lastfm_tags, bpm_val, lyrics) =
        tokio::join!(artists_fut, lastfm_fut, bpm_fut, lyrics_fut);

    let artist_genres: Vec<String> = artists_res
        .as_ref()
        .map(|v| v.iter().flat_map(|a| a.genres.clone()).collect())
        .unwrap_or_default();

    let mut genre_set: BTreeSet<String> = BTreeSet::new();
    for g in artist_genres.into_iter().chain(lastfm_tags.into_iter()) {
        let normalized = g.trim().to_lowercase();
        if !normalized.is_empty() {
            genre_set.insert(normalized);
        }
    }
    let genres: Vec<String> = genre_set.into_iter().collect();

    let info = TrackInfo {
        id: track.id.clone(),
        name: track.name.clone(),
        artists: track
            .artists
            .iter()
            .map(|a| Artist {
                id: a.id.clone(),
                name: a.name.clone(),
            })
            .collect(),
        album: Album {
            id: track.album.id.clone(),
            name: track.album.name.clone(),
            release_date: track.album.release_date.clone(),
            image_url: track
                .album
                .images
                .as_ref()
                .and_then(|imgs| imgs.first())
                .map(|i| i.url.clone()),
        },
        duration_ms: track.duration_ms,
        explicit: track.explicit,
        popularity: track.popularity,
        isrc: track.external_ids.and_then(|e| e.isrc),
        spotify_url: track.external_urls.and_then(|e| e.spotify),
        genres,
        bpm: bpm_val.map(|t| Bpm {
            tempo: t,
            source: "getsongbpm",
        }),
        lyrics,
    };

    Ok(Json(info))
}
