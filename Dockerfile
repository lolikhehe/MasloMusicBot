# syntax=docker/dockerfile:1

# ---------- Build stage ----------
FROM rust:1-slim-bookworm AS builder

WORKDIR /build
RUN apt-get update \
 && apt-get install -y --no-install-recommends build-essential ca-certificates \
 && rm -rf /var/lib/apt/lists/*

COPY Cargo.toml Cargo.lock ./
COPY migrations ./migrations
COPY src ./src

RUN cargo build --release --locked

# ---------- Runtime stage ----------
FROM debian:bookworm-slim

RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates curl ffmpeg \
 && curl -fsSL "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp_linux" -o /usr/local/bin/yt-dlp \
 && chmod 755 /usr/local/bin/yt-dlp \
 && useradd --uid 10001 --create-home --shell /usr/sbin/nologin musicbot \
 && mkdir -p /data/tmp && chown -R musicbot:musicbot /data \
 && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /build/target/release/music-bot /usr/local/bin/music-bot

ENV DATABASE_URL=sqlite:///data/bot.db \
    TMP_DIR=/data/tmp \
    YTDLP_PATH=/usr/local/bin/yt-dlp \
    FFMPEG_PATH=/usr/bin/ffmpeg \
    METRICS_ADDR=0.0.0.0:9090 \
    LOG_LEVEL=info

VOLUME ["/data"]
EXPOSE 9090

USER musicbot
CMD ["music-bot"]
