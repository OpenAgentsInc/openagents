//! Animation state definitions.

/// Animation state - represents the current phase of an animation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AnimatorState {
    /// Not visible, animation complete or not started.
    #[default]
    Exited,
    /// Transitioning to visible state.
    Entering,
    /// Fully visible and stable.
    Entered,
    /// Transitioning to invisible state.
    Exiting,
}

impl AnimatorState {
    /// Returns true if the element should be rendered.
    #[inline]
    pub fn is_visible(&self) -> bool {
        !matches!(self, AnimatorState::Exited)
    }

    /// Returns true if currently animating.
    #[inline]
    pub fn is_animating(&self) -> bool {
        matches!(self, AnimatorState::Entering | AnimatorState::Exiting)
    }

    /// Returns true if fully entered.
    #[inline]
    pub fn is_entered(&self) -> bool {
        matches!(self, AnimatorState::Entered)
    }

    /// Returns true if fully exited.
    #[inline]
    pub fn is_exited(&self) -> bool {
        matches!(self, AnimatorState::Exited)
    }
}
