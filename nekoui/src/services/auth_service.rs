use std::sync::Arc;

use anyhow::Result;

use crate::{
    api::routes::auth::{LoginRequest, RegisterRequest, UpdateRequest},
    error::AuthError,
    models::{
        refresh_token::RefreshTokenRecord,
        user::{UserRecord, UserRecordRequest},
    },
    repositories::sqlite::{refresh_token_repo::RefreshToken, user_repo::User},
    utils::crypto::argon2_hash,
};

pub struct AuthService {
    pub refresh_token_repo: Arc<RefreshToken>,
    pub user_repo: Arc<User>,
}

impl AuthService {
    pub async fn new(refresh_token_repo: Arc<RefreshToken>, user_repo: Arc<User>) -> Self {
        Self {
            user_repo,
            refresh_token_repo,
        }
    }

    pub async fn register(
        &self,
        register_request: &RegisterRequest,
    ) -> Result<UserRecord, AuthError> {
        let is_exists = self.user_repo.get_by_email(&register_request.email).await?;

        let hashed_password =
            argon2_hash(&register_request.password).map_err(|_| AuthError::FailedToHashPassword)?;

        if is_exists.is_some() {
            Err(AuthError::UserAlreadyExists)
        } else {
            let record = UserRecordRequest {
                user_id: uuid::Uuid::new_v4().to_string(),
                email: register_request.email.clone(),
                display_name: register_request.display_name.clone(),
                password_hash: hashed_password,
                avatar_url: None,
                created_at: chrono::Utc::now(),
            };

            let result = self
                .user_repo
                .create(&record)
                .await?
                .ok_or(AuthError::FailedToCreateUser)?;

            Ok(result)
        }
    }

    pub async fn login(
        &self,
        login_request: &LoginRequest,
    ) -> Result<RefreshTokenRecord, AuthError> {
        let user = self
            .user_repo
            .get_by_email(&login_request.email)
            .await?
            .ok_or(AuthError::UserNotFound)?;

        let hashed_password =
            argon2_hash(&login_request.password).map_err(|_| AuthError::FailedToHashPassword)?;

        if user.password_hash != hashed_password {
            return Err(AuthError::PasswordIncorrect);
        }

        let token = match self.refresh_token_repo.get(&user.user_id).await? {
            Some(token) => token,
            None => self
                .refresh_token_repo
                .generate(&user.user_id)
                .await?
                .ok_or(AuthError::FailedToGenerateRefreshToken)?,
        };

        Ok(token)
    }

    pub async fn refresh(&self, token_hash: &str) -> Result<()> {
        // let token = self
        //     .refresh_token_repo
        //     .validate(token_hash)
        //     .await?
        //     .ok_or_else(|| anyhow::anyhow!("refresh token not found or expired"))?;
        // let user_id = token.user_id.clone();
        //
        // self.refresh_token_repo.revoke(token_hash).await?;
        //
        // let new_token = self
        //     .refresh_token_repo
        //     .generate(&user_id)
        //     .await?
        //     .ok_or_else(|| anyhow::anyhow!("failed to generate refresh token"))?;
        //
        // Ok(new_token)
        //ToDo: Access token実装次第追加
        Ok(())
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

    pub async fn update_user(&self, user_id: &str, update_request: &UpdateRequest) -> Result<()> {
        let result = self.user_repo.update(user_id, update_request).await?;

        if result.rows_affected() == 0 {
            Err(anyhow::anyhow!("user not found"))
        } else {
            Ok(())
        }
    }
}
