//! The run's trajectory: every Jev request, generation, and command,
//! appended as an ATIF step when it happens.
//!
//! The judge, the generator, and the shell each hold a clone of one
//! [`Recorder`], so the loop itself stays unaware of recording. The run
//! renders the steps with `atif::document::document`, which produces the
//! `ATIF-v1.7` shape Harbor validates.

use std::cell::RefCell;
use std::rc::Rc;

use atif::document::Step;

/// A shared, append-only list of trajectory steps.
#[derive(Clone, Default)]
pub struct Recorder(Rc<RefCell<Vec<Step>>>);

impl Recorder {
    /// Appends one step.
    pub fn push(&self, step: Step) {
        self.0.borrow_mut().push(step);
    }

    /// Every step so far, in order.
    pub fn steps(&self) -> Vec<Step> {
        self.0.borrow().clone()
    }
}
