use axum::{
    extract::{ws::WebSocketUpgrade, Path, State},
    response::IntoResponse,
    routing::get,
    Router,
};
use futures::{SinkExt, StreamExt as FuturesStreamExt};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{info, warn};
use uuid::Uuid;

use super::{AppState, MessageResponse};

// ── WebSocket Message Types ───────────────────────────────────────────────────

#[derive(Deserialize)]
struct WsClientMessage {
    #[serde(rename = "type")]
    r#type: String,
    content: Option<String>,
}

#[derive(Serialize)]
struct WsServerMessage {
    #[serde(rename = "type")]
    r#type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    full_content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    message: Option<String>,
}

// ── Handlers ──────────────────────────────────────────────────────────────────

/// GET /ws/conversations/{id} - WebSocket endpoint for conversations
pub async fn websocket_handler(
    State(state): State<AppState>,
    ws: WebSocketUpgrade,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_websocket(socket, state, id))
}

/// Handle WebSocket connection
async fn handle_websocket(
    socket: axum::extract::ws::WebSocket,
    state: AppState,
    conversation_id: Uuid,
) {
    let (sender, mut receiver) = FuturesStreamExt::split(socket);
    let session_key = nekoui_domain::agent::session::SessionKey::anonymous(conversation_id);

    info!(conversation_id = %conversation_id, "WebSocket connected");

    let ws_sender = Arc::new(Mutex::new(sender));

    while let Some(Ok(msg)) = FuturesStreamExt::next(&mut receiver).await {
        use axum::extract::ws::Message;

        match msg {
            Message::Text(text) => {
                // Parse JSON message
                match serde_json::from_str::<WsClientMessage>(&text) {
                    Ok(WsClientMessage {
                        r#type: msg_type,
                        content,
                    }) => match msg_type.as_str() {
                        "message" => {
                            if let Some(content) = content {
                                let sender = ws_sender.clone();
                                let agent = state.http_state.agent.clone();
                                let store = state.store.clone();
                                let sk = session_key.clone();
                                let cid = conversation_id;
                                let now = chrono::Utc::now().to_rfc3339();

                                tokio::spawn(async move {
                                    // Store user message
                                    store
                                        .add_message(
                                            &cid,
                                            MessageResponse {
                                                id: Uuid::new_v4().to_string(),
                                                role: "user".to_string(),
                                                content: content.clone(),
                                                created_at: now.clone(),
                                            },
                                        )
                                        .await;

                                    match agent.submit(sk, None, content).await {
                                        Ok(response) => {
                                            // Store assistant response
                                            let resp_id = Uuid::new_v4().to_string();
                                            store
                                                .add_message(
                                                    &cid,
                                                    MessageResponse {
                                                        id: resp_id,
                                                        role: "assistant".to_string(),
                                                        content: response.clone(),
                                                        created_at: chrono::Utc::now()
                                                            .to_rfc3339(),
                                                    },
                                                )
                                                .await;

                                            let done_msg = WsServerMessage {
                                                r#type: "done".to_string(),
                                                full_content: Some(response),
                                                message: None,
                                            };
                                            let mut sender = sender.lock().await;
                                            if let Err(e) = sender
                                                .send(Message::Text(
                                                    serde_json::to_string(&done_msg)
                                                        .unwrap_or_default()
                                                        .into(),
                                                ))
                                                .await
                                            {
                                                warn!(
                                                    error = %e,
                                                    "failed to send WS done message"
                                                );
                                            }
                                        }
                                        Err(e) => {
                                            let err_msg = WsServerMessage {
                                                r#type: "error".to_string(),
                                                full_content: None,
                                                message: Some(format!("{}", e)),
                                            };
                                            let mut sender = sender.lock().await;
                                            let _ = sender
                                                .send(Message::Text(
                                                    serde_json::to_string(&err_msg)
                                                        .unwrap_or_default()
                                                        .into(),
                                                ))
                                                .await;
                                        }
                                    }
                                });
                            }
                        }
                        "ping" => {
                            let pong = WsServerMessage {
                                r#type: "pong".to_string(),
                                full_content: None,
                                message: None,
                            };
                            let mut sender = ws_sender.lock().await;
                            let _ = sender
                                .send(Message::Text(
                                    serde_json::to_string(&pong)
                                        .unwrap_or_default()
                                        .into(),
                                ))
                                .await;
                        }
                        _ => {
                            warn!(type = %msg_type, "unknown WS message type");
                        }
                    },
                    Err(e) => {
                        warn!(error = %e, "failed to parse WS message");
                    }
                }
            }
            Message::Close(_) => break,
            _ => {}
        }
    }

    info!(conversation_id = %conversation_id, "WebSocket disconnected");
}

// ── Router ────────────────────────────────────────────────────────────────────

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/:id", get(websocket_handler))
        .with_state(state)
}
