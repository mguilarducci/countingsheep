// Load: sustained traffic at a fixed arrival rate. Open model — the request
// rate does not slow when the server does, so tail latency stays honest
// (avoids coordinated omission). Runs the single-event workload, then the
// batch workload, so they don't compete for the generator.
//
//   k6 run loadtest/k6/load.js
//   k6 run -e RATE=2000 -e DURATION=1m -e BATCH_SIZE=100 loadtest/k6/load.js
//
// Thresholds below are the PROPOSED gates from the plan — ratify them against a
// real baseline before trusting a pass/fail verdict.
import http from 'k6/http';
import { SHEEPS_URL, SINGLE_CT, BATCH_CT } from './lib/config.js';
import { singleEvent, batchEvents } from './lib/payload.js';
import { checkAccepted } from './lib/checks.js';

const RATE = Number(__ENV.RATE || 1000); // requests/sec per workload
const DURATION = __ENV.DURATION || '30s';
const BATCH_SIZE = Number(__ENV.BATCH_SIZE || 100);

export const options = {
  scenarios: {
    single: {
      executor: 'constant-arrival-rate',
      exec: 'single',
      rate: RATE,
      timeUnit: '1s',
      duration: DURATION,
      preAllocatedVUs: 50,
      maxVUs: 500,
    },
    batch: {
      executor: 'constant-arrival-rate',
      exec: 'batch',
      rate: RATE,
      timeUnit: '1s',
      duration: DURATION,
      preAllocatedVUs: 50,
      maxVUs: 500,
      startTime: DURATION, // start after the single workload finishes
    },
  },
  thresholds: {
    http_req_failed: ['rate<0.001'], // server-side errors are real failures
    dropped_iterations: ['count<1'], // a drop = load we never delivered (saturation), not just a slow request
    'http_req_duration{scenario:single}': ['p(95)<5', 'p(99)<15'],
    'http_req_duration{scenario:batch}': ['p(95)<25'],
  },
};

export function single() {
  const res = http.post(SHEEPS_URL, singleEvent(), { headers: { 'Content-Type': SINGLE_CT } });
  checkAccepted(res);
}

export function batch() {
  const res = http.post(SHEEPS_URL, batchEvents(BATCH_SIZE), { headers: { 'Content-Type': BATCH_CT } });
  checkAccepted(res);
}
