//! PostHog error-tracking configuration, read from the environment.

use std::fmt;

use countingsheep_env_vars::var;

/// PostHog error-tracking configuration.
///
/// The API key is a secret: it is read only from the environment and is
/// redacted from [`fmt::Debug`] so it can never reach logs.
#[derive(Clone)]
pub struct PostHogConfig {
    api_key: Option<String>,
    enabled: bool,
    host: Option<String>,
}

/// The resolved settings for when error tracking should actually run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActivePostHog<'a> {
    /// PostHog project API key.
    pub api_key: &'a str,
    /// Ingestion host, or `None` to use the SDK default (US).
    pub host: Option<&'a str>,
}

/// Why error tracking is not running, for an unambiguous startup log.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisabledReason {
    /// `POSTHOG_ENABLED=false` — the explicit kill-switch.
    Killswitch,
    /// `POSTHOG_API_KEY` is unset or blank.
    NoApiKey,
}

impl fmt::Display for DisabledReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DisabledReason::Killswitch => f.write_str("POSTHOG_ENABLED=false"),
            DisabledReason::NoApiKey => f.write_str("POSTHOG_API_KEY not set"),
        }
    }
}

impl Default for PostHogConfig {
    /// A disabled-by-absence default: the kill-switch is on, but there is no
    /// key, so [`PostHogConfig::active`] is `None`. Used by tests and by any
    /// `Server` built without reading the environment.
    fn default() -> Self {
        Self {
            api_key: None,
            enabled: true,
            host: None,
        }
    }
}

impl fmt::Debug for PostHogConfig {
    /// Never renders the API key; only whether one is present.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PostHogConfig")
            .field("api_key", &self.api_key.as_ref().map(|_| "<redacted>"))
            .field("enabled", &self.enabled)
            .field("host", &self.host)
            .finish()
    }
}

impl PostHogConfig {
    /// Reads `POSTHOG_API_KEY`, `POSTHOG_ENABLED` (default `true`), and
    /// `POSTHOG_HOST` from the environment. Blank values are treated as unset.
    pub fn from_environment() -> anyhow::Result<Self> {
        let api_key = non_blank(var("POSTHOG_API_KEY")?);
        let enabled = match var("POSTHOG_ENABLED")? {
            Some(raw) => parse_enabled(&raw)?,
            None => true,
        };
        let host = non_blank(var("POSTHOG_HOST")?);

        Ok(Self {
            api_key,
            enabled,
            host,
        })
    }

    /// The resolved settings when capture should run: enabled *and* a key
    /// present. `None` means error tracking is off — see
    /// [`PostHogConfig::disabled_reason`] for why.
    pub fn active(&self) -> Option<ActivePostHog<'_>> {
        if !self.enabled {
            return None;
        }
        let api_key = self.api_key.as_deref()?;
        Some(ActivePostHog {
            api_key,
            host: self.host.as_deref(),
        })
    }

    /// Why capture is off, or `None` when it is active. The kill-switch takes
    /// precedence over a missing key so the log names the operator's own action.
    pub fn disabled_reason(&self) -> Option<DisabledReason> {
        if !self.enabled {
            return Some(DisabledReason::Killswitch);
        }
        if self.api_key.is_none() {
            return Some(DisabledReason::NoApiKey);
        }
        None
    }
}

/// Treats a blank / whitespace-only value as unset, without mutating a real one.
fn non_blank(value: Option<String>) -> Option<String> {
    value.filter(|v| !v.trim().is_empty())
}

/// Parses `POSTHOG_ENABLED`. Accepts common truthy/falsey spellings; anything
/// else is a misconfiguration with an actionable message rather than a silent
/// default.
fn parse_enabled(raw: &str) -> anyhow::Result<bool> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "true" | "1" | "yes" | "on" => Ok(true),
        "false" | "0" | "no" | "off" => Ok(false),
        other => anyhow::bail!(
            "POSTHOG_ENABLED must be one of true/false/1/0/yes/no/on/off, got {other:?}"
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inactive_without_a_key() {
        let config = PostHogConfig {
            api_key: None,
            enabled: true,
            host: None,
        };
        assert!(config.active().is_none());
        assert_eq!(config.disabled_reason(), Some(DisabledReason::NoApiKey));
    }

    #[test]
    fn active_when_key_present_and_enabled() {
        let config = PostHogConfig {
            api_key: Some("phc_key".to_string()),
            enabled: true,
            host: None,
        };
        let active = config.active().expect("should be active");
        assert_eq!(active.api_key, "phc_key");
        assert_eq!(active.host, None);
        assert_eq!(config.disabled_reason(), None);
    }

    #[test]
    fn killswitch_disables_even_with_a_key() {
        let config = PostHogConfig {
            api_key: Some("phc_key".to_string()),
            enabled: false,
            host: Some("https://eu.i.posthog.com".to_string()),
        };
        assert!(config.active().is_none());
        assert_eq!(config.disabled_reason(), Some(DisabledReason::Killswitch));
    }

    #[test]
    fn active_carries_the_host_when_set() {
        let config = PostHogConfig {
            api_key: Some("k".to_string()),
            enabled: true,
            host: Some("https://eu.i.posthog.com".to_string()),
        };
        assert_eq!(
            config.active().unwrap().host,
            Some("https://eu.i.posthog.com")
        );
    }

    #[test]
    fn parse_enabled_accepts_truthy_and_falsey_spellings() {
        for raw in ["true", "1", "YES", "On"] {
            assert!(parse_enabled(raw).unwrap(), "{raw} should be truthy");
        }
        for raw in ["false", "0", "no", "OFF"] {
            assert!(!parse_enabled(raw).unwrap(), "{raw} should be falsey");
        }
    }

    #[test]
    fn parse_enabled_rejects_garbage_with_an_actionable_message() {
        let error = parse_enabled("maybe").unwrap_err();
        assert!(
            error.to_string().contains("POSTHOG_ENABLED must be"),
            "message should name the fix, got: {error}"
        );
    }

    #[test]
    fn non_blank_treats_whitespace_as_unset() {
        assert_eq!(non_blank(Some("   ".to_string())), None);
        assert_eq!(non_blank(None), None);
        // A real value is returned untouched (presence check, not trimming).
        assert_eq!(non_blank(Some(" k ".to_string())), Some(" k ".to_string()));
    }

    #[test]
    fn debug_redacts_the_api_key() {
        let config = PostHogConfig {
            api_key: Some("phc_supersecret".to_string()),
            enabled: true,
            host: None,
        };
        let rendered = format!("{config:?}");
        assert!(
            !rendered.contains("supersecret"),
            "api key leaked into Debug: {rendered}"
        );
        assert!(rendered.contains("redacted"));
    }
}
