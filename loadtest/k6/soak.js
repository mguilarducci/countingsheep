// Soak: moderate, steady load for a long window to surface leaks and drift —
// RSS growth, fd growth, latency creep. While this runs, watch the server
// process in another shell, e.g.:
//
//   while :; do ps -o rss= -p "$(pgrep -f target/release/server)"; sleep 10; done
//
//   k6 run loadtest/k6/soak.js
//   k6 run -e RATE=500 -e DURATION=45m loadtest/k6/soak.js
//
import http from 'k6/http';
import { SHEEPS_URL, SINGLE_CT } from './lib/config.js';
import { singleEvent } from './lib/payload.js';
import { checkAccepted } from './lib/checks.js';

const RATE = Number(__ENV.RATE || 500);
const DURATION = __ENV.DURATION || '30m';

export const options = {
  scenarios: {
    soak: {
      executor: 'constant-arrival-rate',
      rate: RATE,
      timeUnit: '1s',
      duration: DURATION,
      preAllocatedVUs: 50,
      maxVUs: 500,
    },
  },
  thresholds: {
    http_req_failed: ['rate<0.001'],
    // Latency should stay flat across the whole run; a rising p99 is the drift
    // signal soak exists to catch.
    http_req_duration: ['p(99)<25'],
  },
};

export default function () {
  const res = http.post(SHEEPS_URL, singleEvent(), { headers: { 'Content-Type': SINGLE_CT } });
  checkAccepted(res);
}
