use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Serialize;

#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    #[error("user already exists")]
    UserAlreadyExists,
    #[error("user not found")]
    UserNotFound,
    #[error("password incorrect")]
    PasswordIncorrect,
    #[error("failed to hash password")]
    FailedToHashPassword,
    #[error("failed to create user")]
    FailedToCreateUser,
    #[error("failed to generate refresh token")]
    FailedToGenerateRefreshToken,
    #[error("invalid or expired refresh token")]
    InvalidRefreshToken,
    #[error(transparent)]
    Database(#[from] sqlx::Error),
}

impl AuthError {
    pub fn status_code(&self) -> StatusCode {
        match self {
            Self::UserAlreadyExists => StatusCode::FORBIDDEN,
            Self::UserNotFound => StatusCode::NOT_FOUND,
            Self::PasswordIncorrect => StatusCode::UNAUTHORIZED,
            Self::FailedToHashPassword
            | Self::FailedToGenerateRefreshToken
            | Self::FailedToCreateUser
            | Self::Database(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Self::InvalidRefreshToken => StatusCode::UNAUTHORIZED,
        }
    }

    pub fn code(&self) -> &'static str {
        match self {
            Self::UserAlreadyExists => "USER_ALREADY_EXISTS",
            Self::UserNotFound => "USER_NOT_FOUND",
            Self::PasswordIncorrect => "PASSWORD_INCORRECT",
            Self::FailedToHashPassword => "FAILED_TO_HASH_PASSWORD",
            Self::FailedToCreateUser => "FAILED_TO_CREATE_USER",
            Self::FailedToGenerateRefreshToken => "FAILED_TO_GENERATE_REFRESH_TOKEN",
            Self::InvalidRefreshToken => "INVALID_REFRESH_TOKEN",
            Self::Database(_) => "DATABASE_ERROR",
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum JwtError {
    #[error("invalid token")]
    InvalidToken,
    #[error("token expired")]
    Expired,
    #[error("invalid token type")]
    InvalidTokenType,
    #[error("jwt error: {0}")]
    JsonWebToken(#[from] jsonwebtoken::errors::Error),
}

impl JwtError {
    pub fn from_jsonwebtoken_error(err: jsonwebtoken::errors::Error) -> Self {
        match err.kind() {
            jsonwebtoken::errors::ErrorKind::ExpiredSignature => Self::Expired,
            _ => Self::InvalidToken,
        }
    }

    pub fn into_app_error(self) -> AppError {
        match self {
            Self::Expired => AppError::token_expired(),
            Self::InvalidToken | Self::InvalidTokenType | Self::JsonWebToken(_) => {
                AppError::invalid_access_token()
            }
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ApiErrorDetail {
    pub code: String,
    pub message: String,
}

#[derive(Debug)]
pub struct AppError {
    pub status: StatusCode,
    pub code: String,
    pub message: String,
}

impl AppError {
    pub fn new(status: StatusCode, code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            status,
            code: code.into(),
            message: message.into(),
        }
    }

    pub fn unauthorized(code: &'static str, message: &'static str) -> Self {
        Self::new(StatusCode::UNAUTHORIZED, code, message)
    }

    pub fn missing_authorization_header() -> Self {
        Self::unauthorized(
            "MISSING_AUTHORIZATION_HEADER",
            "Authorization header is required",
        )
    }

    pub fn invalid_authorization_header() -> Self {
        Self::unauthorized(
            "INVALID_AUTHORIZATION_HEADER",
            "Authorization header is invalid",
        )
    }

    pub fn invalid_bearer_scheme() -> Self {
        Self::unauthorized(
            "INVALID_AUTHORIZATION_HEADER",
            "Authorization header must use Bearer scheme",
        )
    }

    pub fn invalid_access_token() -> Self {
        Self::unauthorized("INVALID_ACCESS_TOKEN", "Access token is required")
    }

    pub fn token_expired() -> Self {
        Self::unauthorized("TOKEN_EXPIRED", "Access token has expired")
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let body = ApiErrorResponse {
            success: false,
            data: None,
            error: Some(ApiErrorDetail {
                code: self.code,
                message: self.message,
            }),
        };

        (self.status, Json(body)).into_response()
    }
}

impl From<anyhow::Error> for AppError {
    fn from(err: anyhow::Error) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            code: "INTERNAL_SERVER_ERROR".to_string(),
            message: err.to_string(),
        }
    }
}

#[derive(Debug, Serialize)]
struct ApiErrorResponse {
    success: bool,
    data: Option<()>,
    error: Option<ApiErrorDetail>,
}
