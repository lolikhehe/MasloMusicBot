# Webhook gateway (Rust only)

A single small Rust binary does three jobs — **no nginx, Caddy, Python or Node**:

1. Gets and renews a Let's Encrypt certificate automatically (ACME TLS-ALPN-01).
2. Serves a tiny landing page at `/` and `/healthz`.
3. Verifies `X-Telegram-Bot-Api-Secret-Token` and proxies `/telegram/webhook`
   to the bot's own axum listener at `http://bot:8080`.

Source: `crates/gateway/src/main.rs`. Image: `Dockerfile.gateway`.

## 1. Get a domain + a small VM through GitHub Education

- GitHub Student Developer Pack → free domain from e.g. Namecheap (`.me`,
  often free for the first year).
- A tiny always-on VM with a public IPv4: Oracle Cloud Always Free, or the
  DigitalOcean/Vultr credit included in the pack.
- Open **TCP 80 and 443** in the VM's firewall / security list.

## 2. DNS

Create an `A` record:

```
music.example.com.   A   <VM public IP>
```

If you use Cloudflare, keep the record **DNS only** (grey cloud). ACME
TLS-ALPN-01 needs a direct TLS connection to port 443; Cloudflare's orange
proxy terminates TLS itself and will break certificate issuance.

## 3. Configure

On the VM, clone this repo and edit `.env`:

```env
WEBHOOK_DOMAIN=music.example.com
ACME_EMAIL=you@example.com
ACME_PROD=false
WEBHOOK_SECRET=<same 32-char token you will give the bot>
```

Keep `ACME_PROD=false` for the first run — Let's Encrypt staging has loose
rate limits while you debug DNS/firewall.

## 4. Start the gateway

```sh
docker compose --profile webhook up -d --build gateway
docker logs -f music-bot-gateway-1
```

Wait for `HTTPS listener up` and an ACME event with a certificate. Then:

```sh
curl -I https://music.example.com/
curl -I https://music.example.com/healthz
```

With staging certificates your browser will warn — that is expected.

## 5. Switch to a production certificate

```env
ACME_PROD=true
```

```sh
docker compose --profile webhook up -d --force-recreate gateway
```

Wait for a fresh certificate, then `curl -I` should show no TLS warning.
**Telegram will not accept a staging certificate** in `setWebhook`, so do this
before step 6.

## 6. Point the bot at the gateway

In the same `.env`, set exactly the public URL and the same secret:

```env
WEBHOOK_URL=https://music.example.com/telegram/webhook
WEBHOOK_SECRET=<same 32-char token>
```

Recreate the bot (keep the `local` profile because `.env` still contains
`TELEGRAM_API_URL` for the bundled Bot API server):

```sh
docker compose --profile local up -d --force-recreate bot
```

Check:

```sh
docker logs music-bot-bot-1 | Select-String 'webhook listener'
# INFO telegram::bot: starting webhook listener
```

Then send `/help` to the bot in Telegram. The gateway log should show a proxy
request, and the mock-free real Bot API will deliver the answer.

You can also query Telegram directly:

```sh
curl -s "https://api.telegram.org/bot<TOKEN>/getWebhookInfo"
```

Look for `url` = your URL, `has_custom_certificate: false`, and no
`last_error`.

## 7. Rollback to polling

Remove `WEBHOOK_URL` from `.env` and recreate the bot. The gateway can stay
running harmlessly, or stop it with `docker compose --profile webhook stop gateway`.

## Security notes

- The gateway checks the secret header in constant time before proxying, so
  unauthorized traffic never reaches the Docker network. The bot checks it
  again on its own listener.
- If `WEBHOOK_SECRET` is empty, the gateway logs a warning and accepts any
  request that knows the URL — don't do that in production.
- Certificates and the ACME account are stored in the `acme-cache` Docker
  volume on disk D — do not delete it, or Let's Encrypt rate limits bite.
- To rotate the secret, change it in both places and recreate both containers.

## Why not Cloudflare Tunnel?

A tunnel would remove the need for an open port 80/443 and DNS A record, but
then the "site" would be a Cloudflare service rather than our Rust binary.
This gateway keeps everything in Rust and works with any VM that has a public
IP. If you do use a tunnel, set `ACME_PROD=false` is irrelevant — terminate
TLS at the tunnel and proxy plain HTTP to the gateway on a custom port; that
is a different setup and not covered here.
