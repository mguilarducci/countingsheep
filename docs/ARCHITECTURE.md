# Architecture

countingsheep is a thin Axum foundation. The root crate does web *wiring*;
reusable, app-agnostic capabilities live in workspace crates under `crates/`.

## Crate layout

| Crate | Responsibility |
| --- | --- |
| `countingsheep` (root) | App state, router, middleware wiring, error type, config, binary, usage-event ingestion (`src/ingest/`) |
| `countingsheep_env_vars` | `dotenvy`-backed env-var helpers |
| `countingsheep_test_utils` | In-process `TestApp` harness for integration tests |

## Dependency direction

`bin/server.rs` -> root lib -> `countingsheep_env_vars`. The config layer reads
through `countingsheep_env_vars`. `countingsheep_test_utils` depends *up* on the
root crate (a harness is coupled to what it tests) and is used as a
dev-dependency.

## Seams (where to extend)

- **Routes:** `src/router.rs`. This is a private module (`mod router`), unlike
  the `pub` `error`/`middleware`/`config` modules — add routes by editing the
  router builder here, not by composing it from outside the crate.
- **Global middleware:** `src/middleware.rs` (`apply_axum_middleware`). App-
  specific layers (auth, sessions, rate-limiting, metrics, CORS) go here.
  Trailing-slash normalization must wrap the router at the make-service level.
- **Errors:** add a variant to `AppError` in `src/error.rs`.
- **Config:** add fields to `Server` in `src/config/server.rs`, reading through
  `countingsheep_env_vars`. Exception: the `DEV_DOCKER`/`HEROKU` bind-exposure
  flags are read from the real process environment in `bin/server.rs` *before*
  `.env` is loaded, so a stray `.env` can never flip the bind address.
- **Observability:** `src/util/tracing.rs` configures the subscriber.
- **Ingestion:** `src/ingest/`. `sheep.rs` holds the `Sheep` type and a pure,
  IO-free `validate()` (CloudEvents v1.0.2; collects every failure at once);
  `handler.rs` holds the `POST /api/v1/sheeps` handler, which dispatches on
  `Content-Type`: `application/cloudevents+json` is a single event;
  `application/cloudevents-batch+json` is a JSON array of events. The handler
  owns its content-type gate and JSON parse, so errors keep our
  `{ "errors": [...] }` shape rather than Axum's. Batches are **all-or-nothing**:
  every event is validated and, only if all pass, every event is recorded — so a
  partial failure records nothing. Batch validation errors carry the offending
  event's `index`; single-event errors omit it. The batch size is capped by
  `MAX_BATCH_EVENTS` (config, default 1000), checked before validation; oversized
  or empty batches are rejected with `400`. `record_accepted()` is the single
  named seam for where an accepted event goes — today a structured `tracing`
  event; durable storage or a broker slots in there (THA-17).

## Why "reference, not copy"

The structure mirrors crates.io's seams and crate split, but deliberately omits
its database, auth, sessions, rate-limiting, metrics, and Sentry — those are
added per-app.
