use axum::{Router, http::StatusCode, routing::get};

use crate::{api::routes::*, state::ServerState};

pub fn build_routes() -> Router<ServerState> {
    let api_router = Router::new()
        .route("/health", get(StatusCode::OK))
        .nest("/auth", auth::router());

    Router::<ServerState>::new().nest("/api/v1", api_router)
}
