# Contributing

## Setup

Open the repo in the [dev container](../.devcontainer) (any editor with Dev
Containers support) for a ready-made toolchain at full CI parity — the pinned
Rust toolchain plus `just`, `cargo-nextest`, `cargo-deny`, and `cargo-machete`.
Or set it up locally:

1. Install the toolchain (`rustup` reads `rust-toolchain.toml`).
2. Install [`just`](https://github.com/casey/just) and
   [`cargo-nextest`](https://nexte.st/).
3. `cp .env.sample .env`.
4. Start a local Kafka broker — the server requires one (`docker compose -f
   docker-compose.kafka.yml up -d`). Tests use a fake producer and need no
   broker; the gated round-trip in `tests/kafka_integration.rs` is the exception.

## Workflow

- `just run` — start the server.
- `just test` — run the suite (`cargo nextest`).
- `just lint` — formatting + clippy (warnings are errors).
- `just fix` — auto-fix formatting/clippy.
- `just check` — everything CI runs.

## Conventions

- Tests follow the TDD cycle (test -> red -> implement -> green -> commit).
- Integration tests use `countingsheep_test_utils::TestApp` and live in
  `tests/`.
- Load and performance tests live in `loadtest/` (a local k6 + oha harness for
  the ingestion endpoint); run `loadtest/run.sh smoke` before pushing. See
  [loadtest/README.md](../loadtest/README.md).
- New public functions and types get doc comments.
- Commits follow Conventional Commits (`feat`, `fix`, `refactor`, ...).
