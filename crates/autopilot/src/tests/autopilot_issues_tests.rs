use crate::commands::{Command, parse_command};

#[test]
fn parse_issue_tracker_commands() {
    assert_eq!(
        parse_command("/issue-tracker"),
        Some(Command::AutopilotIssues)
    );
    assert_eq!(
        parse_command("/issue-tracker refresh"),
        Some(Command::AutopilotIssuesRefresh)
    );
    assert_eq!(
        parse_command("/autopilot-issues status"),
        Some(Command::AutopilotIssues)
    );
    assert_eq!(parse_command("/issue-db"), Some(Command::AutopilotIssues));
}
