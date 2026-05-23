use std::sync::Arc;

use anyhow::Result;
use nekoui_agent::runtime::AgentRuntime;
use nekoui_config::loader::Config;
use nekoui_infra::{http_server::HttpServer, web_ui_agent::WebUiAgent};
use tracing::info;

pub enum ChatClient {
    Http(HttpServer),
}

impl ChatClient {
    pub async fn initialize(config: &Config, runtime: AgentRuntime) -> Result<Self> {
        let _mcp_servers = nekoui_config::mcp_config::load_mcp_servers()?;

        info!("initializing HTTP/WebSocket API server");
        let runtime: Arc<dyn WebUiAgent> = Arc::new(runtime);
        let server_config = config.server.clone();
        let http_server = HttpServer::new(runtime, server_config);

        Ok(Self::Http(http_server))
    }

    pub fn platform_name(&self) -> &'static str {
        match self {
            Self::Http(_) => "http",
        }
    }

    pub async fn run(self) -> Result<()> {
        match self {
            Self::Http(server) => server.serve().await,
        }
    }
}
