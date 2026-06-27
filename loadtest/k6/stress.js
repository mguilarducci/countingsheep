// Stress: ramp the arrival rate well past comfort to find the saturation point
// and how the service degrades. Expect a throughput plateau near the 4-worker-
// thread ceiling (CORE_THREADS in bin/server.rs). Watch the output for the
// first 408s (request timeout) and any 500 (a caught panic — that's a bug).
//
//   k6 run loadtest/k6/stress.js
//   k6 run -e PEAK=20000 loadtest/k6/stress.js
//
import http from 'k6/http';
import { SHEEPS_URL, SINGLE_CT } from './lib/config.js';
import { singleEvent } from './lib/payload.js';
import { checkAccepted } from './lib/checks.js';

const PEAK = Number(__ENV.PEAK || 10000); // peak requests/sec to ramp toward

export const options = {
  scenarios: {
    stress: {
      executor: 'ramping-arrival-rate',
      startRate: 500,
      timeUnit: '1s',
      preAllocatedVUs: 100,
      maxVUs: 2000,
      stages: [
        { target: Math.round(PEAK * 0.25), duration: '30s' },
        { target: Math.round(PEAK * 0.5), duration: '30s' },
        { target: Math.round(PEAK * 0.75), duration: '30s' },
        { target: PEAK, duration: '30s' },
        { target: PEAK, duration: '30s' },
        { target: 0, duration: '15s' },
      ],
    },
  },
  thresholds: {
    // Stress is about finding the knee, not passing. Only hard-fail on a flood
    // of errors; slow responses and the odd 408 at the top are the data.
    http_req_failed: ['rate<0.05'],
  },
};

export default function () {
  const res = http.post(SHEEPS_URL, singleEvent(), { headers: { 'Content-Type': SINGLE_CT } });
  checkAccepted(res);
}
