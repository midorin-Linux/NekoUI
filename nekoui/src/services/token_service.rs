use std::sync::Arc;
use anyhow::Result;

use crate::{
    models::{refresh_token::TokenPair, user::UserRecord},
    repositories::sqlite::{refresh_token_repo::RefreshToken, user_repo::User},
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

    pub async fn generate(&self) -> Result<TokenPair> {

    }
}
