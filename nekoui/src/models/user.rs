#[derive(Clone, Debug, sqlx::FromRow)]
pub struct UserRecord {
    pub id: u64,
    pub user_id: String,
    pub email: String,
    pub display_name: Option<String>,
    pub password_hash: String,
    pub avatar_url: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}
