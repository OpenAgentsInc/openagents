use crate::commands::{Command, parse_command};

#[test]
fn parse_pylon_jobs_commands() {
    assert_eq!(parse_command("/pylon jobs"), Some(Command::PylonJobs));
    assert_eq!(
        parse_command("/pylon jobs refresh"),
        Some(Command::PylonJobsRefresh)
    );
}
