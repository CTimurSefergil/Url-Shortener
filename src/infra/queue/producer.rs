use lapin::{BasicProperties, Channel, options::BasicPublishOptions};
use serde::{Deserialize, Serialize};

use crate::errors::AppError;

const EXCHANGE: &str = "";
const QUEUE: &str = "url_sync";

/// Message published when a new URL is created in PostgreSQL.
/// Consumer writes it to Cassandra.
#[derive(Debug, Serialize, Deserialize)]
pub struct UrlSyncMessage {
    pub short_code: String,
    pub original_url: String,
    pub created_at_ms: i64,
    pub expires_at_ms: i64,
    pub ttl_secs: Option<i32>,
}

pub struct QueueProducer {
    channel: Channel,
}

impl QueueProducer {
    pub fn new(channel: Channel) -> Self {
        Self { channel }
    }

    /// Declare the queue (idempotent).
    pub async fn declare_queue(&self) -> Result<(), AppError> {
        self.channel
            .queue_declare(
                QUEUE.into(),
                lapin::options::QueueDeclareOptions::default(),
                lapin::types::FieldTable::default(),
            )
            .await
            .map_err(|e| AppError::Internal(format!("queue declare error: {e}")))?;
        Ok(())
    }

    /// Publish a URL sync message.
    pub async fn publish(&self, msg: &UrlSyncMessage) -> Result<(), AppError> {
        let payload = serde_json::to_vec(msg)
            .map_err(|e| AppError::Internal(format!("queue serialize error: {e}")))?;

        self.channel
            .basic_publish(
                EXCHANGE.into(),
                QUEUE.into(),
                BasicPublishOptions::default(),
                &payload,
                BasicProperties::default()
                    .with_content_type("application/json".into())
                    .with_delivery_mode(2), // persistent
            )
            .await
            .map_err(|e| AppError::Internal(format!("queue publish error: {e}")))?
            .await
            .map_err(|e| AppError::Internal(format!("queue confirm error: {e}")))?;
        Ok(())
    }
}
