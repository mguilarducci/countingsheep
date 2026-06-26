#!/usr/bin/env bash
# Local harness runner: builds a release server, starts it, waits for health,
# runs the chosen tier against it, then tears it down. "All local" per the
# THA-20 plan.
#
#   loadtest/run.sh smoke
#   loadtest/run.sh load
#   loadtest/run.sh baseline              # oha baseline blasts
#   RUST_LOG=warn loadtest/run.sh load    # mute the log sink to isolate its cost
#   PORT=9001 loadtest/run.sh stress
#
# Tiers: smoke | load | stress | spike | soak | baseline
set -euo pipefail

TIER="${1:-}"
if [[ -z "$TIER" ]]; then
  echo "usage: loadtest/run.sh <smoke|load|stress|spike|soak|baseline>" >&2
  exit 2
fi

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PORT="${PORT:-8888}"
export PORT
export RUST_LOG="${RUST_LOG:-info}"
BASE_URL="http://127.0.0.1:${PORT}"

echo "== building release server =="
cargo build --release --bin server --manifest-path "$ROOT/Cargo.toml"

echo "== starting server (PORT=$PORT RUST_LOG=$RUST_LOG) =="
"$ROOT/target/release/server" &
SERVER_PID=$!
cleanup() {
  kill "$SERVER_PID" 2>/dev/null || true
  wait "$SERVER_PID" 2>/dev/null || true
}
trap cleanup EXIT

# Wait for /health (up to ~10s).
for i in $(seq 1 50); do
  if curl -fsS "$BASE_URL/health" >/dev/null 2>&1; then break; fi
  if [[ $i -eq 50 ]]; then
    echo "server did not become healthy at $BASE_URL" >&2
    exit 1
  fi
  sleep 0.2
done
echo "server healthy at $BASE_URL"
echo

case "$TIER" in
  smoke | load | stress | spike | soak)
    if ! command -v k6 >/dev/null 2>&1; then
      echo "k6 not found. Install: brew install k6 (macOS) — https://grafana.com/docs/k6/latest/set-up/install-k6/" >&2
      exit 127
    fi
    k6 run -e BASE_URL="$BASE_URL" "$ROOT/loadtest/k6/${TIER}.js"
    ;;
  baseline)
    TARGET="$BASE_URL" "$ROOT/loadtest/oha/baseline.sh"
    ;;
  *)
    echo "unknown tier: $TIER (expected smoke|load|stress|spike|soak|baseline)" >&2
    exit 2
    ;;
esac
