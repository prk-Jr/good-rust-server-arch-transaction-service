use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum WebhookStatus {
    #[default]
    Pending,
    Processing,
    Completed,
    Failed,
}

impl AsRef<str> for WebhookStatus {
    fn as_ref(&self) -> &str {
        match self {
            Self::Pending => "PENDING",
            Self::Processing => "PROCESSING",
            Self::Completed => "COMPLETED",
            Self::Failed => "FAILED",
        }
    }
}

impl std::fmt::Display for WebhookStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_ref())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookEvent {
    pub id: Uuid,
    pub endpoint_id: Uuid,
    pub event_type: String,
    pub payload: serde_json::Value,
    pub status: WebhookStatus,
    pub created_at: DateTime<Utc>,
    pub processed_at: Option<DateTime<Utc>>,
    pub attempts: i32,
    pub last_error: Option<String>,
}

impl WebhookEvent {
    pub fn new(
        endpoint_id: Uuid,
        event_type: impl Into<String>,
        payload: serde_json::Value,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            endpoint_id,
            event_type: event_type.into(),
            payload,
            status: WebhookStatus::Pending,
            created_at: Utc::now(),
            processed_at: None,
            attempts: 0,
            last_error: None,
        }
    }
}

/// A registered webhook endpoint for a business.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookEndpoint {
    pub id: Uuid,
    pub url: String,
    pub secret: String,
    pub events: Vec<String>, // Event types to subscribe to, e.g., ["transaction.created"]
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
}

/// Wrapper type for webhook endpoint ID.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct WebhookEndpointId(pub Uuid);

impl WebhookEndpointId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    pub fn from_uuid(id: Uuid) -> Self {
        Self(id)
    }
}

impl Default for WebhookEndpointId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for WebhookEndpointId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for WebhookEndpointId {
    type Err = uuid::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}
