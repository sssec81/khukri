use std::future::Future;
use std::time::Duration;

use rand::Rng;
use tokio::time::sleep;
use tracing::warn;

use crate::config::RetryConfig;
use crate::error::{KhukriError, Result};

/// Returns true when a status code means a permanent failure — do not retry.
pub fn is_permanent_failure(status: u16) -> bool {
    matches!(status, 400 | 401 | 403 | 404 | 405 | 410)
}

/// Compute back-off delay with ±10% jitter.
/// delay = base_delay_ms * 2^attempt  ±10%
fn backoff_delay(base_ms: u64, attempt: u8) -> Duration {
    let base = base_ms.saturating_mul(1u64 << attempt);
    let jitter_range = (base / 10).max(1);
    let jitter = rand::thread_rng().gen_range(0..=jitter_range);
    let sign: i64 = if rand::thread_rng().gen_bool(0.5) { 1 } else { -1 };
    let delay_ms = (base as i64 + sign * jitter as i64).max(0) as u64;
    Duration::from_millis(delay_ms)
}

/// Retry `f` up to `config.max_retries` times on transient errors.
/// Permanent errors (404, 403, etc.) surface immediately without retry.
pub async fn with_retry<F, Fut, T>(config: &RetryConfig, mut f: F) -> Result<T>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T>>,
{
    let mut attempt = 0u8;
    loop {
        match f().await {
            Ok(val) => return Ok(val),
            Err(KhukriError::PermanentError { status, url }) => {
                // Never retry permanent failures.
                return Err(KhukriError::PermanentError { status, url });
            }
            Err(e) => {
                if attempt >= config.max_retries {
                    return Err(KhukriError::MaxRetriesExceeded { attempts: attempt });
                }
                let delay = backoff_delay(config.base_delay_ms, attempt);
                warn!(
                    "Attempt {} failed ({}), retrying in {}ms",
                    attempt + 1,
                    e,
                    delay.as_millis()
                );
                sleep(delay).await;
                attempt += 1;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU8, Ordering};
    use std::sync::Arc;

    fn fast_config() -> RetryConfig {
        RetryConfig {
            max_retries: 3,
            base_delay_ms: 1, // tiny delay so tests are fast
        }
    }

    #[tokio::test]
    async fn test_two_failures_then_success() {
        let counter = Arc::new(AtomicU8::new(0));
        let c = counter.clone();

        let result = with_retry(&fast_config(), || {
            let c = c.clone();
            async move {
                let n = c.fetch_add(1, Ordering::SeqCst);
                if n < 2 {
                    Err(KhukriError::Io(std::io::Error::new(
                        std::io::ErrorKind::ConnectionReset,
                        "transient",
                    )))
                } else {
                    Ok(42u32)
                }
            }
        })
        .await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);
        // called exactly 3 times: fail, fail, success
        assert_eq!(counter.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn test_permanent_error_not_retried() {
        let counter = Arc::new(AtomicU8::new(0));
        let c = counter.clone();

        let result: Result<u32> = with_retry(&fast_config(), || {
            let c = c.clone();
            async move {
                c.fetch_add(1, Ordering::SeqCst);
                Err(KhukriError::PermanentError {
                    status: 404,
                    url: "http://example.com/file.bin".into(),
                })
            }
        })
        .await;

        assert!(result.is_err());
        // must not retry — called exactly once
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_exceeds_max_retries() {
        let config = RetryConfig {
            max_retries: 2,
            base_delay_ms: 1,
        };
        let counter = Arc::new(AtomicU8::new(0));
        let c = counter.clone();

        let result: Result<u32> = with_retry(&config, || {
            let c = c.clone();
            async move {
                c.fetch_add(1, Ordering::SeqCst);
                Err(KhukriError::Io(std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    "timeout",
                )))
            }
        })
        .await;

        assert!(matches!(result, Err(KhukriError::MaxRetriesExceeded { .. })));
    }
}
