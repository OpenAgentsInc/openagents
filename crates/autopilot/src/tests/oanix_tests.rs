use crate::commands::{parse_command, Command};

#[test]
fn parse_oanix_commands() {
    assert_eq!(parse_command("/oanix"), Some(Command::Oanix));
    assert_eq!(parse_command("/oanix status"), Some(Command::Oanix));
    assert_eq!(parse_command("/oanix refresh"), Some(Command::OanixRefresh));
}
