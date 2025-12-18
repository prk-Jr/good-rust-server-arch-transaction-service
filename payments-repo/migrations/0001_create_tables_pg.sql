CREATE TABLE IF NOT EXISTS accounts (
    id UUID PRIMARY KEY,
    name TEXT NOT NULL,
    balance BIGINT NOT NULL DEFAULT 0,
    currency TEXT NOT NULL DEFAULT 'USD',
    created_at TIMESTAMPTZ NOT NULL
);
--SPLIT--
CREATE TABLE IF NOT EXISTS transactions (
    id UUID PRIMARY KEY,
    direction TEXT NOT NULL,
    amount BIGINT NOT NULL,
    currency TEXT NOT NULL,
    source_account_id UUID,
    destination_account_id UUID,
    idempotency_key TEXT UNIQUE,
    reference TEXT,
    created_at TIMESTAMPTZ NOT NULL
);
--SPLIT--
CREATE INDEX IF NOT EXISTS idx_transactions_source ON transactions(source_account_id);
--SPLIT--
CREATE INDEX IF NOT EXISTS idx_transactions_dest ON transactions(destination_account_id);
--SPLIT--
CREATE INDEX IF NOT EXISTS idx_transactions_idempotency ON transactions(idempotency_key);
