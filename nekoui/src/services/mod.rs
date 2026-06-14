use std::sync::Arc;

use crate::{repositories::sqlite::SqliteRepository, services::auth_service::AuthService};

pub mod auth_service;
pub mod token_service;

pub struct Services {
    auth_service: Arc<AuthService>,
}

impl Services {
    pub async fn new(sqlite_repository: &SqliteRepository) -> Self {
        let refresh_token_repo = sqlite_repository.refresh_token().await;
        let user_repo = sqlite_repository.user().await;

        let auth_service =
            Arc::new(AuthService::new(refresh_token_repo.clone(), user_repo.clone()).await);

        Self { auth_service }
    }

    pub async fn auth_service(&self) -> &Arc<AuthService> {
        &self.auth_service
    }
}
