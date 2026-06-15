use std::sync::Arc;

use self::token_service::TokenService;
use crate::{repositories::sqlite::SqliteRepository, services::auth_service::AuthService};

pub mod auth_service;
pub mod token_service;

pub struct Services {
    auth_service: Arc<AuthService>,
    token_service: Arc<TokenService>,
}

impl Services {
    pub async fn new(sqlite_repository: &SqliteRepository, jwt_secret: Arc<String>) -> Self {
        let refresh_token_repo = sqlite_repository.refresh_token().await;
        let user_repo = sqlite_repository.user().await;

        let token_service =
            Arc::new(TokenService::new(refresh_token_repo.clone(), user_repo.clone()).await);

        let auth_service = Arc::new(
            AuthService::new(
                refresh_token_repo.clone(),
                user_repo.clone(),
                token_service.clone(),
                jwt_secret,
            )
            .await,
        );

        Self {
            auth_service,
            token_service,
        }
    }

    pub async fn auth_service(&self) -> &Arc<AuthService> {
        &self.auth_service
    }

    pub async fn token_service(&self) -> &Arc<TokenService> {
        &self.token_service
    }
}
