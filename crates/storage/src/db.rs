//! SQLite persistence. Only metadata and Telegram file identifiers are stored;
//! audio bytes are never kept by the bot.

use common::domain::{CachedTrack, Lang, Provider, Stats, TrackKey, TrackMeta, UserRecord};
use common::error::{AppError, AppResult};
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
use sqlx::{Row, SqlitePool};
use std::path::Path;
use std::str::FromStr;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const SCHEMA: &str = include_str!("../../../migrations/0001_init.sql");

#[derive(Clone)]
pub struct Db {
    pool: SqlitePool,
}

impl Db {
    pub async fn connect(database_url: &str) -> AppResult<Self> {
        if let Some(path) = database_url.strip_prefix("sqlite://")
            && path != ":memory:"
            && let Some(parent) = Path::new(path).parent()
            && !parent.as_os_str().is_empty()
        {
            std::fs::create_dir_all(parent)?;
        }

        let options = SqliteConnectOptions::from_str(database_url)
            .map_err(|error| AppError::Config(format!("DATABASE_URL: {error}")))?
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal)
            .busy_timeout(Duration::from_secs(10))
            .foreign_keys(true);

        let pool = SqlitePoolOptions::new()
            .max_connections(8)
            .min_connections(1)
            .connect_with(options)
            .await
            .map_err(db_err)?;

        sqlx::raw_sql(SCHEMA).execute(&pool).await.map_err(db_err)?;

        Ok(Self { pool })
    }

    pub async fn get_track(&self, key: &TrackKey) -> AppResult<Option<CachedTrack>> {
        let row = sqlx::query(
            "SELECT track_key, file_id, file_unique_id, source_url, provider, title,
                    performer, album, duration_secs, thumbnail_url
             FROM tracks WHERE track_key = ?",
        )
        .bind(key.as_str())
        .fetch_optional(&self.pool)
        .await
        .map_err(db_err)?;

        Ok(row.map(|row| {
            let provider = row
                .get::<String, _>("provider")
                .parse::<Provider>()
                .unwrap_or(Provider::YouTube);
            let duration = row
                .get::<Option<i64>, _>("duration_secs")
                .and_then(|value| u32::try_from(value).ok());
            CachedTrack {
                key: TrackKey(row.get::<String, _>("track_key")),
                file_id: row.get("file_id"),
                file_unique_id: row.get("file_unique_id"),
                source_url: row.get("source_url"),
                provider,
                meta: TrackMeta {
                    title: row.get("title"),
                    performer: row.get("performer"),
                    album: row.get("album"),
                    duration_secs: duration,
                    thumbnail_url: row.get("thumbnail_url"),
                },
            }
        }))
    }

    pub async fn put_track(&self, track: &CachedTrack) -> AppResult<()> {
        let now = unix_now();
        sqlx::query(
            "INSERT INTO tracks
                (track_key, file_id, file_unique_id, source_url, provider, title, performer,
                 album, duration_secs, thumbnail_url, created_at, last_used_at, uses)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 1)
             ON CONFLICT(track_key) DO UPDATE SET
                file_id = excluded.file_id,
                file_unique_id = excluded.file_unique_id,
                source_url = excluded.source_url,
                provider = excluded.provider,
                title = excluded.title,
                performer = excluded.performer,
                album = excluded.album,
                duration_secs = excluded.duration_secs,
                thumbnail_url = excluded.thumbnail_url,
                last_used_at = excluded.last_used_at,
                uses = tracks.uses + 1",
        )
        .bind(track.key.as_str())
        .bind(&track.file_id)
        .bind(&track.file_unique_id)
        .bind(&track.source_url)
        .bind(track.provider.as_str())
        .bind(&track.meta.title)
        .bind(&track.meta.performer)
        .bind(&track.meta.album)
        .bind(track.meta.duration_secs.map(i64::from))
        .bind(&track.meta.thumbnail_url)
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(())
    }

    pub async fn touch_track(&self, key: &TrackKey) -> AppResult<()> {
        sqlx::query("UPDATE tracks SET last_used_at = ?, uses = uses + 1 WHERE track_key = ?")
            .bind(unix_now())
            .bind(key.as_str())
            .execute(&self.pool)
            .await
            .map_err(db_err)?;
        Ok(())
    }

    pub async fn purge_track(&self, key: &TrackKey) -> AppResult<bool> {
        let result = sqlx::query("DELETE FROM tracks WHERE track_key = ?")
            .bind(key.as_str())
            .execute(&self.pool)
            .await
            .map_err(db_err)?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn stats(&self) -> AppResult<Stats> {
        let tracks: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tracks")
            .fetch_one(&self.pool)
            .await
            .map_err(db_err)?;
        let users: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
            .fetch_one(&self.pool)
            .await
            .map_err(db_err)?;
        let sends: i64 = sqlx::query_scalar("SELECT COALESCE(SUM(uses), 0) FROM tracks")
            .fetch_one(&self.pool)
            .await
            .map_err(db_err)?;
        Ok(Stats {
            tracks,
            users,
            sends,
        })
    }

    pub async fn ensure_user(&self, id: i64, fallback: Lang) -> AppResult<UserRecord> {
        let now = unix_now();
        sqlx::query(
            "INSERT INTO users (user_id, language, quality, banned, created_at, last_seen_at)
             VALUES (?, ?, 192, 0, ?, ?)
             ON CONFLICT(user_id) DO UPDATE SET last_seen_at = excluded.last_seen_at",
        )
        .bind(id)
        .bind(fallback.code())
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(db_err)?;

        let row = sqlx::query("SELECT language, quality, banned FROM users WHERE user_id = ?")
            .bind(id)
            .fetch_one(&self.pool)
            .await
            .map_err(db_err)?;

        Ok(UserRecord {
            lang: Lang::from_code(row.get::<String, _>("language").as_str()).unwrap_or(Lang::En),
            quality: u32::try_from(row.get::<i64, _>("quality")).unwrap_or(192),
            banned: row.get::<i64, _>("banned") != 0,
        })
    }

    pub async fn set_language(&self, id: i64, lang: Lang) -> AppResult<()> {
        sqlx::query("UPDATE users SET language = ?, last_seen_at = ? WHERE user_id = ?")
            .bind(lang.code())
            .bind(unix_now())
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(db_err)?;
        Ok(())
    }

    pub async fn set_quality(&self, id: i64, quality: u32) -> AppResult<()> {
        let quality = quality.clamp(64, 320) as i64;
        sqlx::query("UPDATE users SET quality = ?, last_seen_at = ? WHERE user_id = ?")
            .bind(quality)
            .bind(unix_now())
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(db_err)?;
        Ok(())
    }

    pub async fn set_banned(&self, id: i64, banned: bool) -> AppResult<bool> {
        let result = sqlx::query("UPDATE users SET banned = ? WHERE user_id = ?")
            .bind(i64::from(banned))
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(db_err)?;
        Ok(result.rows_affected() > 0)
    }
}

fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

fn db_err(error: sqlx::Error) -> AppError {
    AppError::Other(error.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn test_db() -> Db {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("test.db");
        // Keep the directory alive for the lifetime of the database file only
        // in this test helper; SQLite keeps the handle open.
        let db = Db::connect(&format!("sqlite://{}", path.display()))
            .await
            .expect("db");
        std::mem::forget(dir);
        db
    }

    #[tokio::test]
    async fn stores_tracks_and_users() {
        let db = test_db().await;
        let key = TrackKey::new("youtube:abc");
        let track = CachedTrack {
            key: key.clone(),
            file_id: "FILE".into(),
            file_unique_id: "UNIQUE".into(),
            source_url: Some("https://youtube.com/watch?v=abc".into()),
            provider: Provider::YouTube,
            meta: TrackMeta {
                title: "Song".into(),
                performer: Some("Artist".into()),
                duration_secs: Some(123),
                ..Default::default()
            },
        };
        db.put_track(&track).await.expect("put");
        let cached = db.get_track(&key).await.expect("get").expect("some");
        assert_eq!(cached.file_id, "FILE");
        assert_eq!(cached.meta.performer.as_deref(), Some("Artist"));

        let user = db.ensure_user(42, Lang::Uk).await.expect("user");
        assert_eq!(user.lang, Lang::Uk);
        db.set_quality(42, 320).await.expect("quality");
        db.set_language(42, Lang::En).await.expect("lang");
        let user = db.ensure_user(42, Lang::Uk).await.expect("user");
        assert_eq!(user.quality, 320);
        assert_eq!(user.lang, Lang::En);

        assert!(db.purge_track(&key).await.expect("purge"));
        assert!(db.get_track(&key).await.expect("get").is_none());
    }
}
