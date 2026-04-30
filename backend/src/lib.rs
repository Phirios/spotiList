pub mod auth;
pub mod auto;
pub mod bpm;
pub mod db;
pub mod embeddings;
pub mod error;
pub mod lastfm;
pub mod library;
pub mod sync;
pub mod lyrics;
pub mod me;
pub mod model;
pub mod playlists;
pub mod routes;
pub mod spotify;
pub mod user;

use std::sync::Arc;

use axum_extra::extract::cookie::Key;

#[derive(Clone)]
pub struct AppState {
    pub http: reqwest::Client,
    pub spotify: Arc<spotify::SpotifyClient>,
    pub bpm: Arc<bpm::GetSongBpmClient>,
    pub lyrics: Arc<lyrics::LrcLibClient>,
    pub lastfm: Arc<lastfm::LastFmClient>,
    pub embedder: Arc<embeddings::EmbedderClient>,
    pub embedder_model: String,
    pub db: db::Pool,
    pub cookie_key: Key,
    pub cookie_secure: bool,
    pub spotify_oauth: auth::SpotifyOAuthConfig,
    pub web_url_after_login: String,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub spotify_client_id: String,
    pub spotify_client_secret: String,
    pub spotify_redirect_uri: String,
    pub web_url_after_login: String,
    pub database_url: String,
    pub session_secret: String,
    pub cookie_secure: bool,
    pub embedder_url: String,
    pub embedder_model: String,
    pub getsongbpm_api_key: Option<String>,
    pub lastfm_api_key: Option<String>,
    pub user_agent: String,
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        Ok(Self {
            spotify_client_id: req_env("SPOTIFY_CLIENT_ID")?,
            spotify_client_secret: req_env("SPOTIFY_CLIENT_SECRET")?,
            spotify_redirect_uri: req_env("SPOTIFY_REDIRECT_URI")?,
            web_url_after_login: std::env::var("WEB_URL_AFTER_LOGIN")
                .unwrap_or_else(|_| "/dashboard".into()),
            database_url: req_env("DATABASE_URL")?,
            session_secret: req_env("SESSION_SECRET")?,
            cookie_secure: std::env::var("COOKIE_SECURE")
                .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
                .unwrap_or(false),
            embedder_url: std::env::var("EMBEDDER_URL")
                .unwrap_or_else(|_| "http://spoti-embedder.nlp-project.svc.cluster.local:8000".into()),
            embedder_model: std::env::var("EMBEDDER_MODEL")
                .unwrap_or_else(|_| "sentence-transformers/all-MiniLM-L6-v2".into()),
            getsongbpm_api_key: std::env::var("GETSONGBPM_API_KEY").ok(),
            lastfm_api_key: std::env::var("LASTFM_API_KEY").ok(),
            user_agent: std::env::var("USER_AGENT")
                .unwrap_or_else(|_| "spoti-backend/0.1 (https://spoti.phirios.com)".into()),
        })
    }
}

fn req_env(key: &str) -> anyhow::Result<String> {
    std::env::var(key).map_err(|_| anyhow::anyhow!("{key} not set"))
}

pub async fn build_state(cfg: Config) -> anyhow::Result<AppState> {
    let http = reqwest::Client::builder()
        .user_agent(&cfg.user_agent)
        .timeout(std::time::Duration::from_secs(90))
        .build()?;

    let pool = db::connect(&cfg.database_url).await?;

    if cfg.session_secret.as_bytes().len() < 32 {
        anyhow::bail!("SESSION_SECRET must be at least 32 bytes");
    }
    use sha2::{Digest, Sha512};
    let derived = Sha512::digest(cfg.session_secret.as_bytes());
    let cookie_key = Key::from(&derived);

    Ok(AppState {
        http: http.clone(),
        spotify: Arc::new(spotify::SpotifyClient::new(
            http.clone(),
            cfg.spotify_client_id.clone(),
            cfg.spotify_client_secret.clone(),
        )),
        bpm: Arc::new(bpm::GetSongBpmClient::new(
            http.clone(),
            cfg.getsongbpm_api_key,
            pool.clone(),
        )),
        lyrics: Arc::new(lyrics::LrcLibClient::new(http.clone(), pool.clone())),
        lastfm: Arc::new(lastfm::LastFmClient::new(
            http.clone(),
            cfg.lastfm_api_key,
            pool.clone(),
        )),
        embedder: Arc::new(embeddings::EmbedderClient::new(
            http.clone(),
            cfg.embedder_url,
        )),
        embedder_model: cfg.embedder_model,
        db: pool,
        cookie_key,
        cookie_secure: cfg.cookie_secure,
        spotify_oauth: auth::SpotifyOAuthConfig {
            client_id: cfg.spotify_client_id,
            client_secret: cfg.spotify_client_secret,
            redirect_uri: cfg.spotify_redirect_uri,
        },
        web_url_after_login: cfg.web_url_after_login,
    })
}
