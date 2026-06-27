# countingsheep — agent guide

## Layout

- `src/` — root app crate (router, middleware, error, config, binary,
  usage-event ingestion in `src/ingest/`, error tracking in
  `src/observability/`).
- `crates/countingsheep_env_vars` — env-var helpers.
- `crates/countingsheep_test_utils` — `TestApp` integration harness.
- `loadtest/` — local k6 + oha load-testing harness for the ingestion endpoint
  (`loadtest/run.sh <tier>`; see `loadtest/README.md`).
- `docs/ARCHITECTURE.md` — the architecture map and extension seams.

## Conventions

- Web framework: Axum 0.8 (edition 2024). Compose via `build_handler`.
- Errors: return `AppResult<T>`; render through `AppError` (`src/error.rs`).
- Config: read env vars through `countingsheep_env_vars`. Exception: bind-
  exposure flags (`DEV_DOCKER`/`HEROKU`) are read from the real process env in
  `bin/server.rs` before `.env` loads, so `.env` can't change the bind address.
- Logging: `tracing`; the subscriber is set up in `src/util/tracing.rs`.
- Error tracking: panics and 5xx are captured to PostHog from
  `src/observability/error_tracking.rs`. Safe by default (a no-op unless
  `POSTHOG_API_KEY` is set and `POSTHOG_ENABLED` is not `false`), fire-and-
  forget, and always logged — logs stay the source of truth.
- Tests: `cargo nextest`; integration tests use `TestApp`.

## Commands

- `just check` — fmt + clippy (`-D warnings`) + tests. Run before committing.

## Out of scope (added per-app)

Database, auth, sessions, rate-limiting, metrics, CORS.
