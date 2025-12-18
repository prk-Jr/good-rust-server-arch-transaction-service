use crate::Repo;
use payments_types::{WebhookEvent, WebhookStatus};
use std::time::Duration;
use tokio::time::sleep;
use tracing::{error, info, instrument};

pub struct WebhookWorker {
    repo: Repo,
    client: reqwest::Client,
    target_url: String,
}

impl WebhookWorker {
    pub fn new(repo: Repo, target_url: String) -> Self {
        Self {
            repo,
            client: reqwest::Client::new(),
            target_url,
        }
    }

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

    #[instrument(skip(self, event), fields(event_id = %event.id))]
    async fn process_event(&self, event: WebhookEvent) {
        info!(
            "Sending webhook {} to {}",
            event.event_type, self.target_url
        );

        // TODO: specific payload formatting / signing
        let result = self
            .client
            .post(&self.target_url)
            .json(&event.payload)
            .send()
            .await;

        let (status, last_error) = match result {
            Ok(resp) => {
                if resp.status().is_success() {
                    (WebhookStatus::Completed, None)
                } else {
                    (
                        WebhookStatus::Failed,
                        Some(format!("HTTP {}", resp.status())),
                    )
                }
            }
            Err(e) => (WebhookStatus::Failed, Some(e.to_string())),
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
