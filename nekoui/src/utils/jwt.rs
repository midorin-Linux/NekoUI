use chrono::{Duration, Utc};
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation, decode, encode};
use serde::{Deserialize, Serialize};

use crate::error::JwtError;

pub const ACCESS_TOKEN_TTL: Duration = Duration::minutes(15);
const JWT_ALGORITHM: Algorithm = Algorithm::HS256;
const ACCESS_TOKEN_TYPE: &str = "access";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JwtClaims {
    pub sub: String,
    pub typ: String,
    pub iat: usize,
    pub exp: usize,
}

pub fn issue_access_token(user_id: &str, jwt_secret: &str) -> Result<String, JwtError> {
    let now = Utc::now();
    let iat = now.timestamp() as usize;
    let exp = (now + ACCESS_TOKEN_TTL).timestamp() as usize;

    let claims = JwtClaims {
        sub: user_id.to_string(),
        typ: ACCESS_TOKEN_TYPE.to_string(),
        iat,
        exp,
    };

    let token = encode(
        &Header::new(JWT_ALGORITHM),
        &claims,
        &EncodingKey::from_secret(jwt_secret.as_bytes()),
    )?;

    Ok(token)
}

pub fn verify_access_token(token: &str, jwt_secret: &str) -> Result<JwtClaims, JwtError> {
    let mut validation = Validation::new(JWT_ALGORITHM);
    validation.validate_aud = false;

    let claims = decode::<JwtClaims>(
        token,
        &DecodingKey::from_secret(jwt_secret.as_bytes()),
        &validation,
    )
    .map_err(JwtError::from_jsonwebtoken_error)?
    .claims;

    if claims.typ != ACCESS_TOKEN_TYPE {
        return Err(JwtError::InvalidTokenType);
    }

    Ok(claims)
}
