use axum::http::StatusCode;

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
    #[error(transparent)]
    Database(#[from] sqlx::Error),
}

impl AuthError {
    pub fn status_code(&self) -> StatusCode {
        match self {
            Self::UserAlreadyExists => StatusCode::FORBIDDEN,
            Self::UserNotFound => StatusCode::NOT_FOUND,
            Self::PasswordIncorrect => StatusCode::UNAUTHORIZED,
            Self::FailedToHashPassword | Self::FailedToGenerateRefreshToken | Self::FailedToCreateUser | Self::Database(_) => {
                StatusCode::INTERNAL_SERVER_ERROR
            }
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
            Self::Database(_) => "DATABASE_ERROR",
        }
    }
}

// #[derive(Debug, thiserror::Error)]
// pub enum TokenError {}
// 
// impl TokenError {
//     pub fn status_code(&self) -> StatusCode {
//         match self {}
//     }
// 
//     pub fn code(&self) -> &'static str {
//         match self {}
//     }
// }
