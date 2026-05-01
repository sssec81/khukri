use std::cmp::Ordering;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use reqwest::header::{HeaderName, HeaderValue};
use url::Url;

use crate::error::{KhukriError, Result};

// ── Priority ──────────────────────────────────────────────────────────────────

/// Download priority — determines scheduling order in the queue.
/// BinaryHeap is a max-heap, so High > Normal > Low by the Ord impl below.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Priority {
    #[default]
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

// ── Path helpers ──────────────────────────────────────────────────────────────

/// Canonicalize `path` by resolving the deepest existing ancestor, then
/// re-appending any non-existent tail components verbatim.
///
/// This lets us safely check containment even when the output file or its
/// parent directory hasn't been created yet, while still resolving any
/// symlinks that do exist on the path.
fn canonicalize_with_nonexistent_tail(path: &Path) -> io::Result<PathBuf> {
    // Accumulate non-existent tail components (in reverse order).
    let mut tail: Vec<std::ffi::OsString> = Vec::new();
    let mut cursor = path.to_path_buf();

    loop {
        if cursor.exists() {
            let mut canonical = fs::canonicalize(&cursor)?;
            for component in tail.into_iter().rev() {
                canonical.push(component);
            }
            return Ok(canonical);
        }

        // Pop the last component; error if we run out of path.
        let name = cursor
            .file_name()
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::NotFound,
                    "no existing ancestor found for output path",
                )
            })?
            .to_os_string();

        tail.push(name);
        cursor = cursor.parent().map(PathBuf::from).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "no existing ancestor found for output path",
            )
        })?;
    }
}

// ── DownloadConfig ────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct DownloadConfig {
    pub url: String,
    pub file_path: PathBuf,
    pub allowed_root: Option<PathBuf>,
    /// Override the auto-calculated thread count. None = use formula.
    pub override_threads: Option<u8>,
    pub retry: RetryConfig,
    pub priority: Priority,
    pub throttle: ThrottleConfig,
    pub proxy_url: Option<String>,
    pub custom_headers: Vec<(String, String)>,
}

impl DownloadConfig {
    pub fn new(url: impl Into<String>, file_path: impl Into<PathBuf>) -> Self {
        Self {
            url: url.into(),
            file_path: file_path.into(),
            allowed_root: None,
            override_threads: None,
            retry: RetryConfig::default(),
            priority: Priority::default(),
            throttle: ThrottleConfig::default(),
            proxy_url: None,
            custom_headers: Vec::new(),
        }
    }

    pub fn validate(&self) -> Result<()> {
        if self.url.trim().is_empty() {
            return Err(KhukriError::InvalidConfig {
                field: "url",
                reason: "URL must not be empty".to_string(),
            });
        }

        // Validate URL scheme and block private/localhost addresses.
        let parsed_url = url::Url::parse(&self.url).map_err(|e| KhukriError::InvalidConfig {
            field: "url",
            reason: format!("invalid URL: {e}"),
        })?;

        let scheme = parsed_url.scheme();
        if scheme != "http" && scheme != "https" {
            return Err(KhukriError::InvalidConfig {
                field: "url",
                reason: format!("URL scheme '{}' not allowed (http/https only)", scheme),
            });
        }

        if let Some(host) = parsed_url.host_str() {
            let lower = host.to_lowercase();
            // Block localhost, private IPs (10.0.0.0/8, 172.16.0.0/12, 192.168.0.0/16), and loopback addresses.
            if lower == "localhost"
                || lower == "127.0.0.1"
                || lower == "::1"
                || lower == "[::1]"
                || lower.starts_with("10.")
                || lower.starts_with("172.16.")
                || lower.starts_with("192.168.")
                || lower.starts_with("169.254.")
            // link-local
            {
                return Err(KhukriError::InvalidConfig {
                    field: "url",
                    reason: format!("private or localhost addresses not allowed in downloads"),
                });
            }
        }

        if self.file_path.as_os_str().is_empty() {
            return Err(KhukriError::InvalidConfig {
                field: "file_path",
                reason: "output path must not be empty".to_string(),
            });
        }

        if let Some(root) = &self.allowed_root {
            let canonical_root =
                fs::canonicalize(root).map_err(|e| KhukriError::InvalidConfig {
                    field: "allowed_root",
                    reason: format!("cannot canonicalize allowed root: {e}"),
                })?;

            // Canonicalize only the deepest existing ancestor so validation
            // succeeds even when the output file (or its parent) doesn't exist
            // yet. Symlinks in the existing prefix are resolved, preventing
            // traversal via symlink chains.
            let canonical_target =
                canonicalize_with_nonexistent_tail(&self.file_path).map_err(|e| {
                    KhukriError::InvalidConfig {
                        field: "file_path",
                        reason: format!("cannot resolve output path: {e}"),
                    }
                })?;

            if !canonical_target.starts_with(&canonical_root) {
                return Err(KhukriError::InvalidConfig {
                    field: "file_path",
                    reason: format!(
                        "path '{}' is outside allowed root '{}'",
                        canonical_target.display(),
                        canonical_root.display()
                    ),
                });
            }
        }

        if let Some(threads) = self.override_threads {
            if threads == 0 || threads > 128 {
                return Err(KhukriError::InvalidConfig {
                    field: "override_threads",
                    reason: "must be in range 1..=128".to_string(),
                });
            }
        }

        if let Some(proxy_url) = &self.proxy_url {
            if proxy_url.trim().is_empty() {
                return Err(KhukriError::InvalidConfig {
                    field: "proxy_url",
                    reason: "proxy URL must not be empty".to_string(),
                });
            }

            reqwest::Proxy::all(proxy_url).map_err(|e| KhukriError::InvalidConfig {
                field: "proxy_url",
                reason: format!("invalid proxy URL: {e}"),
            })?;
        }

        for (name, value) in &self.custom_headers {
            HeaderName::from_bytes(name.as_bytes()).map_err(|e| KhukriError::InvalidConfig {
                field: "custom_headers",
                reason: format!("invalid header name '{name}': {e}"),
            })?;
            HeaderValue::from_str(value).map_err(|e| KhukriError::InvalidConfig {
                field: "custom_headers",
                reason: format!("invalid header value for '{name}': {e}"),
            })?;
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

    #[test]
    fn test_validate_rejects_path_outside_allowed_root() {
        let stamp = format!(
            "khukri_cfg_test_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let root = std::env::temp_dir().join(format!("{stamp}_root"));
        let outside = std::env::temp_dir().join(format!("{stamp}_outside"));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::create_dir_all(&outside).unwrap();

        let mut cfg = DownloadConfig::new("https://example.com/file.bin", outside.join("x.bin"));
        cfg.allowed_root = Some(root.clone());

        let result = cfg.validate();
        assert!(matches!(
            result,
            Err(KhukriError::InvalidConfig {
                field: "file_path",
                ..
            })
        ));

        let _ = std::fs::remove_dir_all(root);
        let _ = std::fs::remove_dir_all(outside);
    }

    #[test]
    fn test_validate_accepts_nonexistent_output_file_inside_root() {
        let stamp = format!(
            "khukri_cfg_nonexistent_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let root = std::env::temp_dir().join(format!("{stamp}_root"));
        std::fs::create_dir_all(&root).unwrap();

        // The file itself does not exist — only the root directory does.
        let mut cfg = DownloadConfig::new(
            "https://example.com/file.bin",
            root.join("subdir").join("file.bin"),
        );
        cfg.allowed_root = Some(root.clone());

        // Should not error even though neither subdir/ nor file.bin exist.
        assert!(
            cfg.validate().is_ok(),
            "validation failed for non-existent nested path inside root"
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[cfg(windows)]
    #[test]
    fn test_validate_rejects_symlink_escape_windows() {
        use std::os::windows::fs::symlink_dir;

        let stamp = format!(
            "khukri_cfg_symlink_win_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let root = std::env::temp_dir().join(format!("{stamp}_root"));
        let outside = std::env::temp_dir().join(format!("{stamp}_outside"));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::create_dir_all(&outside).unwrap();

        let link = root.join("escape");
        // Requires Developer Mode or elevated privileges on Windows.
        if symlink_dir(&outside, &link).is_err() {
            let _ = std::fs::remove_dir_all(&root);
            let _ = std::fs::remove_dir_all(&outside);
            return; // Skip if symlink creation is not permitted.
        }

        let mut cfg = DownloadConfig::new("https://example.com/file.bin", link.join("file.bin"));
        cfg.allowed_root = Some(root.clone());

        let result = cfg.validate();
        assert!(
            matches!(
                result,
                Err(KhukriError::InvalidConfig {
                    field: "file_path",
                    ..
                })
            ),
            "Windows symlink escape should be rejected, got: {result:?}"
        );

        let _ = std::fs::remove_dir_all(root);
        let _ = std::fs::remove_dir_all(outside);
    }

    #[cfg(unix)]
    #[test]
    fn test_validate_rejects_symlink_escape() {
        use std::os::unix::fs::symlink;

        let stamp = format!(
            "khukri_cfg_symlink_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let root = std::env::temp_dir().join(format!("{stamp}_root"));
        let outside = std::env::temp_dir().join(format!("{stamp}_outside"));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::create_dir_all(&outside).unwrap();

        // Create a symlink inside root/ that points outside root/.
        let link = root.join("escape");
        symlink(&outside, &link).unwrap();

        let mut cfg = DownloadConfig::new("https://example.com/file.bin", link.join("file.bin"));
        cfg.allowed_root = Some(root.clone());

        let result = cfg.validate();
        assert!(
            matches!(
                result,
                Err(KhukriError::InvalidConfig {
                    field: "file_path",
                    ..
                })
            ),
            "symlink escape should be rejected, got: {result:?}"
        );

        let _ = std::fs::remove_dir_all(root);
        let _ = std::fs::remove_dir_all(outside);
    }
}
