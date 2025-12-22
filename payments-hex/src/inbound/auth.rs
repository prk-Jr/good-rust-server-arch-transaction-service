//! Authentication middleware for API key validation.

use std::sync::Arc;

use axum::{
    Json,
    body::Body,
    extract::State,
    http::{Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};

use payments_types::TransactionRepository;

use super::handlers::AppState;

/// Extracts the API key from the Authorization header.
/// Expected format: "Bearer <api_key>" or just "<api_key>"
fn extract_api_key(auth_header: Option<&str>) -> Option<&str> {
    let header = auth_header?;
    if header.starts_with("Bearer ") {
        Some(header.strip_prefix("Bearer ").unwrap())
    } else {
        Some(header)
    }
}

/// Authentication middleware that validates API keys.
///
/// This middleware:
/// 1. Extracts the API key from the Authorization header
/// 2. Hashes it using SHA-256
/// 3. Verifies the hash against the database
/// 4. Returns 401 Unauthorized if validation fails
///
/// Endpoints that bypass authentication:
/// - `/health` - Health check endpoint
/// - `POST /api/bootstrap` - Creates the first API key (only works when no keys exist)
pub async fn auth_middleware<R: TransactionRepository>(
    State(state): State<Arc<AppState<R>>>,
    request: Request<Body>,
    next: Next,
) -> Response {
    // Skip authentication for health endpoint
    if request.uri().path() == "/health" {
        return next.run(request).await;
    }

    // Skip authentication for bootstrap endpoint (it has its own protection)
    if request.uri().path() == "/api/bootstrap" && request.method() == axum::http::Method::POST {
        return next.run(request).await;
    }

    // Extract API key from Authorization header
    let auth_header = request
        .headers()
        .get("Authorization")
        .and_then(|v| v.to_str().ok());

    let api_key = match extract_api_key(auth_header) {
        Some(key) if !key.is_empty() => key,
        _ => {
            return unauthorized_response("Missing or invalid Authorization header");
        }
    };

    // Hash the API key
    let key_hash = payments_repo::security::hash_api_key(api_key);

    // Verify against database
    match state.service.repo().verify_api_key_hash(&key_hash).await {
        Ok(Some(_api_key)) => {
            // API key is valid, proceed with the request
            next.run(request).await
        }
        Ok(None) => {
            // API key not found or inactive
            unauthorized_response("Invalid API key")
        }
        Err(e) => {
            // Database error
            tracing::error!("API key verification failed: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": "Internal server error",
                    "code": 500
                })),
            )
                .into_response()
        }
    }
}

fn unauthorized_response(message: &str) -> Response {
    (
        StatusCode::UNAUTHORIZED,
        Json(serde_json::json!({
            "error": message,
            "code": 401
        })),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_api_key_bearer() {
        assert_eq!(
            extract_api_key(Some("Bearer sk_test_123")),
            Some("sk_test_123")
        );
    }

    #[test]
    fn test_extract_api_key_raw() {
        assert_eq!(extract_api_key(Some("sk_test_123")), Some("sk_test_123"));
    }

    #[test]
    fn test_extract_api_key_none() {
        assert_eq!(extract_api_key(None), None);
    }
}
