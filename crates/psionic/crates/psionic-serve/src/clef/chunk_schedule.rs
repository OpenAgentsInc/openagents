//! Bounded cohorts of independent backbone chunks.
//!
//! Each chunk owns its request state. The executor may group the chunks' dense
//! projections, but must keep their attention and recurrent state separate.
//! Returning an unfinished chunk appends it behind requests that are already
//! ready. Dropping an unfinished chunk cancels that request and releases its
//! reservation; it never makes partially updated state available again.

use std::collections::{BTreeMap, VecDeque};
use std::sync::{Arc, Mutex, MutexGuard};

/// Limits for one model's scheduler.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Limits {
    pub max_requests: usize,
    pub max_batch_requests: usize,
    pub max_batch_tokens: usize,
    pub max_reserved_bytes: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct RequestId(u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AdmissionError {
    InvalidLength,
    Full,
    Memory,
    IdExhausted,
}

struct Request<T> {
    id: RequestId,
    state: T,
    first: usize,
    tokens: usize,
    chunk_tokens: usize,
    boundaries: Vec<usize>,
}

struct Reservation {
    bytes: u64,
    cancelled: bool,
}

struct Inner<T> {
    limits: Limits,
    next_id: u64,
    bytes: u64,
    reservations: BTreeMap<RequestId, Reservation>,
    ready: VecDeque<Request<T>>,
}

impl<T> Inner<T> {
    fn release(&mut self, id: RequestId) {
        if let Some(reservation) = self.reservations.remove(&id) {
            self.bytes -= reservation.bytes;
        }
    }
}

/// Request ownership and scheduling, independent of a device or runtime.
pub(crate) struct ChunkScheduler<T> {
    inner: Arc<Mutex<Inner<T>>>,
}

impl<T> ChunkScheduler<T> {
    pub fn new(limits: Limits) -> Result<Self, &'static str> {
        if limits.max_requests == 0
            || limits.max_batch_requests == 0
            || limits.max_batch_tokens == 0
            || limits.max_reserved_bytes == 0
        {
            return Err("chunk scheduler limits must be positive");
        }
        Ok(Self {
            inner: Arc::new(Mutex::new(Inner {
                limits,
                next_id: 0,
                bytes: 0,
                reservations: BTreeMap::new(),
                ready: VecDeque::new(),
            })),
        })
    }

    /// Reserves the request's maximum device allocation before it runs.
    /// A refusal returns the caller's state without dropping it.
    pub fn admit(
        &self,
        state: T,
        tokens: usize,
        chunk_tokens: usize,
        reserved_bytes: u64,
    ) -> Result<RequestId, (AdmissionError, T)> {
        self.admit_with_boundaries(state, tokens, chunk_tokens, reserved_bytes, &[])
    }

    /// Preserves required chunk boundaries, such as a reusable prefix's end.
    pub fn admit_with_boundaries(
        &self,
        state: T,
        tokens: usize,
        chunk_tokens: usize,
        reserved_bytes: u64,
        boundaries: &[usize],
    ) -> Result<RequestId, (AdmissionError, T)> {
        let mut inner = lock(&self.inner);
        let error = if tokens == 0
            || chunk_tokens == 0
            || boundaries.iter().any(|&end| end == 0 || end >= tokens)
            || boundaries.windows(2).any(|pair| pair[0] >= pair[1])
        {
            Some(AdmissionError::InvalidLength)
        } else if inner.reservations.len() >= inner.limits.max_requests {
            Some(AdmissionError::Full)
        } else if reserved_bytes > inner.limits.max_reserved_bytes.saturating_sub(inner.bytes) {
            Some(AdmissionError::Memory)
        } else if inner.next_id == u64::MAX {
            Some(AdmissionError::IdExhausted)
        } else {
            None
        };
        if let Some(error) = error {
            return Err((error, state));
        }
        let id = RequestId(inner.next_id);
        inner.next_id += 1;
        inner.bytes += reserved_bytes;
        inner.reservations.insert(
            id,
            Reservation {
                bytes: reserved_bytes,
                cancelled: false,
            },
        );
        inner.ready.push_back(Request {
            id,
            state,
            first: 0,
            tokens,
            chunk_tokens,
            boundaries: boundaries.to_vec(),
        });
        Ok(id)
    }

    /// Takes at most one chunk from each ready request, within the token budget.
    /// New requests can join the next cohort while this one executes.
    pub fn next_batch(&self) -> Vec<Chunk<T>> {
        let mut inner = lock(&self.inner);
        let mut available = inner.limits.max_batch_tokens;
        let mut batch = Vec::new();
        while available > 0 && batch.len() < inner.limits.max_batch_requests {
            let Some(request) = inner.ready.pop_front() else {
                break;
            };
            let end = request
                .boundaries
                .iter()
                .copied()
                .find(|&end| end > request.first)
                .unwrap_or(request.tokens);
            let count = request.chunk_tokens.min(end - request.first).min(available);
            available -= count;
            batch.push(Chunk {
                inner: Arc::clone(&self.inner),
                request: Some(request),
                count,
            });
        }
        batch
    }

    /// Removes a queued request and returns its state for cleanup. An executing request keeps its
    /// reservation until its chunk finishes or drops, so cancellation cannot
    /// admit another request over memory still used by a submitted GPU command.
    pub fn cancel(&self, id: RequestId) -> Option<Completed<T>> {
        let mut inner = lock(&self.inner);
        if let Some(position) = inner.ready.iter().position(|request| request.id == id) {
            let request = inner.ready.remove(position)?;
            return Some(Completed {
                inner: Arc::clone(&self.inner),
                request: Some(request),
            });
        }
        if let Some(reservation) = inner.reservations.get_mut(&id) {
            reservation.cancelled = true;
        }
        None
    }

    pub fn reserved(&self) -> (usize, u64) {
        let inner = lock(&self.inner);
        (inner.reservations.len(), inner.bytes)
    }
}

/// An exclusive request state and the next contiguous token range to execute.
pub(crate) struct Chunk<T> {
    inner: Arc<Mutex<Inner<T>>>,
    request: Option<Request<T>>,
    count: usize,
}

pub(crate) enum Completion<T> {
    Ready,
    Finished(Completed<T>),
    Cancelled(Completed<T>),
}

/// Keeps the request's memory reservation until its head and cleanup finish.
/// Dropping this value releases both the state and its reservation.
pub(crate) struct Completed<T> {
    inner: Arc<Mutex<Inner<T>>>,
    request: Option<Request<T>>,
}

impl<T> Completed<T> {
    pub fn id(&self) -> RequestId {
        self.request.as_ref().expect("completed request").id
    }

    pub fn state_mut(&mut self) -> &mut T {
        &mut self.request.as_mut().expect("completed request").state
    }
}

impl<T> Drop for Completed<T> {
    fn drop(&mut self) {
        if let Some(request) = self.request.take() {
            let id = request.id;
            // Free device buffers before making their reservation available.
            drop(request);
            lock(&self.inner).release(id);
        }
    }
}

impl<T> Chunk<T> {
    pub fn id(&self) -> RequestId {
        self.request.as_ref().expect("live chunk").id
    }

    pub fn range(&self) -> std::ops::Range<usize> {
        let first = self.request.as_ref().expect("live chunk").first;
        first..first + self.count
    }

    pub fn state_mut(&mut self) -> &mut T {
        &mut self.request.as_mut().expect("live chunk").state
    }

    /// Call only after all work using this chunk's state has completed.
    pub fn complete(mut self) -> Completion<T> {
        let mut request = self.request.take().expect("live chunk");
        let mut inner = lock(&self.inner);
        let cancelled = inner
            .reservations
            .get(&request.id)
            .is_some_and(|entry| entry.cancelled);
        request.first += self.count;
        if cancelled || request.first == request.tokens {
            let completed = Completed {
                inner: Arc::clone(&self.inner),
                request: Some(request),
            };
            if cancelled {
                Completion::Cancelled(completed)
            } else {
                Completion::Finished(completed)
            }
        } else {
            inner.ready.push_back(request);
            Completion::Ready
        }
    }
}

impl<T> Drop for Chunk<T> {
    fn drop(&mut self) {
        if let Some(request) = self.request.take() {
            let id = request.id;
            drop(request);
            lock(&self.inner).release(id);
        }
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn limits() -> Limits {
        Limits {
            max_requests: 3,
            max_batch_requests: 2,
            max_batch_tokens: 4,
            max_reserved_bytes: 12,
        }
    }

    #[test]
    fn admission_is_bounded_and_returns_refused_state() {
        let scheduler = ChunkScheduler::new(limits()).unwrap();
        let first = scheduler.admit("first", 8, 2, 7).unwrap();
        assert_eq!(
            scheduler.admit("too large", 1, 1, 6),
            Err((AdmissionError::Memory, "too large"))
        );
        assert_eq!(
            scheduler.admit("overflow", 1, 1, u64::MAX),
            Err((AdmissionError::Memory, "overflow"))
        );
        scheduler.admit("second", 1, 1, 3).unwrap();
        scheduler.admit("third", 1, 1, 2).unwrap();
        assert_eq!(
            scheduler.admit("fourth", 1, 1, 0),
            Err((AdmissionError::Full, "fourth"))
        );
        let mut cancelled = scheduler.cancel(first).expect("queued request");
        assert_eq!(*cancelled.state_mut(), "first");
        drop(cancelled);
        assert_eq!(scheduler.reserved(), (2, 5));
        assert!(scheduler.admit("replacement", 3, 1, 7).is_ok());
    }

    #[test]
    fn cohorts_preserve_request_state_and_admit_new_short_work() {
        let scheduler = ChunkScheduler::new(limits()).unwrap();
        let a = scheduler.admit((10, Vec::new()), 7, 3, 4).unwrap();
        let b = scheduler.admit((100, Vec::new()), 4, 3, 4).unwrap();
        let mut first = scheduler.next_batch();
        assert_eq!(
            first
                .iter()
                .map(|chunk| (chunk.id(), chunk.range()))
                .collect::<Vec<_>>(),
            vec![(a, 0..3), (b, 0..1)]
        );
        let c = scheduler.admit((1000, Vec::new()), 1, 1, 4).unwrap();
        let mut finished = BTreeMap::new();
        let mut first_next = None;
        loop {
            for mut chunk in first.drain(..) {
                let range = chunk.range();
                let (base, rows) = chunk.state_mut();
                for token in range {
                    rows.push(*base + token);
                }
                if let Completion::Finished(mut request) = chunk.complete() {
                    finished.insert(request.id(), request.state_mut().1.clone());
                }
            }
            first = scheduler.next_batch();
            if first.is_empty() {
                break;
            }
            first_next.get_or_insert(first[0].id());
        }
        assert_eq!(first_next, Some(c));
        assert_eq!(finished[&a], (10..17).collect::<Vec<_>>());
        assert_eq!(finished[&b], (100..104).collect::<Vec<_>>());
        assert_eq!(finished[&c], vec![1000]);
        assert_eq!(scheduler.reserved(), (0, 0));
    }

    #[test]
    fn cancellation_keeps_inflight_memory_reserved_until_completion() {
        let scheduler = ChunkScheduler::new(limits()).unwrap();
        let id = scheduler.admit(42, 8, 2, 12).unwrap();
        let chunk = scheduler.next_batch().pop().unwrap();
        assert!(scheduler.cancel(id).is_none());
        assert_eq!(scheduler.reserved(), (1, 12));
        assert_eq!(
            scheduler.admit(99, 1, 1, 1),
            Err((AdmissionError::Memory, 99))
        );
        let Completion::Cancelled(mut completed) = chunk.complete() else {
            panic!("cancelled request was scheduled again");
        };
        assert_eq!(completed.id(), id);
        assert_eq!(*completed.state_mut(), 42);
        assert_eq!(scheduler.reserved(), (1, 12));
        drop(completed);
        assert!(scheduler.next_batch().is_empty());
        assert_eq!(scheduler.reserved(), (0, 0));
    }

    #[test]
    fn dropped_chunk_releases_only_its_own_request() {
        let scheduler = ChunkScheduler::new(limits()).unwrap();
        scheduler.admit("failed", 8, 2, 5).unwrap();
        let other = scheduler.admit("healthy", 2, 2, 7).unwrap();
        let mut batch = scheduler.next_batch();
        let healthy = batch.pop().unwrap();
        drop(batch);
        assert_eq!(scheduler.reserved(), (1, 7));
        let Completion::Finished(mut completed) = healthy.complete() else {
            panic!("healthy request did not finish");
        };
        assert_eq!(completed.id(), other);
        assert_eq!(*completed.state_mut(), "healthy");
        assert_eq!(scheduler.reserved(), (1, 7));
        drop(completed);
        assert_eq!(scheduler.reserved(), (0, 0));
    }

    #[test]
    fn cohorts_preserve_prefix_boundaries_without_skipping_tokens() {
        let scheduler = ChunkScheduler::new(limits()).unwrap();
        scheduler
            .admit_with_boundaries((), 10, 4, 1, &[3, 8])
            .unwrap();
        let mut ranges = Vec::new();
        loop {
            let Some(chunk) = scheduler.next_batch().pop() else {
                break;
            };
            ranges.push(chunk.range());
            drop(chunk.complete());
        }
        assert_eq!(ranges, vec![0..3, 3..7, 7..8, 8..10]);
        assert_eq!(scheduler.reserved(), (0, 0));
        assert!(matches!(
            scheduler.admit_with_boundaries((), 10, 4, 1, &[5, 3]),
            Err((AdmissionError::InvalidLength, ()))
        ));
    }
}
