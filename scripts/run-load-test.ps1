# Full load stand: k6 -> mock Bot API -> 3 bot replicas -> Postgres + Prometheus.
#
#   powershell -ExecutionPolicy Bypass -File scripts/run-load-test.ps1 -Rps 50 -Duration 2m
#   powershell -ExecutionPolicy Bypass -File scripts/run-load-test.ps1 -Down
#
# This script NEVER invents benchmark numbers. It prints what Prometheus and k6
# actually report; if no samples exist, it says so explicitly.

param(
    [int]$Rps = 50,
    [string]$Duration = '2m',
    [switch]$Down,
    [switch]$Keep
)

$ErrorActionPreference = 'Stop'
$compose = @('-f', 'load/docker-compose.load.yml')

function Invoke-Compose {
    param([string[]]$Arguments)
    & docker compose @compose @Arguments
    if ($LASTEXITCODE -ne 0) { throw "docker compose $($Arguments -join ' ') failed ($LASTEXITCODE)" }
}

if ($Down) {
    Invoke-Compose @('down', '-v', '--remove-orphans')
    Write-Host 'Load stand removed.'
    exit 0
}

Write-Host "Starting the load stand (RPS=$Rps, duration=$Duration)..."
Invoke-Compose @('up', '-d', '--build')

# The mock API is the injector; without it k6 would fail fast.
Write-Host 'Waiting for the mock API health check...'
$deadline = (Get-Date).AddMinutes(5)
do {
    $healthy = $false
    try {
        $response = Invoke-WebRequest -Uri 'http://127.0.0.1:18081/healthz' -UseBasicParsing -TimeoutSec 2
        $healthy = ($response.StatusCode -eq 200)
    } catch {
        Start-Sleep -Seconds 2
    }
} until ($healthy -or (Get-Date) -gt $deadline)
if (-not $healthy) { throw 'mock API did not become healthy' }

Write-Host 'Running k6...'
Invoke-Compose @('run', '--rm', '-e', "RPS=$Rps", '-e', "DURATION=$Duration", 'k6')

function Query-Prometheus {
    param([string]$Expression)
    $url = "http://127.0.0.1:19090/api/v1/query?query=$([uri]::EscapeDataString($Expression))"
    try {
        $payload = Invoke-RestMethod -Uri $url -TimeoutSec 10
        if ($payload.status -ne 'success' -or $payload.data.result.Count -eq 0) {
            return 'NO SAMPLES'
        }
        return (($payload.data.result | ForEach-Object {
            '{0} = {1}' -f $_.metric.job, ([math]::Round([double]$_.value[1], 4))
        }) -join ', ')
    } catch {
        return "QUERY FAILED: $_"
    }
}

Write-Host "`nReal bot percentiles from Prometheus (NOT k6 injection latency):"
Write-Host ('  handler p95 (s) : ' + (Query-Prometheus 'histogram_quantile(0.95, sum by (le) (rate(music_bot_request_seconds_bucket[5m])))'))
Write-Host ('  handler p99 (s) : ' + (Query-Prometheus 'histogram_quantile(0.99, sum by (le) (rate(music_bot_request_seconds_bucket[5m])))'))
Write-Host ('  cache-hit p95(s): ' + (Query-Prometheus 'histogram_quantile(0.95, sum by (le) (rate(music_bot_request_seconds_bucket{job="music-bot"}[5m])))'))
Write-Host ('  errors / downloads: ' + (Query-Prometheus 'sum(rate(music_bot_download_errors_total[5m])) / clamp_min(sum(rate(music_bot_downloads_total[5m])), 1)'))
Write-Host '  injected (mock): ' + (Query-Prometheus 'mock_injected_total')
Write-Host '  alerts firing: ' + (Query-Prometheus 'sum(ALERTS{alertstate="firing"})')

Write-Host "`nRaw scrape: http://127.0.0.1:19090/graph"
Write-Host 'Raw k6 summary: target/k6-summary.json'

if (-not $Keep) {
    Write-Host 'Tearing down (use -Keep to leave the stand running)...'
    Invoke-Compose @('down', '-v', '--remove-orphans')
}
