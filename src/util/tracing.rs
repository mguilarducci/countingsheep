//! `tracing` subscriber setup for the binary and for tests.

use countingsheep_env_vars::var;
use tracing_subscriber::filter::LevelFilter;
use tracing_subscriber::prelude::*;
use tracing_subscriber::{EnvFilter, fmt};

/// Initializes the global `tracing` subscriber.
///
/// Honors `RUST_LOG` (default `INFO`). Emits compact human-readable logs,
/// or JSON when `RUST_LOG_FORMAT=json`. Requires `.env` to already be loaded
/// (see [`countingsheep_env_vars::load`]) so `RUST_LOG` is visible to the
/// `EnvFilter`.
pub fn init() -> anyhow::Result<()> {
    let env_filter = EnvFilter::builder()
        .with_default_directive(LevelFilter::INFO.into())
        .from_env_lossy();

    let json = matches!(var("RUST_LOG_FORMAT")?.as_deref(), Some("json"));

    if json {
        tracing_subscriber::registry()
            .with(fmt::layer().json().with_filter(env_filter))
            .init();
    } else {
        tracing_subscriber::registry()
            .with(fmt::layer().compact().with_filter(env_filter))
            .init();
    }

    Ok(())
}

/// Initializes a test subscriber. Idempotent — safe to call from every test.
pub fn init_for_test() {
    let env_filter = EnvFilter::builder()
        .with_default_directive(LevelFilter::DEBUG.into())
        .from_env_lossy();

    let _ = fmt()
        .compact()
        .with_env_filter(env_filter)
        .with_test_writer()
        .try_init();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn init_for_test_is_idempotent() {
        init_for_test();
        init_for_test(); // second call must not panic
    }
}
