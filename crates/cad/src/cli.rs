use serde::{Deserialize, Serialize};

pub const CAD_CLI_SCAFFOLD_ISSUE_ID: &str = "VCAD-PARITY-083";
pub const CAD_CLI_IMPLEMENTATION_ISSUE_ID: &str = "VCAD-PARITY-084";
pub const CAD_CLI_APP_NAME: &str = "openagents-cad-cli";
pub const CAD_CLI_REFERENCE_COMMAND: &str = "vcad";
pub const CAD_CLI_STUB_EXIT_CODE: i32 = 3;
pub const CAD_CLI_SCAFFOLD_COMMANDS: [&str; 3] = ["export", "import", "info"];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CadCliCommand {
    Export,
    Import,
    Info,
    Help,
}

impl CadCliCommand {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Export => "export",
            Self::Import => "import",
            Self::Info => "info",
            Self::Help => "help",
        }
    }

    fn from_token(token: &str) -> Option<Self> {
        match token {
            "export" => Some(Self::Export),
            "import" => Some(Self::Import),
            "info" => Some(Self::Info),
            "help" | "-h" | "--help" => Some(Self::Help),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CadCliInvocation {
    command: CadCliCommand,
    passthrough: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CadCliRunOutcome {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
}

impl CadCliRunOutcome {
    fn success(stdout: String) -> Self {
        Self {
            exit_code: 0,
            stdout,
            stderr: String::new(),
        }
    }

    fn failure(exit_code: i32, stderr: String) -> Self {
        Self {
            exit_code,
            stdout: String::new(),
            stderr,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum CadCliParseError {
    UnknownCommand(String),
}

pub fn run_cli_env_args(args: Vec<String>) -> CadCliRunOutcome {
    let tokens = args.iter().map(String::as_str).collect::<Vec<_>>();
    run_cli_tokens(&tokens)
}

pub fn run_cli_tokens(tokens: &[&str]) -> CadCliRunOutcome {
    match parse_cli_tokens(tokens) {
        Ok(invocation) => execute_invocation(invocation),
        Err(CadCliParseError::UnknownCommand(command)) => CadCliRunOutcome::failure(
            2,
            format!(
                "unknown command: {command}\n\n{}",
                root_help_text(env!("CARGO_PKG_VERSION"))
            ),
        ),
    }
}

fn parse_cli_tokens(tokens: &[&str]) -> Result<CadCliInvocation, CadCliParseError> {
    let Some(command_token) = tokens.get(1).copied() else {
        return Ok(CadCliInvocation {
            command: CadCliCommand::Help,
            passthrough: Vec::new(),
        });
    };

    let Some(command) = CadCliCommand::from_token(command_token) else {
        return Err(CadCliParseError::UnknownCommand(command_token.to_string()));
    };

    Ok(CadCliInvocation {
        command,
        passthrough: tokens
            .iter()
            .skip(2)
            .map(|token| (*token).to_string())
            .collect(),
    })
}

fn execute_invocation(invocation: CadCliInvocation) -> CadCliRunOutcome {
    if invocation.command == CadCliCommand::Help {
        return CadCliRunOutcome::success(root_help_text(env!("CARGO_PKG_VERSION")));
    }

    if is_help_request(&invocation.passthrough) {
        return CadCliRunOutcome::success(subcommand_help_text(invocation.command));
    }

    CadCliRunOutcome::failure(
        CAD_CLI_STUB_EXIT_CODE,
        scaffold_stub_error(invocation.command),
    )
}

fn is_help_request(args: &[String]) -> bool {
    args.iter()
        .any(|arg| arg == "--help" || arg == "-h" || arg == "help")
}

fn root_help_text(version: &str) -> String {
    format!(
        "{CAD_CLI_APP_NAME} {version}\n\nUSAGE:\n  {CAD_CLI_APP_NAME} <COMMAND> [ARGS]\n  {CAD_CLI_APP_NAME} --help\n\nCOMMANDS:\n  export   Export CAD document to a target format (scaffold)\n  import   Import CAD data into OpenAgents CAD document (scaffold)\n  info     Inspect CAD document metadata and mesh summary (scaffold)\n  help     Print command help\n\nScaffold status: {CAD_CLI_SCAFFOLD_ISSUE_ID} provides command surface only; handlers land in {CAD_CLI_IMPLEMENTATION_ISSUE_ID}."
    )
}

fn subcommand_help_text(command: CadCliCommand) -> String {
    format!(
        "USAGE:\n  {CAD_CLI_APP_NAME} {} [ARGS]\n\nScaffold status: {} command parser is present from {CAD_CLI_SCAFFOLD_ISSUE_ID}; behavior lands in {CAD_CLI_IMPLEMENTATION_ISSUE_ID}.",
        command.as_str(),
        command.as_str(),
    )
}

fn scaffold_stub_error(command: CadCliCommand) -> String {
    format!(
        "{} command scaffold is present; implementation lands in {CAD_CLI_IMPLEMENTATION_ISSUE_ID}",
        command.as_str()
    )
}

#[cfg(test)]
mod tests {
    use super::{CAD_CLI_SCAFFOLD_COMMANDS, CAD_CLI_STUB_EXIT_CODE, run_cli_tokens};

    #[test]
    fn root_help_lists_scaffold_commands() {
        let outcome = run_cli_tokens(&["openagents-cad-cli", "--help"]);
        assert_eq!(outcome.exit_code, 0);
        for command in CAD_CLI_SCAFFOLD_COMMANDS {
            assert!(outcome.stdout.contains(command));
        }
    }

    #[test]
    fn scaffold_commands_return_stub_exit_code() {
        for command in CAD_CLI_SCAFFOLD_COMMANDS {
            let outcome = run_cli_tokens(&["openagents-cad-cli", command]);
            assert_eq!(outcome.exit_code, CAD_CLI_STUB_EXIT_CODE);
            assert!(
                outcome
                    .stderr
                    .contains("implementation lands in VCAD-PARITY-084")
            );
        }
    }

    #[test]
    fn unknown_command_returns_usage_error() {
        let outcome = run_cli_tokens(&["openagents-cad-cli", "unknown"]);
        assert_eq!(outcome.exit_code, 2);
        assert!(outcome.stderr.contains("unknown command: unknown"));
        assert!(outcome.stderr.contains("USAGE:"));
    }
}
