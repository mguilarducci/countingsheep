# countingsheep — agent guide

## Layout

- `src/` — root app crate (router, middleware, error, config, binary,
  usage-event ingestion in `src/ingest/`).
- `crates/countingsheep_env_vars` — env-var helpers.
- `crates/countingsheep_test_utils` — `TestApp` integration harness.
- `docs/ARCHITECTURE.md` — the architecture map and extension seams.

## Conventions

- Web framework: Axum 0.8 (edition 2024). Compose via `build_handler`.
- Errors: return `AppResult<T>`; render through `AppError` (`src/error.rs`).
- Config: read env vars through `countingsheep_env_vars`. Exception: bind-
  exposure flags (`DEV_DOCKER`/`HEROKU`) are read from the real process env in
  `bin/server.rs` before `.env` loads, so `.env` can't change the bind address.
- Logging: `tracing`; the subscriber is set up in `src/util/tracing.rs`.
- Tests: `cargo nextest`; integration tests use `TestApp`.

## Commands

- `just check` — fmt + clippy (`-D warnings`) + tests. Run before committing.

## Out of scope (added per-app)

Database, auth, sessions, rate-limiting, metrics, CORS.
