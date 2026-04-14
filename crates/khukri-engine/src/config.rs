use std::cmp::Ordering;
use std::path::PathBuf;

// ── Priority ──────────────────────────────────────────────────────────────────

/// Download priority — determines scheduling order in the queue.
/// BinaryHeap is a max-heap, so High > Normal > Low by the Ord impl below.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Priority {
    Low,
    Normal,
    High,
}

impl Priority {
    pub fn as_str(&self) -> &'static str {
        match self {
            Priority::Low => "low",
            Priority::Normal => "normal",
            Priority::High => "high",
        }
    }

    fn rank(&self) -> u8 {
        match self {
            Priority::Low => 0,
            Priority::Normal => 1,
            Priority::High => 2,
        }
    }
}

impl PartialOrd for Priority {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Priority {
    fn cmp(&self, other: &Self) -> Ordering {
        self.rank().cmp(&other.rank())
    }
}

impl Default for Priority {
    fn default() -> Self {
        Priority::Normal
    }
}

// ── ThrottleConfig ────────────────────────────────────────────────────────────

/// Bandwidth cap for a download. `None` = unlimited.
#[derive(Debug, Clone, Default)]
pub struct ThrottleConfig {
    pub bytes_per_sec: Option<u64>,
}

// ── RetryConfig ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct RetryConfig {
    pub max_retries: u8,
    pub base_delay_ms: u64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            base_delay_ms: 1_000,
        }
    }
}

// ── DownloadConfig ────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct DownloadConfig {
    pub url: String,
    pub file_path: PathBuf,
    /// Override the auto-calculated thread count. None = use formula.
    pub override_threads: Option<u8>,
    pub retry: RetryConfig,
    pub priority: Priority,
    pub throttle: ThrottleConfig,
}

impl DownloadConfig {
    pub fn new(url: impl Into<String>, file_path: impl Into<PathBuf>) -> Self {
        Self {
            url: url.into(),
            file_path: file_path.into(),
            override_threads: None,
            retry: RetryConfig::default(),
            priority: Priority::default(),
            throttle: ThrottleConfig::default(),
        }
    }
}
