// Shared request params and the accepted-response check used by every load tier.

import { check } from 'k6';
import { SINGLE_CT, BATCH_CT } from './config.js';

export const singleParams = { headers: { 'Content-Type': SINGLE_CT } };
export const batchParams = { headers: { 'Content-Type': BATCH_CT } };

// Assert a successful ingestion: 202 Accepted with an empty body. Returns the
// check result so callers can branch if they want.
export function checkAccepted(res) {
  return check(res, {
    'status is 202': (r) => r.status === 202,
    'body is empty': (r) => !r.body,
  });
}
