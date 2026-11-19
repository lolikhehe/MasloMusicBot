# syntax=docker/dockerfile:1

# ---------- Dependency planner (cargo-chef) ----------
FROM lukemathwalker/cargo-chef:latest-rust-1-slim-bookworm AS chef
WORKDIR /app

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# ---------- Build ----------
FROM chef AS builder
# Compile-time SQL: the checked query metadata in .sqlx is committed, so the
# build never needs a live database or DATABASE_URL.
ENV SQLX_OFFLINE=true
RUN apt-get update \
 && apt-get install -y --no-install-recommends build-essential ca-certificates \
 && rm -rf /var/lib/apt/lists/*
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --locked --recipe-path recipe.json
COPY . .
RUN cargo build --release --locked --package app --bin music-bot --bin music-bot-worker

# ---------- Runtime ----------
FROM debian:bookworm-slim

ARG YTDLP_VERSION=2026.10.26
ARG YTDLP_SHA256=767ba9dce7052aa196f8b12652f433d76de128ba168966fa84b6394d5e6c6795
ARG DENO_VERSION=2.9.8
ARG DENO_SHA256=e890975bc2549807bbcc0b584ffafda72cc7c93349dd99c80483dd784f6c4609

RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates libchromaprint-tools curl ffmpeg tini unzip \
 && rm -rf /var/lib/apt/lists/* \
 && curl -fSL "https://github.com/yt-dlp/yt-dlp/releases/download/${YTDLP_VERSION}/yt-dlp_linux" \
      -o /usr/local/bin/yt-dlp \
 && echo "${YTDLP_SHA256}  /usr/local/bin/yt-dlp" | sha256sum -c - \
 && chmod 755 /usr/local/bin/yt-dlp \
 && curl -fSL "https://github.com/denoland/deno/releases/download/v${DENO_VERSION}/deno-x86_64-unknown-linux-gnu.zip" \
      -o /tmp/deno.zip \
 && echo "${DENO_SHA256}  /tmp/deno.zip" | sha256sum -c - \
 && unzip /tmp/deno.zip -d /usr/local/bin \
 && chmod 755 /usr/local/bin/deno \
 && rm /tmp/deno.zip \
 && useradd --system --uid 10001 --create-home --shell /usr/sbin/nologin bot

WORKDIR /app
COPY --from=builder /app/target/release/music-bot /usr/local/bin/music-bot
COPY --from=builder /app/target/release/music-bot-worker /usr/local/bin/music-bot-worker
COPY docker/entrypoint.sh /usr/local/bin/docker-entrypoint
RUN chmod 755 /usr/local/bin/docker-entrypoint \
 && mkdir -p /data && chown -R bot:bot /data

# Image default is intentionally NOT a working credential: compose/.env must
# supply DATABASE_URL with the real password.
ENV DATABASE_URL=postgresql://musicbot:CHANGE_ME@postgres:5432/musicbot \
    TMP_DIR=/data/tmp \
    TMPDIR=/data/tmp \
    # yt-dlp keeps a small cache; container root fs is read-only, keep it on the volume
    XDG_CACHE_HOME=/data/tmp/cache \
    YTDLP_PATH=/usr/local/bin/yt-dlp \
    FFMPEG_PATH=/usr/bin/ffmpeg \
    # refresh yt-dlp from the latest release on every container start;
    # falls back to the pinned binary when the network is unavailable
    YTDLP_AUTO_UPDATE=true \
    # deno's cache must live on the writable volume: the container root fs is read-only
    DENO_DIR=/data/tmp/deno \
    METRICS_ADDR=0.0.0.0:9090 \
    WEBHOOK_ADDR=0.0.0.0:8080 \
    LOG_LEVEL=info

VOLUME ["/data"]
EXPOSE 9090 8080

USER bot
ENTRYPOINT ["/usr/bin/tini", "--", "/usr/local/bin/docker-entrypoint", "music-bot"]
