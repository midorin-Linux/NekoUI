use std::sync::Arc;

use nekoui_config::loader::ServerConfig;
use tracing::info;

use crate::routes;
use crate::web_ui_agent::WebUiAgent;

#[derive(Clone)]
pub struct HttpServerState {
    pub agent: Arc<dyn WebUiAgent>,
    pub config: ServerConfig,
}

pub struct HttpServer {
    state: HttpServerState,
}

impl HttpServer {
    pub fn new(agent: Arc<dyn WebUiAgent>, config: ServerConfig) -> Self {
        Self {
            state: HttpServerState { agent, config },
        }
    }

    pub async fn serve(self) -> anyhow::Result<()> {
        let state = self.state;
        let addr: std::net::SocketAddr = state.config.bind_address.parse()?;

        // Create conversation store
        let store = Arc::new(routes::ConversationStore::default());

        // Build application router
        let app = routes::build_routes(state, store);

        info!(addr = %addr, "starting HTTP/WebSocket server");
        let listener = tokio::net::TcpListener::bind(addr).await?;
        axum::serve(listener, app).await?;
        Ok(())
    }
}
