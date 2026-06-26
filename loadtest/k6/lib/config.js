// Shared configuration, driven by environment variables so the same scripts run
// against loopback or any other target without edits.
//
//   BASE_URL   default http://127.0.0.1:8888
//
// Usage: k6 run -e BASE_URL=http://host:port loadtest/k6/smoke.js

export const BASE_URL = __ENV.BASE_URL || 'http://127.0.0.1:8888';
export const SHEEPS_URL = `${BASE_URL}/api/v1/sheeps`;
export const HEALTH_URL = `${BASE_URL}/health`;

// The two content types the ingestion endpoint accepts.
export const SINGLE_CT = 'application/cloudevents+json';
export const BATCH_CT = 'application/cloudevents-batch+json';
