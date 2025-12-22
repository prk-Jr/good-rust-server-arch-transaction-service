CREATE TABLE IF NOT EXISTS webhook_events (
    id UUID PRIMARY KEY,
    endpoint_id UUID NOT NULL,
    event_type TEXT NOT NULL,
    payload JSONB NOT NULL,
    status TEXT NOT NULL DEFAULT 'PENDING',
    created_at TIMESTAMPTZ NOT NULL,
    processed_at TIMESTAMPTZ,
    attempts INTEGER NOT NULL DEFAULT 0,
    last_error TEXT
);

CREATE INDEX IF NOT EXISTS idx_webhook_status ON webhook_events(status, created_at);

