use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
};
use nekoui_agent::session::Message;
use tracing::debug;

use crate::{response::ApiResponse, routes::AppState};

pub async fn get_messages(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let session_key = match state.http_state.agent.session_manager().get_session_key(id) {
        Ok(key) => key,
        Err(_) => {
            return ApiResponse::error(
                StatusCode::BAD_REQUEST,
                "INVALID_SESSION_ID",
                "Invalid session id",
            );
        }
    };

    match state.http_state.agent.session_manager().get(&session_key) {
        Ok(session) => {
            let session_guard = session.lock().await;
            let messages = session_guard
                .messages
                .iter()
                .cloned()
                .collect::<Vec<Message>>();
            ApiResponse::success(messages)
        }
        Err(e) => {
            debug!("Session not found: {:?}", e);
            ApiResponse::error(
                StatusCode::NOT_FOUND,
                "SESSION_NOT_FOUND",
                "Session not found",
            )
        }
    }
}

