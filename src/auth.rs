use jsonwebtoken::{decode, DecodingKey, Validation, Algorithm};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tracing::{debug, error, warn, instrument};
use uuid::Uuid;
use warp::{Filter, Rejection, Reply};

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

impl warp::reject::Reject for AuthError {}

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

// MOCK AUTH FILTER - FOR DEVELOPMENT ONLY
pub fn with_mock_auth() -> impl Filter<Extract = (UserInfo,), Error = Rejection> + Clone {
    warp::query::<std::collections::HashMap<String, String>>()
        .and_then(|params: std::collections::HashMap<String, String>| async move {
            let username = params.get("username")
                .unwrap_or(&"test_user".to_string())
                .clone();
            
            let user_id = params.get("user_id")
                .and_then(|id| Uuid::parse_str(id).ok())
                .unwrap_or_else(|| Uuid::new_v4());

            debug!(
                user_id = %user_id,
                username = %username,
                "Mock auth - user authenticated"
            );

            Ok::<UserInfo, Rejection>(UserInfo {
                user_id,
                username,
            })
        })
}

// Real JWT auth filter
pub fn with_auth(
    jwt_secret: String,
) -> impl Filter<Extract = (UserInfo,), Error = Rejection> + Clone {
    let validator = JwtValidator::new(&jwt_secret);
    
    warp::header::optional::<String>("authorization")
        .and(warp::any().map(move || validator.clone()))
        .and_then(|auth_header: Option<String>, validator: JwtValidator| async move {
            extract_user_info(auth_header, &validator).await
                .map_err(|e| {
                    warn!(error = %e, "Authentication failed");
                    warp::reject::custom(e)
                })
        })
}

#[instrument(skip(validator))]
async fn extract_user_info(
    auth_header: Option<String>,
    validator: &JwtValidator,
) -> Result<UserInfo, AuthError> {
    // Extract token from Authorization header
    let auth_value = auth_header.ok_or(AuthError::MissingHeader)?;
    let token = auth_value
        .strip_prefix("Bearer ")
        .ok_or(AuthError::InvalidFormat)?;

    validator.validate_token(token)
}

pub async fn handle_auth_rejection(err: Rejection) -> Result<impl Reply, std::convert::Infallible> {
    if let Some(auth_error) = err.find::<AuthError>() {
        let code = match auth_error {
            AuthError::MissingHeader | AuthError::InvalidFormat => warp::http::StatusCode::BAD_REQUEST,
            AuthError::TokenExpired | AuthError::InvalidToken(_) | AuthError::InvalidUserId => {
                warp::http::StatusCode::UNAUTHORIZED
            }
        };
        
        error!(error = %auth_error, status_code = %code, "Authentication error");
        
        let json = warp::reply::json(&serde_json::json!({
            "error": auth_error.to_string(),
            "code": code.as_u16()
        }));
        
        Ok(warp::reply::with_status(json, code))
    } else {
        let code = warp::http::StatusCode::INTERNAL_SERVER_ERROR;
        let json = warp::reply::json(&serde_json::json!({
            "error": "Internal server error"
        }));
        
        Ok(warp::reply::with_status(json, code))
    }
}