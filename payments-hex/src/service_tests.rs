//! PaymentService unit tests.

#[cfg(test)]
pub(crate) mod tests {
    use std::collections::HashMap;
    use std::sync::Mutex;

    use async_trait::async_trait;

    use payments_types::{
        Account, AccountId, AppError, CreateAccountRequest, Currency, DepositRequest, DomainError,
        Money, RepoError, Transaction, TransactionId, TransactionRepository, TransferRequest,
        WithdrawRequest,
    };

    use crate::PaymentService;

    /// Simple in-memory repository for testing the service layer.
    pub struct MockRepo {
        accounts: Mutex<HashMap<AccountId, Account>>,
        transactions: Mutex<Vec<Transaction>>,
    }

    impl MockRepo {
        pub fn new() -> Self {
            Self {
                accounts: Mutex::new(HashMap::new()),
                transactions: Mutex::new(Vec::new()),
            }
        }
    }

    #[async_trait]
    impl TransactionRepository for MockRepo {
        async fn create_account(&self, req: CreateAccountRequest) -> Result<Account, RepoError> {
            let account = Account::new(req.name, req.currency).map_err(RepoError::Domain)?;
            self.accounts
                .lock()
                .unwrap()
                .insert(account.id, account.clone());
            Ok(account)
        }

        async fn get_account(&self, id: AccountId) -> Result<Option<Account>, RepoError> {
            Ok(self.accounts.lock().unwrap().get(&id).cloned())
        }

        async fn list_accounts(&self) -> Result<Vec<Account>, RepoError> {
            Ok(self.accounts.lock().unwrap().values().cloned().collect())
        }

        async fn deposit(&self, req: DepositRequest) -> Result<Transaction, RepoError> {
            let mut accounts = self.accounts.lock().unwrap();
            let account = accounts
                .get_mut(&req.account_id)
                .ok_or(RepoError::NotFound)?;
            let money = Money::new(req.amount, req.currency).map_err(RepoError::Domain)?;
            account.credit(money).map_err(RepoError::Domain)?;
            let tx =
                Transaction::deposit(req.account_id, money, req.idempotency_key, req.reference);
            self.transactions.lock().unwrap().push(tx.clone());
            Ok(tx)
        }

        async fn withdraw(&self, req: WithdrawRequest) -> Result<Transaction, RepoError> {
            let mut accounts = self.accounts.lock().unwrap();
            let account = accounts
                .get_mut(&req.account_id)
                .ok_or(RepoError::NotFound)?;
            let money = Money::new(req.amount, req.currency).map_err(RepoError::Domain)?;
            account.debit(money).map_err(RepoError::Domain)?;
            let tx =
                Transaction::withdrawal(req.account_id, money, req.idempotency_key, req.reference);
            self.transactions.lock().unwrap().push(tx.clone());
            Ok(tx)
        }

        async fn transfer(&self, req: TransferRequest) -> Result<Transaction, RepoError> {
            let mut accounts = self.accounts.lock().unwrap();
            let from = accounts
                .get(&req.from_account_id)
                .ok_or(RepoError::NotFound)?;
            let to = accounts
                .get(&req.to_account_id)
                .ok_or(RepoError::NotFound)?;

            if from.currency() != to.currency() {
                return Err(RepoError::Domain(DomainError::CrossCurrencyTransfer));
            }

            let money = Money::new(req.amount, req.currency).map_err(RepoError::Domain)?;

            let from = accounts.get_mut(&req.from_account_id).unwrap();
            from.debit(money).map_err(RepoError::Domain)?;

            let to = accounts.get_mut(&req.to_account_id).unwrap();
            to.credit(money).map_err(RepoError::Domain)?;

            let tx = Transaction::transfer(
                req.from_account_id,
                req.to_account_id,
                money,
                req.idempotency_key,
                req.reference,
            );
            self.transactions.lock().unwrap().push(tx.clone());
            Ok(tx)
        }

        async fn find_by_idempotency_key(
            &self,
            _key: &str,
        ) -> Result<Option<Transaction>, RepoError> {
            Ok(None)
        }

        async fn get_transaction(
            &self,
            id: TransactionId,
        ) -> Result<Option<Transaction>, RepoError> {
            Ok(self
                .transactions
                .lock()
                .unwrap()
                .iter()
                .find(|t| t.id == id)
                .cloned())
        }

        async fn list_transactions_for_account(
            &self,
            account_id: AccountId,
        ) -> Result<Vec<Transaction>, RepoError> {
            Ok(self
                .transactions
                .lock()
                .unwrap()
                .iter()
                .filter(|t| {
                    t.source_account_id == Some(account_id)
                        || t.destination_account_id == Some(account_id)
                })
                .cloned()
                .collect())
        }
    }

    #[tokio::test]
    async fn test_create_account_success() {
        let service = PaymentService::new(MockRepo::new());

        let req = CreateAccountRequest {
            name: "Test Account".to_string(),
            currency: Currency::USD,
        };

        let account = service.create_account(req).await.unwrap();

        assert_eq!(account.name, "Test Account");
        assert_eq!(account.balance.amount(), 0);
    }

    #[tokio::test]
    async fn test_create_account_empty_name_fails() {
        let service = PaymentService::new(MockRepo::new());

        let req = CreateAccountRequest {
            name: "   ".to_string(),
            currency: Currency::USD,
        };

        let result = service.create_account(req).await;

        assert!(matches!(result, Err(AppError::BadRequest(_))));
    }

    #[tokio::test]
    async fn test_deposit_success() {
        let service = PaymentService::new(MockRepo::new());

        let account = service
            .create_account(CreateAccountRequest {
                name: "Test".to_string(),
                currency: Currency::USD,
            })
            .await
            .unwrap();

        let tx = service
            .deposit(DepositRequest {
                account_id: account.id,
                amount: 1000,
                currency: Currency::USD,
                idempotency_key: None,
                reference: None,
            })
            .await
            .unwrap();

        assert_eq!(tx.amount.amount(), 1000);
    }

    #[tokio::test]
    async fn test_deposit_zero_amount_fails() {
        let service = PaymentService::new(MockRepo::new());

        let account = service
            .create_account(CreateAccountRequest {
                name: "Test".to_string(),
                currency: Currency::USD,
            })
            .await
            .unwrap();

        let result = service
            .deposit(DepositRequest {
                account_id: account.id,
                amount: 0,
                currency: Currency::USD,
                idempotency_key: None,
                reference: None,
            })
            .await;

        assert!(matches!(result, Err(AppError::BadRequest(_))));
    }

    #[tokio::test]
    async fn test_deposit_negative_amount_fails() {
        let service = PaymentService::new(MockRepo::new());

        let account = service
            .create_account(CreateAccountRequest {
                name: "Test".to_string(),
                currency: Currency::USD,
            })
            .await
            .unwrap();

        let result = service
            .deposit(DepositRequest {
                account_id: account.id,
                amount: -100,
                currency: Currency::USD,
                idempotency_key: None,
                reference: None,
            })
            .await;

        assert!(matches!(result, Err(AppError::BadRequest(_))));
    }

    #[tokio::test]
    async fn test_transfer_to_same_account_fails() {
        let service = PaymentService::new(MockRepo::new());

        let account = service
            .create_account(CreateAccountRequest {
                name: "Test".to_string(),
                currency: Currency::USD,
            })
            .await
            .unwrap();

        service
            .deposit(DepositRequest {
                account_id: account.id,
                amount: 1000,
                currency: Currency::USD,
                idempotency_key: None,
                reference: None,
            })
            .await
            .unwrap();

        let result = service
            .transfer(TransferRequest {
                from_account_id: account.id,
                to_account_id: account.id,
                amount: 100,
                currency: Currency::USD,
                idempotency_key: None,
                reference: None,
            })
            .await;

        assert!(matches!(result, Err(AppError::BadRequest(_))));
    }

    #[tokio::test]
    async fn test_get_account_not_found() {
        let service = PaymentService::new(MockRepo::new());

        let result = service.get_account(AccountId::new()).await;

        assert!(matches!(result, Err(AppError::NotFound(_))));
    }

    #[tokio::test]
    async fn test_list_transactions() {
        let service = PaymentService::new(MockRepo::new());

        let account = service
            .create_account(CreateAccountRequest {
                name: "Test".to_string(),
                currency: Currency::USD,
            })
            .await
            .unwrap();

        service
            .deposit(DepositRequest {
                account_id: account.id,
                amount: 1000,
                currency: Currency::USD,
                idempotency_key: None,
                reference: None,
            })
            .await
            .unwrap();

        let transactions = service.list_transactions(account.id).await.unwrap();

        assert_eq!(transactions.len(), 1);
    }
}
