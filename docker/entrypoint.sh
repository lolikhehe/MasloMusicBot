#!/bin/sh
# Container entrypoint: optionally refresh the yt-dlp binary before starting
# the bot, then exec music-bot via "$@".
#
# Why this exists: YouTube keeps changing its internals, so a pinned yt-dlp
# goes stale within weeks and every fix used to be a manual Dockerfile edit.
# With YTDLP_AUTO_UPDATE=true (the default since the experimental branch) the
# latest standalone yt-dlp is downloaded into the writable /data volume on
# container start, verified (`--version` must print the expected tag), and
# the bot is pointed at it via YTDLP_PATH. The pinned system binary remains
# the fallback: any failure is logged and the bot starts anyway.
#
# Set YTDLP_AUTO_UPDATE=false to keep the pinned binary forever.
# Applies on (re)start only — a long-running container does not update itself.

log() { echo "[entrypoint] $*"; }

PINNED_VERSION="2026.10.26" # keep in sync with ARG YTDLP_VERSION in the Dockerfile
PINNED_BIN="/usr/local/bin/yt-dlp"
UPDATE_DIR="/data/tmp/bin"
UPDATE_BIN="$UPDATE_DIR/yt-dlp"
AUTO_UPDATE="${YTDLP_AUTO_UPDATE:-true}"

if [ "$AUTO_UPDATE" != "true" ]; then
    log "YTDLP_AUTO_UPDATE=$AUTO_UPDATE; using pinned yt-dlp $PINNED_VERSION"
    exec "$@"
fi

tag_json=$(curl --fail --silent --show-error --retry 2 --max-time 30 \
    https://api.github.com/repos/yt-dlp/yt-dlp/releases/latest 2>&1)
tag=$(printf '%s' "$tag_json" | grep -o '"tag_name": *"[^"]*"' | head -n 1 | cut -d'"' -f 4)

if [ -z "$tag" ]; then
    log "latest-release lookup failed; using pinned yt-dlp $PINNED_VERSION"
elif [ "$tag" = "$PINNED_VERSION" ]; then
    log "pinned yt-dlp $PINNED_VERSION is the latest release; using it"
elif [ -x "$UPDATE_BIN" ] && [ "$("$UPDATE_BIN" --version 2>/dev/null)" = "$tag" ]; then
    log "yt-dlp $tag already present in $UPDATE_DIR; using it"
    YTDLP_PATH="$UPDATE_BIN"; export YTDLP_PATH
else
    log "updating yt-dlp $PINNED_VERSION -> $tag"
    mkdir -p "$UPDATE_DIR" || log "cannot create $UPDATE_DIR; keeping pinned binary"
    if curl --fail --silent --show-error --retry 2 --max-time 300 -L \
        "https://github.com/yt-dlp/yt-dlp/releases/download/${tag}/yt-dlp_linux" \
        -o "$UPDATE_BIN.new" \
        && chmod 755 "$UPDATE_BIN.new" \
        && [ "$("$UPDATE_BIN.new" --version 2>/dev/null)" = "$tag" ]; then
        mv "$UPDATE_BIN.new" "$UPDATE_BIN"
        YTDLP_PATH="$UPDATE_BIN"; export YTDLP_PATH
        log "yt-dlp $tag installed at $UPDATE_BIN"
    else
        rm -f "$UPDATE_BIN.new"
        log "update failed or binary mismatch; using pinned yt-dlp $PINNED_VERSION"
    fi
fi

log "starting $*"
exec "$@"
