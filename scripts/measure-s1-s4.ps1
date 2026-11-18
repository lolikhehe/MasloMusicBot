# Capture the metrics needed to close S1-S4 from docs/perf.md.
# This script does NOT invent numbers: it stores raw Prometheus output with a
# timestamp so the operator can compute quantiles afterwards.

$ErrorActionPreference = 'Stop'
$outDir = 'D:\music-bot-measurements'
New-Item -ItemType Directory -Force -Path $outDir | Out-Null
$stamp = Get-Date -Format 'yyyyMMdd-HHmmss'
$file = Join-Path $outDir "metrics-$stamp.txt"

$metrics = & curl.exe -fsS http://127.0.0.1:9090/metrics
if ($LASTEXITCODE -ne 0) { throw "metrics endpoint unreachable" }

$metrics | Set-Content -Path $file -Encoding UTF8

$wanted = $metrics | Select-String -Pattern '^music_bot_(request_seconds|download_seconds|upload_seconds|inline_seconds|downloads_total|cache_hits_total|uploads_total|download_errors_total)'
$wanted | ForEach-Object { Write-Host $_.Line }

Write-Host "`nraw scrape saved to $file"
Write-Host "Next: compute histogram_quantile(0.95, rate(music_bot_request_seconds_bucket[5m]))" `
           "after at least 50 cached sends, then update docs/perf.md."
