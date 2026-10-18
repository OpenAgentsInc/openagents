//! Revision-bound bridge for the Android semantic view adapter.
//!
//! The shell mounts validated JSON on its UI thread, then acknowledges it.
//! Application controllers still authorize and execute every resolved intent.

use rust_native::view::{Activation, FieldChange, ValidatedView, ViewError};
use serde::Serialize;

/// One Android surface lifetime. Disposal is terminal; remount with a new bridge.
#[derive(Default)]
pub struct AndroidView<I> {
    applied: Option<ValidatedView<I>>,
    pending: Option<ValidatedView<I>>,
    disposed: bool,
}

impl<I> AndroidView<I> {
    pub fn new() -> Self {
        Self {
            applied: None,
            pending: None,
            disposed: false,
        }
    }

    /// Call only after the shell has applied this exact instance and revision.
    /// A late completion cannot replace a newer prepared view.
    pub fn acknowledge(&mut self, instance: &str, revision: u64) -> Result<(), ViewError> {
        let pending = self.pending.as_ref().ok_or(ViewError::StaleActivation)?;
        if self.disposed
            || pending.view().instance != instance
            || pending.view().revision != revision
        {
            return Err(ViewError::StaleActivation);
        }
        self.applied = self.pending.take();
        Ok(())
    }

    pub fn activate(&self, event: &Activation) -> Result<&I, ViewError> {
        self.current()?.activate(event)
    }

    pub fn change_field(&self, event: &FieldChange) -> Result<&I, ViewError> {
        self.current()?.change_field(event)
    }

    fn current(&self) -> Result<&ValidatedView<I>, ViewError> {
        if self.disposed {
            return Err(ViewError::StaleActivation);
        }
        self.applied.as_ref().ok_or(ViewError::StaleActivation)
    }

    /// Release presentation data without cancelling any application work.
    pub fn dispose(&mut self) {
        self.disposed = true;
        self.applied = None;
        self.pending = None;
    }
}

impl<I: Serialize> AndroidView<I> {
    /// Prepare bytes without claiming that Android has displayed them.
    pub fn prepare(&mut self, view: ValidatedView<I>) -> Result<Vec<u8>, ViewError> {
        if self.disposed {
            return Err(ViewError::StaleActivation);
        }
        for previous in [self.pending.as_ref(), self.applied.as_ref()]
            .into_iter()
            .flatten()
        {
            if previous.view().instance != view.view().instance
                || previous.view().revision >= view.view().revision
            {
                return Err(ViewError::StaleActivation);
            }
        }
        let json = view.to_json()?;
        self.pending = Some(view);
        Ok(json)
    }
}
