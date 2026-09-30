//! The Windows half of the memory cap: the job object the job already runs
//! in holds its tree's committed memory, and its completion port says
//! whether a process ran into the cap. The parent module has the design.

use super::{Enforcement, Memory};
use crate::windows::Tree;

/// Where one job's cap ended up: the job object that holds it.
#[derive(Clone, Debug)]
pub(crate) struct Placed {
    max: u64,
    tree: Tree,
}

impl Placed {
    /// The cap `max` that the job object under `tree` holds.
    pub(crate) fn new(max: u64, tree: &Tree) -> Self {
        Placed {
            max,
            tree: tree.clone(),
        }
    }

    /// The cap as it stood before the job ran, with nothing learned about
    /// how the job ended.
    pub(crate) fn unsettled(&self) -> Memory {
        Memory {
            max: self.max,
            enforcement: Enforcement::JobObject,
            exceeded: false,
            unenforced: None,
        }
    }

    /// Settles the cap once the job's tree is gone: whether a process in it
    /// asked for more than the cap allowed.
    pub(crate) fn settle(self) -> Memory {
        Memory {
            exceeded: self.tree.exceeded(),
            ..self.unsettled()
        }
    }
}
