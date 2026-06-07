use axum::{extract::{Path, State}, http::StatusCode, response::IntoResponse};
use serde::Serialize;
use tracing::debug;
use crate::{response::ApiResponse, routes::AppState};

#[derive(Serialize)]
struct SessionListItem {
    session_id: String,
    title: String,
    created_at: String,
    last_active: String,
    message_turns: usize,
}

pub async fn create_session(State(state): State<AppState>) -> impl IntoResponse {
    let session_key = state
        .http_state
        .agent
        .session_manager()
        .create_session_key();
    let new_session = state
        .http_state
        .agent
        .session_manager()
        .get_or_create(&session_key);
    let session_guard = new_session.lock().await;

    ApiResponse::success(SessionListItem {
        session_id: session_key.conversation_id.to_string(),
        title: session_guard.title.clone(),
        created_at: session_guard.created_at.to_string(),
        last_active: session_guard.last_active.to_string(),
        message_turns: session_guard.messages.len(),
    })
}

pub async fn list_sessions(State(state): State<AppState>) -> impl IntoResponse {
    let all_session_keys = state.http_state.agent.session_manager().all_keys();
    let mut sessions = Vec::with_capacity(all_session_keys.len());

    for key in all_session_keys {
        if let Ok(session) = state.http_state.agent.session_manager().get(&key) {
            let session_guard = session.lock().await;
            sessions.push((session_guard.last_active, SessionListItem {
                session_id: key.conversation_id.to_string(),
                title: session_guard.title.clone(),
                created_at: session_guard.created_at.to_string(),
                last_active: session_guard.last_active.to_string(),
                message_turns: session_guard.messages.len(),
            }));
        }
    }

    sessions.sort_by(|a, b| b.0.cmp(&a.0));

    ApiResponse::success(sessions.into_iter().map(|(_, item)| item).collect::<Vec<_>>())
}

pub async fn get_session(State(state): State<AppState>, Path(id): Path<String>) -> impl IntoResponse {
    let session_key = match state.http_state.agent.session_manager().get_session_key(id){
        Ok(key) => key,
        Err(_) => {
            return ApiResponse::error(StatusCode::BAD_REQUEST, "INVALID_SESSION_ID", "Invalid session id")
        },
    };

    match state.http_state.agent.session_manager().get(&session_key) {
        Ok(session) => {
            let session_guard = session.lock().await;
            let session_info = SessionListItem {
                session_id: session_key.conversation_id.to_string(),
                title: session_guard.title.clone(),
                created_at: session_guard.created_at.to_string(),
                last_active: session_guard.last_active.to_string(),
                message_turns: session_guard.messages.len(),
            };
            ApiResponse::success(session_info)
        },
        Err(e) => {
            debug!("Session not found: {:?}", e);
            ApiResponse::error(StatusCode::NOT_FOUND, "SESSION_NOT_FOUND", "Session not found")
        }
    }
}
