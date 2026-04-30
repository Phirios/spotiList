use base64::Engine;
use parking_lot::RwLock;
use serde::Deserialize;
use std::time::{Duration, Instant};

use crate::error::{AppError, AppResult};

const TOKEN_URL: &str = "https://accounts.spotify.com/api/token";
const API_BASE: &str = "https://api.spotify.com/v1";

pub struct SpotifyClient {
    http: reqwest::Client,
    client_id: String,
    client_secret: String,
    token: RwLock<Option<CachedToken>>,
}

#[derive(Clone)]
struct CachedToken {
    access_token: String,
    expires_at: Instant,
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    expires_in: u64,
}

#[derive(Debug, Deserialize)]
pub struct SpotifyTrack {
    pub id: String,
    pub name: String,
    pub duration_ms: u64,
    pub explicit: bool,
    pub popularity: u8,
    pub artists: Vec<SpotifyArtistRef>,
    pub album: SpotifyAlbum,
    pub external_ids: Option<SpotifyExternalIds>,
    pub external_urls: Option<SpotifyExternalUrls>,
}

#[derive(Debug, Deserialize)]
pub struct SpotifyArtistRef {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct SpotifyAlbum {
    pub id: String,
    pub name: String,
    pub release_date: Option<String>,
    pub images: Option<Vec<SpotifyImage>>,
}

#[derive(Debug, Deserialize)]
pub struct SpotifyImage {
    pub url: String,
    pub height: Option<u32>,
    pub width: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub struct SpotifyExternalIds {
    pub isrc: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SpotifyExternalUrls {
    pub spotify: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SpotifyArtist {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub genres: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct ArtistsResponse {
    artists: Vec<SpotifyArtist>,
}

impl SpotifyClient {
    pub fn new(http: reqwest::Client, client_id: String, client_secret: String) -> Self {
        Self {
            http,
            client_id,
            client_secret,
            token: RwLock::new(None),
        }
    }

    async fn token(&self) -> AppResult<String> {
        if let Some(t) = self.token.read().clone() {
            if t.expires_at > Instant::now() + Duration::from_secs(30) {
                return Ok(t.access_token);
            }
        }

        let basic = base64::engine::general_purpose::STANDARD
            .encode(format!("{}:{}", self.client_id, self.client_secret));

        let resp = self
            .http
            .post(TOKEN_URL)
            .header("Authorization", format!("Basic {basic}"))
            .form(&[("grant_type", "client_credentials")])
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(AppError::Auth(format!(
                "spotify token request failed: {status}: {body}"
            )));
        }

        let token: TokenResponse = resp.json().await?;
        let cached = CachedToken {
            access_token: token.access_token.clone(),
            expires_at: Instant::now() + Duration::from_secs(token.expires_in),
        };
        *self.token.write() = Some(cached);
        Ok(token.access_token)
    }

    pub async fn get_track(&self, id: &str) -> AppResult<SpotifyTrack> {
        let token = self.token().await?;
        let resp = self
            .http
            .get(format!("{API_BASE}/tracks/{id}"))
            .bearer_auth(token)
            .send()
            .await?;

        match resp.status() {
            s if s.is_success() => {
                let body = resp.text().await?;
                serde_json::from_str(&body).map_err(|e| {
                    tracing::error!(error = %e, body = %body, "failed to parse track response");
                    AppError::Upstream(format!("parse track {id}: {e}"))
                })
            }
            reqwest::StatusCode::NOT_FOUND => {
                Err(AppError::NotFound(format!("track {id} not found")))
            }
            s => {
                let body = resp.text().await.unwrap_or_default();
                Err(AppError::Upstream(format!(
                    "spotify get track failed: {s}: {body}"
                )))
            }
        }
    }

    pub async fn get_artists(&self, ids: &[String]) -> AppResult<Vec<SpotifyArtist>> {
        if ids.is_empty() {
            return Ok(vec![]);
        }
        let token = self.token().await?;
        let joined = ids.join(",");
        let resp = self
            .http
            .get(format!("{API_BASE}/artists"))
            .query(&[("ids", joined.as_str())])
            .bearer_auth(token)
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(AppError::Upstream(format!(
                "spotify get artists failed: {status}: {body}"
            )));
        }

        let body = resp.text().await?;
        serde_json::from_str::<ArtistsResponse>(&body)
            .map(|p| p.artists)
            .map_err(|e| {
                tracing::error!(error = %e, body = %body, "failed to parse artists response");
                AppError::Upstream(format!("parse artists: {e}"))
            })
    }
}
