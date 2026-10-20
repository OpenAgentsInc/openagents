//! A non-queuing generation slot. Move its permit into the worker command:
//! dropping an HTTP caller must not release work still owned by the decoder.

use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

#[derive(Clone, Default)]
pub(super) struct GenerationAdmission {
    busy: Arc<AtomicBool>,
}

impl GenerationAdmission {
    pub(super) fn is_busy(&self) -> bool {
        self.busy.load(Ordering::Acquire)
    }

    pub(super) fn try_acquire(&self) -> Option<GenerationPermit> {
        self.busy
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .ok()
            .map(|_| GenerationPermit(Arc::clone(&self.busy)))
    }
}

pub(super) struct GenerationPermit(Arc<AtomicBool>);

impl Drop for GenerationPermit {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Barrier, atomic::AtomicUsize};

    #[test]
    fn single_generation_competing_callers_share_one_slot() {
        let admission = GenerationAdmission::default();
        let barrier = Arc::new(Barrier::new(9));
        let winners = Arc::new(AtomicUsize::new(0));
        std::thread::scope(|scope| {
            for _ in 0..8 {
                let admission = admission.clone();
                let barrier = Arc::clone(&barrier);
                let winners = Arc::clone(&winners);
                scope.spawn(move || {
                    let permit = admission.try_acquire();
                    if permit.is_some() {
                        winners.fetch_add(1, Ordering::Relaxed);
                    }
                    barrier.wait();
                    drop(permit);
                });
            }
            barrier.wait();
        });
        assert_eq!(winners.load(Ordering::Relaxed), 1);
        assert!(admission.try_acquire().is_some());
    }
}
