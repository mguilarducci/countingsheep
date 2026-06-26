// Smoke: prove the ingestion contract holds before spending any load. One
// request per contract case, each asserting the exact status. Fast enough to
// run before every push. Correctness, not performance — any failed check fails
// the run.
//
//   k6 run loadtest/k6/smoke.js
//   k6 run -e BASE_URL=http://127.0.0.1:8888 loadtest/k6/smoke.js
//
import http from 'k6/http';
import { check, group } from 'k6';
import { SHEEPS_URL, HEALTH_URL, SINGLE_CT, BATCH_CT } from './lib/config.js';
import { singleEvent, batchEvents } from './lib/payload.js';

export const options = {
  vus: 1,
  iterations: 1,
  thresholds: {
    // Every check must pass, or the run exits non-zero.
    checks: ['rate==1.0'],
  },
};

const validBody = () =>
  JSON.stringify({ id: 'smoke-1', source: '/loadtest', type: 'usage.created', specversion: '1.0' });

export default function () {
  group('health is up', () => {
    const res = http.get(HEALTH_URL);
    check(res, {
      'health 200': (r) => r.status === 200,
      'health says ok': (r) => r.body === 'ok',
    });
  });

  group('valid single -> 202', () => {
    const res = http.post(SHEEPS_URL, singleEvent(), { headers: { 'Content-Type': SINGLE_CT } });
    check(res, { '202': (r) => r.status === 202, 'empty body': (r) => !r.body });
  });

  group('valid batch -> 202', () => {
    const res = http.post(SHEEPS_URL, batchEvents(3), { headers: { 'Content-Type': BATCH_CT } });
    check(res, { '202': (r) => r.status === 202 });
  });

  group('missing id -> 400', () => {
    const body = JSON.stringify({ source: '/loadtest', type: 'usage.created', specversion: '1.0' });
    const res = http.post(SHEEPS_URL, body, { headers: { 'Content-Type': SINGLE_CT } });
    check(res, {
      '400': (r) => r.status === 400,
      'detail is "id is required"': (r) => r.json('errors.0.detail') === 'id is required',
    });
  });

  group('wrong content-type -> 415', () => {
    const res = http.post(SHEEPS_URL, validBody(), { headers: { 'Content-Type': 'application/json' } });
    check(res, { '415': (r) => r.status === 415 });
  });

  group('wrong method -> 405', () => {
    const res = http.get(SHEEPS_URL);
    check(res, { '405': (r) => r.status === 405 });
  });

  group('unknown route -> 404', () => {
    const res = http.post(SHEEPS_URL.replace('/sheeps', '/sheep'), validBody(), {
      headers: { 'Content-Type': SINGLE_CT },
    });
    check(res, { '404': (r) => r.status === 404 });
  });
}
