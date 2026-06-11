use std::sync::Arc;

use axum::{Router, http::StatusCode, routing::get};
use nekoui_agent::runtime::Agent;
use nekoui_config::server::ServerConfig;
use nekoui_telemetry::{State, print_log};

use super::routes::*;
use crate::{cors::build_cors_layer, routes::AppState};

#[derive(Clone)]
pub struct HttpServerState {
    pub agent: Arc<Agent>,
    pub config: ServerConfig,
}

pub struct HttpServer {
    state: HttpServerState,
}

impl HttpServer {
    pub fn new(agent: Arc<Agent>, config: ServerConfig) -> Self {
        Self {
            state: HttpServerState { agent, config },
        }
    }

    pub async fn serve(&self) -> anyhow::Result<()> {
        let addr: std::net::SocketAddr =
            format!("127.0.0.1:{}", self.state.config.bind_address).parse()?;

        let app = self.build_routes();

        print_log(
            State::Ok,
            format!("starting HTTP/WebSocket server: http://{}", addr).as_str(),
        );
        let listener = tokio::net::TcpListener::bind(addr).await?;
        axum::serve(listener, app).await?;
        Ok(())
    }

    fn build_routes(&self) -> Router {
        let app_state = AppState {
            http_state: self.state.clone(),
        };

        let cors = build_cors_layer(&self.state.config);

        let api_router = Router::new()
            .route("/health", get(StatusCode::OK))
            .route(
                "/sessions",
                get(sessions::list_sessions).post(sessions::create_session),
            )
            .route(
                "/sessions/{id}",
                get(sessions::get_session)
                    .patch(sessions::patch_session)
                    .delete(sessions::delete_session),
            )
            .route("/sessions/{id}/messages", get(messages::get_messages))
            .with_state(app_state);

        Router::new().nest("/api/v1", api_router).layer(cors)
    }
}
