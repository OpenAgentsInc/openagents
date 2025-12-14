//! Animation manager for orchestrating multiple animators.

use crate::theme::timing;

use super::animator::HudAnimator;
use super::state::AnimatorState;

/// Mode for orchestrating child animations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ManagerMode {
    /// All children animate together simultaneously.
    #[default]
    Parallel,
    /// Children animate one after another with overlap (stagger delay).
    Stagger,
    /// Like Stagger but in reverse order.
    StaggerReverse,
    /// Children animate one at a time, no overlap.
    Sequence,
    /// Like Sequence but in reverse order.
    SequenceReverse,
}

/// Manages multiple child animators with configurable orchestration.
///
/// # Example
///
/// ```ignore
/// let mut manager = AnimatorManager::new(ManagerMode::Stagger)
///     .stagger_offset(5);
///
/// // Add 3 children
/// for _ in 0..3 {
///     manager.add_child(HudAnimator::new());
/// }
///
/// manager.enter();
///
/// // In update loop:
/// manager.tick();
///
/// // Get individual child progress
/// let first_progress = manager.child(0).map(|a| a.progress());
/// ```
pub struct AnimatorManager {
    mode: ManagerMode,
    stagger_offset: u32,
    children: Vec<HudAnimator>,
    /// Frame counter for managing delays.
    frame_counter: u32,
    /// Whether we're currently entering or exiting.
    entering: bool,
}

impl Default for AnimatorManager {
    fn default() -> Self {
        Self::new(ManagerMode::Parallel)
    }
}

impl AnimatorManager {
    /// Create a new manager with the specified mode.
    pub fn new(mode: ManagerMode) -> Self {
        Self {
            mode,
            stagger_offset: timing::STAGGER_OFFSET,
            children: Vec::new(),
            frame_counter: 0,
            entering: true,
        }
    }

    /// Set the stagger offset (frames between children starting).
    pub fn stagger_offset(mut self, frames: u32) -> Self {
        self.stagger_offset = frames;
        self
    }

    /// Add a child animator.
    pub fn add_child(&mut self, animator: HudAnimator) {
        self.children.push(animator);
    }

    /// Get child count.
    pub fn child_count(&self) -> usize {
        self.children.len()
    }

    /// Get child animator at index.
    pub fn child(&self, index: usize) -> Option<&HudAnimator> {
        self.children.get(index)
    }

    /// Get mutable child animator at index.
    pub fn child_mut(&mut self, index: usize) -> Option<&mut HudAnimator> {
        self.children.get_mut(index)
    }

    /// Start entering all children.
    pub fn enter(&mut self) {
        self.entering = true;
        self.frame_counter = 0;

        match self.mode {
            ManagerMode::Parallel => {
                // All start immediately
                for child in &mut self.children {
                    child.enter();
                }
            }
            ManagerMode::Stagger
            | ManagerMode::StaggerReverse
            | ManagerMode::Sequence
            | ManagerMode::SequenceReverse => {
                // Will be handled in tick()
            }
        }
    }

    /// Start exiting all children.
    pub fn exit(&mut self) {
        self.entering = false;
        self.frame_counter = 0;

        match self.mode {
            ManagerMode::Parallel => {
                // All start immediately
                for child in &mut self.children {
                    child.exit();
                }
            }
            ManagerMode::Stagger
            | ManagerMode::StaggerReverse
            | ManagerMode::Sequence
            | ManagerMode::SequenceReverse => {
                // Will be handled in tick()
            }
        }
    }

    /// Tick all children, applying manager timing.
    ///
    /// Returns `true` if any child is still animating.
    pub fn tick(&mut self) -> bool {
        if self.children.is_empty() {
            return false;
        }

        match self.mode {
            ManagerMode::Parallel => self.tick_parallel(),
            ManagerMode::Stagger => self.tick_stagger(false),
            ManagerMode::StaggerReverse => self.tick_stagger(true),
            ManagerMode::Sequence => self.tick_sequence(false),
            ManagerMode::SequenceReverse => self.tick_sequence(true),
        }
    }

    /// Check if all children have finished their animations.
    pub fn is_complete(&self) -> bool {
        self.children.iter().all(|c| !c.is_animating())
    }

    /// Check if any child is currently animating.
    pub fn is_animating(&self) -> bool {
        self.children.iter().any(|c| c.is_animating())
    }

    fn tick_parallel(&mut self) -> bool {
        let mut any_animating = false;
        for child in &mut self.children {
            if child.tick() {
                any_animating = true;
            }
        }
        any_animating
    }

    fn tick_stagger(&mut self, reverse: bool) -> bool {
        let count = self.children.len();
        if count == 0 {
            return false;
        }

        let mut any_animating = false;

        for i in 0..count {
            let order = if reverse { count - 1 - i } else { i };
            let delay = order as u32 * self.stagger_offset;

            let child = &mut self.children[i];

            // Check if this child should start
            if self.entering {
                if child.state() == AnimatorState::Exited && self.frame_counter >= delay {
                    child.enter();
                }
            } else if child.state() == AnimatorState::Entered && self.frame_counter >= delay {
                child.exit();
            }

            if child.tick() {
                any_animating = true;
            }
        }

        self.frame_counter += 1;
        any_animating || self.children.iter().any(|c| c.is_animating())
    }

    fn tick_sequence(&mut self, reverse: bool) -> bool {
        let count = self.children.len();
        if count == 0 {
            return false;
        }

        // Find the current active child
        let indices: Vec<usize> = if reverse {
            (0..count).rev().collect()
        } else {
            (0..count).collect()
        };

        let mut any_animating = false;
        let mut found_active = false;

        for &i in &indices {
            let child = &mut self.children[i];

            if !found_active {
                // Check if this child needs to be activated
                if self.entering {
                    if child.state() == AnimatorState::Exited {
                        child.enter();
                        found_active = true;
                    } else if child.is_animating() {
                        found_active = true;
                    }
                } else if child.state() == AnimatorState::Entered {
                    child.exit();
                    found_active = true;
                } else if child.is_animating() {
                    found_active = true;
                }
            }

            if child.tick() {
                any_animating = true;
            }
        }

        any_animating
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parallel_mode() {
        let mut manager = AnimatorManager::new(ManagerMode::Parallel);
        manager.add_child(HudAnimator::new().enter_duration(5));
        manager.add_child(HudAnimator::new().enter_duration(5));

        manager.enter();

        // Both should be entering
        assert_eq!(manager.child(0).unwrap().state(), AnimatorState::Entering);
        assert_eq!(manager.child(1).unwrap().state(), AnimatorState::Entering);
    }

    #[test]
    fn test_stagger_mode() {
        let mut manager = AnimatorManager::new(ManagerMode::Stagger).stagger_offset(2);

        manager.add_child(HudAnimator::new().enter_duration(5));
        manager.add_child(HudAnimator::new().enter_duration(5));
        manager.add_child(HudAnimator::new().enter_duration(5));

        manager.enter();
        manager.tick(); // frame 0: child 0 starts

        assert_eq!(manager.child(0).unwrap().state(), AnimatorState::Entering);
        assert_eq!(manager.child(1).unwrap().state(), AnimatorState::Exited);
        assert_eq!(manager.child(2).unwrap().state(), AnimatorState::Exited);

        manager.tick(); // frame 1
        manager.tick(); // frame 2: child 1 starts

        assert_eq!(manager.child(1).unwrap().state(), AnimatorState::Entering);
    }

    #[test]
    fn test_child_access() {
        let mut manager = AnimatorManager::new(ManagerMode::Parallel);
        manager.add_child(HudAnimator::new());

        assert!(manager.child(0).is_some());
        assert!(manager.child(1).is_none());
        assert_eq!(manager.child_count(), 1);
    }
}
