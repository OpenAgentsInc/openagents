use crate::commands::{parse_command, Command};

#[test]
fn parse_lm_router_commands() {
    assert_eq!(parse_command("/lm-router"), Some(Command::LmRouter));
    assert_eq!(
        parse_command("/lm-router refresh"),
        Some(Command::LmRouterRefresh)
    );
    assert_eq!(parse_command("/lmrouter status"), Some(Command::LmRouter));
}
