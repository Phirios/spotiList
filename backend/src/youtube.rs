use std::sync::Arc;

use serde::{Deserialize, Serialize};
use sqlx::Row;

use crate::db::Pool;
use crate::embeddings::EmbedderClient;
use crate::error::{AppError, AppResult};

const EMOTION_LABELS: [&str; 7] = [
    "joy", "sadness", "anger", "fear", "surprise", "disgust", "neutral",
];

#[derive(Debug, Clone)]
pub struct EmotionVec {
    pub joy: f32,
    pub sadness: f32,
    pub anger: f32,
    pub fear: f32,
    pub surprise: f32,
    pub disgust: f32,
    pub neutral: f32,
    pub comment_count: i32,
}

pub struct YoutubeClient {
    http: reqwest::Client,
    scraper_url: String,
    embedder: Arc<EmbedderClient>,
    db: Pool,
}

impl YoutubeClient {
    pub fn new(
        http: reqwest::Client,
        scraper_url: String,
        embedder: Arc<EmbedderClient>,
        db: Pool,
    ) -> Self {
        Self {
            http,
            scraper_url,
            embedder,
            db,
        }
    }

    /// Top-level: returns the emotion vector for a track, computing it
    /// (match → comments → emotion) and caching at every step.
    pub async fn ensure_emotion(
        &self,
        spotify_track_id: &str,
        title: &str,
        artist: &str,
        duration_sec: Option<i32>,
    ) -> AppResult<Option<EmotionVec>> {
        if let Some(cached) = self.cached_emotion(spotify_track_id).await? {
            return Ok(Some(cached));
        }

        let video_id = match self
            .ensure_video(spotify_track_id, title, artist, duration_sec)
            .await?
        {
            Some(v) => v,
            None => return Ok(None),
        };

        let comments = self.ensure_comments(&video_id).await?;
        if comments.is_empty() {
            return Ok(None);
        }

        let texts: Vec<String> = comments.iter().map(|c| c.text.clone()).collect();
        let weights: Vec<f32> = comments
            .iter()
            .map(|c| (c.likes as f32 + 1.0).ln())
            .collect();
        let resp = self.embedder.emotion(&texts, Some(&weights)).await?;

        let agg = &resp.aggregate;
        let pick = |name: &str| -> AppResult<f32> {
            let i = resp
                .labels
                .iter()
                .position(|l| l == name)
                .ok_or_else(|| AppError::Upstream(format!("emotion label missing: {name}")))?;
            agg.get(i)
                .copied()
                .ok_or_else(|| AppError::Upstream(format!("emotion score missing: {name}")))
        };
        let vec = EmotionVec {
            joy: pick("joy")?,
            sadness: pick("sadness")?,
            anger: pick("anger")?,
            fear: pick("fear")?,
            surprise: pick("surprise")?,
            disgust: pick("disgust")?,
            neutral: pick("neutral")?,
            comment_count: comments.len() as i32,
        };
        self.store_emotion(spotify_track_id, &vec).await?;
        Ok(Some(vec))
    }

    async fn ensure_video(
        &self,
        spotify_track_id: &str,
        title: &str,
        artist: &str,
        duration_sec: Option<i32>,
    ) -> AppResult<Option<String>> {
        if let Some(v) = self.cached_video(spotify_track_id).await? {
            return Ok(Some(v));
        }
        let resp = self
            .http
            .post(format!("{}/match", self.scraper_url))
            .json(&MatchRequest {
                title,
                artist,
                duration_sec,
            })
            .send()
            .await?;
        if resp.status() == 404 {
            return Ok(None);
        }
        if !resp.status().is_success() {
            let s = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(AppError::Upstream(format!(
                "yt-scraper match failed: {s}: {body}"
            )));
        }
        let m: MatchResponse = resp.json().await?;
        self.store_video(spotify_track_id, &m).await?;
        Ok(Some(m.video_id))
    }

    async fn ensure_comments(&self, video_id: &str) -> AppResult<Vec<Comment>> {
        if let Some(c) = self.cached_comments(video_id).await? {
            return Ok(c);
        }
        let resp = self
            .http
            .post(format!("{}/comments", self.scraper_url))
            .json(&CommentsRequest {
                video_id,
                max_comments: 200,
            })
            .send()
            .await?;
        if !resp.status().is_success() {
            let s = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(AppError::Upstream(format!(
                "yt-scraper comments failed: {s}: {body}"
            )));
        }
        let body: CommentsResponse = resp.json().await?;
        self.store_comments(video_id, &body.comments).await?;
        Ok(body.comments)
    }

    async fn cached_video(&self, id: &str) -> sqlx::Result<Option<String>> {
        let row = sqlx::query(
            "SELECT video_id FROM track_youtube WHERE spotify_track_id = $1",
        )
        .bind(id)
        .fetch_optional(&self.db)
        .await?;
        match row {
            None => Ok(None),
            Some(r) => r.try_get::<Option<String>, _>("video_id"),
        }
    }

    async fn store_video(&self, id: &str, m: &MatchResponse) -> sqlx::Result<()> {
        sqlx::query(
            "INSERT INTO track_youtube \
                (spotify_track_id, video_id, video_title, video_channel, match_score, matched_at) \
             VALUES ($1, $2, $3, $4, $5, now()) \
             ON CONFLICT (spotify_track_id) DO UPDATE SET \
                video_id = EXCLUDED.video_id, \
                video_title = EXCLUDED.video_title, \
                video_channel = EXCLUDED.video_channel, \
                match_score = EXCLUDED.match_score, \
                matched_at = now()",
        )
        .bind(id)
        .bind(&m.video_id)
        .bind(&m.title)
        .bind(&m.channel)
        .bind(m.score)
        .execute(&self.db)
        .await?;
        Ok(())
    }

    async fn cached_comments(&self, video_id: &str) -> AppResult<Option<Vec<Comment>>> {
        let row = sqlx::query("SELECT comments FROM track_comments WHERE video_id = $1")
            .bind(video_id)
            .fetch_optional(&self.db)
            .await?;
        let Some(r) = row else { return Ok(None) };
        let json: serde_json::Value = r.try_get("comments")?;
        let parsed = serde_json::from_value(json).map_err(|e| {
            AppError::Internal(anyhow::anyhow!("track_comments row corrupt: {e}"))
        })?;
        Ok(Some(parsed))
    }

    async fn store_comments(&self, video_id: &str, comments: &[Comment]) -> sqlx::Result<()> {
        let json = serde_json::to_value(comments)
            .expect("comment vec is always JSON-serializable");
        sqlx::query(
            "INSERT INTO track_comments (video_id, comments, fetched_at) \
             VALUES ($1, $2, now()) \
             ON CONFLICT (video_id) DO UPDATE SET \
                comments = EXCLUDED.comments, \
                fetched_at = now()",
        )
        .bind(video_id)
        .bind(json)
        .execute(&self.db)
        .await?;
        Ok(())
    }

    async fn cached_emotion(&self, id: &str) -> sqlx::Result<Option<EmotionVec>> {
        let row = sqlx::query(
            "SELECT joy, sadness, anger, fear, surprise, disgust, neutral, comment_count \
             FROM track_emotion WHERE spotify_track_id = $1",
        )
        .bind(id)
        .fetch_optional(&self.db)
        .await?;
        Ok(row.map(|r| EmotionVec {
            joy: r.get("joy"),
            sadness: r.get("sadness"),
            anger: r.get("anger"),
            fear: r.get("fear"),
            surprise: r.get("surprise"),
            disgust: r.get("disgust"),
            neutral: r.get("neutral"),
            comment_count: r.get("comment_count"),
        }))
    }

    async fn store_emotion(&self, id: &str, e: &EmotionVec) -> sqlx::Result<()> {
        sqlx::query(
            "INSERT INTO track_emotion \
                (spotify_track_id, joy, sadness, anger, fear, surprise, disgust, neutral, comment_count, computed_at) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, now()) \
             ON CONFLICT (spotify_track_id) DO UPDATE SET \
                joy = EXCLUDED.joy, \
                sadness = EXCLUDED.sadness, \
                anger = EXCLUDED.anger, \
                fear = EXCLUDED.fear, \
                surprise = EXCLUDED.surprise, \
                disgust = EXCLUDED.disgust, \
                neutral = EXCLUDED.neutral, \
                comment_count = EXCLUDED.comment_count, \
                computed_at = now()",
        )
        .bind(id)
        .bind(e.joy)
        .bind(e.sadness)
        .bind(e.anger)
        .bind(e.fear)
        .bind(e.surprise)
        .bind(e.disgust)
        .bind(e.neutral)
        .bind(e.comment_count)
        .execute(&self.db)
        .await?;
        Ok(())
    }
}

pub fn emotion_labels() -> &'static [&'static str] {
    &EMOTION_LABELS
}

#[derive(Debug, Serialize)]
struct MatchRequest<'a> {
    title: &'a str,
    artist: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    duration_sec: Option<i32>,
}

#[derive(Debug, Deserialize)]
struct MatchResponse {
    video_id: String,
    title: String,
    channel: String,
    #[allow(dead_code)]
    duration_sec: Option<i32>,
    score: f32,
}

#[derive(Debug, Serialize)]
struct CommentsRequest<'a> {
    video_id: &'a str,
    max_comments: u32,
}

#[derive(Debug, Deserialize)]
struct CommentsResponse {
    #[allow(dead_code)]
    video_id: String,
    comments: Vec<Comment>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Comment {
    text: String,
    #[serde(default)]
    likes: i32,
}
