//! Kafka producer configuration, read from environment.

use std::fmt;

use countingsheep_env_vars::var;

const DEFAULT_TOPIC: &str = "sheeps";
const DEFAULT_CLIENT_ID: &str = "countingsheep";
const DEFAULT_SECURITY_PROTOCOL: &str = "plaintext";

/// Kafka producer settings. The SASL password is redacted from `Debug`.
#[derive(Clone)]
pub struct KafkaConfig {
    brokers: String,
    topic: String,
    client_id: String,
    security_protocol: String,
    sasl_mechanism: Option<String>,
    sasl_username: Option<String>,
    sasl_password: Option<String>,
}

impl Default for KafkaConfig {
    fn default() -> Self {
        Self {
            brokers: String::new(),
            topic: DEFAULT_TOPIC.to_string(),
            client_id: DEFAULT_CLIENT_ID.to_string(),
            security_protocol: DEFAULT_SECURITY_PROTOCOL.to_string(),
            sasl_mechanism: None,
            sasl_username: None,
            sasl_password: None,
        }
    }
}

impl fmt::Debug for KafkaConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("KafkaConfig")
            .field("brokers", &self.brokers)
            .field("topic", &self.topic)
            .field("client_id", &self.client_id)
            .field("security_protocol", &self.security_protocol)
            .field("sasl_mechanism", &self.sasl_mechanism)
            .field("sasl_username", &self.sasl_username)
            .field(
                "sasl_password",
                &self.sasl_password.as_ref().map(|_| "<redacted>"),
            )
            .finish()
    }
}

impl KafkaConfig {
    /// Reads `KAFKA_BROKERS`, `KAFKA_TOPIC`, `KAFKA_CLIENT_ID`,
    /// `KAFKA_SECURITY_PROTOCOL`, and `KAFKA_SASL_*` from the environment.
    /// Blank values fall back to defaults; an unset broker yields an empty
    /// string, which `KafkaProducer::from_config` rejects at startup.
    pub fn from_environment() -> Self {
        // `var()` returns `anyhow::Result<Option<String>>`; treat errors as
        // missing (the failure case is non-UTF-8 var content, which is
        // pathological — fall back gracefully rather than aborting startup).
        let non_blank =
            |v: anyhow::Result<Option<String>>| v.ok().flatten().filter(|s| !s.trim().is_empty());
        Self {
            brokers: non_blank(var("KAFKA_BROKERS")).unwrap_or_default(),
            topic: non_blank(var("KAFKA_TOPIC")).unwrap_or_else(|| DEFAULT_TOPIC.to_string()),
            client_id: non_blank(var("KAFKA_CLIENT_ID"))
                .unwrap_or_else(|| DEFAULT_CLIENT_ID.to_string()),
            security_protocol: non_blank(var("KAFKA_SECURITY_PROTOCOL"))
                .unwrap_or_else(|| DEFAULT_SECURITY_PROTOCOL.to_string()),
            sasl_mechanism: non_blank(var("KAFKA_SASL_MECHANISM")),
            sasl_username: non_blank(var("KAFKA_SASL_USERNAME")),
            sasl_password: non_blank(var("KAFKA_SASL_PASSWORD")),
        }
    }

    /// The bootstrap broker list.
    pub fn brokers(&self) -> &str {
        &self.brokers
    }

    /// The Kafka topic name.
    pub fn topic(&self) -> &str {
        &self.topic
    }

    /// The Kafka client identifier.
    pub fn client_id(&self) -> &str {
        &self.client_id
    }

    /// The security protocol (e.g. `plaintext`, `sasl_ssl`).
    pub fn security_protocol(&self) -> &str {
        &self.security_protocol
    }

    /// SASL credentials `(mechanism, username, password)`, present only when
    /// all three are configured.
    pub fn sasl(&self) -> Option<(&str, &str, &str)> {
        match (
            &self.sasl_mechanism,
            &self.sasl_username,
            &self.sasl_password,
        ) {
            (Some(m), Some(u), Some(p)) => Some((m, u, p)),
            _ => None,
        }
    }
}

#[cfg(test)]
impl KafkaConfig {
    pub(crate) fn for_test(brokers: &str) -> Self {
        Self {
            brokers: brokers.into(),
            ..Self::default()
        }
    }

    pub(crate) fn for_test_sasl(brokers: &str, security_protocol: &str) -> Self {
        Self {
            brokers: brokers.into(),
            security_protocol: security_protocol.into(),
            sasl_mechanism: Some("SCRAM-SHA-256".into()),
            sasl_username: Some("user".into()),
            sasl_password: Some("secret".into()),
            ..Self::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_has_empty_brokers_and_sane_names() {
        let c = KafkaConfig::default();
        assert!(c.brokers().is_empty());
        assert_eq!(c.topic(), "sheeps");
        assert_eq!(c.client_id(), "countingsheep");
    }

    #[test]
    fn debug_redacts_the_sasl_password() {
        let c = KafkaConfig {
            brokers: "localhost:9092".into(),
            topic: "sheeps".into(),
            client_id: "countingsheep".into(),
            security_protocol: "sasl_ssl".into(),
            sasl_mechanism: Some("SCRAM-SHA-256".into()),
            sasl_username: Some("user".into()),
            sasl_password: Some("supersecret".into()),
        };
        let rendered = format!("{c:?}");
        assert!(
            !rendered.contains("supersecret"),
            "password must be redacted"
        );
        assert!(rendered.contains("localhost:9092"), "non-secrets are shown");
    }
}
