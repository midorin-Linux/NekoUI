use std::sync::Arc;

use anyhow::Result;
use nekoui_agent::runtime::Agent;
use nekoui_api_server::server::HttpServer;
use nekoui_config::Config;
use tracing::info;

pub enum ChatClient {
    Http(HttpServer),
}

impl ChatClient {
    pub async fn initialize(config: &Config, runtime: Agent) -> Result<Self> {
        info!("initializing HTTP/WebSocket API server");
        let runtime = Arc::new(runtime);
        let server_config = config.server.clone();
        let http_server = HttpServer::new(runtime, server_config);

        Ok(Self::Http(http_server))
    }

    pub async fn run(self) -> Result<()> {
        match self {
            Self::Http(server) => server.serve().await,
        }
    }
}
