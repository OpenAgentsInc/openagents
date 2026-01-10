use crate::commands::{parse_command, Command};

#[test]
fn parse_gateway_commands() {
    assert_eq!(parse_command("/gateway"), Some(Command::Gateway));
    assert_eq!(parse_command("/gateway refresh"), Some(Command::GatewayRefresh));
    assert_eq!(parse_command("/gateway status"), Some(Command::Gateway));
}
