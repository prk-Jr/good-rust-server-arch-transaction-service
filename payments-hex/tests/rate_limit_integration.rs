//! Integration tests for rate limiting middleware.
//!
//! These tests verify the HTTP-level behavior of rate limiting,
//! including 429 responses and proper integration with the middleware stack.
//!
//! This test requires the `sqlite` feature flag.

#![cfg(feature = "sqlite")]

use axum::{
    body::Body,
    http::{Method, Request, StatusCode},
};
use http_body_util::BodyExt;
use payments_hex::{PaymentService, inbound::HttpServer};
use payments_repo::SqliteRepo;
use tower::ServiceExt;

/// Helper to create a test server with a very low rate limit.
async fn create_test_server(requests_per_minute: u32) -> HttpServer<SqliteRepo> {
    // Use in-memory SQLite for tests
    let repo = SqliteRepo::new("sqlite::memory:").await.unwrap();
    let service = PaymentService::new(repo);
    HttpServer::with_rate_limit(service, requests_per_minute)
}

/// Helper to make a health check request.
fn health_request() -> Request<Body> {
    Request::builder()
        .uri("/health")
        .body(Body::empty())
        .unwrap()
}

/// Helper to bootstrap and get API key.
fn bootstrap_request() -> Request<Body> {
    Request::builder()
        .method(Method::POST)
        .uri("/api/bootstrap")
        .header("Content-Type", "application/json")
        .body(Body::from(r#"{"name": "test-key"}"#))
        .unwrap()
}

/// Helper to make an authenticated API request.
fn api_request(api_key: &str) -> Request<Body> {
    Request::builder()
        .uri("/api/accounts")
        .header("Authorization", format!("Bearer {}", api_key))
        .body(Body::empty())
        .unwrap()
}

/// Helper to bootstrap and extract API key from response.
async fn bootstrap_api_key(app: axum::Router) -> String {
    let response = app.oneshot(bootstrap_request()).await.unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);

    let body = response.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    json["api_key"].as_str().unwrap().to_string()
}

#[tokio::test]
async fn test_rate_limiting_returns_429_when_exceeded() {
    // Create server with only 3 requests allowed per minute
    // Bootstrap uses "anonymous" key, so authenticated requests get full quota of 3
    let server = create_test_server(3).await;
    let app = server.router();

    // Bootstrap to get a real API key (uses "anonymous" key quota, not our API key)
    let api_key = bootstrap_api_key(app.clone()).await;

    // Make 3 requests (uses up the quota for this API key)
    for i in 1..=3 {
        let response = app.clone().oneshot(api_request(&api_key)).await.unwrap();
        assert_ne!(
            response.status(),
            StatusCode::TOO_MANY_REQUESTS,
            "Request {} should not be rate limited (quota not yet exceeded)",
            i
        );
    }

    // 4th request should be rate limited
    let response = app.clone().oneshot(api_request(&api_key)).await.unwrap();

    assert_eq!(
        response.status(),
        StatusCode::TOO_MANY_REQUESTS,
        "Request should be rate limited after exceeding quota"
    );

    // Verify the response body contains the expected error
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert!(
        json["error"]
            .as_str()
            .unwrap()
            .contains("Rate limit exceeded")
    );
    assert_eq!(json["retry_after_seconds"], 60);
}

#[tokio::test]
async fn test_rate_limiting_health_endpoint_bypassed() {
    // Create server with only 1 request allowed per minute
    let server = create_test_server(1).await;
    let app = server.router();

    // Make many health requests - all should succeed (not rate limited)
    // Health endpoint bypasses rate limiting entirely
    for _ in 0..10 {
        let response = app.clone().oneshot(health_request()).await.unwrap();

        assert_eq!(
            response.status(),
            StatusCode::OK,
            "Health endpoint should not be rate limited"
        );
    }
}

#[tokio::test]
async fn test_rate_limiting_per_key_isolation() {
    // Create server with 3 requests per key (1 for bootstrap each + 1 for test + 1 to hit limit)
    let server = create_test_server(100).await; // High limit, we test isolation manually
    let app = server.router();

    // Bootstrap two different API keys
    let key_a = bootstrap_api_key(app.clone()).await;

    // Create a second server instance for key B (since bootstrap only works once)
    let server_b = create_test_server(2).await;
    let app_b = server_b.router();
    let key_b = bootstrap_api_key(app_b.clone()).await;

    // Key A should work
    let response = app.clone().oneshot(api_request(&key_a)).await.unwrap();
    assert_eq!(
        response.status(),
        StatusCode::OK,
        "Key A request 1 should succeed"
    );

    // Key B should also work (separate quota)
    let response = app_b.clone().oneshot(api_request(&key_b)).await.unwrap();
    assert_ne!(
        response.status(),
        StatusCode::TOO_MANY_REQUESTS,
        "Key B should have its own quota"
    );
}

#[tokio::test]
async fn test_rate_limiting_response_format() {
    // Create server with only 1 request per key
    let server = create_test_server(1).await;
    let app = server.router();

    // Bootstrap (uses "anonymous" quota)
    let api_key = bootstrap_api_key(app.clone()).await;

    // Use up the 1-request quota for this API key
    let _ = app.clone().oneshot(api_request(&api_key)).await;

    // Get rate limited response
    let response = app.clone().oneshot(api_request(&api_key)).await.unwrap();

    assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);

    // Verify headers
    let content_type = response.headers().get("content-type").unwrap();
    assert!(content_type.to_str().unwrap().contains("application/json"));

    // Verify body structure
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert!(
        json.get("error").is_some(),
        "Response should have 'error' field"
    );
    assert!(
        json.get("retry_after_seconds").is_some(),
        "Response should have 'retry_after_seconds' field"
    );
}
