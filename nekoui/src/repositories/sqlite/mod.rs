use std::sync::Arc;

use sqlx::{Pool, Sqlite, sqlite::SqlitePoolOptions};

use crate::repositories::sqlite::{refresh_token_repo::RefreshToken, user_repo::User};

pub mod refresh_token_repo;
pub mod user_repo;

#[derive(Clone)]
pub struct SqliteRepository {
    _sqlite_pool: Pool<Sqlite>,
    refresh_token: Arc<RefreshToken>,
    user: Arc<User>,
}

impl SqliteRepository {
    pub async fn new(database_url: &str) -> anyhow::Result<Self> {
        let sqlite_pool = SqlitePoolOptions::new()
            .acquire_timeout(std::time::Duration::from_secs(5))
            .connect(database_url)
            .await?;

        let refresh_token = Arc::new(RefreshToken::new(sqlite_pool.clone()).await);
        let user = Arc::new(user_repo::User::new(sqlite_pool.clone()).await);

        Ok(Self {
            _sqlite_pool: sqlite_pool,
            refresh_token,
            user,
        })
    }

    pub async fn refresh_token(&self) -> &Arc<RefreshToken> {
        &self.refresh_token
    }

    pub async fn user(&self) -> &Arc<User> {
        &self.user
    }
}
