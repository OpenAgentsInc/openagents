use adjutant::dspy::SessionIndex;

use crate::app::dspy::DspySessionSummary;
use crate::commands::{parse_command, Command};

#[test]
fn parse_dspy_commands() {
    assert_eq!(parse_command("/dspy"), Some(Command::Dspy));
    assert_eq!(parse_command("/dspy refresh"), Some(Command::DspyRefresh));
    assert_eq!(parse_command("/dspy auto on"), Some(Command::DspyAuto(true)));
    assert_eq!(parse_command("/dspy auto off"), Some(Command::DspyAuto(false)));
    assert_eq!(
        parse_command("/dspy background off"),
        Some(Command::DspyBackground(false))
    );
}

#[test]
fn dspy_session_summary_tracks_counts() {
    let mut index = SessionIndex::new();
    index.total_sessions = 4;
    index.success_count = 3;
    index.failed_count = 1;
    index.interrupted_count = 0;

    let summary = DspySessionSummary::from_index(&index);
    assert_eq!(summary.total_sessions, 4);
    assert_eq!(summary.success_count, 3);
    assert_eq!(summary.failed_count, 1);
    assert!((summary.success_rate - 0.75).abs() < f32::EPSILON);
}
