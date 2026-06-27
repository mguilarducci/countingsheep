//! Configuration for the HTTP server.

use std::net::{IpAddr, Ipv4Addr};

use anyhow::Context;
use countingsheep_env_vars::var;

use crate::config::PostHogConfig;

/// Default port to bind when `PORT` is unset.
const DEFAULT_PORT: u16 = 8888;

/// Default ceiling on events per batch when `MAX_BATCH_EVENTS` is unset.
const DEFAULT_MAX_BATCH_EVENTS: usize = 1000;

/// The API key inside `posthog` is redacted from this `Debug`, since
/// [`PostHogConfig`]'s own `Debug` impl does the redaction.
#[derive(Debug)]
pub struct Server {
    pub ip: IpAddr,
    pub port: u16,
    /// Maximum number of events accepted in a single batch submission.
    pub max_batch_events: usize,
    /// PostHog error-tracking configuration.
    pub posthog: PostHogConfig,
}

impl Server {
    /// Builds the server configuration from environment variables.
    ///
    /// `expose_externally` decides the bind address: `0.0.0.0` (all interfaces)
    /// when `true`, `127.0.0.1` (loopback only) otherwise. The caller reads it
    /// from the process environment *before* `.env` is loaded — exposing the
    /// server is a deployment signal and must never come from a stray `.env`
    /// file. The port is read from `PORT` (honoring `.env`), defaulting to
    /// `8888`.
    pub fn from_environment(expose_externally: bool) -> anyhow::Result<Self> {
        let ip = Self::bind_ip(expose_externally);

        let port = match var("PORT")? {
            Some(raw) => parse_port(&raw)?,
            None => DEFAULT_PORT,
        };

        let max_batch_events = match var("MAX_BATCH_EVENTS")? {
            Some(raw) => parse_max_batch_events(&raw)?,
            None => DEFAULT_MAX_BATCH_EVENTS,
        };

        let posthog = PostHogConfig::from_environment()?;

        Ok(Server {
            ip,
            port,
            max_batch_events,
            posthog,
        })
    }

    fn bind_ip(expose_externally: bool) -> IpAddr {
        if expose_externally {
            IpAddr::V4(Ipv4Addr::UNSPECIFIED)
        } else {
            IpAddr::V4(Ipv4Addr::LOCALHOST)
        }
    }
}

/// Parses the `PORT` value, with an error message that names the fix.
fn parse_port(raw: &str) -> anyhow::Result<u16> {
    raw.parse().context("PORT must be a valid port number")
}

/// Parses the `MAX_BATCH_EVENTS` value. A cap below `1` would reject every
/// batch (a batch must hold at least one event), so it is rejected as a
/// misconfiguration rather than silently disabling ingestion.
fn parse_max_batch_events(raw: &str) -> anyhow::Result<usize> {
    let value: usize = raw
        .parse()
        .context("MAX_BATCH_EVENTS must be a non-negative integer")?;
    if value < 1 {
        anyhow::bail!("MAX_BATCH_EVENTS must be at least 1");
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposed_binds_all_interfaces() {
        assert_eq!(Server::bind_ip(true), IpAddr::from(Ipv4Addr::UNSPECIFIED));
    }

    #[test]
    fn not_exposed_binds_loopback() {
        assert_eq!(Server::bind_ip(false), IpAddr::from(Ipv4Addr::LOCALHOST));
    }

    #[test]
    fn parse_port_accepts_valid() {
        assert_eq!(parse_port("8080").unwrap(), 8080);
    }

    #[test]
    fn parse_port_rejects_out_of_range_with_actionable_message() {
        let error = parse_port("70000").unwrap_err();
        assert!(
            error
                .to_string()
                .contains("PORT must be a valid port number")
        );
    }

    #[test]
    fn parse_port_rejects_non_numeric() {
        assert!(parse_port("eighty").is_err());
    }

    #[test]
    fn parse_max_batch_events_accepts_valid() {
        assert_eq!(parse_max_batch_events("500").unwrap(), 500);
    }

    #[test]
    fn parse_max_batch_events_rejects_zero_with_actionable_message() {
        let error = parse_max_batch_events("0").unwrap_err();
        assert!(
            error
                .to_string()
                .contains("MAX_BATCH_EVENTS must be at least 1")
        );
    }

    #[test]
    fn parse_max_batch_events_rejects_non_numeric() {
        assert!(parse_max_batch_events("lots").is_err());
    }

    #[test]
    fn default_max_batch_events_is_1000() {
        assert_eq!(DEFAULT_MAX_BATCH_EVENTS, 1000);
    }

    #[test]
    fn debug_delegates_to_the_redacting_posthog_debug() {
        let config = Server {
            ip: [127, 0, 0, 1].into(),
            port: 8888,
            max_batch_events: DEFAULT_MAX_BATCH_EVENTS,
            posthog: PostHogConfig::default(),
        };

        // The derived Debug must route the secret-bearing field through
        // `PostHogConfig`'s own redacting Debug (proven by its
        // `debug_redacts_the_api_key` test) rather than printing its fields
        // directly — that delegation is what keeps the API key out of logs.
        let rendered = format!("{config:?}");
        assert!(rendered.contains("PostHogConfig"));
        assert!(rendered.contains("max_batch_events: 1000"));
    }
}
