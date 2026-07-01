//! An in-memory `Producer` for tests: records messages, or fails on demand.

use std::sync::Mutex;
use std::time::Duration;

use countingsheep::{ProduceError, ProducedMessage, Producer};

/// Records produced messages; optionally fails every `produce` with QueueFull.
#[derive(Debug, Default)]
pub struct FakeProducer {
    produced: Mutex<Vec<ProducedMessage>>,
    fail_queue_full: bool,
}

impl FakeProducer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn failing_queue_full() -> Self {
        Self {
            produced: Mutex::new(Vec::new()),
            fail_queue_full: true,
        }
    }

    /// All messages recorded so far.
    pub fn produced(&self) -> Vec<ProducedMessage> {
        self.produced.lock().unwrap().clone()
    }
}

impl Producer for FakeProducer {
    fn produce(&self, message: &ProducedMessage) -> Result<(), ProduceError> {
        if self.fail_queue_full {
            return Err(ProduceError::QueueFull);
        }
        self.produced.lock().unwrap().push(message.clone());
        Ok(())
    }

    fn flush(&self, _timeout: Duration) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    fn message() -> ProducedMessage {
        ProducedMessage {
            key: "customer-1".into(),
            payload: b"{}".to_vec(),
            specversion: "1.0".into(),
            received_at_unix: 0,
        }
    }

    #[test]
    fn records_produced_messages() {
        let p = FakeProducer::new();
        p.produce(&message()).unwrap();
        assert_eq!(p.produced().len(), 1);
        assert_eq!(p.produced()[0].key, "customer-1");
    }

    #[test]
    fn failing_mode_returns_queue_full() {
        let p = FakeProducer::failing_queue_full();
        assert!(matches!(
            p.produce(&message()),
            Err(ProduceError::QueueFull)
        ));
        assert!(p.produced().is_empty());
    }
}
