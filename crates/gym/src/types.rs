//! Shared types for the Gym module

/// Main tab identifiers for the Gym multi-view system
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GymTab {
    /// Current chat + trajectory viewer (reusable)
    Trajectories,
    /// Terminal-Bench Command Center (4 sub-tabs)
    TBCC,
    /// Real-time HillClimber MAP orchestrator visualization
    HillClimber,
    /// TestGen test generation progress + test list
    TestGen,
    /// Laser-focused regex-log solver
    RegexCrusade,
}

impl Default for GymTab {
    fn default() -> Self {
        Self::Trajectories
    }
}

impl GymTab {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Trajectories => "Trajectories",
            Self::TBCC => "TBCC",
            Self::HillClimber => "HillClimber",
            Self::TestGen => "TestGen",
            Self::RegexCrusade => "Crusade",
        }
    }

    pub fn all() -> &'static [GymTab] {
        &[
            GymTab::Trajectories,
            GymTab::TBCC,
            GymTab::HillClimber,
            GymTab::TestGen,
            GymTab::RegexCrusade,
        ]
    }
}
