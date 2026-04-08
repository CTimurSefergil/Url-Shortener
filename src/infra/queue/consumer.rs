use chrono::{TimeZone, Utc};
use lapin::{Channel, options::{BasicAckOptions, BasicConsumeOptions, BasicNackOptions, BasicQosOptions}};
use std::sync::Arc;

use crate::infra::db::{CassandraUrl, UrlReadRepository};

use super::producer::UrlSyncMessage;

const QUEUE: &str = "url_sync";
const CONSUMER_TAG: &str = "url_sync_consumer";

/// Spawns a background consumer that reads UrlSyncMessage from RabbitMQ
/// and writes them to Cassandra.
pub async fn spawn_consumer(
    channel: Channel,
    cassandra: Arc<dyn UrlReadRepository>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    channel
        .basic_qos(10, BasicQosOptions::default())
        .await?;

    let mut consumer = channel
        .basic_consume(
            QUEUE.into(),
            CONSUMER_TAG.into(),
            BasicConsumeOptions::default(),
            lapin::types::FieldTable::default(),
        )
        .await?;

    actix_web::rt::spawn(async move {
        use futures_util::StreamExt;
        while let Some(delivery) = consumer.next().await {
            match delivery {
                Ok(delivery) => {
                    if let Err(e) = handle_message(&delivery.data, &cassandra).await {
                        tracing::error!(error = %e, "failed to process url_sync message");
                        if let Err(nack_err) = delivery.nack(BasicNackOptions { multiple: false, requeue: true }).await {
                            tracing::error!(error = %nack_err, "failed to nack message");
                        }
                    } else if let Err(e) = delivery.ack(BasicAckOptions::default()).await {
                        tracing::error!(error = %e, "failed to ack message");
                    }
                }
                Err(e) => {
                    tracing::error!(error = %e, "consumer delivery error");
                }
            }
        }
        tracing::warn!("url_sync consumer stopped");
    });

    Ok(())
}

async fn handle_message(
    data: &[u8],
    cassandra: &Arc<dyn UrlReadRepository>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let msg: UrlSyncMessage = serde_json::from_slice(data)?;

    let cassandra_url = CassandraUrl {
        short_code: msg.short_code,
        original_url: msg.original_url,
        created_at: Utc.timestamp_millis_opt(msg.created_at_ms).single().unwrap_or_default(),
        expires_at: Utc.timestamp_millis_opt(msg.expires_at_ms).single().unwrap_or_default(),
    };

    cassandra.insert_url(&cassandra_url, msg.ttl_secs).await?;
    Ok(())
}
