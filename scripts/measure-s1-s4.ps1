# Capture the metrics needed to close S1-S4 from docs/perf.md.
# This script does NOT invent numbers: it stores raw Prometheus output with a
# timestamp so the operator can compute quantiles afterwards.

$ErrorActionPreference = 'Stop'
$outDir = 'D:\music-bot-measurements'
New-Item -ItemType Directory -Force -Path $outDir | Out-Null
$stamp = Get-Date -Format 'yyyyMMdd-HHmmss'
$file = Join-Path $outDir "metrics-$stamp.txt"

try {
    $response = Invoke-WebRequest -Uri 'http://127.0.0.1:9090/metrics' -UseBasicParsing
    $metrics = $response.Content -split "`n"
} catch {
    throw "metrics endpoint unreachable: $_"
}

$metrics | Set-Content -Path $file -Encoding UTF8
if (-not (Test-Path -LiteralPath $file)) { throw "could not write $file" }

$wanted = $metrics | Select-String -Pattern '^music_bot_(request_seconds|download_seconds|upload_seconds|inline_seconds|downloads_total|cache_hits_total|uploads_total|download_errors_total)'
if ($wanted) {
    $wanted | ForEach-Object { Write-Host $_.Line }
} else {
    Write-Host "No music_bot_* samples yet - the bot has not served a request since restart."
}

Write-Host "`nraw scrape saved to $file"
Write-Host "Next: compute histogram_quantile(0.95, rate(music_bot_request_seconds_bucket[5m]))" `
           "after at least 50 cached sends, then update docs/perf.md."
