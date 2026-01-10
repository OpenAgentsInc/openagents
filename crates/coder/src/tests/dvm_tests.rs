use crate::commands::{parse_command, Command};

#[test]
fn parse_dvm_commands() {
    assert_eq!(parse_command("/dvm"), Some(Command::Dvm));
    assert_eq!(
        parse_command("/dvm connect wss://relay.example"),
        Some(Command::DvmConnect("wss://relay.example".to_string()))
    );
    assert_eq!(parse_command("/dvm kind 5050"), Some(Command::DvmKind(5050)));
    assert_eq!(parse_command("/dvm refresh"), Some(Command::DvmRefresh));
}
