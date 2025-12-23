//! SQLite repository integration tests.

#[cfg(test)]
mod tests {
    use payments_types::{
        AccountId, CreateAccountRequest, Currency, DepositRequest, DomainError, RepoError,
        TransactionRepository, TransferRequest, WebhookEndpointId, WithdrawRequest,
    };
    use uuid::Uuid;

    use crate::SqliteRepo;

    async fn setup_repo() -> SqliteRepo {
        SqliteRepo::new("sqlite::memory:").await.unwrap()
    }

    #[tokio::test]
    async fn test_create_account() {
        let repo = setup_repo().await;

        let req = CreateAccountRequest {
            name: "Test Account".to_string(),
            currency: Currency::USD,
        };

        let account = repo.create_account(req).await.unwrap();

        assert_eq!(account.name, "Test Account");
        assert_eq!(account.balance.amount(), 0);
        assert_eq!(account.balance.currency(), Currency::USD);
    }

    #[tokio::test]
    async fn test_get_account() {
        let repo = setup_repo().await;

        let req = CreateAccountRequest {
            name: "Test".to_string(),
            currency: Currency::USD,
        };
        let created = repo.create_account(req).await.unwrap();

        let fetched = repo.get_account(created.id).await.unwrap().unwrap();

        assert_eq!(fetched.id, created.id);
        assert_eq!(fetched.name, "Test");
    }

    #[tokio::test]
    async fn test_get_account_not_found() {
        let repo = setup_repo().await;

        let result = repo.get_account(AccountId::new()).await.unwrap();

        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_list_accounts() {
        let repo = setup_repo().await;

        repo.create_account(CreateAccountRequest {
            name: "Alice".to_string(),
            currency: Currency::USD,
        })
        .await
        .unwrap();

        repo.create_account(CreateAccountRequest {
            name: "Bob".to_string(),
            currency: Currency::EUR,
        })
        .await
        .unwrap();

        let accounts = repo.list_accounts().await.unwrap();

        assert_eq!(accounts.len(), 2);
    }

    #[tokio::test]
    async fn test_deposit() {
        let repo = setup_repo().await;

        let account = repo
            .create_account(CreateAccountRequest {
                name: "Test".to_string(),
                currency: Currency::USD,
            })
            .await
            .unwrap();

        let tx = repo
            .deposit(DepositRequest {
                account_id: account.id,
                amount: 1000,
                currency: Currency::USD,
                idempotency_key: None,
                reference: Some("Initial deposit".to_string()),
            })
            .await
            .unwrap();

        assert_eq!(tx.amount.amount(), 1000);

        let updated = repo.get_account(account.id).await.unwrap().unwrap();
        assert_eq!(updated.balance.amount(), 1000);
    }

    #[tokio::test]
    async fn test_deposit_account_not_found() {
        let repo = setup_repo().await;

        let result = repo
            .deposit(DepositRequest {
                account_id: AccountId::new(),
                amount: 1000,
                currency: Currency::USD,
                idempotency_key: None,
                reference: None,
            })
            .await;

        assert!(matches!(result, Err(RepoError::NotFound)));
    }

    #[tokio::test]
    async fn test_withdraw() {
        let repo = setup_repo().await;

        let account = repo
            .create_account(CreateAccountRequest {
                name: "Test".to_string(),
                currency: Currency::USD,
            })
            .await
            .unwrap();

        repo.deposit(DepositRequest {
            account_id: account.id,
            amount: 1000,
            currency: Currency::USD,
            idempotency_key: None,
            reference: None,
        })
        .await
        .unwrap();

        let tx = repo
            .withdraw(WithdrawRequest {
                account_id: account.id,
                amount: 300,
                currency: Currency::USD,
                idempotency_key: None,
                reference: None,
            })
            .await
            .unwrap();

        assert_eq!(tx.amount.amount(), 300);

        let updated = repo.get_account(account.id).await.unwrap().unwrap();
        assert_eq!(updated.balance.amount(), 700);
    }

    #[tokio::test]
    async fn test_withdraw_insufficient_funds() {
        let repo = setup_repo().await;

        let account = repo
            .create_account(CreateAccountRequest {
                name: "Test".to_string(),
                currency: Currency::USD,
            })
            .await
            .unwrap();

        repo.deposit(DepositRequest {
            account_id: account.id,
            amount: 100,
            currency: Currency::USD,
            idempotency_key: None,
            reference: None,
        })
        .await
        .unwrap();

        let result = repo
            .withdraw(WithdrawRequest {
                account_id: account.id,
                amount: 200,
                currency: Currency::USD,
                idempotency_key: None,
                reference: None,
            })
            .await;

        assert!(matches!(
            result,
            Err(RepoError::Domain(DomainError::InsufficientFunds { .. }))
        ));
    }

    #[tokio::test]
    async fn test_transfer() {
        let repo = setup_repo().await;

        let alice = repo
            .create_account(CreateAccountRequest {
                name: "Alice".to_string(),
                currency: Currency::USD,
            })
            .await
            .unwrap();

        let bob = repo
            .create_account(CreateAccountRequest {
                name: "Bob".to_string(),
                currency: Currency::USD,
            })
            .await
            .unwrap();

        repo.deposit(DepositRequest {
            account_id: alice.id,
            amount: 1000,
            currency: Currency::USD,
            idempotency_key: None,
            reference: None,
        })
        .await
        .unwrap();

        let tx = repo
            .transfer(TransferRequest {
                from_account_id: alice.id,
                to_account_id: bob.id,
                amount: 400,
                currency: Currency::USD,
                idempotency_key: None,
                reference: None,
            })
            .await
            .unwrap();

        assert_eq!(tx.amount.amount(), 400);

        let alice_updated = repo.get_account(alice.id).await.unwrap().unwrap();
        let bob_updated = repo.get_account(bob.id).await.unwrap().unwrap();

        assert_eq!(alice_updated.balance.amount(), 600);
        assert_eq!(bob_updated.balance.amount(), 400);
    }

    #[tokio::test]
    async fn test_transfer_cross_currency_fails() {
        let repo = setup_repo().await;

        let alice = repo
            .create_account(CreateAccountRequest {
                name: "Alice".to_string(),
                currency: Currency::USD,
            })
            .await
            .unwrap();

        let bob = repo
            .create_account(CreateAccountRequest {
                name: "Bob".to_string(),
                currency: Currency::EUR,
            })
            .await
            .unwrap();

        repo.deposit(DepositRequest {
            account_id: alice.id,
            amount: 1000,
            currency: Currency::USD,
            idempotency_key: None,
            reference: None,
        })
        .await
        .unwrap();

        let result = repo
            .transfer(TransferRequest {
                from_account_id: alice.id,
                to_account_id: bob.id,
                amount: 400,
                currency: Currency::USD,
                idempotency_key: None,
                reference: None,
            })
            .await;

        assert!(matches!(
            result,
            Err(RepoError::Domain(DomainError::CrossCurrencyTransfer))
        ));
    }

    #[tokio::test]
    async fn test_idempotency_deposit() {
        let repo = setup_repo().await;

        let account = repo
            .create_account(CreateAccountRequest {
                name: "Test".to_string(),
                currency: Currency::USD,
            })
            .await
            .unwrap();

        let key = "unique-deposit-key".to_string();

        // First deposit
        let _tx1 = repo
            .deposit(DepositRequest {
                account_id: account.id,
                amount: 1000,
                currency: Currency::USD,
                idempotency_key: Some(key.clone()),
                reference: None,
            })
            .await
            .unwrap();

        // Second deposit with same key - should return cached transaction
        let tx2 = repo
            .deposit(DepositRequest {
                account_id: account.id,
                amount: 1000,
                currency: Currency::USD,
                idempotency_key: Some(key.clone()),
                reference: None,
            })
            .await
            .unwrap();

        // The idempotency key lookup should find the original transaction
        let found = repo.find_by_idempotency_key(&key).await.unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().id, tx2.id);

        // Balance should only be credited once (this is the key invariant)
        let updated = repo.get_account(account.id).await.unwrap().unwrap();
        assert_eq!(updated.balance.amount(), 1000);
    }

    #[tokio::test]
    async fn test_list_transactions_for_account() {
        let repo = setup_repo().await;

        let account = repo
            .create_account(CreateAccountRequest {
                name: "Test".to_string(),
                currency: Currency::USD,
            })
            .await
            .unwrap();

        repo.deposit(DepositRequest {
            account_id: account.id,
            amount: 1000,
            currency: Currency::USD,
            idempotency_key: None,
            reference: None,
        })
        .await
        .unwrap();

        repo.withdraw(WithdrawRequest {
            account_id: account.id,
            amount: 200,
            currency: Currency::USD,
            idempotency_key: None,
            reference: None,
        })
        .await
        .unwrap();

        let transactions = repo
            .list_transactions_for_account(account.id)
            .await
            .unwrap();

        assert_eq!(transactions.len(), 2);
    }

    #[tokio::test]
    async fn test_webhook_generation() {
        let repo = setup_repo().await;

        let account = repo
            .create_account(CreateAccountRequest {
                name: "Webhook Test".to_string(),
                currency: Currency::USD,
            })
            .await
            .unwrap();

        // Manually create a webhook event (simulating PaymentService behavior)
        let endpoint_id = WebhookEndpointId(Uuid::new_v4());
        let payload = serde_json::json!({
            "type": "DEPOSIT",
            "amount": 500,
            "currency": "USD",
            "account_id": account.id,
        });

        repo.create_webhook_event(endpoint_id, "DEPOSIT_COMPLETED", payload)
            .await
            .unwrap();

        // 2. Fetch pending webhooks
        let events = repo.get_pending_webhooks(10).await.unwrap();
        assert_eq!(events.len(), 1);

        let event = &events[0];
        assert_eq!(event.event_type, "DEPOSIT_COMPLETED");
        assert_eq!(event.status, payments_types::WebhookStatus::Pending);

        // Check payload
        let payload = &event.payload;
        assert_eq!(payload["type"], "DEPOSIT");
        assert_eq!(payload["amount"], 500);

        // 3. Update status
        repo.update_webhook_status(event.id, payments_types::WebhookStatus::Completed, None)
            .await
            .unwrap();

        // 4. Verify no pending webhooks
        let events_after = repo.get_pending_webhooks(10).await.unwrap();
        assert!(events_after.is_empty());
    }

    // ─────────────────────────────────────────────────────────────────────────────
    // API Key Management Tests
    // ─────────────────────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_create_api_key() {
        let repo = setup_repo().await;

        // Count should start at 0
        let count_before = repo.count_api_keys().await.unwrap();
        assert_eq!(count_before, 0);

        // Create an API key
        let (api_key, raw_key) = repo.create_api_key("test-key").await.unwrap();

        assert_eq!(api_key.name, "test-key");
        assert!(api_key.is_active);
        assert!(raw_key.starts_with("sk_"));
        assert_eq!(raw_key.len(), 35); // "sk_" + 32 chars

        // Count should be 1 now
        let count_after = repo.count_api_keys().await.unwrap();
        assert_eq!(count_after, 1);
    }

    #[tokio::test]
    async fn test_list_api_keys() {
        let repo = setup_repo().await;

        // Create multiple API keys
        repo.create_api_key("key-1").await.unwrap();
        repo.create_api_key("key-2").await.unwrap();
        repo.create_api_key("key-3").await.unwrap();

        // List all keys
        let keys = repo.list_api_keys().await.unwrap();

        assert_eq!(keys.len(), 3);

        // Keys should be ordered by created_at DESC (most recent first)
        let names: Vec<&str> = keys.iter().map(|k| k.name.as_str()).collect();
        assert!(names.contains(&"key-1"));
        assert!(names.contains(&"key-2"));
        assert!(names.contains(&"key-3"));
    }

    #[tokio::test]
    async fn test_delete_api_key() {
        let repo = setup_repo().await;

        // Create an API key
        let (api_key, _raw_key) = repo.create_api_key("to-delete").await.unwrap();

        // Verify it exists
        let count_before = repo.count_api_keys().await.unwrap();
        assert_eq!(count_before, 1);

        // Delete the key
        let deleted = repo.delete_api_key(api_key.id).await.unwrap();
        assert!(deleted);

        // Verify it no longer exists in active keys
        let count_after = repo.count_api_keys().await.unwrap();
        assert_eq!(count_after, 0);

        // List should return empty
        let keys = repo.list_api_keys().await.unwrap();
        assert!(keys.is_empty());
    }

    #[tokio::test]
    async fn test_delete_api_key_not_found() {
        let repo = setup_repo().await;

        // Try to delete a non-existent key
        let fake_id = payments_types::ApiKeyId::new();
        let deleted = repo.delete_api_key(fake_id).await.unwrap();

        assert!(!deleted);
    }

    #[tokio::test]
    async fn test_delete_api_key_twice() {
        let repo = setup_repo().await;

        // Create an API key
        let (api_key, _raw_key) = repo.create_api_key("double-delete").await.unwrap();

        // First delete should succeed
        let deleted_first = repo.delete_api_key(api_key.id).await.unwrap();
        assert!(deleted_first);

        // Second delete should fail (key already inactive)
        let deleted_second = repo.delete_api_key(api_key.id).await.unwrap();
        assert!(!deleted_second);
    }
}
