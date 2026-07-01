# Load & performance testing — THA-20

Harness for exercising the ingestion endpoint `POST /api/v1/sheeps` under load.
The full plan lives on Linear (THA-20); the review artifact was built in `.lavish/`.

## Layout

```
loadtest/
  run.sh              # build release server, start it, run a tier, tear down
  k6/
    smoke.js          # contract check — one request per case, gates on checks
    load.js           # steady arrival rate: single + batch workloads
    stress.js         # ramping arrival rate to the saturation knee
    spike.js          # idle -> burst -> idle, measures recovery
    soak.js           # moderate steady load over a long window (leaks/drift)
    lib/
      config.js       # BASE_URL + content types (env-driven)
      payload.js      # valid CloudEvents builders, varied ids
      checks.js       # shared params + the 202/empty-body check
  oha/
    baseline.sh       # oha reference blasts (single + batch)
```

## Quickstart

`run.sh` builds a `--release` server, waits for `/health`, runs the tier, and
kills the server on exit. Tiers: `smoke | load | stress | spike | soak | baseline`.

```sh
loadtest/run.sh smoke                    # contract check (fast, run before push)
loadtest/run.sh load                     # steady-rate load, single + batch
RUST_LOG=warn loadtest/run.sh load       # mute the log sink to isolate its cost
RATE=2000 DURATION=1m loadtest/run.sh load
PEAK=20000 loadtest/run.sh stress
loadtest/run.sh baseline                 # oha reference numbers (needs `cargo install oha`)
PORT=9001 loadtest/run.sh smoke
```

Run a script directly against an already-running server instead:

```sh
k6 run -e BASE_URL=http://127.0.0.1:8888 loadtest/k6/load.js
TARGET=http://127.0.0.1:8888 loadtest/oha/baseline.sh
```

Tooling: `brew install k6` (macOS) and `cargo install oha`.

## Tunables (env vars)

| Var          | Used by            | Default                   |
|--------------|--------------------|---------------------------|
| `BASE_URL`   | all k6 scripts     | `http://127.0.0.1:8888`   |
| `RATE`       | load, soak         | `1000` (load) / `500` (soak) req/s |
| `DURATION`   | load, soak         | `30s` / `30m`             |
| `BATCH_SIZE` | load, baseline     | `100`                     |
| `PEAK`       | stress             | `10000` req/s             |
| `SPIKE`      | spike              | `10000` req/s             |
| `BASE_RATE`  | spike              | `200` req/s (idle baseline) |
| `RUST_LOG`   | run.sh (server)    | `info` (try `warn`)       |
| `PORT`       | run.sh (server)    | `8888`                    |

## Confirmed decisions (THA-20)

- **Tooling:** k6 (scenario driver + pass/fail thresholds) + oha (quick baselines).
- **Scope:** all five tiers — smoke, load, stress, spike, soak.
- **Where it runs:** all local, against a `--release` build over loopback.
- **Thresholds:** establish a baseline first, then ratify gates at baseline + margin.
  The numbers in the k6 scripts are **proposals**, not final.

## First baseline observations (loopback, release, Apple Silicon laptop)

A short verification run (`RATE=3000 DURATION=8s`, `RUST_LOG=warn`):

- **Single event:** p95 ≈ 0.17 ms, p99 ≈ 0.52 ms, 0 errors — clears the proposed
  single-event gates with huge margin.
- **Batch-of-100 @ 3000/s (300k events/s):** p95 ≈ 88 ms and k6 *dropped*
  iterations it couldn't launch at rate — i.e. this box saturates the batch path
  below 300k events/s. The proposed `p(95)<25ms` batch gate was optimistic;
  ratify it (and the achievable batch rate) from a dedicated baseline.

Treat these as a smoke-of-the-harness, not the official baseline.

## Caveats — what these numbers are NOT (yet)

This harness runs **natively on the host**: `run.sh` builds and runs the
`server` binary directly, and k6/oha run as host processes against it over
loopback. The **server and generator are not containerized** — the only
container in the loop is the Kafka broker (`docker-compose.kafka.yml`, see
above), and there is no production image yet. Deliberate for now (fast to
iterate), but be honest about the gaps:

- **Not a containerized deploy.** Container CPU/memory limits, the Linux network
  stack, and the `0.0.0.0` bind path are all absent. Native numbers are an
  optimistic ceiling stacked on top of the loopback ceiling.
- **Generator and server share the same cores.** k6/oha compete with the 4
  server worker threads for the host CPUs, so results under high load conflate
  "server saturated" with "generator stole cores" — this is the
  `dropped_iterations` + inflated batch tail in the first load run.

**Planned improvement (later):** add a slim production `Dockerfile`, run the
server as a CPU-pinned container, and drive it with k6/oha from the host (or a
second container) — so the numbers reflect a containerized deploy and the
generator stops stealing the server's cores.

## Broker requirement

The server now publishes accepted events to Kafka. Load-test runs therefore
require a running broker. Start the local KRaft broker before any load-test
tier:

```sh
docker compose -f docker-compose.kafka.yml up -d
# … run your tier …
docker compose -f docker-compose.kafka.yml down
```

Set `KAFKA_BROKERS=localhost:9092` (or the relevant address) in your shell or
`.env` file. The broker is **not** started by `run.sh` — bring it up manually.

## Method reminders (the why is in the plan)

- Build with `cargo build --release` — debug numbers are meaningless. `run.sh` does this.
- Open / arrival-rate model, not looping VUs (the k6 scripts use `*-arrival-rate`
  executors so a slow server can't hide tail latency — coordinated omission).
- Run each load tier twice: `RUST_LOG=info` vs `RUST_LOG=warn`, to isolate the
  current log-only sink's cost (the accept "sink" is a `tracing::info!` line).
- Vary the CloudEvent `id` per request (the k6 payloads do); the service has no
  dedup yet, so this models real traffic without relying on that staying true.
- A 408 in results = the 30s request timeout (saturation); a 500 = a caught
  panic (a real bug). Watch for both in stress/spike.
