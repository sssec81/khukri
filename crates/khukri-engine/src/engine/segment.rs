/// KHU-102: threads = clamp(floor(file_size_MB / 50), 4, 64)
pub fn calc_thread_count(file_size_bytes: u64) -> u8 {
    let mb = file_size_bytes / (1024 * 1024);
    (mb / 50).clamp(4, 64) as u8
}

#[derive(Debug, Clone)]
pub struct Segment {
    pub index: usize,
    pub start_byte: u64,
    pub end_byte: u64,
    pub completed: bool,
}

impl Segment {
    pub fn len(&self) -> u64 {
        self.end_byte - self.start_byte + 1
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Split `total_bytes` into `thread_count` contiguous, non-overlapping byte ranges.
pub fn build_segments(total_bytes: u64, thread_count: u8) -> Vec<Segment> {
    if total_bytes == 0 || thread_count == 0 {
        return Vec::new();
    }

    let n = (thread_count as u64).min(total_bytes);
    let base = total_bytes / n;
    let remainder = total_bytes % n;

    let mut start = 0u64;
    let mut out = Vec::with_capacity(n as usize);

    for i in 0..n {
        let len = base + u64::from(i < remainder);
        let end = start + len - 1;
        out.push(Segment {
            index: i as usize,
            start_byte: start,
            end_byte: end,
            completed: false,
        });
        start = end + 1;
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_below_200mb_clamps_to_4() {
        assert_eq!(calc_thread_count(0), 4);
        assert_eq!(calc_thread_count(100 * 1024 * 1024), 4);
        assert_eq!(calc_thread_count(199 * 1024 * 1024), 4);
    }

    #[test]
    fn test_500mb_gives_10() {
        assert_eq!(calc_thread_count(500 * 1024 * 1024), 10);
    }

    #[test]
    fn test_3200mb_gives_64() {
        assert_eq!(calc_thread_count(3200 * 1024 * 1024), 64);
    }

    #[test]
    fn test_10gb_clamped_to_64() {
        assert_eq!(calc_thread_count(10 * 1024 * 1024 * 1024), 64);
    }

    #[test]
    fn test_segments_cover_full_range() {
        let total = 1_000_000u64;
        let segs = build_segments(total, 4);
        assert_eq!(segs[0].start_byte, 0);
        assert_eq!(segs.last().unwrap().end_byte, total - 1);
        // no gaps
        for w in segs.windows(2) {
            assert_eq!(w[0].end_byte + 1, w[1].start_byte);
        }
    }

    #[test]
    fn test_tiny_file_caps_segments() {
        let segs = build_segments(1, 4);
        assert_eq!(segs.len(), 1);
        assert_eq!(segs[0].start_byte, 0);
        assert_eq!(segs[0].end_byte, 0);
    }

    #[test]
    fn test_zero_file_has_no_segments() {
        let segs = build_segments(0, 4);
        assert!(segs.is_empty());
    }
}
