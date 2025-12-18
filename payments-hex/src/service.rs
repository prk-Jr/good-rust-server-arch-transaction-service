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

        self.repo.deposit(req).await.map_err(Into::into)
    }

    /// Withdraws money from an account.
    pub async fn withdraw(&self, req: WithdrawRequest) -> Result<Transaction, AppError> {
        if req.amount <= 0 {
            return Err(AppError::BadRequest("Amount must be positive".into()));
        }

        self.repo.withdraw(req).await.map_err(Into::into)
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

        self.repo.transfer(req).await.map_err(Into::into)
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
}
