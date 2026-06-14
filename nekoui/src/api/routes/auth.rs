use axum::{
    Router,
    response::IntoResponse,
    routing::{get, post},
};

use crate::{api::response::ApiResponse, state::ServerState};

pub fn router() -> Router<ServerState> {
    Router::<ServerState>::new()
        .route("/register", post(register))
        .route("/login", post(login))
        .route("/refresh", post(refresh))
        .route("/logout", post(logout))
        .route("/me", get(get_profile).patch(patch_profile))
}

async fn register() -> impl IntoResponse {
    ApiResponse::success(())
}

async fn login() -> impl IntoResponse {
    ApiResponse::success(())
}

async fn refresh() -> impl IntoResponse {
    ApiResponse::success(())
}

async fn logout() -> impl IntoResponse {
    ApiResponse::success(())
}

async fn get_profile() -> impl IntoResponse {
    ApiResponse::success(())
}

async fn patch_profile() -> impl IntoResponse {
    ApiResponse::success(())
}
