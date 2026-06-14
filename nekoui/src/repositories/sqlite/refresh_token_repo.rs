use anyhow::Result;
use aws_lc_rs::rand;
use sqlx::sqlite::SqliteQueryResult;

use crate::models::refresh_token::RefreshTokenRecord;

#[derive(Clone)]
pub struct RefreshToken {
    pub sqlite_pool: sqlx::Pool<sqlx::Sqlite>,
}

impl RefreshToken {
    pub async fn new(sqlite_pool: sqlx::Pool<sqlx::Sqlite>) -> Self {
        Self { sqlite_pool }
    }

    pub async fn generate(&self, user_id: &str) -> Result<Option<RefreshTokenRecord>, sqlx::Error> {
        let mut token_bytes = vec![0u8; 32];
        rand::fill(&mut token_bytes)
            .map_err(|_| anyhow::anyhow!("failed to generate random bytes"))
            .expect("failed to generate random bytes");

        let hash = aws_lc_rs::digest::digest(&aws_lc_rs::digest::SHA256, &token_bytes);
        let token_hash = hex::encode(hash.as_ref());

        let current_time = chrono::Utc::now();
        let expires_time = current_time + chrono::Duration::seconds(60 * 60 * 24 * 30);

        let result = sqlx::query_as::<_, RefreshTokenRecord>(
            r#"
            INSERT INTO refresh_tokens (user_id, token_hash, expires_at, created_at)
            VALUES (?, ?, ?, ?)
            RETURNING *
            "#,
        )
        .bind(user_id)
        .bind(token_hash)
        .bind(expires_time)
        .bind(current_time)
        .fetch_optional(&self.sqlite_pool)
        .await?;

        Ok(result)
    }

    pub async fn get(&self, user_id: &str) -> Result<Option<RefreshTokenRecord>, sqlx::Error> {
        let current_time = chrono::Utc::now();

        let result = sqlx::query_as::<_, RefreshTokenRecord>(
            r#"
            SELECT id, user_id, token_hash, expires_at, revoked, created_at
            FROM refresh_tokens
            WHERE user_id = ?
              AND revoked = 0
              AND expires_at > ?
            "#,
        )
        .bind(user_id)
        .bind(current_time)
        .fetch_optional(&self.sqlite_pool)
        .await?;

        Ok(result)
    }

    pub async fn validate(
        &self,
        token_hash: &str,
    ) -> Result<Option<RefreshTokenRecord>, sqlx::Error> {
        let current_time = chrono::Utc::now();

        let result = sqlx::query_as::<_, RefreshTokenRecord>(
            r#"
            SELECT id, user_id, token_hash, expires_at, revoked, created_at
            FROM refresh_tokens
            WHERE token_hash = ?
              AND revoked = 0
              AND expires_at > ?
            "#,
        )
        .bind(token_hash)
        .bind(current_time)
        .fetch_optional(&self.sqlite_pool)
        .await?;

        Ok(result)
    }

    pub async fn revoke(&self, token_hash: &str) -> Result<SqliteQueryResult> {
        let result = sqlx::query(
            r#"
            UPDATE refresh_tokens
            SET revoked = 1
            WHERE token_hash = ?
            "#,
        )
        .bind(token_hash)
        .execute(&self.sqlite_pool)
        .await?;

        Ok(result)
    }

    pub async fn rotate(
        &self,
        old_token: &str,
        new_token: &RefreshTokenRecord,
    ) -> Result<SqliteQueryResult> {
        let result = sqlx::query(
            r#"
            UPDATE refresh_tokens
            SET token_hash = ?, expires_at = ?
            WHERE token_hash = ?
            "#,
        )
        .bind(new_token.token_hash.as_str())
        .bind(new_token.expires_at)
        .bind(old_token)
        .execute(&self.sqlite_pool)
        .await?;

        Ok(result)
    }

    pub async fn delete_expired(&self) -> Result<SqliteQueryResult> {
        let current_time = chrono::Utc::now();

        let result = sqlx::query(
            r#"
            DELETE FROM refresh_tokens
            WHERE expires_at < ?
              OR revoked = 1
            "#,
        )
        .bind(current_time)
        .execute(&self.sqlite_pool)
        .await?;

        Ok(result)
    }
}
