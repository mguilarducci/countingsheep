//! Environment-variable helpers wrapping `dotenvy`.
//!
//! Unlike `std::env::var`, these load a `.env` file (via `dotenvy`) and treat
//! an unset variable as `Ok(None)` rather than an error.

/// Loads variables from a `.env` file into the process environment.
///
/// Call this once, early in `main`, before anything reads `RUST_LOG` (the
/// tracing `EnvFilter` reads it straight from the process environment, so it
/// must be populated first). Existing process variables are never overridden,
/// so real environment variables always win over `.env`. A missing `.env` file
/// is not an error.
pub fn load() {
    dotenvy::dotenv().ok();
}

/// Reads an environment variable, returning `Ok(None)` if it is unset.
#[track_caller]
pub fn var(key: &str) -> anyhow::Result<Option<String>> {
    match dotenvy::var(key) {
        Ok(content) => Ok(Some(content)),
        Err(dotenvy::Error::EnvVar(std::env::VarError::NotPresent)) => Ok(None),
        Err(error) => Err(error.into()),
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
}
