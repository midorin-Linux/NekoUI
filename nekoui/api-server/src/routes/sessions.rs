use axum::{extract::State, response::IntoResponse};
use serde::Serialize;

use crate::{response::ApiResponse, routes::AppState};

#[derive(Serialize)]
struct SessionListItem {
    session_id: String,
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
            sessions.push(SessionListItem {
                session_id: key.conversation_id.to_string(),
                created_at: session_guard.created_at.to_string(),
                last_active: session_guard.last_active.to_string(),
                message_turns: session_guard.messages.len(),
            });
        }
    }

    ApiResponse::success(sessions)
}
