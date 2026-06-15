use std::sync::Arc;

use anyhow::Result;

use crate::{
    error::AuthError,
    models::refresh_token::TokenPair,
    repositories::sqlite::{refresh_token_repo::RefreshToken, user_repo::User},
    utils::jwt::{self, JwtClaims},
};

pub struct TokenService {
    pub refresh_token_repo: Arc<RefreshToken>,
    pub user_repo: Arc<User>,
}

impl TokenService {
    pub async fn new(refresh_token_repo: Arc<RefreshToken>, user_repo: Arc<User>) -> Self {
        Self {
            user_repo,
            refresh_token_repo,
        }
    }

    pub async fn generate_token_pair(
        &self,
        user_id: &str,
        jwt_secret: &str,
    ) -> Result<TokenPair, AuthError> {
        let access_key = jwt::issue_access_token(user_id, jwt_secret)
            .map_err(|_| AuthError::FailedToGenerateRefreshToken)?;

        let (refresh_key, _record) = self
            .refresh_token_repo
            .generate(user_id)
            .await?
            .ok_or(AuthError::FailedToGenerateRefreshToken)?;

        Ok(TokenPair {
            access_key,
            refresh_key,
        })
    }

    pub fn verify_access_token(
        &self,
        token: &str,
        jwt_secret: &str,
    ) -> Result<JwtClaims, crate::error::JwtError> {
        jwt::verify_access_token(token, jwt_secret)
    }

    pub async fn rotate_refresh_token(
        &self,
        raw_refresh_token: &str,
        jwt_secret: &str,
    ) -> Result<TokenPair, AuthError> {
        use aws_lc_rs::digest;

        let token_hash = hex::encode(digest::digest(
            &digest::SHA256,
            raw_refresh_token.as_bytes(),
        ));

        let old_record = self
            .refresh_token_repo
            .validate(&token_hash)
            .await?
            .ok_or(AuthError::FailedToGenerateRefreshToken)?;

        let user_id = old_record.user_id.clone();

        self.refresh_token_repo
            .revoke(&token_hash)
            .await
            .map_err(|_| AuthError::FailedToGenerateRefreshToken)?;

        let (new_refresh_key, _new_record) = self
            .refresh_token_repo
            .generate(&user_id)
            .await?
            .ok_or(AuthError::FailedToGenerateRefreshToken)?;

        let access_key = jwt::issue_access_token(&user_id, jwt_secret)
            .map_err(|_| AuthError::FailedToGenerateRefreshToken)?;

        Ok(TokenPair {
            access_key,
            refresh_key: new_refresh_key,
        })
    }

    pub async fn revoke_refresh_token(&self, raw_refresh_token: &str) -> Result<(), AuthError> {
        use aws_lc_rs::digest;

        let token_hash = hex::encode(digest::digest(
            &digest::SHA256,
            raw_refresh_token.as_bytes(),
        ));

        self.refresh_token_repo
            .validate(&token_hash)
            .await?
            .ok_or(AuthError::FailedToGenerateRefreshToken)?;

        self.refresh_token_repo
            .revoke(&token_hash)
            .await
            .map_err(|_| AuthError::FailedToGenerateRefreshToken)?;

        Ok(())
    }
}
