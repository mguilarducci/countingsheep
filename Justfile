# List available commands
_default:
    just --list

# Run the HTTP server
run:
    cargo run --bin server

# Run the full test suite
test:
    cargo nextest run --workspace --no-tests=pass

# Check formatting and lints (no changes)
lint:
    cargo fmt --check --all
    cargo clippy --all-targets --all-features --workspace -- -D warnings

# Auto-fix formatting and lints
fix:
    cargo fmt --all
    cargo clippy --fix --allow-dirty --all-targets --workspace

# Everything CI runs
check: lint test
