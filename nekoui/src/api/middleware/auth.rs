use axum::{
    extract::{Request, State},
    http::header::AUTHORIZATION,
    middleware::Next,
    response::Response,
};

use crate::{error::AppError, state::ServerState, utils::jwt::verify_access_token};

pub async fn auth_middleware(
    State(state): State<ServerState>,
    mut request: Request,
    next: Next,
) -> Result<Response, AppError> {
    let token = extract_bearer_token(request.headers())?;
    let claims = verify_access_token(token, state.jwt_secret.as_str())
        .map_err(crate::error::JwtError::into_app_error)?;

    request.extensions_mut().insert(claims);

    Ok(next.run(request).await)
}

fn extract_bearer_token(headers: &axum::http::HeaderMap) -> Result<&str, AppError> {
    let header_value = headers
        .get(AUTHORIZATION)
        .ok_or_else(AppError::missing_authorization_header)?;
    let header_value = header_value
        .to_str()
        .map_err(|_| AppError::invalid_authorization_header())?;

    let token = header_value
        .strip_prefix("Bearer ")
        .ok_or_else(AppError::invalid_bearer_scheme)?;

    if token.is_empty() {
        return Err(AppError::invalid_access_token());
    }

    Ok(token)
}
