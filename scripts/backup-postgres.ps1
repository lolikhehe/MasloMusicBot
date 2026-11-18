# Dump the bot's Postgres database to D:\music-bot-backups and keep 7 days.
# Run manually or from Task Scheduler; no secrets are stored in this file.
# The password comes from the same env var docker-compose uses.

$ErrorActionPreference = 'Stop'
$password = if ($env:POSTGRES_PASSWORD) { $env:POSTGRES_PASSWORD } else { 'devpass' }
$stamp = Get-Date -Format 'yyyyMMdd-HHmmss'
$dir = 'D:\music-bot-backups'
New-Item -ItemType Directory -Force -Path $dir | Out-Null
$file = Join-Path $dir "musicbot-$stamp.sql"

& docker compose exec -T -e "PGPASSWORD=$password" postgres `
    pg_dump -U musicbot -d musicbot --no-owner --no-privileges `
    2>&1 | Set-Content -Path $file -Encoding UTF8
if ($LASTEXITCODE -ne 0) { throw "pg_dump failed" }

Compress-Archive -Path $file -DestinationPath "$file.zip" -Force
Remove-Item $file

Get-ChildItem $dir -Filter 'musicbot-*.sql.zip' |
    Where-Object LastWriteTime -lt (Get-Date).AddDays(-7) |
    Remove-Item -Force

Write-Host "backup written to $file.zip"
