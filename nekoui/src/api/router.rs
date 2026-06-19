use axum::{Router, http::StatusCode, routing::get};

use crate::{api::routes::*, state::ServerState};

pub fn build_routes(server_state: ServerState) -> Router<ServerState> {
    let api_router = Router::new()
        .route("/health", get(StatusCode::OK))
        .nest("/auth", auth::router(server_state));

    Router::<ServerState>::new().nest("/api/v1", api_router)
}
