//! # Payments Client SDK
//!
//! A typed Rust client for the Payments API.

use payments_types::{
    Account, AccountId, CreateAccountRequest, Currency, DepositRequest, Transaction,
    TransferRequest, WithdrawRequest,
};
use reqwest::Client;
use serde::de::DeserializeOwned;

/// Error type for client operations.
#[derive(Debug, thiserror::Error)]
pub enum ClientError {
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("API error: {status} - {message}")]
    Api { status: u16, message: String },

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

/// Payments API client.
pub struct PaymentsClient {
    base_url: String,
    api_key: Option<String>,
    http: Client,
}

impl PaymentsClient {
    /// Creates a new client.
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into().trim_end_matches('/').to_string(),
            api_key: None,
            http: Client::new(),
        }
    }

    /// Sets the API key for authentication.
    pub fn with_api_key(mut self, api_key: impl Into<String>) -> Self {
        self.api_key = Some(api_key.into());
        self
    }

    /// Checks if the API is healthy.
    pub async fn health(&self) -> Result<bool, ClientError> {
        let resp = self
            .http
            .get(format!("{}/health", self.base_url))
            .send()
            .await?;
        Ok(resp.status().is_success())
    }

    /// Creates a new account.
    pub async fn create_account(
        &self,
        name: &str,
        currency: Currency,
    ) -> Result<Account, ClientError> {
        let req = CreateAccountRequest {
            name: name.to_string(),
            currency,
        };
        self.post("/api/accounts", &req).await
    }

    /// Gets an account by ID.
    pub async fn get_account(&self, id: AccountId) -> Result<Account, ClientError> {
        self.get(&format!("/api/accounts/{}", id)).await
    }

    /// Lists all accounts.
    pub async fn list_accounts(&self) -> Result<Vec<Account>, ClientError> {
        self.get("/api/accounts").await
    }

    /// Deposits money into an account.
    pub async fn deposit(
        &self,
        account_id: AccountId,
        amount: i64,
        currency: Currency,
        idempotency_key: Option<String>,
        reference: Option<String>,
    ) -> Result<Transaction, ClientError> {
        let req = DepositRequest {
            account_id,
            amount,
            currency,
            idempotency_key,
            reference,
        };
        self.post("/api/transactions/deposit", &req).await
    }

    /// Withdraws money from an account.
    pub async fn withdraw(
        &self,
        account_id: AccountId,
        amount: i64,
        currency: Currency,
        idempotency_key: Option<String>,
        reference: Option<String>,
    ) -> Result<Transaction, ClientError> {
        let req = WithdrawRequest {
            account_id,
            amount,
            currency,
            idempotency_key,
            reference,
        };
        self.post("/api/transactions/withdraw", &req).await
    }

    /// Transfers money between accounts.
    pub async fn transfer(
        &self,
        from_account_id: AccountId,
        to_account_id: AccountId,
        amount: i64,
        currency: Currency,
        idempotency_key: Option<String>,
        reference: Option<String>,
    ) -> Result<Transaction, ClientError> {
        let req = TransferRequest {
            from_account_id,
            to_account_id,
            amount,
            currency,
            idempotency_key,
            reference,
        };
        self.post("/api/transactions/transfer", &req).await
    }

    async fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T, ClientError> {
        let mut req = self.http.get(format!("{}{}", self.base_url, path));
        if let Some(key) = &self.api_key {
            req = req.header("X-API-Key", key);
        }
        let resp = req.send().await?;
        self.handle_response(resp).await
    }

    async fn post<T: DeserializeOwned, B: serde::Serialize>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T, ClientError> {
        let mut req = self
            .http
            .post(format!("{}{}", self.base_url, path))
            .json(body);
        if let Some(key) = &self.api_key {
            req = req.header("X-API-Key", key);
        }
        let resp = req.send().await?;
        self.handle_response(resp).await
    }

    async fn handle_response<T: DeserializeOwned>(
        &self,
        resp: reqwest::Response,
    ) -> Result<T, ClientError> {
        let status = resp.status();
        if status.is_success() {
            let body = resp.text().await?;
            Ok(serde_json::from_str(&body)?)
        } else {
            let body = resp.text().await.unwrap_or_default();
            let message = serde_json::from_str::<serde_json::Value>(&body)
                .ok()
                .and_then(|v| v.get("error").and_then(|e| e.as_str()).map(String::from))
                .unwrap_or(body);
            Err(ClientError::Api {
                status: status.as_u16(),
                message,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_creation() {
        let client = PaymentsClient::new("http://localhost:3000");
        assert_eq!(client.base_url, "http://localhost:3000");
    }

    #[test]
    fn test_client_with_trailing_slash() {
        let client = PaymentsClient::new("http://localhost:3000/");
        assert_eq!(client.base_url, "http://localhost:3000");
    }

    #[test]
    fn test_client_with_api_key() {
        let client = PaymentsClient::new("http://localhost:3000").with_api_key("test-key");
        assert_eq!(client.api_key, Some("test-key".to_string()));
    }
}
