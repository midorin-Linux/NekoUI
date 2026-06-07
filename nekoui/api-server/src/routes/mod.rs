use crate::server::HttpServerState;

pub mod messages;
pub mod models;
pub mod providers;
pub mod search;
pub mod sessions;
pub mod settings;

#[derive(Clone)]
pub struct AppState {
    pub http_state: HttpServerState,
}
