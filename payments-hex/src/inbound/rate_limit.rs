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

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_rate_limiter_allows_requests_within_limit() {
        // Allow 5 requests per minute
        let limiter = RateLimiterState::new(5, Duration::from_secs(60));

        // All 5 requests should be allowed
        for i in 1..=5 {
            assert!(limiter.check("test-key"), "Request {} should be allowed", i);
        }
    }

    #[test]
    fn test_rate_limiter_blocks_excess_requests() {
        // Allow only 3 requests per minute
        let limiter = RateLimiterState::new(3, Duration::from_secs(60));

        // First 3 requests should be allowed
        assert!(limiter.check("test-key"), "Request 1 should be allowed");
        assert!(limiter.check("test-key"), "Request 2 should be allowed");
        assert!(limiter.check("test-key"), "Request 3 should be allowed");

        // 4th request should be blocked
        assert!(!limiter.check("test-key"), "Request 4 should be blocked");
        assert!(
            !limiter.check("test-key"),
            "Request 5 should also be blocked"
        );
    }

    #[test]
    fn test_rate_limiter_per_key_isolation() {
        // Allow 2 requests per minute per key
        let limiter = RateLimiterState::new(2, Duration::from_secs(60));

        // Key A uses its quota
        assert!(limiter.check("key-a"), "Key A request 1 should be allowed");
        assert!(limiter.check("key-a"), "Key A request 2 should be allowed");
        assert!(!limiter.check("key-a"), "Key A request 3 should be blocked");

        // Key B should have its own separate quota
        assert!(limiter.check("key-b"), "Key B request 1 should be allowed");
        assert!(limiter.check("key-b"), "Key B request 2 should be allowed");
        assert!(!limiter.check("key-b"), "Key B request 3 should be blocked");
    }

    #[test]
    fn test_rate_limiter_default_config() {
        // Default is 100 requests per 60 seconds
        let limiter = RateLimiterState::default();

        // Should allow 100 requests
        for i in 1..=100 {
            assert!(
                limiter.check("default-key"),
                "Request {} should be allowed",
                i
            );
        }

        // 101st request should be blocked
        assert!(
            !limiter.check("default-key"),
            "Request 101 should be blocked"
        );
    }

    #[tokio::test]
    async fn test_rate_limiter_quota_replenishes() {
        // Allow 2 requests per 100ms (very short period for testing)
        let limiter = RateLimiterState::new(2, Duration::from_millis(100));

        // Use up the quota
        assert!(limiter.check("replenish-key"));
        assert!(limiter.check("replenish-key"));
        assert!(!limiter.check("replenish-key"), "Should be rate limited");

        // Wait for quota to replenish
        tokio::time::sleep(Duration::from_millis(150)).await;

        // Should be allowed again
        assert!(
            limiter.check("replenish-key"),
            "Quota should have replenished"
        );
    }

    #[test]
    fn test_rate_limiter_multiple_keys_independent() {
        let limiter = RateLimiterState::new(1, Duration::from_secs(60));

        // Each unique key gets 1 request
        assert!(limiter.check("key-1"));
        assert!(limiter.check("key-2"));
        assert!(limiter.check("key-3"));

        // But same keys are blocked
        assert!(!limiter.check("key-1"));
        assert!(!limiter.check("key-2"));
        assert!(!limiter.check("key-3"));
    }
}
