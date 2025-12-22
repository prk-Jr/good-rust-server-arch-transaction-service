//! API Key domain type.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::AccountId;

/// Unique identifier for an API key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ApiKeyId(Uuid);

impl ApiKeyId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    pub fn as_uuid(&self) -> &Uuid {
        &self.0
    }
}

impl Default for ApiKeyId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for ApiKeyId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for ApiKeyId {
    type Err = uuid::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

/// An API key for authenticating requests.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiKey {
    pub id: ApiKeyId,
    pub name: String,
    pub key_hash: String,
    pub account_id: Option<AccountId>,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub last_used_at: Option<DateTime<Utc>>,
}

impl ApiKey {
    /// Creates a new API key with the given name and hash.
    pub fn new(name: String, key_hash: String, account_id: Option<AccountId>) -> Self {
        Self {
            id: ApiKeyId::new(),
            name,
            key_hash,
            account_id,
            is_active: true,
            created_at: Utc::now(),
            last_used_at: None,
        }
    }
}
