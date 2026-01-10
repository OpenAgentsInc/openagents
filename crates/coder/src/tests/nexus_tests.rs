use crate::commands::{parse_command, Command};

#[test]
fn parse_nexus_commands() {
    assert_eq!(parse_command("/nexus"), Some(Command::Nexus));
    assert_eq!(
        parse_command("/nexus connect https://nexus.openagents.com/api/stats"),
        Some(Command::NexusConnect(
            "https://nexus.openagents.com/api/stats".to_string()
        ))
    );
    assert_eq!(parse_command("/nexus refresh"), Some(Command::NexusRefresh));
}
