use std::cmp::Ordering;
use std::collections::BinaryHeap;
use std::sync::Arc;

use sqlx::SqlitePool;
use tokio::sync::{Mutex, Notify};
use tracing::{error, info};

use crate::config::DownloadConfig;
use crate::engine::download::start_download;

// ── ActiveGuard ───────────────────────────────────────────────────────────────

/// RAII guard: decrements `active_count` and wakes the scheduler on drop.
/// Ensures the slot is freed even if the download task panics.
struct ActiveGuard {
    inner: Arc<Mutex<QueueInner>>,
    notify: Arc<Notify>,
}

impl Drop for ActiveGuard {
    fn drop(&mut self) {
        let inner = self.inner.clone();
        let notify = self.notify.clone();
        // `drop` is sync; spawn a task to do the async lock.
        tokio::spawn(async move {
            inner.lock().await.active_count -= 1;
            notify.notify_one();
        });
    }
}

// ── PendingEntry ──────────────────────────────────────────────────────────────

/// Wraps a config so BinaryHeap orders by Priority (max-heap → High pops first).
struct PendingEntry {
    config: DownloadConfig,
}

impl PartialEq for PendingEntry {
    fn eq(&self, other: &Self) -> bool {
        self.config.priority == other.config.priority
    }
}
impl Eq for PendingEntry {}

impl PartialOrd for PendingEntry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for PendingEntry {
    fn cmp(&self, other: &Self) -> Ordering {
        // Higher priority = pops first from the max-heap.
        self.config.priority.cmp(&other.config.priority)
    }
}

// ── QueueInner ────────────────────────────────────────────────────────────────

struct QueueInner {
    pending: BinaryHeap<PendingEntry>,
    active_count: usize,
    max_concurrent: usize,
}

// ── DownloadQueue ─────────────────────────────────────────────────────────────

/// Priority-based download queue with configurable concurrency (KHU-107).
///
/// # Usage
/// ```ignore
/// let queue = DownloadQueue::new(3, pool);
/// let q = Arc::new(queue);
///
/// // Scheduler loop — run in a background task.
/// let q2 = q.clone();
/// tokio::spawn(async move { q2.run().await });
///
/// // Push downloads from anywhere.
/// q.push(config).await;
/// ```
pub struct DownloadQueue {
    inner: Arc<Mutex<QueueInner>>,
    pool: SqlitePool,
    notify: Arc<Notify>,
}

impl DownloadQueue {
    /// Create a new queue. `max_concurrent` = 3 matches the PRD default.
    pub fn new(max_concurrent: usize, pool: SqlitePool) -> Self {
        Self {
            inner: Arc::new(Mutex::new(QueueInner {
                pending: BinaryHeap::new(),
                active_count: 0,
                max_concurrent,
            })),
            pool,
            notify: Arc::new(Notify::new()),
        }
    }

    /// Add a download. Higher-priority items will start before lower ones
    /// once a slot is free. Persists the download as `queued` in SQLite
    /// (actual DB write happens inside `start_download`).
    pub async fn push(&self, config: DownloadConfig) {
        info!(
            url = %config.url,
            priority = ?config.priority,
            "Queued"
        );
        self.inner.lock().await.pending.push(PendingEntry { config });
        self.notify.notify_one();
    }

    /// Change the concurrency limit at runtime — no restart needed (KHU-107).
    pub async fn set_max_concurrent(&self, n: usize) {
        self.inner.lock().await.max_concurrent = n;
        // Wake the scheduler in case we just raised the limit.
        self.notify.notify_one();
    }

    /// Returns `(active, pending, max_concurrent)` for observability.
    pub async fn stats(&self) -> (usize, usize, usize) {
        let g = self.inner.lock().await;
        (g.active_count, g.pending.len(), g.max_concurrent)
    }

    /// Scheduler loop. Run this in a dedicated `tokio::spawn` task.
    /// Runs forever (returns only if the enclosing runtime shuts down).
    pub async fn run(self: Arc<Self>) {
        loop {
            self.notify.notified().await;
            self.try_promote().await;
        }
    }

    /// Promote pending → active downloads while slots are available.
    async fn try_promote(&self) {
        loop {
            let entry = {
                let mut g = self.inner.lock().await;
                if g.active_count >= g.max_concurrent {
                    break;
                }
                match g.pending.pop() {
                    Some(e) => {
                        g.active_count += 1;
                        e
                    }
                    None => break,
                }
            };

            let inner = self.inner.clone();
            let pool = self.pool.clone();
            let notify = self.notify.clone();
            let url = entry.config.url.clone();

            tokio::spawn(async move {
                info!(url = %url, "Download started");
                // Decrement active_count via a drop guard so it runs even on panic.
                let _guard = ActiveGuard { inner: inner.clone(), notify: notify.clone() };
                match start_download(entry.config, pool).await {
                    Ok(()) => info!(url = %url, "Download complete"),
                    Err(e) => error!(url = %url, error = %e, "Download failed"),
                }
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Priority;

    fn make_config(url: &str, priority: Priority) -> DownloadConfig {
        let mut c = DownloadConfig::new(url, format!("/tmp/{url}"));
        c.priority = priority;
        c
    }

    /// High-priority entry must pop before Normal before Low from BinaryHeap.
    #[test]
    fn test_priority_ordering() {
        let mut heap: BinaryHeap<PendingEntry> = BinaryHeap::new();
        heap.push(PendingEntry { config: make_config("low.bin", Priority::Low) });
        heap.push(PendingEntry { config: make_config("high.bin", Priority::High) });
        heap.push(PendingEntry { config: make_config("normal.bin", Priority::Normal) });

        assert_eq!(heap.pop().unwrap().config.priority, Priority::High);
        assert_eq!(heap.pop().unwrap().config.priority, Priority::Normal);
        assert_eq!(heap.pop().unwrap().config.priority, Priority::Low);
    }
}
