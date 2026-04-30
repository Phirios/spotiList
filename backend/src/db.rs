use sqlx::postgres::{PgPool, PgPoolOptions};

pub type Pool = PgPool;

pub async fn connect(database_url: &str) -> anyhow::Result<Pool> {
    let pool = PgPoolOptions::new()
        .max_connections(10)
        .acquire_timeout(std::time::Duration::from_secs(10))
        .connect(database_url)
        .await?;
    sqlx::migrate!("./migrations").run(&pool).await?;
    Ok(pool)
}
