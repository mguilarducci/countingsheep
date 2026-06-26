# THA-17 — Trustworthy event timestamps (`occurred_at` / `received_at`)

End-to-end demonstration against the **real `server` binary** (not a unit test).
The server was booted with structured JSON logging
(`RUST_LOG=info RUST_LOG_FORMAT=json PORT=8799 ./target/debug/server`) and four
CloudEvents were POSTed with `curl`. The `record_accepted` seam logs both
timestamps, so the JSON log lines are the operator-facing surface that proves
the behavior.

## Requests (curl → HTTP status)

| # | CloudEvents `time` sent | Result |
|---|---|---|
| 1 | `2026-06-26T12:00:00+02:00` (offset-bearing) | `HTTP 202` |
| 2 | *(omitted)* | `HTTP 202` |
| 3 | `2026-06-20T08:30:00.5Z` (UTC + fractional) | `HTTP 202` |
| 4 | `not-a-timestamp` (malformed) | `HTTP 400` → `{"errors":[{"detail":"time must be RFC 3339, got \"not-a-timestamp\""}]}` |

## Server log at the ingestion seam (`countingsheep::ingest::handler`)

```json
{"message":"sheep accepted","id":"evt-offset","occurred_at":"2026-06-26 10:00:00.0 +00:00:00","received_at":"2026-06-26 20:54:13.155611 +00:00:00"}
{"message":"sheep accepted","id":"evt-notime","occurred_at":"2026-06-26 20:54:13.162806 +00:00:00","received_at":"2026-06-26 20:54:13.162806 +00:00:00"}
{"message":"sheep accepted","id":"evt-utc","occurred_at":"2026-06-20 8:30:00.5 +00:00:00","received_at":"2026-06-26 20:54:13.172527 +00:00:00"}
```
(fields trimmed for readability; full lines also carry `source`, `event_type`, `target`, and the subscriber `timestamp`.)

## How each line proves the intent

- **`evt-offset`** — client `12:00:00+02:00` is **kept** and **normalized to UTC**:
  `occurred_at = 10:00:00 +00:00:00` (same instant, UTC offset). `received_at`
  is the independent server clock (`20:54:13…`), proving the two stamps are
  conceptually separate and `received_at` is taken from our clock, never the wire.
- **`evt-notime`** — no client time → `occurred_at` **defaults to now** and is
  byte-for-byte equal to `received_at` (`20:54:13.162806`).
- **`evt-utc`** — `08:30:00.5Z` is preserved verbatim (fractional second intact)
  as `occurred_at`, again with a distinct `received_at`.
- **`evt-bad`** — a present-but-malformed time is **rejected with HTTP 400**, never
  silently defaulted, so the un-forgeable / trustworthy guarantee holds at the edge.

## Automated tests backing this

`cargo nextest run --workspace --no-tests=pass` — 53 passed, including:
- `ingest::stamp::tests::{missing_time_defaults_occurred_at_to_now_equal_to_received_at, present_time_is_preserved_as_occurred_at_independent_of_received_at, future_occurred_at_is_kept_even_when_after_received_at}`
- `ingest::sheep::tests::{time_is_kept_as_an_offsetdatetime_normalized_to_utc, time_boundary_values_are_handled, time_with_wrong_json_type_is_rejected_not_defaulted}`
- `ingest::handler::tests::record_accepted_logs_both_stamps`
- `countingsheep::sheeps::accepts_sheep_with_offset_time` (full HTTP wire path)
