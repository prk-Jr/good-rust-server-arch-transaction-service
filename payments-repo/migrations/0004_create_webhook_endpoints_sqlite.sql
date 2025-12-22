-- Webhook Endpoints Table (stores registered webhook URLs)
CREATE TABLE IF NOT EXISTS webhook_endpoints (
    id TEXT PRIMARY KEY,
    url TEXT NOT NULL,
    secret TEXT NOT NULL,
    events TEXT NOT NULL DEFAULT '[]',
    is_active INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_webhook_endpoints_active ON webhook_endpoints(is_active);
