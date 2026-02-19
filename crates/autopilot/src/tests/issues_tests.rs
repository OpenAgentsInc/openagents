use crate::commands::{Command, parse_command};

#[test]
fn parse_issues_commands() {
    assert_eq!(parse_command("/issues"), Some(Command::Issues));
    assert_eq!(parse_command("/issues status"), Some(Command::Issues));
    assert_eq!(
        parse_command("/issues refresh"),
        Some(Command::IssuesRefresh)
    );
}
