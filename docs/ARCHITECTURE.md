# Architecture

countingsheep is a thin Axum foundation. The root crate does web *wiring*;
reusable, app-agnostic capabilities live in workspace crates under `crates/`.

## Crate layout

| Crate | Responsibility |
| --- | --- |
| `countingsheep` (root) | App state, router, middleware wiring, error type, config, binary |
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
  `countingsheep_env_vars`.
- **Observability:** `src/util/tracing.rs` configures the subscriber.

## Why "reference, not copy"

The structure mirrors crates.io's seams and crate split, but deliberately omits
its database, auth, sessions, rate-limiting, metrics, and Sentry — those are
added per-app.
