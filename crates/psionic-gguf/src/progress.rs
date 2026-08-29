//! Determinate progress meter for long inference steps.

const BAR_CELLS: usize = 20;

/// `{label} [{bar}] {pct}% {done}/{total} {unit}`
///
/// `total == 0` returns `None` so callers hide the bar.
pub fn format_bar(label: &str, done: u64, total: u64, unit: &str) -> Option<String> {
    if total == 0 {
        return None;
    }
    let pct = ((done.min(total) * 100) / total) as u32;
    let filled = ((done.min(total) * BAR_CELLS as u64) / total) as usize;
    let mut bar = String::with_capacity(BAR_CELLS);
    for i in 0..BAR_CELLS {
        bar.push(if i < filled { '#' } else { '-' });
    }
    Some(format!("{label} [{bar}] {pct}% {done}/{total} {unit}"))
}

/// Whether this tick should paint. Always paints first, last, and every
/// `every` units (same idea as `prefill.pos`).
pub fn should_emit(done: u64, total: u64, every: u64) -> bool {
    if total == 0 {
        return false;
    }
    done == 0 || done == total || (every > 0 && done % every == 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bar_bounds() {
        assert_eq!(format_bar("Prefill", 0, 0, "pos"), None);
        let zero = format_bar("Prefill", 0, 100, "pos").unwrap();
        assert!(zero.contains("[--------------------]"), "{zero}");
        assert!(zero.contains("0%"), "{zero}");
        let half = format_bar("Prefill", 50, 100, "pos").unwrap();
        assert!(half.contains("50%"), "{half}");
        assert!(half.contains("[##########----------]"), "{half}");
        let full = format_bar("Prefill", 100, 100, "pos").unwrap();
        assert!(full.contains("100%"), "{full}");
        assert!(full.contains("[####################]"), "{full}");
    }

    #[test]
    fn emit_throttle() {
        assert!(should_emit(0, 64, 32));
        assert!(!should_emit(1, 64, 32));
        assert!(should_emit(32, 64, 32));
        assert!(should_emit(64, 64, 32));
    }
}
