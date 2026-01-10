use crate::commands::{parse_command, Command};

#[test]
fn parse_rlm_commands() {
    assert_eq!(parse_command("/rlm"), Some(Command::Rlm));
    assert_eq!(parse_command("/rlm status"), Some(Command::Rlm));
    assert_eq!(parse_command("/rlm refresh"), Some(Command::RlmRefresh));
}
