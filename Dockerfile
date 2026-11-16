# syntax=docker/dockerfile:1

# ---------- Dependency planner (cargo-chef) ----------
FROM lukemathwalker/cargo-chef:latest-rust-1-slim-bookworm AS chef
WORKDIR /app

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# ---------- Build ----------
FROM chef AS builder
RUN apt-get update \
 && apt-get install -y --no-install-recommends build-essential ca-certificates \
 && rm -rf /var/lib/apt/lists/*
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --locked --recipe-path recipe.json
COPY . .
RUN cargo build --release --locked --package app --bin music-bot

# ---------- Runtime ----------
FROM debian:bookworm-slim

ARG YTDLP_VERSION=2026.10.26
ARG YTDLP_SHA256=767ba9dce7052aa196f8b12652f433d76de128ba168966fa84b6394d5e6c6795

RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates curl ffmpeg tini \
 && rm -rf /var/lib/apt/lists/* \
 && curl -fSL "https://github.com/yt-dlp/yt-dlp/releases/download/${YTDLP_VERSION}/yt-dlp_linux" \
      -o /usr/local/bin/yt-dlp \
 && echo "${YTDLP_SHA256}  /usr/local/bin/yt-dlp" | sha256sum -c - \
 && chmod 755 /usr/local/bin/yt-dlp \
 && useradd --system --uid 10001 --create-home --shell /usr/sbin/nologin bot

WORKDIR /app
COPY --from=builder /app/target/release/music-bot /usr/local/bin/music-bot
RUN mkdir -p /data && chown -R bot:bot /data

ENV DATABASE_URL=sqlite:///data/bot.db \
    TMP_DIR=/data/tmp \
    TMPDIR=/data/tmp \
    YTDLP_PATH=/usr/local/bin/yt-dlp \
    FFMPEG_PATH=/usr/bin/ffmpeg \
    METRICS_ADDR=0.0.0.0:9090 \
    LOG_LEVEL=info

VOLUME ["/data"]
EXPOSE 9090

USER bot
ENTRYPOINT ["/usr/bin/tini", "--", "music-bot"]
