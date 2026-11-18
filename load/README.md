# Load stand (ambitious sprint item 5)

A self-contained harness: **k6 → mock Bot API → 3 real bot replicas → Postgres**,
with Prometheus scraping the replicas and Alertmanager firing on regressions.

```
k6 (grafana/k6)  ──POST /inject──▶  mock-bot-api:8081
                                      │ getUpdates (one shared queue)
                                      ├──▶ bot1:9090 /metrics ─┐
                                      ├──▶ bot2:9090 /metrics ─┼─▶ prometheus:9090 ─▶ alertmanager
                                      └──▶ bot3:9090 /metrics ─┘
Postgres 17  ◀── all replicas share metadata, rate limits and selections
```

This is the **real `music-bot` binary** (same Dockerfile as production) against
a mock Telegram API and mock LRCLIB/lyrics.ovh. No Telegram token and no public
provider calls are involved. Because `getUpdates` pops from a single queue, an
injected update is handled by exactly one replica — the same property as real
long polling with several processes.

## Run

From the repository root (PowerShell):

```powershell
powershell -ExecutionPolicy Bypass -File scripts/run-load-test.ps1 -Rps 50 -Duration 2m
```

The script starts the stand, waits for the mock API, runs k6, then prints
**actual** Prometheus results for handler p95/p99, the error ratio and firing
alerts. If there are no samples it prints `NO SAMPLES` — it never synthesises a
number. Add `-Keep` to leave the containers up, then:

```powershell
powershell -ExecutionPolicy Bypass -File scripts/run-load-test.ps1 -Down
```

Manual variant:

```sh
docker compose -f load/docker-compose.load.yml up -d --build
docker compose -f load/docker-compose.load.yml run --rm -e RPS=50 -e DURATION=2m k6
# Prometheus UI: http://127.0.0.1:19090
```

## What k6 measures vs what Prometheus measures

- **k6** times `POST /inject` only (its `http_req_duration{name:inject}` p95).
  That is injector latency, *not* bot latency. Its threshold (`<500 ms`) only
  guards the harness itself.
- **Prometheus** scrapes `music_bot_request_seconds` (and download/upload
  histograms) from all three replicas. The p95/p99 printed by the script are
  real `histogram_quantile` results over the whole run.

`docs/perf.md` S1–S4 stay **NOT MEASURED** until these commands are run against
a staging deployment with real Telegram traffic. Local mock numbers from this
stand are labelled as such and must not be copied into the production table.

## Alerts (`load/prometheus/alerts.yml`)

| Alert | Condition |
|---|---|
| `MusicBotDown` | a replica stops exposing `/metrics` for 30 s |
| `MusicBotHighP95` | handler p95 > 1 s for 2 min |
| `MusicBotErrorRate` | failed downloads > 10% of attempts for 2 min |
| `MockApiQueueBacklog` | injector outpaces replicas (queue > 1000 for 1 min) |

Alertmanager sends webhooks to `POST /alert` on the mock API, which logs them
locally. Replace that receiver with Slack/PagerDuty for a real deployment.

## Scaling the stand

Edit the `bot1`/`bot2`/`bot3` services and the `music-bot` target list in
`load/prometheus/prometheus.yml` together. Each additional replica needs its own
`load-botN-data` volume and a distinct host port for direct scraping.
