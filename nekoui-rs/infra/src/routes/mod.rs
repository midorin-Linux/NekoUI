use std::sync::Arc;

use axum::{Router, http::Method, middleware};
use axum::extract::State;
use axum::http::{HeaderMap, Request, StatusCode};
use axum::middleware::Next;
use axum::body::Body;
use nekoui_config::loader::ServerConfig;
use tokio::sync::RwLock;
use tower_http::cors::{AllowOrigin, Any, CorsLayer};
use tracing::info;
use uuid::Uuid;

pub mod conversations;
pub mod messages;
pub mod websocket;
pub mod legacy;

// Re-export commonly used types
pub use conversations::{CreateConversationRequest, ConversationResponse, ConversationListItem};
pub use messages::{SendMessageRequest, MessageResponse};

use crate::http_server::HttpServerState;

// ── Shared Application State ──────────────────────────────────────────────────

#[derive(Clone)]
pub struct AppState {
    pub http_state: HttpServerState,
    pub store: Arc<ConversationStore>,
}

// ── Conversation Store ────────────────────────────────────────────────────────

#[derive(Clone, Default)]
pub struct ConversationStore {
    conversations: Arc<RwLock<Vec<ConversationEntry>>>,
}

#[derive(Clone)]
struct ConversationEntry {
    id: Uuid,
    user_id: Option<String>,
    messages: Vec<MessageResponse>,
}

impl ConversationStore {
    pub async fn create(&self, user_id: Option<String>) -> Uuid {
        let id = Uuid::new_v4();
        let mut convos = self.conversations.write().await;
        convos.push(ConversationEntry {
            id,
            user_id: user_id.clone(),
            messages: Vec::new(),
        });
        info!(conversation_id = %id, user_id = ?user_id, "created new conversation");
        id
    }

    pub async fn list(&self) -> Vec<ConversationListItem> {
        let convos = self.conversations.read().await;
        convos
            .iter()
            .map(|c| ConversationListItem {
                conversation_id: c.id,
                user_id: c.user_id.clone(),
                message_count: c.messages.len(),
            })
            .collect()
    }

    pub async fn add_message(&self, id: &Uuid, msg: MessageResponse) {
        let mut convos = self.conversations.write().await;
        if let Some(conv) = convos.iter_mut().find(|c| &c.id == id) {
            conv.messages.push(msg);
        }
    }

    pub async fn get_messages(&self, id: &Uuid) -> Vec<MessageResponse> {
        let convos = self.conversations.read().await;
        convos
            .iter()
            .find(|c| &c.id == id)
            .map(|c| c.messages.clone())
            .unwrap_or_default()
    }

    pub async fn delete(&self, id: &Uuid) -> bool {
        let mut convos = self.conversations.write().await;
        let len = convos.len();
        convos.retain(|c| &c.id != id);
        convos.len() < len
    }

    pub async fn exists(&self, id: &Uuid) -> bool {
        let convos = self.conversations.read().await;
        convos.iter().any(|c| &c.id == id)
    }
}

// ── Router Building ──────────────────────────────────────────────────────────

/// Build CORS middleware layer
fn build_cors_layer(config: &ServerConfig) -> CorsLayer {
    if config.allowed_origins.is_empty() {
        CorsLayer::new()
            .allow_methods([Method::GET, Method::POST, Method::PATCH, Method::DELETE])
            .allow_headers(Any)
            .allow_origin(AllowOrigin::predicate(
                |origin: &axum::http::HeaderValue, _| {
                    origin
                        .to_str()
                        .map(|s| {
                            s == "http://127.0.0.1"
                                || s.starts_with("http://127.0.0.1:")
                                || s == "http://localhost"
                                || s.starts_with("http://localhost:")
                                || s == "https://127.0.0.1"
                                || s.starts_with("https://127.0.0.1:")
                                || s == "https://localhost"
                                || s.starts_with("https://localhost:")
                        })
                        .unwrap_or(false)
                },
            ))
    } else {
        let origins: Vec<_> = config
            .allowed_origins
            .iter()
            .map(|o| {
                o.parse::<axum::http::HeaderValue>()
                    .expect("invalid allowed origin")
            })
            .collect();
        CorsLayer::new()
            .allow_methods([Method::GET, Method::POST, Method::PATCH, Method::DELETE])
            .allow_headers(Any)
            .allow_origin(AllowOrigin::list(origins))
    }
}

/// Build the complete application router
pub fn build_routes(
    http_state: HttpServerState,
    store: Arc<ConversationStore>,
) -> Router {
    let app_state = AppState {
        http_state: http_state.clone(),
        store: store.clone(),
    };

    let cors = build_cors_layer(&http_state.config);
    let auth_config = http_state.config.clone();

    // Build main routers
    // Note: each nested router already has .with_state(app_state) called,
    // so we don't call it again here
    let api_router = Router::new()
        .nest("/api/conversations", conversations::router(app_state.clone()))
        .nest("/api/conversations", messages::router(app_state.clone()))
        .nest("/api", legacy::router(app_state.clone()))
        .nest("/ws/conversations", websocket::router(app_state));

    // Apply global middleware
    api_router
        .layer(middleware::from_fn_with_state(
            auth_config,
            |config: State<ServerConfig>,
             headers: HeaderMap,
             request: Request<Body>,
             next: Next| {
                async move {
                    if let Some(ref token) = config.auth_token {
                        let auth_header = headers
                            .get(axum::http::header::AUTHORIZATION)
                            .and_then(|v| v.to_str().ok())
                            .unwrap_or("");
                        let provided = auth_header.strip_prefix("Bearer ").unwrap_or("");
                        if provided != token.expose() {
                            return Err(StatusCode::UNAUTHORIZED);
                        }
                    }
                    Ok(next.run(request).await)
                }
            },
        ))
        .layer(cors)
}
