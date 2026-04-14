use crate::error::Result;
use sqlx::SqlitePool;

// ── Types ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct DownloadRow {
    pub id: String,
    pub url: String,
    pub file_path: String,
    pub total_bytes: Option<i64>,
    pub status: String,
    pub priority: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct SegmentRow {
    pub id: i64,
    pub download_id: String,
    pub start_byte: i64,
    pub end_byte: i64,
    pub completed: i64, // 0 | 1
}

// ── Migrations ────────────────────────────────────────────────────────────────

pub async fn run_migrations(pool: &SqlitePool) -> Result<()> {
    sqlx::migrate!("./migrations").run(pool).await?;
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
    now_secs: i64,
) -> Result<()> {
    // SQLite stores integers as i64. No real filesystem supports files > i64::MAX (9.2 EB),
    // so this cast is safe in practice. We preserve the bit pattern — SQLite reads it back
    // correctly via the same cast on the way out.
    #[allow(clippy::cast_possible_wrap)]
    let total = total_bytes.map(|b| b as i64);
    sqlx::query(
        "INSERT INTO downloads (id, url, file_path, total_bytes, status, priority, created_at)
         VALUES (?, ?, ?, ?, 'queued', ?, ?)
         ON CONFLICT(id) DO UPDATE SET
            url = excluded.url,
            file_path = excluded.file_path,
            total_bytes = COALESCE(downloads.total_bytes, excluded.total_bytes),
            priority = excluded.priority",
    )
    .bind(id)
    .bind(url)
    .bind(file_path)
    .bind(total)
    .bind(priority)
    .bind(now_secs)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn set_download_status(pool: &SqlitePool, id: &str, status: &str) -> Result<()> {
    sqlx::query("UPDATE downloads SET status = ? WHERE id = ?")
        .bind(status)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn get_download(pool: &SqlitePool, id: &str) -> Result<Option<DownloadRow>> {
    let row = sqlx::query_as::<_, DownloadRow>("SELECT * FROM downloads WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await?;
    Ok(row)
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
