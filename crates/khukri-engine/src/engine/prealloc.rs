use tokio::fs::File;
use crate::error::{KhukriError, Result};

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

    Ok(())
}

/// Use fallocate(2) to physically reserve disk blocks on Linux.
/// Spawned on the blocking thread pool because fallocate can block.
#[cfg(target_os = "linux")]
async fn linux_fallocate(file: &File, size: u64) -> Result<()> {
    use std::os::unix::io::AsRawFd;
    use nix::fcntl::{fallocate, FallocateFlags};

    let fd = file.as_raw_fd();

    tokio::task::spawn_blocking(move || {
        fallocate(fd, FallocateFlags::empty(), 0, size as i64)
            .map_err(|_| KhukriError::DiskSpaceError { bytes: size })
    })
    .await
    .map_err(KhukriError::Join)??;

    Ok(())
}
