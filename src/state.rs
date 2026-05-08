use sqlx::{migrate::Migrator, postgres::PgPoolOptions, PgPool};

use crate::config::Config;

static MIGRATOR: Migrator = sqlx::migrate!();

#[derive(Clone)]
pub struct AppState {
    pub db_pool: PgPool,
}

impl AppState {
    pub async fn from_config(config: &Config) -> Result<Self, sqlx::Error> {
        let db_pool = PgPoolOptions::new()
            .max_connections(5)
            .connect(&config.database_url)
            .await?;

        MIGRATOR.run(&db_pool).await?;

        Ok(Self { db_pool })
    }
}
