# Devcontainer CI-parity — end-to-end evidence

**Intent (THA-19):** a custom-Dockerfile devcontainer that achieves *full CI parity*,
so every CI check can be run locally inside the container before pushing.

## What was exercised

1. **Built the image** from `.devcontainer/Dockerfile` with the repo root as build
   context (as `devcontainer.json` specifies):
   `docker build -f .devcontainer/Dockerfile -t countingsheep-devcontainer:test .` → exit 0.
2. **Ran the container** with the repo mounted and `CARGO_TARGET_DIR=/tmp/target`
   (keeps the host worktree clean), then executed every command from
   `.github/workflows/ci.yml` plus the developer-facing `just check`.

## Toolchain parity (`01-toolchain-versions.txt`)

| Tool | In container | Required (rust-toolchain.toml / ci.yml) |
|------|-------------|------------------------------------------|
| rustc / cargo | 1.96.0 | channel `1.96.0` ✓ |
| rustfmt | 1.9.0-stable (1.96.0) | component ✓ |
| clippy | 0.1.96 | component ✓ |
| cargo-deny | 0.19.9 | `CARGO_DENY_VERSION=0.19.9` ✓ |
| cargo-machete | 0.9.2 | `CARGO_MACHETE_VERSION=0.9.2` ✓ |
| just / cargo-nextest | 1.54.0 / 0.9.138 | installed ✓ |

## CI checks, all green inside the container (`02-ci-parity-run.txt`)

Run with CI's lint env (`RUSTFLAGS=-D warnings`, `RUSTDOCFLAGS=-D warnings`):

- **Backend / Lint:** `cargo fmt --check --all`, `cargo clippy --all-targets
  --all-features --workspace`, `cargo doc --no-deps --document-private-items` — all clean.
- **Backend / Dependencies:** `cargo deny check` → `advisories ok, bans ok,
  licenses ok, sources ok`; `cargo machete` → no unused dependencies.
- **Backend / Test:** `cargo nextest run --all-features --workspace --no-tests=pass`
  → 46 tests run, 46 passed.
- **`just check`** (lint + test convenience target) → green.

Container shell exited 0 with `ALL CI CHECKS PASSED INSIDE DEVCONTAINER`,
demonstrating full CI parity end-to-end.
