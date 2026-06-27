# PostHog error-tracking — end-to-end evidence (THA-18)

Validates the feature against the author's stated constraints by running the **real
`server` binary** across every PostHog configuration state and exercising the live
HTTP surface. The unit/integration suite (40 focused tests) passes; the artifacts
below show the behavior an operator/end-user actually observes.

## 1. Logs are the source of truth + safe-by-default + env-var kill-switch

The same binary booted four times with different env vars. In **every** state the
service binds and serves `/health` with `200` — PostHog is purely additive and never
blocks startup or request handling. Each state logs *why* capture is on or off.

```
===== 1) Unconfigured (no API key) -> safe default: capture OFF =====
[INFO] error tracking disabled {'reason': 'POSTHOG_API_KEY not set'}
GET /health -> HTTP 200   (service serves traffic normally)

===== 2) Kill-switch: POSTHOG_ENABLED=false (key present) =====
[INFO] error tracking disabled {'reason': 'POSTHOG_ENABLED=false'}
GET /health -> HTTP 200   (service serves traffic normally)

===== 3) Malformed flag: POSTHOG_ENABLED=ture (THA-18 fix: must not crash) =====
[WARN] error tracking disabled {'reason': 'POSTHOG_ENABLED must be one of true/false/1/0/yes/no/on/off, got "ture"; treating error tracking as disabled'}
GET /health -> HTTP 200   (service serves traffic normally)

===== 4) Enabled: key present + POSTHOG_ENABLED=true =====
[INFO] error tracking enabled {'host': 'https://us.i.posthog.com'}
GET /health -> HTTP 200   (service serves traffic normally)
```

Mapping to the author's constraints:

| Constraint | State that demonstrates it |
|---|---|
| Safe by default (no key ⇒ off) | #1 — `disabled, reason=POSTHOG_API_KEY not set`, still `200` |
| API key from env var | #4 — `POSTHOG_API_KEY` present ⇒ `enabled` |
| Env-var kill-switch | #2 — `POSTHOG_ENABLED=false` disables even with a key |
| Malformed flag never breaks startup (the review commit `b12d82e`) | #3 — `WARN` logged, capture off, server still boots & serves |
| Always log when disabled/unavailable/erroring | #1–#4 — every state logs an explicit reason |

## 2. HTTP error contract that gates PostHog reporting

Live transcript against the running server (PostHog enabled). Only `Internal` (5xx)
and caught panics are reported to PostHog; 4xx are expected client errors and are
**not** reported. None of these 4xx requests produced an `internal server error` or
`Service panicked` log line.

```
$ curl -X GET /health
ok
-> HTTP 200

$ curl -X POST /api/v1/sheeps (-H Content-Type: text/plain)
{"errors":[{"detail":"Content-Type must be application/cloudevents+json or application/cloudevents-batch+json"}]}
-> HTTP 415

$ curl -X POST /api/v1/sheeps (-H Content-Type: application/cloudevents+json)  # body: "{not json"
{"errors":[{"detail":"body must be valid JSON"}]}
-> HTTP 400

$ curl -X GET /api/v1/sheeps
{"errors":[{"detail":"method not allowed"}]}
-> HTTP 405

$ curl -X GET /does-not-exist
{"errors":[{"detail":"not found"}]}
-> HTTP 404
```

## 3. Failure paths covered by tests (fire-and-forget never breaks requests)

The author emphasized TDD on failure paths, not just happy paths. Notable
failure-path tests that pass:

- `middleware::tests::panicking_route_is_caught_and_returns_500` — a panicking route,
  driven through the full middleware stack, is caught and returns `500` (the panic is
  reported as an unhandled exception, not propagated).
- `observability::error_tracking::tests::dispatch_isolates_a_panicking_sink` — a sink
  that itself panics is caught and the report dropped; it can never unwind into request
  handling.
- `config::posthog::tests::garbage_enabled_disables_capture_with_an_actionable_reason`
  and `parse_enabled_treats_garbage_as_invalid_without_failing` — a malformed
  `POSTHOG_ENABLED` disables capture (no third-party egress) without aborting startup.
- `config::posthog::tests::debug_redacts_the_api_key` — the secret key never reaches
  `Debug`/logs.
- `error::tests::only_internal_errors_are_reported_to_posthog` — only 5xx are reported.

Command: `cargo nextest run --workspace error_tracking config:: middleware:: error::`
→ 40 passed, 0 failed.
