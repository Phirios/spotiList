use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::FromRow;
use uuid::Uuid;

use crate::db::Pool;

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct User {
    pub id: Uuid,
    pub spotify_id: String,
    pub display_name: Option<String>,
    pub email: Option<String>,
    #[serde(skip)]
    pub access_token: String,
    #[serde(skip)]
    pub refresh_token: String,
    pub expires_at: DateTime<Utc>,
    pub scope: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

pub struct UpsertUser<'a> {
    pub spotify_id: &'a str,
    pub display_name: Option<&'a str>,
    pub email: Option<&'a str>,
    pub access_token: &'a str,
    pub refresh_token: &'a str,
    pub expires_at: DateTime<Utc>,
    pub scope: &'a str,
}

pub async fn upsert(pool: &Pool, u: UpsertUser<'_>) -> anyhow::Result<User> {
    let row = sqlx::query_as::<_, User>(
        r#"
        INSERT INTO users (spotify_id, display_name, email, access_token, refresh_token, expires_at, scope)
        VALUES ($1, $2, $3, $4, $5, $6, $7)
        ON CONFLICT (spotify_id) DO UPDATE SET
            display_name = EXCLUDED.display_name,
            email = EXCLUDED.email,
            access_token = EXCLUDED.access_token,
            refresh_token = EXCLUDED.refresh_token,
            expires_at = EXCLUDED.expires_at,
            scope = EXCLUDED.scope,
            updated_at = now()
        RETURNING *
        "#,
    )
    .bind(u.spotify_id)
    .bind(u.display_name)
    .bind(u.email)
    .bind(u.access_token)
    .bind(u.refresh_token)
    .bind(u.expires_at)
    .bind(u.scope)
    .fetch_one(pool)
    .await?;
    Ok(row)
}

pub async fn find_by_id(pool: &Pool, id: Uuid) -> anyhow::Result<Option<User>> {
    let row = sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = $1")
        .bind(id)
        .fetch_optional(pool)
        .await?;
    Ok(row)
}

pub async fn update_tokens(
    pool: &Pool,
    id: Uuid,
    access_token: &str,
    refresh_token: Option<&str>,
    expires_at: DateTime<Utc>,
) -> anyhow::Result<()> {
    if let Some(refresh) = refresh_token {
        sqlx::query(
            "UPDATE users SET access_token=$2, refresh_token=$3, expires_at=$4, updated_at=now() WHERE id=$1",
        )
        .bind(id)
        .bind(access_token)
        .bind(refresh)
        .bind(expires_at)
        .execute(pool)
        .await?;
    } else {
        sqlx::query(
            "UPDATE users SET access_token=$2, expires_at=$3, updated_at=now() WHERE id=$1",
        )
        .bind(id)
        .bind(access_token)
        .bind(expires_at)
        .execute(pool)
        .await?;
    }
    Ok(())
}
