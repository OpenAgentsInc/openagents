use crate::commands::{parse_command, Command};

#[test]
fn parse_directives_commands() {
    assert_eq!(parse_command("/directives"), Some(Command::Directives));
    assert_eq!(
        parse_command("/directives refresh"),
        Some(Command::DirectivesRefresh)
    );
    assert_eq!(
        parse_command("/directive status"),
        Some(Command::Directives)
    );
}
