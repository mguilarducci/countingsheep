//! Kafka publishing — the terminus of ingestion. A `Producer` ships an
//! already-serialized `ProducedMessage`; serialization (`serialize_flattened`,
//! added with the handler wiring) keeps the domain types crate-private.

use std::fmt;
use std::time::Duration;

use rdkafka::ClientConfig;
use rdkafka::error::KafkaError;
use rdkafka::message::{Header, OwnedHeaders};
use rdkafka::producer::{FutureProducer, FutureRecord, Producer as _};
use rdkafka::types::RDKafkaErrorCode;
use tracing::warn;

use crate::config::KafkaConfig;

/// One message ready for Kafka: partition key, serialized value, and the two
/// attributes carried as headers.
#[derive(Debug, Clone, PartialEq)]
pub struct ProducedMessage {
    pub key: String,
    pub payload: Vec<u8>,
    pub specversion: String,
    pub received_at_unix: i64,
}

/// Why a synchronous enqueue failed. Broker-side delivery failures are not
/// reported here — they resolve later, on the spawned delivery future.
#[derive(Debug)]
pub enum ProduceError {
    /// The local producer queue is full (overload back-pressure).
    QueueFull,
    /// Any other enqueue-time error from the client.
    Backend(String),
}

/// Emits an accepted sheep to Kafka.
pub trait Producer: Send + Sync + fmt::Debug {
    /// Enqueue one message. Non-blocking; `Err` means *local* back-pressure.
    fn produce(&self, message: &ProducedMessage) -> Result<(), ProduceError>;
    /// Drain buffered messages on shutdown (bounded by `timeout`).
    fn flush(&self, timeout: Duration);
}

/// A `Producer` backed by librdkafka's async `FutureProducer`.
pub struct KafkaProducer {
    client: FutureProducer,
    topic: String,
}

impl fmt::Debug for KafkaProducer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("KafkaProducer")
            .field("topic", &self.topic)
            .finish()
    }
}

impl KafkaProducer {
    /// Build the producer from config. Errors if no broker is configured (D6).
    pub fn from_config(config: &KafkaConfig) -> anyhow::Result<Self> {
        anyhow::ensure!(!config.brokers().is_empty(), "KAFKA_BROKERS must be set");

        let mut client_config = ClientConfig::new();
        client_config
            .set("bootstrap.servers", config.brokers())
            .set("client.id", config.client_id())
            .set("security.protocol", config.security_protocol())
            .set("enable.idempotence", "true")
            .set("acks", "all")
            .set("compression.type", "lz4")
            .set("message.timeout.ms", "30000");

        if let Some((mechanism, username, password)) = config.sasl() {
            client_config
                .set("sasl.mechanism", mechanism)
                .set("sasl.username", username)
                .set("sasl.password", password);
        }

        let client: FutureProducer = client_config.create()?;
        Ok(Self {
            client,
            topic: config.topic().to_string(),
        })
    }
}

impl Producer for KafkaProducer {
    fn produce(&self, message: &ProducedMessage) -> Result<(), ProduceError> {
        let received_at = message.received_at_unix.to_string();
        let headers = OwnedHeaders::new()
            .insert(Header {
                key: "specversion",
                value: Some(message.specversion.as_str()),
            })
            .insert(Header {
                key: "received_at",
                value: Some(received_at.as_str()),
            });

        let record = FutureRecord::to(&self.topic)
            .key(&message.key)
            .payload(&message.payload)
            .headers(headers);

        match self.client.send_result(record) {
            Ok(delivery_future) => {
                // Don't await: observe the broker outcome off the request path.
                tokio::spawn(async move {
                    match delivery_future.await {
                        Ok(Ok(_)) => {}
                        Ok(Err((err, _msg))) => warn!(%err, "kafka delivery failed"),
                        Err(_cancelled) => warn!("kafka delivery future cancelled"),
                    }
                });
                Ok(())
            }
            Err((KafkaError::MessageProduction(RDKafkaErrorCode::QueueFull), _record)) => {
                Err(ProduceError::QueueFull)
            }
            Err((err, _record)) => Err(ProduceError::Backend(err.to_string())),
        }
    }

    fn flush(&self, timeout: Duration) {
        let _ = self.client.flush(timeout);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_config_rejects_empty_brokers() {
        let config = KafkaConfig::default(); // empty brokers
        assert!(
            KafkaProducer::from_config(&config).is_err(),
            "an unconfigured broker must fail startup (D6)"
        );
    }

    #[test]
    fn from_config_builds_with_brokers() {
        let config = KafkaConfig::for_test("localhost:9092");
        assert!(KafkaProducer::from_config(&config).is_ok());
    }
}
