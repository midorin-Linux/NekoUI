use std::sync::Arc;

use anyhow::Result;

use super::token_service::TokenService;
use crate::{
    api::routes::auth::{LoginRequest, RegisterRequest, UpdateRequest},
    error::AuthError,
    models::{
        refresh_token::TokenPair,
        user::{UserRecord, UserRecordRequest},
    },
    repositories::sqlite::{refresh_token_repo::RefreshToken, user_repo::User},
    utils::crypto::argon2_hash,
};

pub struct AuthService {
    pub refresh_token_repo: Arc<RefreshToken>,
    pub user_repo: Arc<User>,
    pub token_service: Arc<TokenService>,
    pub jwt_secret: Arc<String>,
}

impl AuthService {
    pub async fn new(
        refresh_token_repo: Arc<RefreshToken>,
        user_repo: Arc<User>,
        token_service: Arc<TokenService>,
        jwt_secret: Arc<String>,
    ) -> Self {
        Self {
            user_repo,
            refresh_token_repo,
            token_service,
            jwt_secret,
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

    pub async fn login(&self, login_request: &LoginRequest) -> Result<TokenPair, AuthError> {
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

        let token_pair = self
            .token_service
            .generate_token_pair(&user.user_id, &self.jwt_secret)
            .await?;

        Ok(token_pair)
    }

    pub async fn refresh(&self, raw_refresh_token: &str) -> Result<TokenPair, AuthError> {
        self.token_service
            .rotate_refresh_token(raw_refresh_token, &self.jwt_secret)
            .await
    }

    pub async fn logout(&self, raw_refresh_token: &str) -> Result<(), AuthError> {
        self.token_service
            .revoke_refresh_token(raw_refresh_token)
            .await
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
