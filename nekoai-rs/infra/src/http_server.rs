use std::{net::SocketAddr, sync::Arc};

use axum::{
    Router,
    body::Body,
    extract::State,
    http::{HeaderMap, Method, StatusCode},
    middleware::{self, Next},
    response::sse::Event,
    routing::get,
};
use nekoai_config::loader::WebUiConfig;
use tokio_stream::{StreamExt, wrappers::BroadcastStream};
use tower_http::cors::{AllowOrigin, Any, CorsLayer};

use crate::web_ui_agent::WebUiAgent;

#[derive(Clone)]
pub struct HttpServerState {
    pub agent: Arc<dyn WebUiAgent>,
    pub config: WebUiConfig,
}

pub struct HttpServer {
    state: HttpServerState,
}

impl HttpServer {
    pub fn new(agent: Arc<dyn WebUiAgent>, config: WebUiConfig) -> Self {
        Self {
            state: HttpServerState { agent, config },
        }
    }

    pub async fn serve(self, addr: SocketAddr) -> anyhow::Result<()> {
        let state = self.state;

        // Build CORS layer: restrict to allowed origins, fallback to loopback-only
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

        // Auth middleware that reads expected token from state
        let auth_mw = middleware::from_fn_with_state(
            state.config.clone(),
            |config: State<WebUiConfig>, headers: HeaderMap, request: Request<Body>, next: Next| async move {
                if let Some(ref token) = config.auth_token {
                    let auth_header = headers
                        .get(axum::http::header::AUTHORIZATION)
                        .and_then(|v| v.to_str().ok())
                        .unwrap_or("");
                    let provided = auth_header.strip_prefix("Bearer ").unwrap_or("");
                    if provided != token {
                        return Err(StatusCode::UNAUTHORIZED);
                    }
                }
                Ok(next.run(request).await)
            },
        );

        let app = Router::new()
            .route("/api/events", get(sse_handler))
            .route("/api/metrics", get(metrics_handler))
            .layer(auth_mw)
            .layer(cors)
            .with_state(state);

        tracing::info!(addr = %addr, "starting HTTP server");
        let listener = tokio::net::TcpListener::bind(addr).await?;
        axum::serve(listener, app).await?;
        Ok(())
    }
}

async fn sse_handler(
    State(state): State<HttpServerState>,
) -> axum::response::Sse<impl tokio_stream::Stream<Item = Result<Event, std::convert::Infallible>>>
{
    let rx = state.agent.event_bus().subscribe();
    let stream = BroadcastStream::new(rx).filter_map(|result| match result {
        Ok(event) => match serde_json::to_string(&event) {
            Ok(json) => Some(Ok(Event::default().data(json))),
            Err(e) => {
                tracing::error!(target: "http_server", error = %e, "failed to serialize event");
                None
            }
        },
        Err(tokio_stream::wrappers::errors::BroadcastStreamRecvError::Lagged(n)) => {
            tracing::warn!(target: "http_server", skipped = n, "SSE client lagged");
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
