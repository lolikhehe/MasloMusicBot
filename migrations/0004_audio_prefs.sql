-- Per-user output preferences for the ffmpeg post-processing step.
-- 'auto' keeps whatever yt-dlp produced (no extra transcode).

ALTER TABLE users ADD COLUMN audio_format TEXT NOT NULL DEFAULT 'auto';
ALTER TABLE users ADD COLUMN normalize_audio BIGINT NOT NULL DEFAULT 0;
