#!/bin/sh
# Container entrypoint: optionally refresh the yt-dlp binary before starting
# the bot, then exec music-bot via "$@".
#
# Why this exists: YouTube keeps changing its internals, so a pinned yt-dlp
# goes stale within weeks and every fix used to be a manual Dockerfile edit.
# With YTDLP_AUTO_UPDATE=true (the default) the latest standalone yt-dlp is
# downloaded into the writable /data volume on container start. Its SHA-256 is
# checked against the release's SHA2-256SUMS BEFORE anything is executed — the
# binary is never run just to compare a version string. The pinned image binary
# remains the fallback: any failure is logged and the bot starts anyway.
#
# Set YTDLP_AUTO_UPDATE=false to keep the pinned binary forever.
# Applies on (re)start only — a long-running container does not update itself.

log() { echo "[entrypoint] $*"; }

PINNED_VERSION="2026.10.26" # keep in sync with ARG YTDLP_VERSION in the Dockerfile
PINNED_BIN="/usr/local/bin/yt-dlp"
UPDATE_DIR="/data/tmp/bin"
UPDATE_BIN="$UPDATE_DIR/yt-dlp"
SUMS_FILE="$UPDATE_DIR/SHA2-256SUMS"
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
elif [ -x "$UPDATE_BIN" ] && [ -f "$SUMS_FILE" ] \
    && [ "$("$UPDATE_BIN" --version 2>/dev/null)" = "$tag" ] \
    && [ "$(grep -E '[[:space:]]yt-dlp_linux$' "$SUMS_FILE" | cut -d' ' -f1)" = \
         "$(sha256sum "$UPDATE_BIN" | cut -d' ' -f1)" ]; then
    # Previously installed and hash-verified in an earlier container start.
    log "yt-dlp $tag already present and hash-verified in $UPDATE_DIR; using it"
    YTDLP_PATH="$UPDATE_BIN"; export YTDLP_PATH
else
    log "updating yt-dlp $PINNED_VERSION -> $tag"
    mkdir -p "$UPDATE_DIR" || log "cannot create $UPDATE_DIR; keeping pinned binary"
    release_url="https://github.com/yt-dlp/yt-dlp/releases/download/${tag}"
    if curl --fail --silent --show-error --retry 2 --max-time 300 -L \
        "${release_url}/yt-dlp_linux" -o "$UPDATE_BIN.new" \
        && curl --fail --silent --show-error --retry 2 --max-time 30 -L \
        "${release_url}/SHA2-256SUMS" -o "$SUMS_FILE.new"; then
        expected=$(grep -E '[[:space:]]yt-dlp_linux$' "$SUMS_FILE.new" | cut -d' ' -f1)
        actual=$(sha256sum "$UPDATE_BIN.new" | cut -d' ' -f1)
        if [ -n "$expected" ] && [ "$expected" = "$actual" ]; then
            chmod 755 "$UPDATE_BIN.new"
            # Version check is now only a sanity check, never the trust decision.
            if [ "$("$UPDATE_BIN.new" --version 2>/dev/null)" = "$tag" ]; then
                mv "$UPDATE_BIN.new" "$UPDATE_BIN"
                mv "$SUMS_FILE.new" "$SUMS_FILE"
                YTDLP_PATH="$UPDATE_BIN"; export YTDLP_PATH
                log "yt-dlp $tag installed at $UPDATE_BIN (sha256 verified)"
            else
                log "yt-dlp $tag version mismatch after hash check; using pinned binary"
            fi
        else
            log "yt-dlp $tag checksum mismatch; refusing to execute it; using pinned binary"
        fi
    else
        log "yt-dlp $tag download failed; using pinned binary"
    fi
    rm -f "$UPDATE_BIN.new" "$SUMS_FILE.new"
fi

log "starting $*"
exec "$@"
