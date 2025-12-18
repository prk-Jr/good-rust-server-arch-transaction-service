//! Configuration loading from environment.

use std::env;

/// Application configuration.
pub struct Config {
    pub port: u16,
    pub database_url: String,
}

impl Config {
    /// Loads configuration from environment variables.
    pub fn from_env() -> anyhow::Result<Self> {
        let port = env::var("PORT")
            .unwrap_or_else(|_| "3000".to_string())
            .parse()?;

        let database_url = env::var("DATABASE_URL")
            .map_err(|_| anyhow::anyhow!("DATABASE_URL environment variable is required"))?;

        Ok(Self { port, database_url })
    }
}
