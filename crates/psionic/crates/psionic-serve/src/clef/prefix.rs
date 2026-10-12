//! Bounded exact prefix reuse. Each cache belongs to one loaded model instance.

/// A single prefix snapshot. Matching compares every token and the chunk size;
/// schema suffixes are deliberately absent from the key.
pub(crate) struct PrefixCache<T> {
    capacity: usize,
    entry: Option<Entry<T>>,
}

struct Entry<T> {
    tokens: Vec<u32>,
    chunk: usize,
    bytes: usize,
    value: T,
}

impl<T> PrefixCache<T> {
    pub(crate) fn new(capacity: usize) -> Self {
        Self {
            capacity,
            entry: None,
        }
    }

    pub(crate) fn admits(&self, tokens: usize, state_bytes: usize) -> bool {
        tokens
            .checked_mul(size_of::<u32>())
            .and_then(|bytes| bytes.checked_add(state_bytes))
            .is_some_and(|bytes| self.capacity != 0 && bytes <= self.capacity)
    }

    pub(crate) fn get(&self, tokens: &[u32], chunk: usize) -> Option<&T> {
        self.entry
            .as_ref()
            .filter(|entry| entry.chunk == chunk && entry.tokens == tokens)
            .map(|entry| &entry.value)
    }

    /// Publishes only a complete snapshot. An oversized candidate leaves the
    /// previous entry intact and never allocates a copy of its token key.
    pub(crate) fn insert(
        &mut self,
        tokens: &[u32],
        chunk: usize,
        state_bytes: usize,
        value: T,
    ) -> bool {
        if tokens.is_empty() || chunk == 0 || !self.admits(tokens.len(), state_bytes) {
            return false;
        }
        self.entry = Some(Entry {
            tokens: tokens.to_vec(),
            chunk,
            bytes: state_bytes + size_of_val(tokens),
            value,
        });
        true
    }

    pub(crate) fn clear(&mut self) {
        self.entry = None;
    }

    pub(crate) fn retained_bytes(&self) -> usize {
        self.entry.as_ref().map_or(0, |entry| entry.bytes)
    }
}

/// Keeps cold and warm suffixes on the same chunk boundaries. No span may
/// overlap the prefix because restored memory does not include schema sums.
pub(crate) fn reusable_boundary(
    prefix: usize,
    length: usize,
    spans: &[(usize, usize)],
) -> Option<usize> {
    (prefix > 0
        && prefix < length
        && spans
            .iter()
            .all(|&(start, end)| start >= prefix && start < end && end <= length))
    .then_some(prefix)
}

pub(crate) fn chunk_len(
    first: usize,
    length: usize,
    chunk: usize,
    boundary: Option<usize>,
) -> usize {
    let end = boundary.filter(|&end| first < end).unwrap_or(length);
    chunk.min(end.saturating_sub(first))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::rc::Rc;

    #[test]
    fn exact_prefix_reuses_across_schemas_and_rejects_token_or_chunk_changes() {
        let mut cache = PrefixCache::new(64);
        assert!(cache.insert(&[1, 2, 3], 16, 8, "state and memory"));
        for suffix in [vec![10], vec![20, 30, 40]] {
            let mut prompt = vec![1, 2, 3];
            prompt.extend(suffix);
            assert_eq!(cache.get(&prompt[..3], 16), Some(&"state and memory"));
        }
        assert!(cache.get(&[1, 2, 4], 16).is_none());
        assert!(cache.get(&[1, 2], 16).is_none());
        assert!(cache.get(&[1, 2, 3, 4], 16).is_none());
        assert!(cache.get(&[1, 2, 3], 32).is_none());
        assert!(PrefixCache::<&str>::new(64).get(&[1, 2, 3], 16).is_none());
    }

    #[test]
    fn byte_cap_includes_token_key_and_does_not_evict_for_oversized_candidates() {
        let mut cache = PrefixCache::new(20);
        assert!(cache.insert(&[1, 2, 3], 16, 8, 1));
        assert_eq!(cache.retained_bytes(), 20);
        assert!(!cache.insert(&[4, 5, 6], 16, 9, 2));
        assert_eq!(cache.get(&[1, 2, 3], 16), Some(&1));
        assert!(!cache.admits(usize::MAX, 1));
        assert!(!cache.admits(1, usize::MAX));
        assert!(!PrefixCache::<()>::new(0).insert(&[1], 1, 0, ()));
        assert!(!cache.insert(&[], 16, 0, 2));
        assert!(!cache.insert(&[4], 0, 0, 2));
    }

    #[test]
    fn replacement_releases_old_state_and_new_model_has_no_entry() {
        struct DropSpy(Rc<Cell<usize>>);
        impl Drop for DropSpy {
            fn drop(&mut self) {
                self.0.set(self.0.get() + 1);
            }
        }
        let drops = Rc::new(Cell::new(0));
        let mut cache = PrefixCache::new(32);
        cache.insert(&[1], 8, 8, DropSpy(drops.clone()));
        cache.insert(&[2], 8, 8, DropSpy(drops.clone()));
        assert_eq!(drops.get(), 1);
        assert!(cache.get(&[1], 8).is_none());
        assert!(PrefixCache::<DropSpy>::new(32).get(&[2], 8).is_none());
        drop(cache);
        assert_eq!(drops.get(), 2);
    }

    #[test]
    fn prefix_split_keeps_cold_and_warm_suffix_chunks_identical() {
        for prefix in [1, 15, 16, 17, 31, 32, 33, 65] {
            for chunk in [1, 16, 32, 64] {
                let length = prefix + 37;
                let boundary = reusable_boundary(prefix, length, &[(prefix, length)]);
                let chunks = |mut first| {
                    let mut result = Vec::new();
                    while first < length {
                        let n = chunk_len(first, length, chunk, boundary);
                        assert!(n > 0);
                        result.push((first, n));
                        first += n;
                    }
                    result
                };
                let cold = chunks(0);
                assert!(cold.iter().any(|&(first, n)| first + n == prefix));
                assert_eq!(
                    cold.into_iter()
                        .filter(|&(first, _)| first >= prefix)
                        .collect::<Vec<_>>(),
                    chunks(prefix)
                );
            }
        }
    }

    #[test]
    fn invalid_boundaries_and_prefix_spans_disable_reuse() {
        for prefix in [0, 10, 11, usize::MAX] {
            assert_eq!(reusable_boundary(prefix, 10, &[(8, 10)]), None);
        }
        assert_eq!(reusable_boundary(8, 10, &[(7, 9)]), None);
        assert_eq!(reusable_boundary(8, 10, &[(8, 11)]), None);
        assert_eq!(reusable_boundary(8, 10, &[(9, 8)]), None);
        assert_eq!(reusable_boundary(8, 10, &[(8, 8)]), None);
        assert_eq!(reusable_boundary(8, 10, &[(8, 10)]), Some(8));
    }
}
