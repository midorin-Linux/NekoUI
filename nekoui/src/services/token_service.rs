use std::sync::Arc;
use crate::models::refresh_token::Tokens;
use crate::models::user::UserRecord;
use crate::repositories::sqlite::refresh_token_repo::RefreshToken;
use crate::repositories::sqlite::user_repo::User;

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
    
    // pub async fn get_keys(&self, refresh_token) -> Result<Tokens> {}
}
