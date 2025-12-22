//! Payment Application Service
//!
//! Orchestrates domain operations through the repository port.
//! Contains NO infrastructure logic - pure business orchestration.

use payments_types::{
    Account, AccountId, AppError, CreateAccountRequest, DepositRequest, Transaction, TransactionId,
    TransactionRepository, TransferRequest, WithdrawRequest,
};

/// Application service for payment operations.
///
/// Generic over `R: TransactionRepository` - the adapter is injected at compile time.
/// This enables:
/// - Swapping repositories without code changes
/// - Testing with in-memory repo
/// - Compile-time checks for port implementation
pub struct PaymentService<R: TransactionRepository> {
    repo: R,
}

impl<R: TransactionRepository> PaymentService<R> {
    /// Creates a new payment service with the given repository.
    pub fn new(repo: R) -> Self {
        Self { repo }
    }

    /// Returns a reference to the underlying repository.
    pub fn repo(&self) -> &R {
        &self.repo
    }

    // ─────────────────────────────────────────────────────────────────────────────
    // Account Operations
    // ─────────────────────────────────────────────────────────────────────────────

    /// Creates a new account.
    pub async fn create_account(&self, req: CreateAccountRequest) -> Result<Account, AppError> {
        // Validation could be added here
        if req.name.trim().is_empty() {
            return Err(AppError::BadRequest("Account name cannot be empty".into()));
        }

        self.repo.create_account(req).await.map_err(Into::into)
    }

    /// Gets an account by ID.
    pub async fn get_account(&self, id: AccountId) -> Result<Account, AppError> {
        self.repo
            .get_account(id)
            .await
            .map_err(Into::into)
            .and_then(|opt| opt.ok_or_else(|| AppError::NotFound(format!("Account {}", id))))
    }

    /// Lists all accounts.
    pub async fn list_accounts(&self) -> Result<Vec<Account>, AppError> {
        self.repo.list_accounts().await.map_err(Into::into)
    }

    // ─────────────────────────────────────────────────────────────────────────────
    // Transaction Operations
    // ─────────────────────────────────────────────────────────────────────────────

    /// Deposits money into an account.
    pub async fn deposit(&self, req: DepositRequest) -> Result<Transaction, AppError> {
        // Business validation
        if req.amount <= 0 {
            return Err(AppError::BadRequest("Amount must be positive".into()));
        }

        let transaction = self.repo.deposit(req).await.map_err(AppError::from)?;

        // Trigger webhook
        let payload = serde_json::json!({
            "transaction_id": transaction.id,
            "account_id": transaction.destination_account_id,
            "amount": transaction.amount.amount(),
            "currency": transaction.amount.currency(),
            "reference": transaction.reference,
        });
        self.trigger_webhook("deposit.success", payload).await;

        Ok(transaction)
    }

    /// Withdraws money from an account.
    pub async fn withdraw(&self, req: WithdrawRequest) -> Result<Transaction, AppError> {
        if req.amount <= 0 {
            return Err(AppError::BadRequest("Amount must be positive".into()));
        }

        let transaction = self.repo.withdraw(req).await.map_err(AppError::from)?;

        // Trigger webhook
        let payload = serde_json::json!({
            "transaction_id": transaction.id,
            "account_id": transaction.source_account_id,
            "amount": transaction.amount.amount(),
            "currency": transaction.amount.currency(),
            "reference": transaction.reference,
        });
        self.trigger_webhook("withdraw.success", payload).await;

        Ok(transaction)
    }

    /// Transfers money between accounts.
    pub async fn transfer(&self, req: TransferRequest) -> Result<Transaction, AppError> {
        if req.amount <= 0 {
            return Err(AppError::BadRequest("Amount must be positive".into()));
        }

        if req.from_account_id == req.to_account_id {
            return Err(AppError::BadRequest(
                "Cannot transfer to the same account".into(),
            ));
        }

        let transaction = self.repo.transfer(req).await.map_err(AppError::from)?;

        // Trigger webhook
        let payload = serde_json::json!({
            "transaction_id": transaction.id,
            "from_account_id": transaction.source_account_id,
            "to_account_id": transaction.destination_account_id,
            "amount": transaction.amount.amount(),
            "currency": transaction.amount.currency(),
            "reference": transaction.reference,
        });
        self.trigger_webhook("transfer.success", payload).await;

        Ok(transaction)
    }

    // ─────────────────────────────────────────────────────────────────────────────
    // Transaction History
    // ─────────────────────────────────────────────────────────────────────────────

    /// Gets a transaction by ID.
    pub async fn get_transaction(&self, id: TransactionId) -> Result<Transaction, AppError> {
        self.repo
            .get_transaction(id)
            .await
            .map_err(Into::into)
            .and_then(|opt| opt.ok_or_else(|| AppError::NotFound(format!("Transaction {}", id))))
    }

    /// Lists transactions for an account.
    pub async fn list_transactions(
        &self,
        account_id: AccountId,
    ) -> Result<Vec<Transaction>, AppError> {
        // Verify account exists first
        let _ = self.get_account(account_id).await?;

        self.repo
            .list_transactions_for_account(account_id)
            .await
            .map_err(Into::into)
    }

    // ─────────────────────────────────────────────────────────────────────────────
    // Webhook Logic
    // ─────────────────────────────────────────────────────────────────────────────

    async fn trigger_webhook(&self, event_type: &str, payload: serde_json::Value) {
        use payments_types::WebhookEndpointId;

        // 1. List all endpoints (naive approach, better would be to filter in DB)
        let endpoints = match self.repo.list_webhook_endpoints().await {
            Ok(eps) => eps,
            Err(e) => {
                tracing::error!("Failed to list webhooks for trigger: {}", e);
                return;
            }
        };

        // 2. Filter interesting endpoints
        // Note: For demo simplicity, we match exact string. A regex or wildcard system is better.
        let targets: Vec<_> = endpoints
            .into_iter()
            .filter(|ep| ep.is_active && ep.events.contains(&event_type.to_string()))
            .collect();

        for endpoint in targets {
            let endpoint_id = WebhookEndpointId::from_uuid(endpoint.id);
            // 3. Create event in DB
            if let Err(e) = self
                .repo
                .create_webhook_event(endpoint_id, event_type, payload.clone())
                .await
            {
                tracing::error!("Failed to persist webhook event: {}", e);
                continue;
            }

            // 4. Send event (Fire and forget via tokio spawn)
            let url = endpoint.url.clone();
            let payload = payload.clone();
            let event_type = event_type.to_string();

            tokio::spawn(async move {
                let client = reqwest::Client::new();
                // Construct standard wrapper if needed, or just send payload
                // Usually webhooks wrap: { "event": "type", "data": payload }
                let body = serde_json::json!({
                    "event": event_type,
                    "data": payload
                });

                tracing::info!("Sending webhook {} to {}", event_type, url);

                match client.post(&url).json(&body).send().await {
                    Ok(resp) => {
                        if !resp.status().is_success() {
                            tracing::warn!(
                                "Webhook to {} failed with status {}",
                                url,
                                resp.status()
                            );
                        } else {
                            tracing::info!("Webhook sent to {}", url);
                        }
                    }
                    Err(e) => {
                        tracing::error!("Failed to send webhook request to {}: {}", url, e);
                    }
                }
            });
        }
    }
}
