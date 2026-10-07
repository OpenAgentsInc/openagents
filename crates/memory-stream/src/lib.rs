//! Memory retrieval scored the way *Generative Agents* (Park and others,
//! 2023) scores it: recency, importance, and relevance, each min-max
//! normalized over the candidates and summed with weights
//! (`docs/verse/generative-agents.md`, item 1).
//!
//! The crate is pure arithmetic and a small in-memory stream. It reads no
//! file, keeps no clock, and calls no model, so Alice's briefing in
//! `crates/coder` and Everglade's townsfolk (which build for `wasm32`)
//! share it. The caller supplies the times, the importance, and the
//! relevance; this crate turns them into one ranking.

/// The hourly recency decay the owner chose: a record's weight halves in
/// about 69 hours since it was last carried.
pub const RECENCY_DECAY: f64 = 0.99;

/// The lowest and highest importance, as the paper's 1-to-10 scale.
pub const IMPORTANCE_MIN: f64 = 1.0;
pub const IMPORTANCE_MAX: f64 = 10.0;

/// Recency after `hours` without access: [`RECENCY_DECAY`] to the power
/// `hours`. A negative span, such as a clock that stepped back, counts as
/// zero.
#[must_use]
pub fn recency(hours: f64) -> f64 {
    RECENCY_DECAY.powf(hours.max(0.0))
}

/// [`recency`] between two Unix-second times.
#[must_use]
pub fn recency_since(now: u64, last_access: u64) -> f64 {
    recency(now.saturating_sub(last_access) as f64 / 3600.0)
}

/// `importance` held to [`IMPORTANCE_MIN`] through [`IMPORTANCE_MAX`]; a
/// value that isn't a number is the minimum.
#[must_use]
pub fn clamp_importance(importance: f64) -> f64 {
    if importance.is_nan() {
        IMPORTANCE_MIN
    } else {
        importance.clamp(IMPORTANCE_MIN, IMPORTANCE_MAX)
    }
}

/// Min-max normalization to 0 through 1. When every value is the same,
/// or there are none, every result is 0: a term that can't tell the
/// candidates apart adds nothing to the ranking. A value that isn't a
/// number counts as the minimum.
#[must_use]
pub fn min_max(values: &[f64]) -> Vec<f64> {
    let finite = values.iter().copied().filter(|v| v.is_finite());
    let lo = finite.clone().fold(f64::INFINITY, f64::min);
    let hi = finite.fold(f64::NEG_INFINITY, f64::max);
    let span = hi - lo;
    values
        .iter()
        .map(|v| {
            if span.is_nan() || span <= 0.0 || !v.is_finite() {
                0.0
            } else {
                (v - lo) / span
            }
        })
        .collect()
}

/// How much each term counts. The paper weights all three equally.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Weights {
    pub recency: f64,
    pub importance: f64,
    pub relevance: f64,
}

impl Default for Weights {
    fn default() -> Self {
        Self {
            recency: 1.0,
            importance: 1.0,
            relevance: 1.0,
        }
    }
}

/// One candidate's raw terms: recency 0 through 1, importance 1 through
/// 10, and relevance on whatever scale the caller's measure uses (cosine
/// similarity, BM25), since normalization removes the scale.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Terms {
    pub recency: f64,
    pub importance: f64,
    pub relevance: f64,
}

/// One candidate's normalized terms and their weighted sum.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Score {
    pub recency: f64,
    pub importance: f64,
    pub relevance: f64,
    pub total: f64,
}

/// Each candidate's [`Score`], in the order given: every term min-max
/// normalized over `terms`, then summed under `weights`.
#[must_use]
pub fn score(terms: &[Terms], weights: Weights) -> Vec<Score> {
    let column = |f: fn(&Terms) -> f64| min_max(&terms.iter().map(f).collect::<Vec<_>>());
    let recency = column(|t| t.recency);
    let importance = column(|t| t.importance);
    let relevance = column(|t| t.relevance);
    (0..terms.len())
        .map(|i| Score {
            recency: recency[i],
            importance: importance[i],
            relevance: relevance[i],
            total: weights.recency * recency[i]
                + weights.importance * importance[i]
                + weights.relevance * relevance[i],
        })
        .collect()
}

/// Candidate indexes, best first: by total, then the more recent, then the
/// earlier index, so equal inputs always rank the same way.
#[must_use]
pub fn ranked(scores: &[Score]) -> Vec<usize> {
    let mut order: Vec<usize> = (0..scores.len()).collect();
    order.sort_by(|&a, &b| {
        scores[b]
            .total
            .total_cmp(&scores[a].total)
            .then(scores[b].recency.total_cmp(&scores[a].recency))
            .then(a.cmp(&b))
    });
    order
}

/// One record in a [`Stream`].
#[derive(Clone, Debug, PartialEq)]
pub struct Memory<T> {
    pub item: T,
    /// When it was written, Unix seconds.
    pub created: u64,
    /// When a retrieval last returned it, Unix seconds.
    pub last_access: Option<u64>,
    /// 1 through 10.
    pub importance: f64,
}

impl<T> Memory<T> {
    /// The time recency counts from: the last access, or creation.
    #[must_use]
    pub fn accessed(&self) -> u64 {
        self.last_access.unwrap_or(self.created)
    }
}

/// A bounded memory stream for a character that keeps its memory in
/// process, such as a townsperson: at most `capacity` records, the oldest
/// written dropped first. Retrieval marks what it returns as accessed.
#[derive(Clone, Debug)]
pub struct Stream<T> {
    capacity: usize,
    weights: Weights,
    memories: Vec<Memory<T>>,
}

impl<T> Stream<T> {
    /// An empty stream of at most `capacity` records (at least one).
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity: capacity.max(1),
            weights: Weights::default(),
            memories: Vec::new(),
        }
    }

    /// A stream of at most `capacity` records holding `memories`, as a
    /// save stored them: when there are more, the newest written stay.
    #[must_use]
    pub fn from_memories(capacity: usize, mut memories: Vec<Memory<T>>) -> Self {
        let capacity = capacity.max(1);
        memories.sort_by_key(|m| m.created);
        let extra = memories.len().saturating_sub(capacity);
        memories.drain(..extra);
        for m in &mut memories {
            m.importance = clamp_importance(m.importance);
        }
        Self {
            capacity,
            weights: Weights::default(),
            memories,
        }
    }

    /// The records, oldest written first, for a save to store.
    #[must_use]
    pub fn into_memories(self) -> Vec<Memory<T>> {
        self.memories
    }

    /// The same stream ranking under `weights`.
    #[must_use]
    pub fn with_weights(mut self, weights: Weights) -> Self {
        self.weights = weights;
        self
    }

    /// Writes `item` at `created` with `importance`, dropping the oldest
    /// record when the stream is full.
    pub fn push(&mut self, item: T, created: u64, importance: f64) {
        if self.memories.len() >= self.capacity {
            let oldest = self
                .memories
                .iter()
                .enumerate()
                .min_by_key(|(_, m)| m.created)
                .map_or(0, |(i, _)| i);
            self.memories.remove(oldest);
        }
        self.memories.push(Memory {
            item,
            created,
            last_access: None,
            importance: clamp_importance(importance),
        });
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.memories.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.memories.is_empty()
    }

    /// Every record, oldest written first.
    #[must_use]
    pub fn memories(&self) -> &[Memory<T>] {
        &self.memories
    }

    /// The `limit` best records at `now`, best first, with their scores;
    /// `relevance` rates each item against whatever the caller is looking
    /// for. Each returned record's last access becomes `now`.
    pub fn retrieve(
        &mut self,
        now: u64,
        limit: usize,
        relevance: impl Fn(&T) -> f64,
    ) -> Vec<(&T, Score)> {
        let terms: Vec<Terms> = self
            .memories
            .iter()
            .map(|m| Terms {
                recency: recency_since(now, m.accessed()),
                importance: m.importance,
                relevance: relevance(&m.item),
            })
            .collect();
        let scores = score(&terms, self.weights);
        let picked: Vec<usize> = ranked(&scores).into_iter().take(limit).collect();
        for &i in &picked {
            self.memories[i].last_access = Some(now);
        }
        picked
            .into_iter()
            .map(|i| (&self.memories[i].item, scores[i]))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    #[test]
    fn a_stream_round_trips_through_its_memories_and_keeps_the_newest() {
        let mut s = Stream::new(3);
        for i in 0..3u64 {
            s.push(i, i * 10, 5.0);
        }
        let back = Stream::from_memories(2, s.into_memories());
        let kept: Vec<u64> = back.memories().iter().map(|m| m.item).collect();
        assert_eq!(kept, vec![1, 2]);
    }

    #[test]
    fn recency_halves_in_about_69_hours_and_ignores_negative_spans() {
        assert!(close(recency(0.0), 1.0));
        assert!(close(recency(1.0), 0.99));
        let half = recency(69.0);
        assert!((0.49..0.51).contains(&half), "{half}");
        assert!(close(recency(-5.0), 1.0));
        assert!(close(recency_since(3600 * 10, 0), 0.99f64.powi(10)));
        assert!(
            close(recency_since(0, 100), 1.0),
            "a clock that stepped back"
        );
    }

    #[test]
    fn min_max_maps_onto_zero_to_one_and_flat_columns_to_zero() {
        assert_eq!(min_max(&[2.0, 4.0, 3.0]), vec![0.0, 1.0, 0.5]);
        assert_eq!(min_max(&[7.0, 7.0]), vec![0.0, 0.0]);
        assert!(min_max(&[]).is_empty());
        assert_eq!(min_max(&[1.0, f64::NAN, 3.0]), vec![0.0, 0.0, 1.0]);
        assert!(close(clamp_importance(42.0), 10.0));
        assert!(close(clamp_importance(f64::NAN), 1.0));
    }

    #[test]
    fn the_three_terms_count_equally_after_normalization() {
        let terms = [
            // Fresh, mundane, off topic.
            Terms {
                recency: 1.0,
                importance: 1.0,
                relevance: 0.0,
            },
            // Old, important, on topic: relevance on a BM25 scale.
            Terms {
                recency: 0.2,
                importance: 9.0,
                relevance: 12.0,
            },
            // In between.
            Terms {
                recency: 0.6,
                importance: 5.0,
                relevance: 6.0,
            },
        ];
        let scores = score(&terms, Weights::default());
        assert!(close(scores[0].total, 1.0));
        assert!(close(scores[1].total, 2.0));
        assert!(close(scores[2].total, 1.5));
        assert_eq!(ranked(&scores), vec![1, 2, 0]);
        let recency_only = Weights {
            recency: 1.0,
            importance: 0.0,
            relevance: 0.0,
        };
        assert_eq!(ranked(&score(&terms, recency_only)), vec![0, 2, 1]);
    }

    #[test]
    fn a_stream_drops_its_oldest_and_retrieval_refreshes_recency() {
        let mut stream = Stream::new(3);
        stream.push("saw the river", 0, 2.0);
        stream.push("the bridge collapsed", 3600, 9.0);
        stream.push("ate bread", 7200, 1.0);
        stream.push("met the miller", 10_800, 3.0);
        assert_eq!(stream.len(), 3, "the oldest went");
        assert!(stream.memories().iter().all(|m| m.item != "saw the river"));
        let now = 3600 * 100;
        let best = stream.retrieve(now, 1, |item| f64::from(u8::from(item.contains("bridge"))));
        assert_eq!(best[0].0, &"the bridge collapsed");
        let bridge = stream
            .memories()
            .iter()
            .find(|m| m.item == "the bridge collapsed")
            .unwrap();
        assert_eq!(bridge.last_access, Some(now));
        // Asked about nothing in particular, the record just recalled is
        // the most recent, and it is still the most important.
        let again = stream.retrieve(now + 60, 3, |_| 0.0);
        assert_eq!(again[0].0, &"the bridge collapsed");
    }
}
