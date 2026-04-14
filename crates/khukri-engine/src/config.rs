use std::cmp::Ordering;
use std::path::PathBuf;

use crate::error::{KhukriError, Result};

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

    pub fn validate(&self) -> Result<()> {
        if self.url.trim().is_empty() {
            return Err(KhukriError::InvalidConfig {
                field: "url",
                reason: "URL must not be empty".to_string(),
            });
        }

        if self.file_path.as_os_str().is_empty() {
            return Err(KhukriError::InvalidConfig {
                field: "file_path",
                reason: "output path must not be empty".to_string(),
            });
        }

        if let Some(threads) = self.override_threads {
            if threads == 0 || threads > 128 {
                return Err(KhukriError::InvalidConfig {
                    field: "override_threads",
                    reason: "must be in range 1..=128".to_string(),
                });
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::KhukriError;

    #[test]
    fn test_validate_rejects_zero_override_threads() {
        let mut cfg = DownloadConfig::new("https://example.com/file.bin", "out.bin");
        cfg.override_threads = Some(0);
        assert!(matches!(
            cfg.validate(),
            Err(KhukriError::InvalidConfig {
                field: "override_threads",
                ..
            })
        ));
    }

    #[test]
    fn test_validate_accepts_reasonable_config() {
        let cfg = DownloadConfig::new("https://example.com/file.bin", "out.bin");
        assert!(cfg.validate().is_ok());
    }
}
