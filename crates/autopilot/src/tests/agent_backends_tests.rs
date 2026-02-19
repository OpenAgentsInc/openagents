use crate::commands::{Command, parse_command};

#[test]
fn parses_agent_backends_commands() {
    assert_eq!(
        parse_command("/agent-backends").unwrap(),
        Command::AgentBackends
    );
    assert_eq!(
        parse_command("/agent-backends refresh").unwrap(),
        Command::AgentBackendsRefresh
    );
    assert_eq!(parse_command("/backends").unwrap(), Command::AgentBackends);
}
