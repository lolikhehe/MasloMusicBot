// k6 load scenario for MasloMusicBot.
//
// k6 -> mock Bot API (`POST /inject`) -> N bot replicas (long polling) -> Postgres.
// The percentile threshold in this script measures the INJECTION endpoint only.
// End-to-end handler percentiles are read from the bots' Prometheus `/metrics`
// after the run (see load/README.md and scripts/run-load-test.ps1). No number
// is hard-coded as a "result" here.

import http from 'k6/http';
import { check, sleep } from 'k6';
import { Counter } from 'k6/metrics';

const BASE_URL = __ENV.BASE_URL || 'http://127.0.0.1:18081';
const RPS = Number(__ENV.RPS || 50);
const DURATION = __ENV.DURATION || '2m';
const COMMANDS = ['/help', '/me', '/history 7d', '/lyrics Artist — Title'];

const accepted = new Counter('updates_accepted');

export const options = {
  scenarios: {
    inject: {
      executor: 'constant-arrival-rate',
      rate: RPS,
      timeUnit: '1s',
      duration: DURATION,
      preAllocatedVUs: 20,
      maxVUs: 200,
      gracefulStop: '10s',
    },
  },
  thresholds: {
    http_req_failed: ['rate<0.01'],
    // This is k6 -> mock latency, deliberately loose; the meaningful S1-style
    // p95 is exported by the bot and scraped by Prometheus.
    'http_req_duration{name:inject}': ['p(95)<500'],
    updates_accepted: ['count>0'],
  },
  summaryTrendStats: ['avg', 'min', 'med', 'p(90)', 'p(95)', 'p(99)', 'max'],
};

export default function () {
  const user_id = (__VU * 1000 + (__ITER % 500)) + 1;
  const text = COMMANDS[__ITER % COMMANDS.length];
  const payload = JSON.stringify({ user_id, text });
  const response = http.post(`${BASE_URL}/inject`, payload, {
    headers: { 'Content-Type': 'application/json' },
    tags: { name: 'inject' },
    timeout: '5s',
  });
  const ok = check(response, {
    'inject returns 200': (r) => r.status === 200,
    'inject acknowledged': (r) => {
      try {
        return r.json('ok') === true;
      } catch (_e) {
        return false;
      }
    },
  });
  if (ok) accepted.add(1);
  sleep(0.05);
}

export function handleSummary(data) {
  const out = process.env.SUMMARY_JSON || 'target/k6-summary.json';
  return {
    stdout: `\nk6 injection summary (NOT end-to-end): ${JSON.stringify(
      {
        accepted: data.metrics.updates_accepted?.values?.count ?? 0,
        inject_p95_ms:
          data.metrics['http_req_duration{name:inject}']?.values?.['p(95)'] ?? null,
        failed_rate: data.metrics.http_req_failed?.values?.rate ?? null,
      },
      null,
      2,
    )}\nRead real bot percentiles from Prometheus, see load/README.md.\n`,
    [out]: JSON.stringify(data, null, 2),
  };
}
