use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use sqlx::Row;

use crate::db::Pool;
use crate::error::{AppError, AppResult};

#[derive(Debug, Serialize)]
pub struct EmbedRequest<'a> {
    pub texts: &'a [String],
    pub normalize: bool,
}

#[derive(Debug, Deserialize)]
pub struct EmbedResponse {
    pub model: String,
    pub dim: usize,
    pub embeddings: Vec<Vec<f32>>,
}

pub struct EmbedderClient {
    http: reqwest::Client,
    base_url: String,
}

impl EmbedderClient {
    pub fn new(http: reqwest::Client, base_url: String) -> Self {
        Self { http, base_url }
    }

    pub async fn embed(&self, texts: &[String]) -> AppResult<EmbedResponse> {
        let resp = self
            .http
            .post(format!("{}/embed", self.base_url))
            .json(&EmbedRequest {
                texts,
                normalize: true,
            })
            .send()
            .await?;
        if !resp.status().is_success() {
            let s = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(AppError::Upstream(format!(
                "embedder failed: {s}: {body}"
            )));
        }
        Ok(resp.json().await?)
    }
}

pub struct CachedEmbedding {
    pub track_id: String,
    pub embedding: Vec<f32>,
}

pub async fn fetch_cached(
    pool: &Pool,
    track_ids: &[String],
    model: &str,
) -> anyhow::Result<HashMap<String, Vec<f32>>> {
    if track_ids.is_empty() {
        return Ok(HashMap::new());
    }
    let rows = sqlx::query(
        "SELECT spotify_track_id, embedding FROM track_embeddings \
         WHERE model = $1 AND spotify_track_id = ANY($2)",
    )
    .bind(model)
    .bind(track_ids)
    .fetch_all(pool)
    .await?;
    let mut out = HashMap::with_capacity(rows.len());
    for r in rows {
        let id: String = r.try_get("spotify_track_id")?;
        let emb: Vec<f32> = r.try_get("embedding")?;
        out.insert(id, emb);
    }
    Ok(out)
}

pub async fn upsert_many(
    pool: &Pool,
    model: &str,
    rows: &[(String, String, Vec<f32>)],
) -> anyhow::Result<()> {
    if rows.is_empty() {
        return Ok(());
    }
    let mut tx = pool.begin().await?;
    for (id, text, emb) in rows {
        sqlx::query(
            "INSERT INTO track_embeddings (spotify_track_id, track_text, embedding, model) \
             VALUES ($1, $2, $3, $4) \
             ON CONFLICT (spotify_track_id) DO UPDATE SET \
                track_text = EXCLUDED.track_text, \
                embedding  = EXCLUDED.embedding, \
                model      = EXCLUDED.model, \
                updated_at = now()",
        )
        .bind(id)
        .bind(text)
        .bind(emb)
        .bind(model)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(())
}

/// Cosine similarity for two vectors of identical length.
/// Embeddings produced by the embedder are already L2-normalized, so this
/// reduces to the dot product, but we don't rely on that.
pub fn cosine(a: &[f32], b: &[f32]) -> f32 {
    let mut dot = 0.0f32;
    let mut na = 0.0f32;
    let mut nb = 0.0f32;
    for i in 0..a.len() {
        dot += a[i] * b[i];
        na += a[i] * a[i];
        nb += b[i] * b[i];
    }
    if na == 0.0 || nb == 0.0 {
        0.0
    } else {
        dot / (na.sqrt() * nb.sqrt())
    }
}
