use axum::{
    Router,
    extract::{Extension, Json, State},
    response::IntoResponse,
    routing::{get, post},
};
use serde::{Deserialize, Serialize};

use crate::{api::response::ApiResponse, state::ServerState, utils::jwt::JwtClaims};

#[derive(Debug, Deserialize)]
pub struct RegisterRequest {
    pub email: String,
    pub password: String,
    pub display_name: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct RegisterResponse {
    pub refresh_token: String,
}

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
struct LoginResponse {
    refresh_token: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateRequest {
    pub display_name: Option<String>,
    pub avatar_url: Option<String>,
}

pub fn router() -> Router<ServerState> {
    let protected = Router::new()
        .route("/refresh", post(refresh))
        .route("/logout", post(logout))
        .route("/me", get(get_profile).patch(patch_profile));

    Router::<ServerState>::new()
        .route("/register", post(register))
        .route("/login", post(login))
        .merge(protected)
}

async fn register(
    State(state): State<ServerState>,
    Json(register_request): Json<RegisterRequest>,
) -> impl IntoResponse {
    let _user_record = if let Err(err) = state
        .services
        .auth_service()
        .await
        .register(&register_request)
        .await
    {
        ApiResponse::error(err.status_code(), err.code(), err.to_string())
    } else {
        ApiResponse::success(())
    };
}

async fn login(
    State(state): State<ServerState>,
    Json(login_request): Json<LoginRequest>,
) -> impl IntoResponse {
    let _token = if let Err(err) = state
        .services
        .auth_service()
        .await
        .login(&login_request)
        .await
    {
        ApiResponse::error(err.status_code(), err.code(), err.to_string())
    } else {
        ApiResponse::success(())
    };
}

async fn refresh(
    State(_state): State<ServerState>,
    Extension(claims): Extension<JwtClaims>,
) -> impl IntoResponse {
    ApiResponse::success(())
}

async fn logout(
    State(_state): State<ServerState>,
    Extension(claims): Extension<JwtClaims>,
) -> impl IntoResponse {
    ApiResponse::success(())
}

async fn get_profile(
    State(_state): State<ServerState>,
    Extension(claims): Extension<JwtClaims>,
) -> impl IntoResponse {
    ApiResponse::success(())
}

async fn patch_profile(
    State(_state): State<ServerState>,
    Extension(claims): Extension<JwtClaims>,
) -> impl IntoResponse {
    ApiResponse::success(())
}
