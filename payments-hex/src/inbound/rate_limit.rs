//! Rate limiting middleware using Governor.
//!
//! Implements per-API-key rate limiting with a token bucket algorithm.

use axum::{
    Json,
    body::Body,
    extract::State,
    http::{Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use dashmap::DashMap;
use governor::{
    Quota, RateLimiter,
    clock::DefaultClock,
    state::{InMemoryState, NotKeyed},
};
use serde_json::json;
use std::{num::NonZeroU32, sync::Arc, time::Duration};

/// Rate limiter state shared across requests.
pub struct RateLimiterState {
    /// Per-key rate limiters
    limiters: DashMap<String, Arc<RateLimiter<NotKeyed, InMemoryState, DefaultClock>>>,
    /// Default quota for new keys
    quota: Quota,
}

impl Default for RateLimiterState {
    fn default() -> Self {
        Self::new(100, Duration::from_secs(60))
    }
}

impl RateLimiterState {
    /// Creates a new rate limiter state.
    ///
    /// # Arguments
    /// * `requests` - Number of requests allowed per period
    /// * `period` - Time period for the quota
    pub fn new(requests: u32, period: Duration) -> Self {
        let quota = Quota::with_period(period)
            .unwrap()
            .allow_burst(NonZeroU32::new(requests).unwrap());

        Self {
            limiters: DashMap::new(),
            quota,
        }
    }

    /// Checks if a request should be rate limited.
    /// Returns true if the request is allowed, false if rate limited.
    pub fn check(&self, key: &str) -> bool {
        let limiter = self
            .limiters
            .entry(key.to_string())
            .or_insert_with(|| Arc::new(RateLimiter::direct(self.quota)));

        limiter.check().is_ok()
    }
}

/// Rate limiting middleware.
/// Expects the API key hash to be extracted by auth middleware first.
pub async fn rate_limit_middleware(
    State(limiter): State<Arc<RateLimiterState>>,
    request: Request<Body>,
    next: Next,
) -> Response {
    // Skip rate limiting for health endpoint
    if request.uri().path() == "/health" {
        return next.run(request).await;
    }

    // Get API key from Authorization header for rate limiting
    let key = request
        .headers()
        .get("Authorization")
        .and_then(|h| h.to_str().ok())
        .map(|s| s.trim_start_matches("Bearer ").to_string())
        .unwrap_or_else(|| "anonymous".to_string());

    // Check rate limit
    if !limiter.check(&key) {
        return (
            StatusCode::TOO_MANY_REQUESTS,
            Json(json!({
                "error": "Rate limit exceeded. Please try again later.",
                "retry_after_seconds": 60
            })),
        )
            .into_response();
    }

    next.run(request).await
}
