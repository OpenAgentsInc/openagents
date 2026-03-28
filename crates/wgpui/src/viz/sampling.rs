pub fn sample_history_series(raw: &[f32], sample_count: usize) -> Vec<f32> {
    if raw.is_empty() || sample_count == 0 {
        return Vec::new();
    }
    if sample_count == 1 {
        return vec![*raw.last().unwrap_or(&0.0)];
    }

    let steps = raw.len().saturating_sub(1);
    (0..sample_count)
        .map(|index| {
            let pos = index as f32 / (sample_count.saturating_sub(1)) as f32;
            let sample_pos = pos * steps as f32;
            let low = sample_pos.floor() as usize;
            let high = sample_pos.ceil() as usize;
            let blend = sample_pos - low as f32;
            if steps == 0 {
                raw[0]
            } else {
                raw[low] + (raw[high] - raw[low]) * blend
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::sample_history_series;

    #[test]
    fn sample_history_series_handles_empty_input() {
        assert!(sample_history_series(&[], 32).is_empty());
        assert!(sample_history_series(&[1.0, 2.0], 0).is_empty());
    }

    #[test]
    fn sample_history_series_keeps_endpoints() {
        let values = sample_history_series(&[1.0, 3.0, 9.0], 5);
        assert_eq!(values.first().copied(), Some(1.0));
        assert_eq!(values.last().copied(), Some(9.0));
    }

    #[test]
    fn sample_history_series_reduces_single_value() {
        assert_eq!(sample_history_series(&[4.2], 1), vec![4.2]);
        assert_eq!(sample_history_series(&[4.2], 3), vec![4.2, 4.2, 4.2]);
    }
}
