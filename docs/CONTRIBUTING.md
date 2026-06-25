# Contributing

## Setup

1. Install the toolchain (`rustup` reads `rust-toolchain.toml`).
2. Install [`just`](https://github.com/casey/just) and
   [`cargo-nextest`](https://nexte.st/).
3. `cp .env.sample .env`.

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
- New public functions and types get doc comments.
- Commits follow Conventional Commits (`feat`, `fix`, `refactor`, ...).
