use crate::commands::{Command, parse_command};

#[test]
fn parse_pylon_commands() {
    assert_eq!(parse_command("/pylon"), Some(Command::PylonEarnings));
    assert_eq!(
        parse_command("/pylon earnings"),
        Some(Command::PylonEarnings)
    );
    assert_eq!(
        parse_command("/pylon earnings refresh"),
        Some(Command::PylonEarningsRefresh)
    );
    assert_eq!(
        parse_command("/pylon refresh"),
        Some(Command::PylonEarningsRefresh)
    );
}
