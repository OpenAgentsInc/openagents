//! Per-request memory views published only after their consuming batch succeeds.

/// Completed normalized memory views, indexed by evidence layer.
pub(super) struct MemoryViews<T> {
    cached: Vec<Option<T>>,
}

/// A newly allocated view that has not yet completed its consuming batch.
pub(super) struct PendingMemoryView<T> {
    pub(super) layer: usize,
    pub(super) buffer: T,
    pub(super) byte_len: usize,
}

impl<T> MemoryViews<T> {
    /// Creates an empty cache for one request without requiring `T: Clone`.
    pub(super) fn new(layers: usize) -> Self {
        Self {
            cached: (0..layers).map(|_| None).collect(),
        }
    }

    /// Allocates a missing view without publishing it to subsequent batches.
    pub(super) fn prepare(
        &self,
        evidence: Option<usize>,
        create: impl FnOnce(usize) -> Result<(T, usize), String>,
    ) -> Result<Option<PendingMemoryView<T>>, String> {
        let Some(layer) = evidence else {
            return Ok(None);
        };
        let cached = self
            .cached
            .get(layer)
            .ok_or_else(|| format!("memory view: evidence layer {layer} is out of range"))?;
        if cached.is_some() {
            return Ok(None);
        }
        let (buffer, byte_len) = create(layer)?;
        Ok(Some(PendingMemoryView {
            layer,
            buffer,
            byte_len,
        }))
    }

    /// Selects the raw rows, a pending view for this batch, or a completed view.
    pub(super) fn select<'a>(
        &'a self,
        raw: &'a T,
        evidence: Option<usize>,
        pending: Option<&'a PendingMemoryView<T>>,
    ) -> Result<&'a T, String> {
        let Some(layer) = evidence else {
            if pending.is_some() {
                return Err(String::from(
                    "memory view: raw rows cannot use a pending view",
                ));
            }
            return Ok(raw);
        };
        let cached = self
            .cached
            .get(layer)
            .ok_or_else(|| format!("memory view: evidence layer {layer} is out of range"))?;
        if let Some(pending) = pending {
            if pending.layer != layer {
                return Err(format!(
                    "memory view: pending layer {} does not match evidence layer {layer}",
                    pending.layer
                ));
            }
            return Ok(&pending.buffer);
        }
        cached
            .as_ref()
            .ok_or_else(|| format!("memory view: evidence layer {layer} has not completed"))
    }

    /// Publishes a pending view after successful completion and returns its bytes.
    /// Existing entries are never replaced or counted again.
    pub(super) fn complete(
        &mut self,
        pending: Option<PendingMemoryView<T>>,
        completion: Result<(), String>,
    ) -> Result<usize, String> {
        completion?;
        let Some(pending) = pending else {
            return Ok(0);
        };
        let cached = self.cached.get_mut(pending.layer).ok_or_else(|| {
            format!(
                "memory view: evidence layer {} is out of range",
                pending.layer
            )
        })?;
        if cached.is_some() {
            return Ok(0);
        }
        *cached = Some(pending.buffer);
        Ok(pending.byte_len)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::{Cell, RefCell};
    use std::rc::Rc;

    struct DropSpy {
        id: usize,
        drops: Rc<RefCell<Vec<usize>>>,
    }

    impl DropSpy {
        fn new(id: usize, drops: &Rc<RefCell<Vec<usize>>>) -> Self {
            Self {
                id,
                drops: Rc::clone(drops),
            }
        }
    }

    impl Drop for DropSpy {
        fn drop(&mut self) {
            self.drops.borrow_mut().push(self.id);
        }
    }

    #[test]
    fn raw_and_warm_reads_do_not_allocate_and_cold_view_stays_private() {
        let mut views = MemoryViews::new(2);
        let raw = String::from("raw");
        let allocations = Cell::new(0);
        let create = |layer| {
            allocations.set(allocations.get() + 1);
            Ok((format!("normalized {layer}"), 64))
        };
        assert!(views.prepare(None, create).unwrap().is_none());
        assert!(std::ptr::eq(views.select(&raw, None, None).unwrap(), &raw));
        assert_eq!(allocations.get(), 0);

        let pending = views.prepare(Some(1), create).unwrap().unwrap();
        assert_eq!(allocations.get(), 1);
        assert_eq!(pending.layer, 1);
        assert_eq!(pending.byte_len, 64);
        assert!(views.select(&raw, Some(1), None).is_err());
        assert!(std::ptr::eq(
            views.select(&raw, Some(1), Some(&pending)).unwrap(),
            &pending.buffer,
        ));
        assert_eq!(views.complete(Some(pending), Ok(())).unwrap(), 64);
        assert_eq!(views.select(&raw, Some(1), None).unwrap(), "normalized 1");
        assert!(views.prepare(Some(1), create).unwrap().is_none());
        assert_eq!(allocations.get(), 1);
        assert_eq!(views.complete(None, Ok(())).unwrap(), 0);
    }

    #[test]
    fn invalid_evidence_is_rejected_before_the_factory_runs() {
        let views = MemoryViews::<usize>::new(2);
        let allocated = Cell::new(false);
        for layer in [2, usize::MAX] {
            assert!(
                views
                    .prepare(Some(layer), |_| {
                        allocated.set(true);
                        Ok((1, 8))
                    })
                    .is_err()
            );
            assert!(views.select(&0, Some(layer), None).is_err());
        }
        assert!(!allocated.get());
        let empty = MemoryViews::<usize>::new(0);
        assert_eq!(*empty.select(&7, None, None).unwrap(), 7);
        assert!(empty.prepare(Some(0), |_| Ok((1, 8))).is_err());
    }

    #[test]
    fn factory_error_leaves_the_entry_available_for_retry() {
        let mut views = MemoryViews::<usize>::new(1);
        let failure = views.prepare(Some(0), |_| Err(String::from("allocation failed")));
        assert!(matches!(failure, Err(error) if error == "allocation failed"));
        assert!(views.select(&0, Some(0), None).is_err());
        let retry = views.prepare(Some(0), |_| Ok((42, 16))).unwrap();
        assert_eq!(views.complete(retry, Ok(())).unwrap(), 16);
        assert_eq!(*views.select(&0, Some(0), None).unwrap(), 42);
    }

    #[test]
    fn dropping_pending_after_encoding_failure_does_not_publish_it() {
        let views = MemoryViews::new(1);
        let drops = Rc::new(RefCell::new(Vec::new()));
        let raw = DropSpy::new(0, &drops);
        let pending = views
            .prepare(Some(0), |_| Ok((DropSpy::new(1, &drops), 32)))
            .unwrap();
        drop(pending);
        assert_eq!(&*drops.borrow(), &[1]);
        assert!(views.select(&raw, Some(0), None).is_err());
        assert!(
            views
                .prepare(Some(0), |_| Ok((DropSpy::new(2, &drops), 32)))
                .unwrap()
                .is_some()
        );
    }

    #[test]
    fn failed_completion_drops_pending_and_preserves_completed_views() {
        let mut views = MemoryViews::new(2);
        let drops = Rc::new(RefCell::new(Vec::new()));
        let raw = DropSpy::new(0, &drops);
        let first = views
            .prepare(Some(0), |_| Ok((DropSpy::new(1, &drops), 32)))
            .unwrap();
        assert_eq!(views.complete(first, Ok(())).unwrap(), 32);
        let second = views
            .prepare(Some(1), |_| Ok((DropSpy::new(2, &drops), 64)))
            .unwrap();
        assert_eq!(
            views.complete(second, Err(String::from("command failed"))),
            Err(String::from("command failed"))
        );
        assert_eq!(&*drops.borrow(), &[2]);
        assert_eq!(views.select(&raw, Some(0), None).unwrap().id, 1);
        assert!(views.select(&raw, Some(1), None).is_err());
        assert_eq!(
            views.complete(None, Err(String::from("warm command failed"))),
            Err(String::from("warm command failed"))
        );
        assert_eq!(views.select(&raw, Some(0), None).unwrap().id, 1);
        assert_eq!(&*drops.borrow(), &[2]);
    }

    #[test]
    fn successful_completion_counts_bytes_once_and_never_replaces_a_view() {
        let mut views = MemoryViews::new(1);
        let drops = Rc::new(RefCell::new(Vec::new()));
        let raw = DropSpy::new(0, &drops);
        let first = views
            .prepare(Some(0), |_| Ok((DropSpy::new(1, &drops), 32)))
            .unwrap();
        let duplicate = views
            .prepare(Some(0), |_| Ok((DropSpy::new(2, &drops), 64)))
            .unwrap();
        let mut bytes = views.complete(first, Ok(())).unwrap();
        bytes += views.complete(duplicate, Ok(())).unwrap();
        bytes += views.complete(None, Ok(())).unwrap();
        assert_eq!(bytes, 32);
        assert_eq!(views.select(&raw, Some(0), None).unwrap().id, 1);
        assert_eq!(&*drops.borrow(), &[2]);
    }

    #[test]
    fn selection_rejects_a_pending_view_for_a_different_layer_or_raw_rows() {
        let views = MemoryViews::new(2);
        let pending = views.prepare(Some(0), |_| Ok((42, 16))).unwrap().unwrap();
        assert!(views.select(&0, Some(1), Some(&pending)).is_err());
        assert!(views.select(&0, None, Some(&pending)).is_err());
        assert!(views.select(&0, Some(2), Some(&pending)).is_err());
    }

    #[test]
    fn a_new_request_starts_empty_and_keeps_its_own_views() {
        let drops = Rc::new(RefCell::new(Vec::new()));
        let raw = DropSpy::new(0, &drops);
        let mut previous = MemoryViews::new(1);
        let old = previous
            .prepare(Some(0), |_| Ok((DropSpy::new(1, &drops), 32)))
            .unwrap();
        previous.complete(old, Ok(())).unwrap();
        let mut next = MemoryViews::new(1);
        assert!(next.select(&raw, Some(0), None).is_err());
        let new = next
            .prepare(Some(0), |_| Ok((DropSpy::new(2, &drops), 64)))
            .unwrap();
        assert_eq!(next.complete(new, Ok(())).unwrap(), 64);
        assert_eq!(next.select(&raw, Some(0), None).unwrap().id, 2);
        assert_eq!(previous.select(&raw, Some(0), None).unwrap().id, 1);
        drop(previous);
        assert_eq!(&*drops.borrow(), &[1]);
        assert_eq!(next.select(&raw, Some(0), None).unwrap().id, 2);
        drop(next);
        assert_eq!(&*drops.borrow(), &[1, 2]);
    }
}
