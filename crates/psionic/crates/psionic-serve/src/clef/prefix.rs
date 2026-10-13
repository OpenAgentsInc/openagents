//! Bounded exact prefix reuse. Each cache belongs to one loaded model instance.

/// Prefix snapshots, least recently used first out. Matching compares every
/// token and the chunk size; schema suffixes are deliberately absent from the
/// key. Metal keeps one entry ([`PrefixCache::new`]); CUDA keeps many under
/// one byte budget ([`PrefixCache::with_entries`]).
pub(crate) struct PrefixCache<T> {
    capacity: usize,
    max_entries: usize,
    /// Least recently used first.
    entries: Vec<Entry<T>>,
}

struct Entry<T> {
    tokens: Vec<u32>,
    chunk: usize,
    bytes: usize,
    value: T,
}

impl<T> PrefixCache<T> {
    /// One snapshot: a new entry replaces the old one.
    pub(crate) fn new(capacity: usize) -> Self {
        Self::with_entries(capacity, 1)
    }

    /// Up to `max_entries` snapshots within `capacity` bytes.
    pub(crate) fn with_entries(capacity: usize, max_entries: usize) -> Self {
        Self {
            capacity,
            max_entries: max_entries.max(1),
            entries: Vec::new(),
        }
    }

    /// The bytes an entry for `tokens` prefix tokens and `state_bytes` of
    /// state retains, if any entry of that size fits the budget.
    fn entry_bytes(&self, tokens: usize, state_bytes: usize) -> Option<usize> {
        tokens
            .checked_mul(size_of::<u32>())
            .and_then(|bytes| bytes.checked_add(state_bytes))
            .filter(|&bytes| self.capacity != 0 && bytes <= self.capacity)
    }

    pub(crate) fn admits(&self, tokens: usize, state_bytes: usize) -> bool {
        self.entry_bytes(tokens, state_bytes).is_some()
    }

    /// The matching snapshot, now the most recently used.
    pub(crate) fn get(&mut self, tokens: &[u32], chunk: usize) -> Option<&T> {
        let index = self
            .entries
            .iter()
            .position(|entry| entry.chunk == chunk && entry.tokens == tokens)?;
        let entry = self.entries.remove(index);
        self.entries.push(entry);
        self.entries.last().map(|entry| &entry.value)
    }

    /// Evicts least recently used entries until `reserve` more bytes (an
    /// entry about to be captured, or pending captures) fit the budget and
    /// one more entry fits the entry limit. `false` when `reserve` alone
    /// exceeds the budget; nothing is evicted then.
    pub(crate) fn make_room(&mut self, reserve: usize) -> bool {
        if self.capacity == 0 || reserve > self.capacity {
            return false;
        }
        while !self.entries.is_empty()
            && (self.retained_bytes() + reserve > self.capacity
                || self.entries.len() >= self.max_entries)
        {
            self.entries.remove(0);
        }
        true
    }

    /// Publishes only a complete snapshot, replacing an entry with the same
    /// key and evicting the least recently used ones it needs room from. An
    /// oversized candidate leaves every entry intact and never allocates a
    /// copy of its token key.
    pub(crate) fn insert(
        &mut self,
        tokens: &[u32],
        chunk: usize,
        state_bytes: usize,
        value: T,
    ) -> bool {
        if tokens.is_empty() || chunk == 0 {
            return false;
        }
        let Some(bytes) = self.entry_bytes(tokens.len(), state_bytes) else {
            return false;
        };
        self.entries
            .retain(|entry| entry.chunk != chunk || entry.tokens != tokens);
        self.make_room(bytes);
        self.entries.push(Entry {
            tokens: tokens.to_vec(),
            chunk,
            bytes,
            value,
        });
        true
    }

    pub(crate) fn clear(&mut self) {
        self.entries.clear();
    }

    pub(crate) fn retained_bytes(&self) -> usize {
        self.entries.iter().map(|entry| entry.bytes).sum()
    }

    pub(crate) fn len(&self) -> usize {
        self.entries.len()
    }

    pub(crate) fn capacity(&self) -> usize {
        self.capacity
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

/// [`reusable_boundary`] with the prefix rounded down to a multiple of
/// `align`. CUDA's blocked delta scan (#11444) is chunk-invariant only when
/// every chunk starts at a multiple of 16 and falls back to the per-token
/// scan otherwise, so a boundary off that grid would give the split request
/// different rounding from an unsplit (cache-off) one (#11442).
pub(crate) fn aligned_boundary(
    prefix: usize,
    length: usize,
    spans: &[(usize, usize)],
    align: usize,
) -> Option<usize> {
    reusable_boundary(prefix - prefix % align.max(1), length, spans)
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
    fn aligned_boundaries_round_down_and_keep_every_chunk_on_the_grid() {
        assert_eq!(
            aligned_boundary(4003, 5000, &[(4003, 5000)], 16),
            Some(4000)
        );
        assert_eq!(
            aligned_boundary(4000, 5000, &[(4000, 5000)], 16),
            Some(4000)
        );
        assert_eq!(aligned_boundary(15, 40, &[(15, 40)], 16), None);
        assert_eq!(aligned_boundary(17, 40, &[(17, 40)], 1), Some(17));
        for prefix in [16, 17, 31, 33, 4003] {
            let length = prefix + 5000;
            let boundary = aligned_boundary(prefix, length, &[(prefix, length)], 16);
            let mut first = 0;
            while first < length {
                assert_eq!(first % 16, 0, "prefix {prefix}: chunk at {first}");
                first += chunk_len(first, length, 2048, boundary);
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

    #[test]
    fn lru_keeps_several_snapshots_and_evicts_the_least_recently_used() {
        let mut cache = PrefixCache::with_entries(3 * (8 + 4), 8);
        assert!(cache.insert(&[1], 8, 8, "a"));
        assert!(cache.insert(&[2], 8, 8, "b"));
        assert!(cache.insert(&[3], 8, 8, "c"));
        assert_eq!(cache.len(), 3);
        // a hit makes "a" the most recently used, so "b" goes first
        assert_eq!(cache.get(&[1], 8), Some(&"a"));
        assert!(cache.insert(&[4], 8, 8, "d"));
        assert!(cache.get(&[2], 8).is_none());
        assert_eq!(cache.get(&[1], 8), Some(&"a"));
        assert_eq!(cache.get(&[3], 8), Some(&"c"));
        assert_eq!(cache.get(&[4], 8), Some(&"d"));
        assert_eq!(cache.retained_bytes(), 36);
        // the same key replaces its entry instead of adding one
        assert!(cache.insert(&[4], 8, 8, "e"));
        assert_eq!(cache.len(), 3);
        assert_eq!(cache.get(&[4], 8), Some(&"e"));
        // a bigger entry evicts as many as it needs, oldest first
        assert!(cache.insert(&[5], 8, 20, "f"));
        assert_eq!(cache.len(), 2);
        assert!(cache.get(&[1], 8).is_none());
        assert_eq!(cache.get(&[4], 8), Some(&"e"));
        // an oversized candidate evicts nothing
        assert!(!cache.insert(&[6], 8, 64, "g"));
        assert_eq!(cache.len(), 2);
    }

    #[test]
    fn entry_limit_and_reservations_evict_before_capture() {
        let mut cache = PrefixCache::with_entries(1000, 2);
        cache.insert(&[1], 8, 8, 1);
        cache.insert(&[2], 8, 8, 2);
        cache.insert(&[3], 8, 8, 3);
        assert_eq!(cache.len(), 2);
        assert!(cache.get(&[1], 8).is_none());
        // room for one more entry under the limit means evicting one
        assert!(cache.make_room(12));
        assert_eq!(cache.len(), 1);
        assert_eq!(cache.get(&[3], 8), Some(&3));
        assert!(!cache.make_room(1001));
        assert_eq!(cache.len(), 1);
        let mut tight = PrefixCache::with_entries(30, 8);
        tight.insert(&[1], 8, 8, 1);
        tight.insert(&[2], 8, 8, 2);
        assert!(tight.make_room(10));
        assert_eq!(tight.len(), 1);
        assert!(!PrefixCache::<()>::with_entries(0, 8).make_room(0));
    }
}
