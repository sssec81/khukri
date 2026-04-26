use crate::error::{KhukriError, Result};
use tokio::fs::File;

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[tokio::test]
    async fn preallocate_success_on_real_file() {
        let dir = std::env::temp_dir();
        let path = dir.join(format!(
            "khukri_prealloc_test_{}.tmp",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let file = tokio::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&path)
            .await
            .unwrap();
        let result = preallocate(&file, 4096).await;
        drop(file);
        let _ = tokio::fs::remove_file(&path).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn preallocate_zero_bytes_succeeds() {
        let dir = std::env::temp_dir();
        let path = dir.join(format!(
            "khukri_prealloc_zero_{}.tmp",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let file = tokio::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&path)
            .await
            .unwrap();
        let result = preallocate(&file, 0).await;
        drop(file);
        let _ = tokio::fs::remove_file(&path).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn preallocate_read_only_file_returns_disk_space_error() {
        let dir = std::env::temp_dir();
        let path = dir.join(format!(
            "khukri_prealloc_ro_{}.tmp",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        // Write a byte so the file exists, then open read-only.
        {
            let mut f = std::fs::File::create(&path).unwrap();
            f.write_all(b"x").unwrap();
        }
        let ro_file = tokio::fs::OpenOptions::new()
            .read(true)
            .write(false)
            .open(&path)
            .await
            .unwrap();
        let result = preallocate(&ro_file, 1024 * 1024).await;
        drop(ro_file);
        let _ = tokio::fs::remove_file(&path).await;
        assert!(matches!(result, Err(KhukriError::DiskSpaceError { .. })));
    }
}

/// Pre-allocate `size` bytes on disk before segment writes.
/// Prevents fragmentation and ensures we fail fast on insufficient space.
///
/// - Linux:   fallocate(2) — true block reservation via nix
/// - Windows: SetEndOfFile (called internally by std set_len)
/// - macOS:   ftruncate fallback via set_len
pub async fn preallocate(file: &File, size: u64) -> Result<()> {
    // set_len works on all platforms as baseline:
    //   Windows → SetFilePointer + SetEndOfFile
    //   Unix    → ftruncate
    file.set_len(size)
        .await
        .map_err(|_| KhukriError::DiskSpaceError { bytes: size })?;

    // On Linux, follow up with fallocate for true block reservation.
    #[cfg(target_os = "linux")]
    linux_fallocate(file, size).await?;

    // TODO: implement fcntl(F_PREALLOCATE) via spawn_blocking for better macOS parity.
    #[cfg(target_os = "macos")]
    macos_preallocate_stub(size);

    Ok(())
}

#[cfg(target_os = "macos")]
fn macos_preallocate_stub(size: u64) {
    tracing::debug!(
        bytes = size,
        "macOS preallocation fallback currently uses set_len"
    );
}

/// Use fallocate(2) to physically reserve disk blocks on Linux.
/// Spawned on the blocking thread pool because fallocate can block.
#[cfg(target_os = "linux")]
async fn linux_fallocate(file: &File, size: u64) -> Result<()> {
    use nix::errno::Errno;
    use nix::fcntl::{fallocate, FallocateFlags};

    let file_clone = file.try_clone().await
        .map_err(|_| KhukriError::Io(std::io::Error::new(
            std::io::ErrorKind::Other,
            "failed to clone file for fallocate operation"
        )))?;

    tokio::task::spawn_blocking(move || {
        use std::os::unix::io::AsRawFd;
        let fd = file_clone.as_raw_fd();
        match fallocate(fd, FallocateFlags::empty(), 0, size as i64) {
            Ok(()) => Ok(()),
            Err(Errno::EOPNOTSUPP) | Err(Errno::ENOSYS) | Err(Errno::EINVAL) => {
                tracing::warn!(
                    bytes = size,
                    "fallocate unsupported on filesystem; continuing with set_len fallback"
                );
                Ok(())
            }
            Err(_) => Err(KhukriError::DiskSpaceError { bytes: size }),
        }
    })
    .await
    .map_err(KhukriError::Join)??;

    Ok(())
}
