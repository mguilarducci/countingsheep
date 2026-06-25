# countingsheep

A small Axum (Rust, edition 2024) web application built on a deliberately thin,
well-factored foundation. Use it as the base for a new service by copying the
repository, then adding your domain code.

## Architecture at a glance

- `build_handler(App) -> Router` (`src/lib.rs`) is the single public entry the
  binary uses — tests use the same seam.
- `apply_axum_middleware` (`src/middleware.rs`) is where global middleware is
  layered (trace, catch-panic, timeout, compression).
- `AppError` (`src/error.rs`) renders a uniform JSON error envelope:
  `{ "errors": [{ "detail": "..." }] }`.
- Workspace crates under `crates/` hold reusable, app-agnostic capabilities
  (`countingsheep_env_vars`, `countingsheep_test_utils`).

See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for the full map.

## Quickstart

```sh
cp .env.sample .env
just run      # start the server (http://127.0.0.1:8888)
just test     # run the test suite
just check    # fmt + clippy + tests (what CI runs)
```

Requires the toolchain pinned in `rust-toolchain.toml`, plus
[`just`](https://github.com/casey/just) and
[`cargo-nextest`](https://nexte.st/).
