# Architecture

countingsheep is a thin Axum foundation. The root crate does web *wiring*;
reusable, app-agnostic capabilities live in workspace crates under `crates/`.

## Crate layout

| Crate | Responsibility |
| --- | --- |
| `countingsheep` (root) | App state, router, middleware wiring, error type, config, binary, usage-event ingestion (`src/ingest/`), Kafka publishing (`src/ingest/producer.rs`), error tracking (`src/observability/`) |
| `countingsheep_env_vars` | `dotenvy`-backed env-var helpers |
| `countingsheep_test_utils` | In-process `TestApp` harness for integration tests |

## Dependency direction

`bin/server.rs` -> root lib -> `countingsheep_env_vars`. The config layer reads
through `countingsheep_env_vars`. `countingsheep_test_utils` depends *up* on the
root crate (a harness is coupled to what it tests) and is used as a
dev-dependency.

## Seams (where to extend)

- **Routes:** `src/router.rs`. This is a private module (`mod router`), like the
  internal `error` and `middleware` modules — add routes by editing the router
  builder here, not by composing it from outside the crate.
- **Global middleware:** `src/middleware.rs` (`apply_axum_middleware`). App-
  specific layers (auth, sessions, rate-limiting, metrics, CORS) go here.
  Trailing-slash normalization must wrap the router at the make-service level.
- **Errors:** add a variant to `AppError` in `src/error.rs`.
- **Config:** add fields to `Server` in `src/config/server.rs`, reading through
  `countingsheep_env_vars`. Per-subsystem configs (`PostHogConfig`,
  `KafkaConfig`) live in sibling `src/config/*.rs` modules and are aggregated
  into `Server`. Exception: the `DEV_DOCKER`/`HEROKU` bind-exposure flags are
  read from the real process environment in `bin/server.rs` *before* `.env` is
  loaded, so a stray `.env` can never flip the bind address.
- **Observability:** `src/util/tracing.rs` configures the subscriber.
  `src/observability/error_tracking.rs` captures panics and 5xx (`AppError::
  Internal`) to PostHog as `$exception` events. It is safe by default: a no-op
  when `POSTHOG_API_KEY` is unset or `POSTHOG_ENABLED=false`, delivered
  fire-and-forget so it never blocks or fails a request, and always logged
  (enable/disable and failures) so logs stay the source of truth. The two
  capture seams are the `CatchPanicLayer` panic handler in `src/middleware.rs`
  (handled = false) and `AppError::into_response` in `src/error.rs`
  (handled = true); both reach a process-global reporter because their
  signatures cannot carry `AppState`.
- **Ingestion:** `src/ingest/`. `sheep.rs` holds the `Sheep` type and a pure,
  IO-free `validate()` (CloudEvents v1.0.2; collects every failure at once;
  keeps `time` as a UTC `OffsetDateTime`). `stamp.rs` holds the equally pure
  `stamp(sheep, now) -> AcceptedSheep`, adding the two guaranteed timestamps
  `occurred_at` (the client's `time`, defaulted to `now` when absent) and
  `received_at` (our clock). `handler.rs` holds the `POST /api/v1/sheeps`
  handler, which dispatches on `Content-Type`: `application/cloudevents+json` is
  a single event; `application/cloudevents-batch+json` is a JSON array of
  events. The handler owns its content-type gate and JSON parse, so errors keep
  our `{ "errors": [...] }` shape rather than Axum's. Batch *validation* is
  **all-or-nothing**: every event is validated and, only if all pass, are
  events recorded — so a validation failure records nothing. Publishing then
  runs per event, so a mid-batch producer failure (queue full → `503`) can
  leave earlier events already enqueued. Batch validation errors carry the
  offending event's `index`; single-event errors omit it. The
  batch size is capped by `MAX_BATCH_EVENTS` (config, default 1000), checked
  before validation; oversized or empty batches are rejected with `400`. The
  handler reads the clock exactly once per request (`OffsetDateTime::now_utc()`)
  so validation and stamping stay testable. `record_accepted()` is the
  ingestion terminus: it emits a structured `tracing` event carrying both
  timestamps and publishes the event to Kafka through the `Producer` seam (see
  **Publishing** below).
- **Publishing:** `src/ingest/producer.rs`. `record_accepted` serializes each
  accepted event with `serialize_flattened` — a flat CloudEvents JSON payload
  (`id`, `type`, `source`, `subject`, `time` as unix seconds, `data`) keyed by
  `subject`, with `specversion` and `received_at` carried as Kafka headers —
  and hands it to a `Producer`. The trait has two impls: `KafkaProducer`
  (librdkafka's async `FutureProducer`) for the binary, and `FakeProducer` (in
  `countingsheep_test_utils`) for tests. Delivery is non-blocking: `produce`
  only enqueues and returns, and the broker outcome is observed off the request
  path — so a full local queue surfaces as `503` (`AppError::ServiceUnavailable`,
  which adds a `Retry-After` header) and any other enqueue error as `500`. The
  producer is built in `bin/server.rs` from `KafkaConfig`; **Kafka is required**
  — an unset `KAFKA_BROKERS` fails startup — and the buffer is flushed on
  graceful shutdown. Configure it with `KAFKA_*` env vars (see `.env.sample`);
  the local dev broker is `docker-compose.kafka.yml`.


## Why "reference, not copy"

The structure mirrors crates.io's seams and crate split, but deliberately omits
its database, auth, sessions, rate-limiting, and metrics — those are added
per-app. Two capabilities are built in rather than left per-app: a thin,
safe-by-default PostHog error-tracking integration (crates.io's Sentry slot —
see the Observability seam) and Kafka publishing of accepted events (the
ingestion terminus, required at startup — see the Publishing seam).
