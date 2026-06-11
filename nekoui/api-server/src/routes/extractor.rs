use std::sync::Arc;

use axum::{
    extract::{FromRequestParts, Path},
    http::{StatusCode, request::Parts},
};
use nekoui_agent::session::Session;
use nekoui_domain::session::SessionKey;
use tokio::sync::Mutex;

use crate::{response::ApiResponse, routes::AppState};

pub struct ResolvedSession {
    pub key: SessionKey,
    pub session: Arc<Mutex<Session>>,
}

impl FromRequestParts<AppState> for ResolvedSession {
    type Rejection = ApiResponse<()>;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let Path(id): Path<String> =
            Path::from_request_parts(parts, state).await.map_err(|_| {
                ApiResponse::error(
                    StatusCode::BAD_REQUEST,
                    "INVALID_PATH",
                    "Missing or invalid session id in path",
                )
            })?;

        let key = state
            .http_state
            .agent
            .session_manager()
            .get_session_key(id)
            .map_err(|_| {
                ApiResponse::error(
                    StatusCode::BAD_REQUEST,
                    "INVALID_SESSION_ID",
                    "Invalid session id format",
                )
            })?;

        let session = state
            .http_state
            .agent
            .session_manager()
            .get(&key)
            .map_err(|_| {
                ApiResponse::error(
                    StatusCode::NOT_FOUND,
                    "SESSION_NOT_FOUND",
                    "Session not found",
                )
            })?;

        Ok(Self { key, session })
    }
}
