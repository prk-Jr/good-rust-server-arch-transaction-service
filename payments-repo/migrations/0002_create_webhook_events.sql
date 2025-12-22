CREATE TABLE IF NOT EXISTS webhook_events (
    id TEXT PRIMARY KEY,
    endpoint_id TEXT,
    event_type TEXT NOT NULL,
    payload TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'PENDING',
    created_at TEXT NOT NULL,
    processed_at TEXT,
    attempts INTEGER NOT NULL DEFAULT 0,
    last_error TEXT
);
