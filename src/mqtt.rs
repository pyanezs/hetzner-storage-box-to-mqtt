use crate::config::Mqtt as MqttConfig;
use crate::retry::Retryable;
use rumqttc::{AsyncClient, Event, MqttOptions, Packet, QoS};
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct Message {
    pub topic: String,
    pub payload: String,
    pub retain: bool,
}

#[derive(Debug)]
pub enum MqttError {
    Connection(rumqttc::ConnectionError),
    Client(rumqttc::ClientError),
    /// Publishes were sent but acks weren't all observed before the deadline.
    AckTimeout,
}

impl std::fmt::Display for MqttError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MqttError::Connection(e) => write!(f, "MQTT connection error: {e}"),
            MqttError::Client(e) => write!(f, "MQTT client error: {e}"),
            MqttError::AckTimeout => write!(f, "timed out waiting for MQTT publish acks"),
        }
    }
}

impl std::error::Error for MqttError {}

impl Retryable for MqttError {
    fn is_retryable(&self) -> bool {
        // Connect+publish+disconnect is retried as one unit; publishes are
        // idempotent (retained state/discovery values), so retrying the whole
        // block on any failure is safe.
        true
    }
}

/// Connects, publishes every message (QoS 1, retained), waits for all acks (or a
/// timeout), then disconnects. Intended to be driven through `retry::with_retry` as
/// a single unit — see module docs above.
pub async fn publish_all(cfg: &MqttConfig, messages: &[Message]) -> Result<(), MqttError> {
    if cfg.tls {
        tracing::warn!("mqtt.tls = true but TLS is not yet implemented — connecting in plaintext");
    }

    let mut options = MqttOptions::new(&cfg.client_id, &cfg.host, cfg.port);
    options.set_credentials(&cfg.username, &cfg.password);
    options.set_keep_alive(Duration::from_secs(30));

    let (client, mut eventloop) = AsyncClient::new(options, 10);

    for msg in messages {
        client
            .publish(
                &msg.topic,
                QoS::AtLeastOnce,
                msg.retain,
                msg.payload.clone(),
            )
            .await
            .map_err(MqttError::Client)?;
    }

    let expected_acks = messages.len();
    let mut acked = 0;
    let wait_for_acks = async {
        while acked < expected_acks {
            match eventloop.poll().await {
                Ok(Event::Incoming(Packet::PubAck(_))) => acked += 1,
                Ok(_) => continue,
                Err(e) => return Err(MqttError::Connection(e)),
            }
        }
        Ok(())
    };

    tokio::time::timeout(Duration::from_secs(15), wait_for_acks)
        .await
        .map_err(|_| MqttError::AckTimeout)??;

    client.disconnect().await.map_err(MqttError::Client)?;

    // Drain the disconnect from the eventloop so the connection closes cleanly.
    let _ = tokio::time::timeout(Duration::from_secs(5), eventloop.poll()).await;

    Ok(())
}
