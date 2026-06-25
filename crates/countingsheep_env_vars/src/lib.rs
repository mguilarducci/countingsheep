//! Environment-variable helpers wrapping `dotenvy`.
//!
//! Unlike `std::env::var`, these load a `.env` file (via `dotenvy`) and treat
//! an unset variable as `Ok(None)` rather than an error.

use anyhow::{Context, anyhow};
use std::error::Error;
use std::str::FromStr;

/// Reads an environment variable, returning `Ok(None)` if it is unset.
#[track_caller]
pub fn var(key: &str) -> anyhow::Result<Option<String>> {
    match dotenvy::var(key) {
        Ok(content) => Ok(Some(content)),
        Err(dotenvy::Error::EnvVar(std::env::VarError::NotPresent)) => Ok(None),
        Err(error) => Err(error.into()),
    }
}

/// Reads an environment variable, failing if it is unset.
#[track_caller]
pub fn required_var(key: &str) -> anyhow::Result<String> {
    required(var(key), key)
}

/// Reads and parses an environment variable, returning `Ok(None)` if unset.
#[track_caller]
pub fn var_parsed<R>(key: &str) -> anyhow::Result<Option<R>>
where
    R: FromStr,
    R::Err: Error + Send + Sync + 'static,
{
    match var(key) {
        Ok(Some(content)) => Ok(Some(
            content
                .parse()
                .with_context(|| format!("Failed to parse {key} environment variable"))?,
        )),
        Ok(None) => Ok(None),
        Err(error) => Err(error),
    }
}

/// Reads and parses an environment variable, failing if it is unset.
#[track_caller]
pub fn required_var_parsed<R>(key: &str) -> anyhow::Result<R>
where
    R: FromStr,
    R::Err: Error + Send + Sync + 'static,
{
    required(var_parsed(key), key)
}

fn required<T>(res: anyhow::Result<Option<T>>, key: &str) -> anyhow::Result<T> {
    match res {
        Ok(Some(value)) => Ok(value),
        Ok(None) => Err(anyhow!("Failed to find required {key} environment variable")),
        Err(error) => Err(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const UNSET: &str = "COUNTINGSHEEP_DEFINITELY_UNSET_VAR";

    #[test]
    fn unset_var_returns_none() {
        assert_eq!(var(UNSET).unwrap(), None);
    }

    #[test]
    fn unset_required_var_errors() {
        assert!(required_var(UNSET).is_err());
    }

    #[test]
    fn unset_var_parsed_returns_none() {
        let parsed: Option<u16> = var_parsed(UNSET).unwrap();
        assert_eq!(parsed, None);
    }
}
