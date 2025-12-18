-- Payments database schema

CREATE TABLE IF NOT EXISTS accounts (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    balance BIGINT NOT NULL DEFAULT 0,
    currency TEXT NOT NULL DEFAULT 'USD',
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS transactions (
    id TEXT PRIMARY KEY,
    direction TEXT NOT NULL,
    amount BIGINT NOT NULL,
    currency TEXT NOT NULL,
    source_account_id TEXT,
    destination_account_id TEXT,
    idempotency_key TEXT UNIQUE,
    reference TEXT,
    created_at TEXT NOT NULL
);
