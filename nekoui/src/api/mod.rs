pub use super::utils::logging::{State, print_log};
pub use crate::wizard::ServerConfig;
use crate::{api::middleware::cors::build_cors_layer, state::ServerState};

pub mod middleware;
pub mod response;
pub mod router;
pub mod routes;

pub struct HttpServer {
    server_config: ServerConfig,
    server_state: ServerState,
}

impl HttpServer {
    pub fn new(server_config: ServerConfig, server_state: ServerState) -> Self {
        Self {
            server_config,
            server_state,
        }
    }

    pub async fn serve(&self) -> anyhow::Result<()> {
        let addr: std::net::SocketAddr =
            format!("127.0.0.1:{}", self.server_config.bind_address).parse()?;

        let routes = router::build_routes(self.server_state.clone());

        let cors = build_cors_layer(&self.server_config.allowed_origins);

        let app = routes.layer(cors).with_state(self.server_state.clone());

        print_log(
            State::Ok,
            format!("starting HTTP/WebSocket server: http://{}", addr).as_str(),
        );
        let listener = tokio::net::TcpListener::bind(addr).await?;
        axum::serve(listener, app).await?;
        Ok(())
    }
}
