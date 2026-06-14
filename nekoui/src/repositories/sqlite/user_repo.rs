use anyhow::Result;
use sqlx::sqlite::SqliteQueryResult;

use crate::{
    api::routes::auth::UpdateRequest,
    models::user::{UserRecord, UserRecordRequest},
};

#[derive(Clone)]
pub struct User {
    pub sqlite_pool: sqlx::Pool<sqlx::Sqlite>,
}

impl User {
    pub async fn new(sqlite_pool: sqlx::Pool<sqlx::Sqlite>) -> Self {
        Self { sqlite_pool }
    }

    pub async fn get_by_email(&self, email: &str) -> Result<Option<UserRecord>, sqlx::Error> {
        let result = sqlx::query_as::<_, UserRecord>(
            r#"
                SELECT id, user_id, email, display_name, password_hash, avatar_url, created_at
                FROM users
                WHERE email = ?
                "#,
        )
        .bind(email)
        .fetch_optional(&self.sqlite_pool)
        .await?;

        Ok(result)
    }

    pub async fn get_by_id(&self, user_id: &str) -> Result<Option<UserRecord>, sqlx::Error> {
        let result = sqlx::query_as::<_, UserRecord>(
            r#"
                SELECT id, user_id, email, display_name, password_hash, avatar_url, created_at
                FROM users
                WHERE user_id = ?
                "#,
        )
        .bind(user_id)
        .fetch_optional(&self.sqlite_pool)
        .await?;

        Ok(result)
    }

    pub async fn create(
        &self,
        user_record: &UserRecordRequest,
    ) -> Result<Option<UserRecord>, sqlx::Error> {
        let result = sqlx::query_as::<_, UserRecord>(
            r#"
                INSERT INTO users (user_id, email, display_name, password_hash, avatar_url, created_at)
                VALUES (?, ?, ?, ?, ?, ?)
                RETURNING *;
            "#
        )
            .bind(&user_record.user_id)
            .bind(&user_record.email)
            .bind(&user_record.display_name)
            .bind(&user_record.password_hash)
            .bind(&user_record.avatar_url)
            .bind(user_record.created_at)
            .fetch_optional(&self.sqlite_pool)
            .await?;

        Ok(result)
    }

    pub async fn delete(
        &self,
        email: &str,
        password_hash: &str,
    ) -> Result<SqliteQueryResult, sqlx::Error> {
        let result = sqlx::query(
            r#"
                DELETE FROM users
                WHERE email = ?
                  AND password_hash = ?
            "#,
        )
        .bind(email)
        .bind(password_hash)
        .execute(&self.sqlite_pool)
        .await?;

        Ok(result)
    }

    pub async fn update(
        &self,
        user_id: &str,
        update_request: &UpdateRequest,
    ) -> Result<SqliteQueryResult, sqlx::Error> {
        let result = sqlx::query(
            r#"
                UPDATE users
                SET display_name = ?, avatar_url = ?
                WHERE user_id = ?
                "#,
        )
        .bind(&update_request.display_name)
        .bind(&update_request.avatar_url)
        .bind(user_id)
        .execute(&self.sqlite_pool)
        .await?;

        Ok(result)
    }
}
