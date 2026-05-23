use std::sync::Arc;

use axum::{
    Json, Router,
    body::Body,
    extract::{
        Path, State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    http::{HeaderMap, Method, Request, StatusCode},
    middleware::{self, Next},
    response::sse::Event,
    routing::{delete, get, post},
};
use futures::{SinkExt, StreamExt as FuturesStreamExt};
use nekoui_config::loader::ServerConfig;
use nekoui_domain::agent::session::SessionKey;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tokio_stream::wrappers::BroadcastStream;
use tower_http::cors::{AllowOrigin, Any, CorsLayer};
use tracing::{error, info, warn};
use uuid::Uuid;

use crate::web_ui_agent::WebUiAgent;

#[derive(Clone)]
pub struct HttpServerState {
    pub agent: Arc<dyn WebUiAgent>,
    pub config: ServerConfig,
}

pub struct HttpServer {
    state: HttpServerState,
}

// ── Request/Response types ────────────────────────────────────────────────────

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

// ── In-memory conversation store ──────────────────────────────────────────────

#[derive(Clone, Default)]
struct ConversationStore {
    conversations: Arc<RwLock<Vec<ConversationEntry>>>,
}

#[derive(Clone)]
struct ConversationEntry {
    id: Uuid,
    user_id: Option<String>,
    messages: Vec<MessageResponse>,
}

impl ConversationStore {
    async fn create(&self, user_id: Option<String>) -> Uuid {
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

    async fn list(&self) -> Vec<ConversationListItem> {
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

    async fn add_message(&self, id: &Uuid, msg: MessageResponse) {
        let mut convos = self.conversations.write().await;
        if let Some(conv) = convos.iter_mut().find(|c| &c.id == id) {
            conv.messages.push(msg);
        }
    }

    async fn get_messages(&self, id: &Uuid) -> Vec<MessageResponse> {
        let convos = self.conversations.read().await;
        convos
            .iter()
            .find(|c| &c.id == id)
            .map(|c| c.messages.clone())
            .unwrap_or_default()
    }

    async fn delete(&self, id: &Uuid) -> bool {
        let mut convos = self.conversations.write().await;
        let len = convos.len();
        convos.retain(|c| &c.id != id);
        convos.len() < len
    }

    async fn exists(&self, id: &Uuid) -> bool {
        let convos = self.conversations.read().await;
        convos.iter().any(|c| &c.id == id)
    }
}

// ── HttpServer implementation ─────────────────────────────────────────────────

impl HttpServer {
    pub fn new(agent: Arc<dyn WebUiAgent>, config: ServerConfig) -> Self {
        Self {
            state: HttpServerState { agent, config },
        }
    }

    pub async fn serve(self) -> anyhow::Result<()> {
        let state = self.state;
        let addr: std::net::SocketAddr = state.config.bind_address.parse()?;

        let conversation_store = ConversationStore::default();

        // Build CORS layer
        let cors = if state.config.allowed_origins.is_empty() {
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
            let origins: Vec<_> = state
                .config
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
        };

        // Auth middleware
        let auth_mw = middleware::from_fn_with_state(
            state.config.clone(),
            |config: State<ServerConfig>,
             headers: HeaderMap,
             request: Request<Body>,
             next: Next| async move {
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
            },
        );

        let app = Router::new()
            // REST API
            .route("/api/conversations", {
                let store = conversation_store.clone();
                post(move |body: Json<CreateConversationRequest>| {
                    let store = store.clone();
                    async move {
                        let conversation_id = store.create(body.user_id.clone()).await;
                        Json(ConversationResponse {
                            conversation_id,
                            created_at: chrono::Utc::now().to_rfc3339(),
                        })
                    }
                })
            })
            .route("/api/conversations", {
                let store = conversation_store.clone();
                get(move || {
                    let store = store.clone();
                    async move { Json(store.list().await) }
                })
            })
            .route("/api/conversations/{id}", {
                let store = conversation_store.clone();
                let agent = state.agent.clone();
                delete(move |Path(id): Path<Uuid>| {
                    let store = store.clone();
                    let agent = agent.clone();
                    async move {
                        if store.delete(&id).await {
                            agent
                                .submit(SessionKey::anonymous(id), None, String::new())
                                .await
                                .ok();
                            StatusCode::NO_CONTENT
                        } else {
                            StatusCode::NOT_FOUND
                        }
                    }
                })
            })
            .route("/api/conversations/{id}/messages", {
                let store = conversation_store.clone();
                get(move |Path(id): Path<Uuid>| {
                    let store = store.clone();
                    async move {
                        if !store.exists(&id).await {
                            return (StatusCode::NOT_FOUND, Json(Vec::<MessageResponse>::new()));
                        }
                        let messages = store.get_messages(&id).await;
                        (StatusCode::OK, Json(messages))
                    }
                })
            })
            .route("/api/conversations/{id}/messages", {
                let store = conversation_store.clone();
                let agent = state.agent.clone();
                post(
                    move |Path(id): Path<Uuid>, body: Json<SendMessageRequest>| {
                        let store = store.clone();
                        let agent = agent.clone();
                        async move {
                            if !store.exists(&id).await {
                                return (
                                    StatusCode::NOT_FOUND,
                                    Json(MessageResponse {
                                        id: String::new(),
                                        role: "assistant".to_string(),
                                        content: "Conversation not found".to_string(),
                                        created_at: chrono::Utc::now().to_rfc3339(),
                                    }),
                                );
                            }
                            let session_key = SessionKey::new(id, body.user_id.clone());
                            match agent
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
                                ),
                                Err(e) => (
                                    StatusCode::INTERNAL_SERVER_ERROR,
                                    Json(MessageResponse {
                                        id: String::new(),
                                        role: "assistant".to_string(),
                                        content: format!("Error: {}", e),
                                        created_at: chrono::Utc::now().to_rfc3339(),
                                    }),
                                ),
                            }
                        }
                    },
                )
            })
            // WebSocket endpoint
            .route("/ws/conversations/{id}", {
                let store = conversation_store.clone();
                let agent = state.agent.clone();
                get(move |ws: WebSocketUpgrade, Path(id): Path<Uuid>| {
                    let store = store.clone();
                    let agent = agent.clone();
                    async move {
                        let conversation_id = id;
                        ws.on_upgrade(move |socket| {
                            handle_websocket(socket, agent, store, conversation_id)
                        })
                    }
                })
            })
            // Legacy endpoints
            .route("/api/events", get(sse_handler))
            .route("/api/metrics", get(metrics_handler))
            .layer(auth_mw)
            .layer(cors)
            .with_state(state);

        info!(addr = %addr, "starting HTTP/WebSocket server");
        let listener = tokio::net::TcpListener::bind(addr).await?;
        axum::serve(listener, app).await?;
        Ok(())
    }
}

// ── WebSocket handler ─────────────────────────────────────────────────────────

async fn handle_websocket(
    socket: WebSocket,
    agent: Arc<dyn WebUiAgent>,
    store: ConversationStore,
    conversation_id: Uuid,
) {
    let (sender, mut receiver) = FuturesStreamExt::split(socket);
    let session_key = SessionKey::anonymous(conversation_id);

    info!(conversation_id = %conversation_id, "WebSocket connected");

    let ws_sender = Arc::new(tokio::sync::Mutex::new(sender));

    while let Some(Ok(msg)) = FuturesStreamExt::next(&mut receiver).await {
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
                                let agent = agent.clone();
                                let store = store.clone();
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
                                                        created_at: chrono::Utc::now().to_rfc3339(),
                                                    },
                                                )
                                                .await;

                                            let done_msg = serde_json::json!({
                                                "type": "done",
                                                "full_content": response
                                            });
                                            let mut sender = sender.lock().await;
                                            if let Err(e) = sender
                                                .send(Message::Text(done_msg.to_string().into()))
                                                .await
                                            {
                                                warn!(
                                                    error = %e,
                                                    "failed to send WS done message"
                                                );
                                            }
                                        }
                                        Err(e) => {
                                            let err_msg = serde_json::json!({
                                                "type": "error",
                                                "message": format!("{}", e)
                                            });
                                            let mut sender = sender.lock().await;
                                            let _ = sender
                                                .send(Message::Text(err_msg.to_string().into()))
                                                .await;
                                        }
                                    }
                                });
                            }
                        }
                        "ping" => {
                            let pong = serde_json::json!({ "type": "pong" });
                            let mut sender = ws_sender.lock().await;
                            let _ = sender.send(Message::Text(pong.to_string().into())).await;
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

#[derive(Deserialize)]
struct WsClientMessage {
    #[serde(rename = "type")]
    r#type: String,
    content: Option<String>,
}

// ── SSE handler (keep existing) ───────────────────────────────────────────────

async fn sse_handler(
    State(state): State<HttpServerState>,
) -> axum::response::Sse<impl tokio_stream::Stream<Item = Result<Event, std::convert::Infallible>>>
{
    let rx = state.agent.event_bus().subscribe();
    let stream =
        tokio_stream::StreamExt::filter_map(BroadcastStream::new(rx), |result| match result {
            Ok(event) => match serde_json::to_string(&event) {
                Ok(json) => Some(Ok(Event::default().data(json))),
                Err(e) => {
                    error!(target: "http_server", error = %e, "failed to serialize event");
                    None
                }
            },
            Err(tokio_stream::wrappers::errors::BroadcastStreamRecvError::Lagged(n)) => {
                warn!(target: "http_server", skipped = n, "SSE client lagged");
                None
            }
        });
    axum::response::Sse::new(stream).keep_alive(
        axum::response::sse::KeepAlive::new()
            .interval(std::time::Duration::from_secs(15))
            .text("keep-alive"),
    )
}

async fn metrics_handler(State(state): State<HttpServerState>) -> String {
    state.agent.metrics().collect_prometheus()
}
