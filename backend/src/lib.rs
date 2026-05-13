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
pub mod youtube;

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
    pub youtube: Arc<youtube::YoutubeClient>,
    pub db: db::Pool,
    pub cookie_key: Key,
    pub cookie_secure: bool,
    pub spotify_oauth: auth::SpotifyOAuthConfig,
    pub web_url_after_login: String,
    pub oauth_state_key: [u8; 32],
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
    pub yt_scraper_url: String,
    pub getsongbpm_api_key: Option<String>,
    pub lastfm_api_key: Option<String>,
    pub user_agent: String,
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        let mut missing: Vec<&str> = Vec::new();
        let req = |key: &'static str, missing: &mut Vec<&'static str>| -> String {
            match std::env::var(key) {
                Ok(v) if !v.trim().is_empty() => v,
                _ => {
                    missing.push(key);
                    String::new()
                }
            }
        };
        let opt_nonempty = |key: &str| -> Option<String> {
            std::env::var(key).ok().filter(|v| !v.trim().is_empty())
        };

        let spotify_client_id = req("SPOTIFY_CLIENT_ID", &mut missing);
        let spotify_client_secret = req("SPOTIFY_CLIENT_SECRET", &mut missing);
        let spotify_redirect_uri = req("SPOTIFY_REDIRECT_URI", &mut missing);
        let database_url = req("DATABASE_URL", &mut missing);
        let session_secret = req("SESSION_SECRET", &mut missing);
        let web_url_after_login = req("WEB_URL_AFTER_LOGIN", &mut missing);
        let embedder_url = req("EMBEDDER_URL", &mut missing);
        let yt_scraper_url = req("YT_SCRAPER_URL", &mut missing);

        if !missing.is_empty() {
            anyhow::bail!(
                "missing or empty required env vars: {}",
                missing.join(", ")
            );
        }

        Ok(Self {
            spotify_client_id,
            spotify_client_secret,
            spotify_redirect_uri,
            web_url_after_login,
            database_url,
            session_secret,
            cookie_secure: std::env::var("COOKIE_SECURE")
                .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
                .unwrap_or(false),
            embedder_url,
            yt_scraper_url,
            embedder_model: std::env::var("EMBEDDER_MODEL")
                .unwrap_or_else(|_| "sentence-transformers/all-MiniLM-L6-v2".into()),
            getsongbpm_api_key: opt_nonempty("GETSONGBPM_API_KEY"),
            lastfm_api_key: opt_nonempty("LASTFM_API_KEY"),
            user_agent: std::env::var("USER_AGENT")
                .unwrap_or_else(|_| "spoti-backend/0.1 (https://spoti.phirios.com)".into()),
        })
    }
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
    use sha2::{Digest, Sha256, Sha512};
    let derived = Sha512::digest(cfg.session_secret.as_bytes());
    let cookie_key = Key::from(&derived);
    let mut h = Sha256::new();
    h.update(cfg.session_secret.as_bytes());
    h.update(b"oauth-state-v1");
    let oauth_state_key: [u8; 32] = h.finalize().into();

    let embedder = Arc::new(embeddings::EmbedderClient::new(
        http.clone(),
        cfg.embedder_url,
    ));
    let youtube = Arc::new(youtube::YoutubeClient::new(
        http.clone(),
        cfg.yt_scraper_url,
        embedder.clone(),
        pool.clone(),
    ));

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
        embedder,
        embedder_model: cfg.embedder_model,
        youtube,
        db: pool,
        cookie_key,
        cookie_secure: cfg.cookie_secure,
        spotify_oauth: auth::SpotifyOAuthConfig {
            client_id: cfg.spotify_client_id,
            client_secret: cfg.spotify_client_secret,
            redirect_uri: cfg.spotify_redirect_uri,
        },
        web_url_after_login: cfg.web_url_after_login,
        oauth_state_key,
    })
}
