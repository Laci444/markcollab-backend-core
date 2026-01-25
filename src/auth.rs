use axum::{
    http::{request::Parts, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use jsonwebtoken::{decode, Algorithm, DecodingKey, Validation};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;
use tracing::{debug, error, instrument, warn};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JwtClaims {
    pub sub: String,        // Subject (user_id)
    pub username: String,   // Username for display
    pub exp: u64,          // Expiration timestamp
    pub iat: u64,          // Issued at timestamp
}

#[derive(Debug, Clone)]
pub struct UserInfo {
    pub user_id: Uuid,
    pub username: String,
}

#[derive(Error, Debug)]
pub enum AuthError {
    #[error("Missing Authorization header")]
    MissingHeader,
    #[error("Invalid Authorization header format")]
    InvalidFormat,
    #[error("Invalid JWT token: {0}")]
    InvalidToken(String),
    #[error("Token expired")]
    TokenExpired,
    #[error("Invalid user ID format")]
    InvalidUserId,
}

impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        let status = match &self {
            AuthError::MissingHeader | AuthError::InvalidFormat => StatusCode::BAD_REQUEST,
            AuthError::TokenExpired | AuthError::InvalidToken(_) | AuthError::InvalidUserId => {
                StatusCode::UNAUTHORIZED
            }
        };

        error!(error = %self, status_code = %status, "Authentication error");

        let body = Json(serde_json::json!({
            "error": self.to_string(),
            "code": status.as_u16()
        }));

        (status, body).into_response()
    }
}

pub struct JwtValidator {
    decoding_key: DecodingKey,
    validation: Validation,
}

impl Clone for JwtValidator {
    fn clone(&self) -> Self {
        Self {
            decoding_key: self.decoding_key.clone(),
            validation: self.validation.clone(),
        }
    }
}

impl JwtValidator {
    pub fn new(secret: &str) -> Self {
        let mut validation = Validation::new(Algorithm::HS256);
        validation.validate_aud = false;
        validation.validate_exp = true;

        Self {
            decoding_key: DecodingKey::from_secret(secret.as_ref()),
            validation,
        }
    }

    #[instrument(skip(self, token), fields(token_length = token.len()))]
    pub fn validate_token(&self, token: &str) -> Result<UserInfo, AuthError> {
        debug!("Validating JWT token");

        let token_data = decode::<JwtClaims>(
            token,
            &self.decoding_key,
            &self.validation,
        ).map_err(|e| {
            error!(error = %e, "JWT decode failed");
            match e.kind() {
                jsonwebtoken::errors::ErrorKind::ExpiredSignature => AuthError::TokenExpired,
                _ => AuthError::InvalidToken(e.to_string()),
            }
        })?;

        let claims = token_data.claims;

        let user_id = Uuid::parse_str(&claims.sub)
            .map_err(|_| {
                error!(subject = %claims.sub, "Invalid UUID format in subject");
                AuthError::InvalidUserId
            })?;

        let user_info = UserInfo {
            user_id,
            username: claims.username,
        };

        debug!(
            user_id = %user_info.user_id,
            username = %user_info.username,
            "JWT validated successfully"
        );

        Ok(user_info)
    }
}

// Unified auth extractor that wraps both approaches
#[derive(Clone)]
pub struct AuthState {
    pub validator: Option<JwtValidator>,
    pub use_mock: bool,
}

impl AuthState {
    pub fn mock() -> Self {
        Self {
            validator: None,
            use_mock: true,
        }
    }

    pub fn jwt(jwt_secret: String) -> Self {
        Self {
            validator: Some(JwtValidator::new(&jwt_secret)),
            use_mock: false,
        }
    }

    pub async fn extract_user_info(&self, parts: &Parts) -> Result<UserInfo, AuthError> {
        if self.use_mock {
            // Mock authentication from query params
            let query = parts.uri.query().unwrap_or("");
            let params: HashMap<String, String> = form_urlencoded::parse(query.as_bytes())
                .into_owned()
                .collect();

            let username = params.get("username")
                .cloned()
                .unwrap_or_else(|| "test_user".to_string());

            let user_id = params.get("user_id")
                .and_then(|id| Uuid::parse_str(id).ok())
                .unwrap_or_else(|| Uuid::new_v4());

            debug!(
                user_id = %user_id,
                username = %username,
                "Mock auth - user authenticated"
            );

            Ok(UserInfo {
                user_id,
                username,
            })
        } else {
            // JWT authentication from header
            let validator = self.validator.as_ref()
                .ok_or(AuthError::MissingHeader)?;

            let auth_header = parts.headers
                .get(http::header::AUTHORIZATION)
                .and_then(|h| h.to_str().ok())
                .ok_or(AuthError::MissingHeader)?;

            let token = auth_header
                .strip_prefix("Bearer ")
                .ok_or(AuthError::InvalidFormat)?;

            validator.validate_token(token)
        }
    }
}