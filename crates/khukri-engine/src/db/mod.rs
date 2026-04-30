use crate::error::Result;
use sqlx::migrate::Migrator;
use sqlx::sqlite::SqliteRow;
use sqlx::Row;
use sqlx::SqlitePool;

static MIGRATOR: Migrator = sqlx::migrate!("./migrations");

// ── Types ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct DownloadRow {
    pub id: String,
    pub url: String,
    pub file_path: String,
    pub total_bytes: Option<i64>,
    pub status: String,
    pub priority: String,
    pub throttle_bytes_per_sec: Option<i64>,
    pub media_quality: Option<String>,
    pub request_source: Option<String>,
    pub failure_reason: Option<String>,
    pub created_at: i64,
}

impl<'r> sqlx::FromRow<'r, SqliteRow> for DownloadRow {
    fn from_row(row: &'r SqliteRow) -> std::result::Result<Self, sqlx::Error> {
        Ok(Self {
            id: row.try_get("id")?,
            url: row.try_get("url")?,
            file_path: row.try_get("file_path")?,
            total_bytes: row.try_get("total_bytes")?,
            status: row.try_get("status")?,
            priority: row.try_get("priority")?,
            throttle_bytes_per_sec: row.try_get("throttle_bytes_per_sec")?,
            media_quality: row.try_get("media_quality")?,
            request_source: row.try_get("request_source")?,
            failure_reason: row.try_get("failure_reason")?,
            created_at: row.try_get("created_at")?,
        })
    }
}

#[derive(Debug, Clone)]
pub struct SegmentRow {
    pub id: i64,
    pub download_id: String,
    pub start_byte: i64,
    pub end_byte: i64,
    pub completed: i64, // 0 | 1
}

impl<'r> sqlx::FromRow<'r, SqliteRow> for SegmentRow {
    fn from_row(row: &'r SqliteRow) -> std::result::Result<Self, sqlx::Error> {
        Ok(Self {
            id: row.try_get("id")?,
            download_id: row.try_get("download_id")?,
            start_byte: row.try_get("start_byte")?,
            end_byte: row.try_get("end_byte")?,
            completed: row.try_get("completed")?,
        })
    }
}

// ── Migrations ────────────────────────────────────────────────────────────────

pub async fn run_migrations(pool: &SqlitePool) -> Result<()> {
    MIGRATOR.run(pool).await?;
    Ok(())
}

// ── Downloads ─────────────────────────────────────────────────────────────────

pub async fn upsert_download(
    pool: &SqlitePool,
    id: &str,
    url: &str,
    file_path: &str,
    total_bytes: Option<u64>,
    priority: &str,
    throttle_bytes_per_sec: Option<u64>,
    now_secs: i64,
) -> Result<()> {
    // SQLite stores integers as i64. No real filesystem supports files > i64::MAX (9.2 EB),
    // so this cast is safe in practice. We preserve the bit pattern — SQLite reads it back
    // correctly via the same cast on the way out.
    #[allow(clippy::cast_possible_wrap)]
    let total = total_bytes.map(|b| b as i64);
    #[allow(clippy::cast_possible_wrap)]
    let throttle = throttle_bytes_per_sec.map(|b| b as i64);
    sqlx::query(
        "INSERT INTO downloads (
            id,
            url,
            file_path,
            total_bytes,
            status,
            priority,
            throttle_bytes_per_sec,
            failure_reason,
            created_at
         )
         VALUES (?, ?, ?, ?, 'queued', ?, ?, NULL, ?)
         ON CONFLICT(id) DO UPDATE SET
            url = excluded.url,
            file_path = excluded.file_path,
            total_bytes = COALESCE(downloads.total_bytes, excluded.total_bytes),
            priority = excluded.priority,
            throttle_bytes_per_sec = excluded.throttle_bytes_per_sec,
            failure_reason = NULL",
    )
    .bind(id)
    .bind(url)
    .bind(file_path)
    .bind(total)
    .bind(priority)
    .bind(throttle)
    .bind(now_secs)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn set_download_status(pool: &SqlitePool, id: &str, status: &str) -> Result<()> {
    sqlx::query(
        "UPDATE downloads
         SET status = ?, failure_reason = CASE WHEN ? = 'failed' THEN failure_reason ELSE NULL END
         WHERE id = ?",
    )
    .bind(status)
    .bind(status)
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn set_download_request_metadata(
    pool: &SqlitePool,
    id: &str,
    media_quality: Option<&str>,
    request_source: Option<&str>,
) -> Result<()> {
    sqlx::query(
        "UPDATE downloads
         SET media_quality = ?, request_source = ?
         WHERE id = ?",
    )
    .bind(media_quality)
    .bind(request_source)
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn set_download_file_path(pool: &SqlitePool, id: &str, file_path: &str) -> Result<()> {
    sqlx::query(
        "UPDATE downloads
         SET file_path = ?
         WHERE id = ?",
    )
    .bind(file_path)
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

/// Atomically move all downloads whose current status is in `from_statuses`
/// to `to_status` in a single UPDATE statement.
pub async fn set_download_status_where(
    pool: &SqlitePool,
    from_statuses: &[&str],
    to_status: &str,
) -> Result<()> {
    if from_statuses.is_empty() {
        return Ok(());
    }
    // Build `IN (?, ?, ...)` — sqlx doesn't support slice binding directly.
    let placeholders = from_statuses
        .iter()
        .map(|_| "?")
        .collect::<Vec<_>>()
        .join(", ");
    let sql = format!(
        "UPDATE downloads SET status = ?, failure_reason = NULL WHERE status IN ({placeholders})"
    );
    let mut query = sqlx::query(&sql).bind(to_status);
    for s in from_statuses {
        query = query.bind(*s);
    }
    query.execute(pool).await?;
    Ok(())
}

pub async fn get_download(pool: &SqlitePool, id: &str) -> Result<Option<DownloadRow>> {
    let row = sqlx::query_as::<_, DownloadRow>("SELECT * FROM downloads WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await?;
    Ok(row)
}

pub async fn list_downloads(pool: &SqlitePool) -> Result<Vec<DownloadRow>> {
    let rows = sqlx::query_as::<_, DownloadRow>(
        "SELECT * FROM downloads ORDER BY created_at DESC, id DESC",
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn set_download_failed(pool: &SqlitePool, id: &str, reason: &str) -> Result<()> {
    sqlx::query("UPDATE downloads SET status = 'failed', failure_reason = ? WHERE id = ?")
        .bind(reason)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn set_download_cancelled(pool: &SqlitePool, id: &str) -> Result<()> {
    sqlx::query("UPDATE downloads SET status = 'cancelled', failure_reason = NULL WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn delete_download(pool: &SqlitePool, id: &str) -> Result<()> {
    sqlx::query("DELETE FROM downloads WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

// ── Segments ──────────────────────────────────────────────────────────────────

pub async fn insert_segments(
    pool: &SqlitePool,
    download_id: &str,
    segments: &[(u64, u64)], // (start_byte, end_byte)
) -> Result<()> {
    // All-or-nothing: a crash mid-loop must not leave partial segment state.
    let mut tx = pool.begin().await?;
    for (start, end) in segments {
        sqlx::query(
            "INSERT INTO segments (download_id, start_byte, end_byte, completed)
             VALUES (?, ?, ?, 0)",
        )
        .bind(download_id)
        .bind(*start as i64) // i64 cast safe: no filesystem supports > 9.2 EB offsets
        .bind(*end as i64)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(())
}

pub async fn delete_segments(pool: &SqlitePool, download_id: &str) -> Result<()> {
    sqlx::query("DELETE FROM segments WHERE download_id = ?")
        .bind(download_id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn mark_segment_complete(pool: &SqlitePool, segment_id: i64) -> Result<()> {
    sqlx::query("UPDATE segments SET completed = 1 WHERE id = ?")
        .bind(segment_id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Returns only incomplete segments — used to skip already-done work on resume.
pub async fn get_incomplete_segments(
    pool: &SqlitePool,
    download_id: &str,
) -> Result<Vec<SegmentRow>> {
    let rows = sqlx::query_as::<_, SegmentRow>(
        "SELECT * FROM segments WHERE download_id = ? AND completed = 0 ORDER BY start_byte",
    )
    .bind(download_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn get_all_segments(pool: &SqlitePool, download_id: &str) -> Result<Vec<SegmentRow>> {
    let rows = sqlx::query_as::<_, SegmentRow>(
        "SELECT * FROM segments WHERE download_id = ? ORDER BY start_byte",
    )
    .bind(download_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn get_segment_formula_version(
    pool: &SqlitePool,
    download_id: &str,
) -> Result<Option<i64>> {
    let row: Option<(Option<i64>,)> =
        sqlx::query_as("SELECT segment_formula_version FROM downloads WHERE id = ?")
            .bind(download_id)
            .fetch_optional(pool)
            .await?;
    Ok(row.and_then(|(v,)| v))
}

pub async fn set_segment_formula_version(
    pool: &SqlitePool,
    download_id: &str,
    version: i64,
) -> Result<()> {
    sqlx::query("UPDATE downloads SET segment_formula_version = ? WHERE id = ?")
        .bind(version)
        .bind(download_id)
        .execute(pool)
        .await?;
    Ok(())
}
