use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::AppState;

// ── Request/Response Types ────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct CreateConversationRequest {
    #[serde(default)]
    pub user_id: Option<String>,
}

#[derive(Serialize)]
pub struct ConversationResponse {
    pub conversation_id: Uuid,
    pub created_at: String,
}

#[derive(Serialize)]
pub struct ConversationListItem {
    pub conversation_id: Uuid,
    pub user_id: Option<String>,
    pub message_count: usize,
}

// ── Handlers ──────────────────────────────────────────────────────────────────

/// POST /api/conversations - Create a new conversation
pub async fn create_conversation(
    State(state): State<AppState>,
    Json(body): Json<CreateConversationRequest>,
) -> impl IntoResponse {
    let conversation_id = state.store.create(body.user_id.clone()).await;
    (
        StatusCode::CREATED,
        Json(ConversationResponse {
            conversation_id,
            created_at: chrono::Utc::now().to_rfc3339(),
        }),
    )
}

/// GET /api/conversations - List all conversations
pub async fn list_conversations(
    State(state): State<AppState>,
) -> impl IntoResponse {
    let conversations = state.store.list().await;
    (StatusCode::OK, Json(conversations))
}

/// DELETE /api/conversations/{id} - Delete a conversation
pub async fn delete_conversation(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    if state.store.delete(&id).await {
        // Notify agent to cleanup session
        state
            .http_state
            .agent
            .submit(
                nekoui_domain::agent::session::SessionKey::anonymous(id),
                None,
                String::new(),
            )
            .await
            .ok();
        StatusCode::NO_CONTENT
    } else {
        StatusCode::NOT_FOUND
    }
}

// ── Router ────────────────────────────────────────────────────────────────────

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/", post(create_conversation))
        .route("/", get(list_conversations))
        .route("/:id", delete(delete_conversation))
        .with_state(state)
}
