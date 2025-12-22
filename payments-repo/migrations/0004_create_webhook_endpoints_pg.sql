-- Webhook Endpoints Table (stores registered webhook URLs)
CREATE TABLE IF NOT EXISTS webhook_endpoints (
    id UUID PRIMARY KEY,
    url TEXT NOT NULL,
    secret TEXT NOT NULL,
    events JSONB NOT NULL DEFAULT '[]',
    is_active BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_webhook_endpoints_active ON webhook_endpoints(is_active);
