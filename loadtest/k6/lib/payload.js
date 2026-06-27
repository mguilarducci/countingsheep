// Builders for the CloudEvents v1.0.2 payloads the ingestion endpoint accepts.
//
// Ids are varied per call so generated traffic looks real. The service has no
// dedup yet, so identical ids would also be accepted — but varying them is the
// honest model and guards against a future dedup layer silently rejecting load.

import exec from 'k6/execution';

// A unique id without external deps: virtual-user id + per-VU iteration + a
// random suffix stays unique across VUs, iterations, and repeated runs.
export function uniqueId() {
  const vu = exec.vu.idInTest;
  const iter = exec.vu.iterationInInstance;
  const rand = Math.random().toString(36).slice(2, 10);
  return `sheep-${vu}-${iter}-${rand}`;
}

// The smallest valid single event, with a varied id.
export function singleEvent() {
  return JSON.stringify({
    id: uniqueId(),
    source: '/loadtest',
    type: 'usage.created',
    specversion: '1.0',
  });
}

// A JSON array of `n` valid events, each with its own varied id.
export function batchEvents(n) {
  const events = [];
  for (let i = 0; i < n; i++) {
    events.push({
      id: uniqueId(),
      source: '/loadtest',
      type: 'usage.created',
      specversion: '1.0',
    });
  }
  return JSON.stringify(events);
}
