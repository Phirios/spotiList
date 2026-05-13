use axum::extract::{Query, State};
use axum::response::Redirect;
use axum::routing::{get, post};
use axum::{Json, Router};
use axum_extra::extract::cookie::{Cookie, Key, PrivateCookieJar, SameSite};
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use chrono::{Duration, Utc};
use hmac::{Hmac, Mac};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use uuid::Uuid;

use crate::error::{AppError, AppResult};
use crate::user::{self, UpsertUser, User};
use crate::AppState;

const SESSION_COOKIE: &str = "spoti_session";
const STATE_TTL: i64 = 600;

type HmacSha256 = Hmac<Sha256>;
const SCOPES: &str = "user-read-email user-read-private user-library-read \
                      playlist-modify-public playlist-modify-private";

const SPOTIFY_AUTHORIZE: &str = "https://accounts.spotify.com/authorize";
const SPOTIFY_TOKEN: &str = "https://accounts.spotify.com/api/token";
const SPOTIFY_ME: &str = "https://api.spotify.com/v1/me";

#[derive(Debug, Clone)]
pub struct SpotifyOAuthConfig {
    pub client_id: String,
    pub client_secret: String,
    pub redirect_uri: String,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/auth/login", get(login))
        .route("/auth/callback", get(callback))
        .route("/auth/logout", post(logout))
        .route("/auth/me", get(me))
}

async fn login(State(state): State<AppState>) -> Redirect {
    let st = mint_state(&state.oauth_state_key);
    let url = format!(
        "{SPOTIFY_AUTHORIZE}?response_type=code&client_id={cid}&scope={scope}&redirect_uri={ruri}&state={st_q}",
        cid = urlencode(&state.spotify_oauth.client_id),
        scope = urlencode(SCOPES),
        ruri = urlencode(&state.spotify_oauth.redirect_uri),
        st_q = urlencode(&st),
    );
    Redirect::to(&url)
}

fn mint_state(key: &[u8; 32]) -> String {
    let mut payload = [0u8; 24];
    rand::thread_rng().fill_bytes(&mut payload[..16]);
    let ts = Utc::now().timestamp();
    payload[16..].copy_from_slice(&ts.to_be_bytes());
    let mut mac = HmacSha256::new_from_slice(key).expect("hmac key");
    mac.update(&payload);
    let sig = mac.finalize().into_bytes();
    let mut out = Vec::with_capacity(24 + 32);
    out.extend_from_slice(&payload);
    out.extend_from_slice(&sig);
    URL_SAFE_NO_PAD.encode(out)
}

fn verify_state(key: &[u8; 32], state: &str) -> Result<(), &'static str> {
    let raw = URL_SAFE_NO_PAD.decode(state).map_err(|_| "bad state encoding")?;
    if raw.len() != 56 {
        return Err("bad state length");
    }
    let (payload, sig) = raw.split_at(24);
    let mut mac = HmacSha256::new_from_slice(key).expect("hmac key");
    mac.update(payload);
    mac.verify_slice(sig).map_err(|_| "state signature invalid")?;
    let ts = i64::from_be_bytes(payload[16..24].try_into().unwrap());
    let age = Utc::now().timestamp() - ts;
    if !(0..=STATE_TTL).contains(&age) {
        return Err("state expired");
    }
    Ok(())
}

#[derive(Debug, Deserialize)]
struct CallbackParams {
    code: Option<String>,
    state: Option<String>,
    error: Option<String>,
}

async fn callback(
    State(state): State<AppState>,
    Query(p): Query<CallbackParams>,
    jar: PrivateCookieJar,
) -> AppResult<(PrivateCookieJar, Redirect)> {
    if let Some(err) = p.error {
        return Err(AppError::Auth(format!("spotify oauth error: {err}")));
    }
    let code = p.code.ok_or_else(|| AppError::Auth("missing code".into()))?;
    let returned_state = p
        .state
        .ok_or_else(|| AppError::Auth("missing state".into()))?;

    verify_state(&state.oauth_state_key, &returned_state)
        .map_err(|e| AppError::Auth(e.into()))?;

    let token = exchange_code(&state, &code).await?;
    let profile = fetch_profile(&state, &token.access_token).await?;
    let expires_at = Utc::now() + Duration::seconds(token.expires_in.max(0));
    let refresh = token
        .refresh_token
        .as_deref()
        .ok_or_else(|| AppError::Auth("no refresh_token".into()))?;

    // Pick the smallest image >= 64px, falling back to the first one.
    let image_url = pick_profile_image(&profile.images);

    let saved = user::upsert(
        &state.db,
        UpsertUser {
            spotify_id: &profile.id,
            display_name: profile.display_name.as_deref(),
            email: profile.email.as_deref(),
            image_url: image_url.as_deref(),
            access_token: &token.access_token,
            refresh_token: refresh,
            expires_at,
            scope: token.scope.as_deref().unwrap_or(SCOPES),
        },
    )
    .await
    .map_err(|e| AppError::Internal(e.into()))?;

    // Kick off a library sync in the background so the user lands on a
    // dashboard that's already populating instead of an empty one.
    crate::sync::spawn_sync(state.clone(), saved.id);

    let session_cookie = Cookie::build((SESSION_COOKIE, saved.id.to_string()))
        .http_only(true)
        .secure(state.cookie_secure)
        .same_site(SameSite::Lax)
        .path("/")
        .max_age(time::Duration::days(30))
        .build();
    let jar = jar.add(session_cookie);

    Ok((jar, Redirect::to(&state.web_url_after_login)))
}

async fn logout(jar: PrivateCookieJar) -> (PrivateCookieJar, Json<serde_json::Value>) {
    let removal = Cookie::build(SESSION_COOKIE).path("/").build();
    (jar.remove(removal), Json(serde_json::json!({"ok": true})))
}

async fn me(
    State(state): State<AppState>,
    jar: PrivateCookieJar,
) -> AppResult<Json<MeResponse>> {
    let user = current_user(&state, &jar).await?;
    Ok(Json(MeResponse {
        id: user.id,
        spotify_id: user.spotify_id,
        display_name: user.display_name,
        email: user.email,
        image_url: user.image_url,
    }))
}

#[derive(Debug, Serialize)]
struct MeResponse {
    id: Uuid,
    spotify_id: String,
    display_name: Option<String>,
    email: Option<String>,
    image_url: Option<String>,
}

fn pick_profile_image(images: &[SpotifyImage]) -> Option<String> {
    if images.is_empty() {
        return None;
    }
    // Prefer a small-but-not-tiny image (~ 64–300px tall) for a 32px avatar.
    images
        .iter()
        .filter(|i| matches!(i.height, Some(h) if (64..=300).contains(&h)))
        .min_by_key(|i| i.height.unwrap_or(u32::MAX))
        .map(|i| i.url.clone())
        .or_else(|| images.first().map(|i| i.url.clone()))
}

pub async fn current_user(state: &AppState, jar: &PrivateCookieJar) -> AppResult<User> {
    let id_str = jar
        .get(SESSION_COOKIE)
        .map(|c| c.value().to_string())
        .ok_or_else(|| AppError::Auth("not logged in".into()))?;
    let id = Uuid::parse_str(&id_str).map_err(|_| AppError::Auth("bad session".into()))?;
    let user = user::find_by_id(&state.db, id)
        .await
        .map_err(|e| AppError::Internal(e.into()))?
        .ok_or_else(|| AppError::Auth("user not found".into()))?;
    Ok(user)
}

/// Returns a valid (non-expired) access token for the given user, refreshing
/// if needed. Persists any new tokens back to the DB.
pub async fn ensure_fresh_token(state: &AppState, user: &mut User) -> AppResult<String> {
    if user.expires_at > Utc::now() + Duration::seconds(30) {
        return Ok(user.access_token.clone());
    }
    let refreshed = refresh_token(state, &user.refresh_token).await?;
    let expires_at = Utc::now() + Duration::seconds(refreshed.expires_in.max(0));
    user.access_token = refreshed.access_token.clone();
    if let Some(rt) = refreshed.refresh_token.clone() {
        user.refresh_token = rt;
    }
    user.expires_at = expires_at;
    user::update_tokens(
        &state.db,
        user.id,
        &user.access_token,
        refreshed.refresh_token.as_deref(),
        expires_at,
    )
    .await
    .map_err(|e| AppError::Internal(e.into()))?;
    Ok(user.access_token.clone())
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    #[serde(default)]
    refresh_token: Option<String>,
    expires_in: i64,
    #[serde(default)]
    scope: Option<String>,
}

async fn exchange_code(state: &AppState, code: &str) -> AppResult<TokenResponse> {
    let basic = basic_auth(
        &state.spotify_oauth.client_id,
        &state.spotify_oauth.client_secret,
    );
    let resp = state
        .http
        .post(SPOTIFY_TOKEN)
        .header("Authorization", format!("Basic {basic}"))
        .form(&[
            ("grant_type", "authorization_code"),
            ("code", code),
            ("redirect_uri", &state.spotify_oauth.redirect_uri),
        ])
        .send()
        .await?;
    if !resp.status().is_success() {
        let s = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(AppError::Auth(format!("token exchange failed: {s}: {body}")));
    }
    Ok(resp.json().await?)
}

async fn refresh_token(state: &AppState, refresh: &str) -> AppResult<TokenResponse> {
    let basic = basic_auth(
        &state.spotify_oauth.client_id,
        &state.spotify_oauth.client_secret,
    );
    let resp = state
        .http
        .post(SPOTIFY_TOKEN)
        .header("Authorization", format!("Basic {basic}"))
        .form(&[("grant_type", "refresh_token"), ("refresh_token", refresh)])
        .send()
        .await?;
    if !resp.status().is_success() {
        let s = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(AppError::Auth(format!("refresh failed: {s}: {body}")));
    }
    Ok(resp.json().await?)
}

#[derive(Debug, Deserialize)]
struct SpotifyMe {
    id: String,
    display_name: Option<String>,
    email: Option<String>,
    #[serde(default)]
    images: Vec<SpotifyImage>,
}

#[derive(Debug, Deserialize)]
struct SpotifyImage {
    url: String,
    #[serde(default)]
    height: Option<u32>,
}

async fn fetch_profile(state: &AppState, token: &str) -> AppResult<SpotifyMe> {
    let resp = state
        .http
        .get(SPOTIFY_ME)
        .bearer_auth(token)
        .send()
        .await?;
    if !resp.status().is_success() {
        let s = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(AppError::Auth(format!("/me failed: {s}: {body}")));
    }
    Ok(resp.json().await?)
}

fn basic_auth(id: &str, secret: &str) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(format!("{id}:{secret}"))
}

fn urlencode(s: &str) -> String {
    url::form_urlencoded::byte_serialize(s.as_bytes()).collect()
}

impl axum::extract::FromRef<AppState> for Key {
    fn from_ref(state: &AppState) -> Self {
        state.cookie_key.clone()
    }
}
