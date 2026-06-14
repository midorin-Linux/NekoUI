use std::sync::Arc;

use anyhow::Result;

use crate::{
    models::{refresh_token::RefreshTokenRecord, user::UserRecord},
    repositories::sqlite::{SqliteRepository, refresh_token_repo::RefreshToken, user_repo::User},
};

pub struct AuthService {
    pub refresh_token_repo: Arc<RefreshToken>,
    pub user_repo: Arc<User>,
}

impl AuthService {
    pub async fn new(sqlite_repository: &SqliteRepository) -> Self {
        let refresh_token_repo = sqlite_repository.refresh_token().await.clone();
        let user_repo = sqlite_repository.user().await.clone();

        Self {
            user_repo,
            refresh_token_repo,
        }
    }

    pub async fn register(&self, user_record: &UserRecord) -> Result<()> {
        let is_exists = self.user_repo.get_by_email(&user_record.email).await?;

        if is_exists.is_some() {
            Err(anyhow::anyhow!("user already exists"))
        } else {
            let result = self.user_repo.create(user_record).await?;

            if result.rows_affected() == 0 {
                Err(anyhow::anyhow!("failed to create user"))
            } else {
                Ok(())
            }
        }
    }

    pub async fn login(&self, user_record: &UserRecord) -> Result<RefreshTokenRecord> {
        let user = self
            .user_repo
            .get_by_email(&user_record.email)
            .await?
            .ok_or_else(|| anyhow::anyhow!("user not found"))?;

        if user.password_hash != user_record.password_hash {
            return Err(anyhow::anyhow!("password mismatch"));
        }

        let token = match self.refresh_token_repo.get(&user.user_id).await? {
            Some(token) => token,
            None => self
                .refresh_token_repo
                .generate(&user.user_id)
                .await?
                .ok_or_else(|| anyhow::anyhow!("failed to generate refresh token"))?,
        };

        Ok(token)
    }

    pub async fn refresh(&self, token_hash: &str) -> Result<RefreshTokenRecord> {
        let token = self
            .refresh_token_repo
            .validate(token_hash)
            .await?
            .ok_or_else(|| anyhow::anyhow!("refresh token not found or expired"))?;
        let user_id = token.user_id.clone();

        self.refresh_token_repo.revoke(token_hash).await?;

        let new_token = self
            .refresh_token_repo
            .generate(&user_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("failed to generate refresh token"))?;

        Ok(new_token)
    }

    pub async fn logout(&self, token_hash: &str) -> Result<()> {
        self.refresh_token_repo
            .validate(token_hash)
            .await?
            .ok_or_else(|| anyhow::anyhow!("refresh token not found or expired"))?;

        self.refresh_token_repo.revoke(token_hash).await?;

        Ok(())
    }

    pub async fn get_user(&self, user_id: &str) -> Result<Option<UserRecord>> {
        Ok(self.user_repo.get_by_id(user_id).await?)
    }

    pub async fn update_user(&self, user_record: &UserRecord) -> Result<()> {
        let result = self.user_repo.update(user_record.clone()).await?;

        if result.rows_affected() == 0 {
            Err(anyhow::anyhow!("user not found"))
        } else {
            Ok(())
        }
    }
}
