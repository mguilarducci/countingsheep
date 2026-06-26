#!/usr/bin/env bash
# Baseline blasts with oha (Rust, single-binary). Establishes the reference
# numbers every k6 threshold gets ratified against. Run against an ALREADY
# RUNNING release server (use ../run.sh baseline, or start one yourself with
# `cargo run --release --bin server`).
#
#   loadtest/oha/baseline.sh
#   TARGET=http://127.0.0.1:8888 N=500000 C=100 BATCH_SIZE=100 loadtest/oha/baseline.sh
#
# oha sends a static body, so all events share one id. That is fine here: the
# service has no dedup, so identical events are all accepted. The k6 scripts
# cover varied-id realism.
set -euo pipefail

TARGET="${TARGET:-http://127.0.0.1:8888}"
N="${N:-200000}"
C="${C:-50}"
BATCH_SIZE="${BATCH_SIZE:-100}"
URL="${TARGET}/api/v1/sheeps"

if ! command -v oha >/dev/null 2>&1; then
  echo "oha not found. Install with: cargo install oha" >&2
  exit 127
fi

single_body='{"id":"baseline-1","source":"/loadtest","type":"usage.created","specversion":"1.0"}'

# Build a batch body of BATCH_SIZE identical valid events.
batch_body="$(python3 - "$BATCH_SIZE" <<'PY'
import json, sys
n = int(sys.argv[1])
event = {"id": "baseline-1", "source": "/loadtest", "type": "usage.created", "specversion": "1.0"}
print(json.dumps([event] * n))
PY
)"

echo "== oha baseline =="
echo "target:    $URL"
echo "requests:  $N   concurrency: $C   batch size: $BATCH_SIZE"
echo

echo "--- single event (application/cloudevents+json) ---"
oha --no-tui -n "$N" -c "$C" -m POST \
  -T 'application/cloudevents+json' \
  -d "$single_body" \
  "$URL"

echo
echo "--- batch of ${BATCH_SIZE} (application/cloudevents-batch+json) ---"
oha --no-tui -n "$N" -c "$C" -m POST \
  -T 'application/cloudevents-batch+json' \
  -d "$batch_body" \
  "$URL"
