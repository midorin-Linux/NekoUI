use std::path::PathBuf;
use std::sync::Arc;

use sqlx::{Pool, Sqlite, sqlite::SqlitePoolOptions};

use crate::repositories::sqlite::{refresh_token_repo::RefreshToken, user_repo::User};

pub mod refresh_token_repo;
pub mod user_repo;

#[cfg(debug_assertions)]
const DATABASE_PATH: &str = "../";

#[cfg(not(debug_assertions))]
const CONFIG_PATH: &str = "";

#[derive(Clone)]
pub struct SqliteRepository {
    _sqlite_pool: Pool<Sqlite>,
    refresh_token: Arc<RefreshToken>,
    user: Arc<User>,
}

impl SqliteRepository {
    pub async fn new(database_url: &str) -> anyhow::Result<Self> {
        let sqlite_path = PathBuf::from(DATABASE_PATH).join(database_url).to_str().unwrap().to_string();

        let sqlite_pool = SqlitePoolOptions::new()
            .acquire_timeout(std::time::Duration::from_secs(5))
            .connect(&sqlite_path)
            .await?;

        let refresh_token = Arc::new(RefreshToken::new(sqlite_pool.clone()).await);
        let user = Arc::new(User::new(sqlite_pool.clone()).await);

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
