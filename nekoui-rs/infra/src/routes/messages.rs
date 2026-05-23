use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::AppState;

// ── Request/Response Types ────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct SendMessageRequest {
    pub content: String,
    #[serde(default)]
    pub user_id: Option<String>,
}

#[derive(Clone, Serialize)]
pub struct MessageResponse {
    pub id: String,
    pub role: String,
    pub content: String,
    pub created_at: String,
}

// ── Handlers ──────────────────────────────────────────────────────────────────

/// GET /api/conversations/{id}/messages - Get all messages in a conversation
pub async fn get_messages(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    if !state.store.exists(&id).await {
        return (StatusCode::NOT_FOUND, Json(Vec::<MessageResponse>::new())).into_response();
    }
    let messages = state.store.get_messages(&id).await;
    (StatusCode::OK, Json(messages)).into_response()
}

/// POST /api/conversations/{id}/messages - Send a message to a conversation
pub async fn send_message(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(body): Json<SendMessageRequest>,
) -> impl IntoResponse {
    if !state.store.exists(&id).await {
        return (
            StatusCode::NOT_FOUND,
            Json(MessageResponse {
                id: String::new(),
                role: "assistant".to_string(),
                content: "Conversation not found".to_string(),
                created_at: chrono::Utc::now().to_rfc3339(),
            }),
        )
            .into_response();
    }

    let session_key = nekoui_domain::agent::session::SessionKey::new(id, body.user_id.clone());
    match state
        .http_state
        .agent
        .submit(session_key, body.user_id.clone(), body.content.clone())
        .await
    {
        Ok(response) => (
            StatusCode::OK,
            Json(MessageResponse {
                id: Uuid::new_v4().to_string(),
                role: "assistant".to_string(),
                content: response,
                created_at: chrono::Utc::now().to_rfc3339(),
            }),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(MessageResponse {
                id: String::new(),
                role: "assistant".to_string(),
                content: format!("Error: {}", e),
                created_at: chrono::Utc::now().to_rfc3339(),
            }),
        )
            .into_response(),
    }
}

// ── Router ────────────────────────────────────────────────────────────────────

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/:id/messages", get(get_messages))
        .route("/:id/messages", post(send_message))
        .with_state(state)
}
