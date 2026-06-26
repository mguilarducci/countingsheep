# syntax=docker/dockerfile:1
#
# Dev container image for countingsheep — full CI parity (see .github/workflows/ci.yml).
# Build context is the repo root (set via "context": ".." in devcontainer.json) so the
# COPY below can reach rust-toolchain.toml at the workspace root.
FROM mcr.microsoft.com/devcontainers/rust:1-bookworm

# Pin the toolchain to match rust-toolchain.toml and install its components.
# Copying the manifest first lets `rustup toolchain install` (no args) read the
# override file and download exactly the pinned channel, and `rustup component
# add` attaches rustfmt and clippy to *that* toolchain — so no repo-config edit
# is required.
COPY rust-toolchain.toml /tmp/rust-toolchain.toml
RUN cd /tmp \
    && rustup toolchain install \
    && rustup component add rustfmt clippy \
    && rm -f /tmp/rust-toolchain.toml

# Dev tooling — mirrors what CI installs. --locked keeps builds reproducible;
# cargo-deny / cargo-machete versions track the env block in ci.yml.
RUN cargo install --locked just cargo-nextest \
    && cargo install --locked cargo-deny@0.19.9 \
    && cargo install --locked cargo-machete@0.9.2
