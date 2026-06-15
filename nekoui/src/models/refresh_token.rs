#[derive(Clone, Debug, sqlx::FromRow)]
pub struct RefreshTokenRecord {
    pub id: u64,
    pub user_id: String,
    pub token_hash: String,
    pub expires_at: chrono::DateTime<chrono::Utc>,
    pub revoked: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive()]
pub struct TokenPair {
    pub access_key: String,
    pub refresh_key: String,
}
