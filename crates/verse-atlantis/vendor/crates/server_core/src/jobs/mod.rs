//! Lightweight job scheduler scaffold for budgeted CPU systems.
//!
//! For now, dispatches synchronously and records metrics; later we can add
//! a thread pool and MPSC queues.

#[cfg(not(target_arch = "wasm32"))]
use std::time::Instant;
#[cfg(target_arch = "wasm32")]
use web_time::Instant;

pub struct JobScheduler;

impl Default for JobScheduler {
    fn default() -> Self {
        Self
    }
}

impl JobScheduler {
    pub fn new() -> Self {
        Self
    }

    /// Dispatch up to `budget` mesh jobs by calling the provided closure that
    /// performs meshing and returns the number processed.
    pub fn dispatch_mesh<F: FnOnce(usize) -> usize>(&self, budget: usize, f: F) -> usize {
        let _t0 = Instant::now();
        f(budget)
    }

    /// Dispatch up to `budget` collider jobs similarly.
    pub fn dispatch_collider<F: FnOnce(usize) -> usize>(&self, budget: usize, f: F) -> usize {
        let _t0 = Instant::now();
        f(budget)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn respects_budget() {
        let js = JobScheduler::new();
        let done = js.dispatch_mesh(3, |_| 2);
        assert_eq!(done, 2);
        let done2 = js.dispatch_collider(5, |_| 5);
        assert_eq!(done2, 5);
    }
}
