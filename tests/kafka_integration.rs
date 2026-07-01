//! Gated integration test against a real Kafka broker.
//!
//! Run manually when a broker is available:
//!   docker compose -f docker-compose.kafka.yml up -d
//!   cargo test -- --ignored kafka_roundtrip_enqueues_without_error
//!
//! The test is `#[ignore]` so `just check` / `cargo nextest run` skip it by
//! default; it does NOT run in CI unless the CI job explicitly passes
//! `--include-ignored`.

use std::time::Duration;

use countingsheep::KafkaProducer;
use countingsheep::config::KafkaConfig;
use countingsheep::{ProducedMessage, Producer};

#[tokio::test(flavor = "multi_thread")]
#[ignore = "requires a running Kafka broker (docker-compose.kafka.yml)"]
async fn kafka_roundtrip_enqueues_without_error() {
    let config = KafkaConfig::from_environment();
    let producer =
        KafkaProducer::from_config(&config).expect("broker configured via KAFKA_BROKERS");

    let msg = ProducedMessage {
        key: "integration-test-subject".into(),
        payload: br#"{"id":"it-1","type":"usage.created","source":"/test","subject":"integration-test-subject","time":0,"data":null}"#.to_vec(),
        specversion: "1.0".into(),
        received_at_unix: 0,
    };

    producer.produce(&msg).expect("enqueue must succeed");
    producer.flush(Duration::from_secs(10));
}
