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
- Kafka publishing: accepted events are published from `src/ingest/producer.rs`
  via the `Producer` seam (`KafkaProducer` in prod, `FakeProducer` in tests).
  Configured with `KAFKA_*` env vars; **required** — an unset `KAFKA_BROKERS`
  fails startup. Delivery is non-blocking (a full local queue → `503`), and the
  buffer is flushed on shutdown. Local broker: `docker-compose.kafka.yml`.
- Tests: `cargo nextest`; integration tests use `TestApp`. A gated broker
  round-trip lives in `tests/kafka_integration.rs` (`#[ignore]`; run it with a
  live broker via `cargo test -- --ignored`).

## Commands

- `just check` — fmt + clippy (`-D warnings`) + tests. Run before committing.

## Out of scope (added per-app)

Database, auth, sessions, rate-limiting, metrics, CORS.
