// Spike: jump from idle to a hard burst and back, to measure the error rate at
// onset and how quickly latency recovers — the "a customer flushed a backlog"
// case.
//
//   k6 run loadtest/k6/spike.js
//   k6 run -e SPIKE=15000 -e BASE_RATE=200 loadtest/k6/spike.js
//
import http from 'k6/http';
import { SHEEPS_URL, SINGLE_CT } from './lib/config.js';
import { singleEvent } from './lib/payload.js';
import { checkAccepted } from './lib/checks.js';

const SPIKE = Number(__ENV.SPIKE || 10000); // peak requests/sec during the burst
const BASE = Number(__ENV.BASE_RATE || 200); // idle baseline requests/sec

export const options = {
  scenarios: {
    spike: {
      executor: 'ramping-arrival-rate',
      startRate: BASE,
      timeUnit: '1s',
      preAllocatedVUs: 100,
      maxVUs: 2000,
      stages: [
        { target: BASE, duration: '20s' }, // idle baseline
        { target: SPIKE, duration: '5s' }, // abrupt ramp up
        { target: SPIKE, duration: '20s' }, // hold the spike
        { target: BASE, duration: '5s' }, // abrupt drop
        { target: BASE, duration: '30s' }, // observe recovery
      ],
    },
  },
  thresholds: {
    http_req_failed: ['rate<0.05'],
  },
};

export default function () {
  const res = http.post(SHEEPS_URL, singleEvent(), { headers: { 'Content-Type': SINGLE_CT } });
  checkAccepted(res);
}
