use axum::extract::{Query, State};
use axum::response::Redirect;
use axum::routing::{get, post};
use axum::{Json, Router};
use axum_extra::extract::cookie::{Cookie, Key, PrivateCookieJar, SameSite};
use chrono::{Duration, Utc};
use rand::distributions::{Alphanumeric, DistString};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::{AppError, AppResult};
use crate::user::{self, UpsertUser, User};
use crate::AppState;

const STATE_COOKIE: &str = "spoti_state";
const SESSION_COOKIE: &str = "spoti_session";
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

async fn login(
    State(state): State<AppState>,
    jar: PrivateCookieJar,
) -> (PrivateCookieJar, Redirect) {
    let csrf = Alphanumeric.sample_string(&mut rand::thread_rng(), 32);
    let cookie = Cookie::build((STATE_COOKIE, csrf.clone()))
        .http_only(true)
        .secure(state.cookie_secure)
        .same_site(SameSite::Lax)
        .path("/")
        .max_age(time::Duration::minutes(10))
        .build();

    let url = format!(
        "{SPOTIFY_AUTHORIZE}?response_type=code&client_id={cid}&scope={scope}&redirect_uri={ruri}&state={st}",
        cid = urlencode(&state.spotify_oauth.client_id),
        scope = urlencode(SCOPES),
        ruri = urlencode(&state.spotify_oauth.redirect_uri),
        st = urlencode(&csrf),
    );

    (jar.add(cookie), Redirect::to(&url))
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

    let stored_state = jar
        .get(STATE_COOKIE)
        .map(|c| c.value().to_string())
        .ok_or_else(|| AppError::Auth("missing state cookie".into()))?;
    if stored_state != returned_state {
        return Err(AppError::Auth("state mismatch".into()));
    }

    let token = exchange_code(&state, &code).await?;
    let profile = fetch_profile(&state, &token.access_token).await?;
    let expires_at = Utc::now() + Duration::seconds(token.expires_in.max(0));
    let refresh = token
        .refresh_token
        .as_deref()
        .ok_or_else(|| AppError::Auth("no refresh_token".into()))?;

    let saved = user::upsert(
        &state.db,
        UpsertUser {
            spotify_id: &profile.id,
            display_name: profile.display_name.as_deref(),
            email: profile.email.as_deref(),
            access_token: &token.access_token,
            refresh_token: refresh,
            expires_at,
            scope: token.scope.as_deref().unwrap_or(SCOPES),
        },
    )
    .await
    .map_err(|e| AppError::Internal(e.into()))?;

    let session_cookie = Cookie::build((SESSION_COOKIE, saved.id.to_string()))
        .http_only(true)
        .secure(state.cookie_secure)
        .same_site(SameSite::Lax)
        .path("/")
        .max_age(time::Duration::days(30))
        .build();
    let removal = Cookie::build(STATE_COOKIE).path("/").build();
    let jar = jar.remove(removal).add(session_cookie);

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
    }))
}

#[derive(Debug, Serialize)]
struct MeResponse {
    id: Uuid,
    spotify_id: String,
    display_name: Option<String>,
    email: Option<String>,
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
