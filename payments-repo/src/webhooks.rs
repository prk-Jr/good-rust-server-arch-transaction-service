use crate::Repo;
use crate::security::sign_webhook;
use payments_types::{WebhookEvent, WebhookStatus};
use std::time::Duration;
use tokio::time::sleep;
use tracing::{error, info, instrument};

/// Worker that processes pending webhook events and sends them to the target URL.
///
/// Webhooks are signed using HMAC-SHA256 for security. The signature is included
/// in the `X-Webhook-Signature` header.
pub struct WebhookWorker {
    repo: Repo,
    client: reqwest::Client,
    target_url: String,
    webhook_secret: String,
}

impl WebhookWorker {
    /// Creates a new webhook worker.
    ///
    /// # Arguments
    /// * `repo` - Repository for fetching and updating webhook events
    /// * `target_url` - URL to send webhooks to
    /// * `webhook_secret` - Secret key for HMAC-SHA256 signing
    pub fn new(repo: Repo, target_url: String, webhook_secret: String) -> Self {
        Self {
            repo,
            client: reqwest::Client::new(),
            target_url,
            webhook_secret,
        }
    }

    /// Runs the webhook worker loop.
    ///
    /// This method runs indefinitely, polling for pending webhooks every second
    /// and processing them.
    #[instrument(skip(self))]
    pub async fn run(self) {
        info!("Starting webhook worker sending to {}", self.target_url);
        loop {
            match self.repo.get_pending_webhooks(10).await {
                Ok(events) => {
                    if !events.is_empty() {
                        info!("Processing {} pending webhooks", events.len());
                        for event in events {
                            self.process_event(event).await;
                        }
                    }
                }
                Err(e) => {
                    error!("Failed to fetch webhooks: {}", e);
                }
            }
            sleep(Duration::from_secs(1)).await;
        }
    }

    /// Processes a single webhook event by sending it to the target URL.
    ///
    /// The payload is signed using HMAC-SHA256 and the signature is included
    /// in the `X-Webhook-Signature` header.
    #[instrument(skip(self, event), fields(event_id = %event.id))]
    async fn process_event(&self, event: WebhookEvent) {
        info!(
            "Sending webhook {} to {}",
            event.event_type, self.target_url
        );

        // Serialize the payload
        let payload_bytes = match serde_json::to_vec(&event.payload) {
            Ok(bytes) => bytes,
            Err(e) => {
                error!("Failed to serialize webhook payload: {}", e);
                if let Err(e) = self
                    .repo
                    .update_webhook_status(
                        event.id,
                        WebhookStatus::Failed,
                        Some(format!("Serialization error: {}", e)),
                    )
                    .await
                {
                    error!("Failed to update webhook status: {}", e);
                }
                return;
            }
        };

        // Sign the payload
        let signature = sign_webhook(&payload_bytes, &self.webhook_secret);

        // Send the webhook with signature header
        let result = self
            .client
            .post(&self.target_url)
            .header("Content-Type", "application/json")
            .header("X-Webhook-Signature", &signature)
            .header("X-Webhook-Event-Id", event.id.to_string())
            .header("X-Webhook-Event-Type", &event.event_type)
            .body(payload_bytes)
            .send()
            .await;

        let (status, last_error) = match result {
            Ok(resp) => {
                if resp.status().is_success() {
                    info!("Webhook delivered successfully");
                    (WebhookStatus::Completed, None)
                } else {
                    let status_code = resp.status();
                    error!("Webhook delivery failed with HTTP {}", status_code);
                    (WebhookStatus::Failed, Some(format!("HTTP {}", status_code)))
                }
            }
            Err(e) => {
                error!("Webhook delivery failed: {}", e);
                (WebhookStatus::Failed, Some(e.to_string()))
            }
        };

        if let Err(e) = self
            .repo
            .update_webhook_status(event.id, status, last_error)
            .await
        {
            error!("Failed to update webhook status: {}", e);
        }
    }
}
