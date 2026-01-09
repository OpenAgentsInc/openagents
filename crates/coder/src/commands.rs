#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Help,
    Clear,
    Compact,
    Model,
    Undo,
    Cancel,
    Bug,
    SessionList,
    SessionResume(String),
    SessionFork,
    SessionExport,
    PermissionMode(String),
    PermissionRules,
    PermissionAllow(Vec<String>),
    PermissionDeny(Vec<String>),
    ToolsList,
    ToolsEnable(Vec<String>),
    ToolsDisable(Vec<String>),
    Config,
    OutputStyle(String),
    Custom(String, Vec<String>),
}

#[derive(Debug, Clone, Copy)]
pub struct CommandSpec {
    pub usage: &'static str,
    pub description: &'static str,
    pub requires_args: bool,
}

const COMMAND_SPECS: &[CommandSpec] = &[
    CommandSpec {
        usage: "/help",
        description: "Show available commands",
        requires_args: false,
    },
    CommandSpec {
        usage: "/clear",
        description: "Clear the current conversation",
        requires_args: false,
    },
    CommandSpec {
        usage: "/compact",
        description: "Compact the context window",
        requires_args: false,
    },
    CommandSpec {
        usage: "/model",
        description: "Select a Claude model",
        requires_args: false,
    },
    CommandSpec {
        usage: "/undo",
        description: "Undo the last exchange",
        requires_args: false,
    },
    CommandSpec {
        usage: "/cancel",
        description: "Interrupt the active request",
        requires_args: false,
    },
    CommandSpec {
        usage: "/bug",
        description: "Report a bug",
        requires_args: false,
    },
    CommandSpec {
        usage: "/session list",
        description: "List recent sessions",
        requires_args: false,
    },
    CommandSpec {
        usage: "/session resume <id>",
        description: "Resume a session by id",
        requires_args: true,
    },
    CommandSpec {
        usage: "/session fork",
        description: "Fork the current session",
        requires_args: false,
    },
    CommandSpec {
        usage: "/session export",
        description: "Export the current session to markdown",
        requires_args: false,
    },
    CommandSpec {
        usage: "/permission mode <mode>",
        description: "Set the permission mode",
        requires_args: true,
    },
    CommandSpec {
        usage: "/permission rules",
        description: "Show permission rules",
        requires_args: false,
    },
    CommandSpec {
        usage: "/permission allow <tool>",
        description: "Allow a tool in permission rules",
        requires_args: true,
    },
    CommandSpec {
        usage: "/permission deny <tool>",
        description: "Deny a tool in permission rules",
        requires_args: true,
    },
    CommandSpec {
        usage: "/tools",
        description: "List available tools",
        requires_args: false,
    },
    CommandSpec {
        usage: "/tools enable <tool>",
        description: "Enable one or more tools",
        requires_args: true,
    },
    CommandSpec {
        usage: "/tools disable <tool>",
        description: "Disable one or more tools",
        requires_args: true,
    },
    CommandSpec {
        usage: "/config",
        description: "Open settings",
        requires_args: false,
    },
    CommandSpec {
        usage: "/output-style <style>",
        description: "Set output style",
        requires_args: true,
    },
];

pub fn command_specs() -> &'static [CommandSpec] {
    COMMAND_SPECS
}

pub fn parse_command(input: &str) -> Option<Command> {
    let trimmed = input.trim();
    if !trimmed.starts_with('/') {
        return None;
    }

    let body = trimmed.trim_start_matches('/');
    if body.is_empty() {
        return Some(Command::Help);
    }

    let mut parts = body.split_whitespace();
    let command = parts.next()?.to_ascii_lowercase();
    let args: Vec<String> = parts.map(|part| part.to_string()).collect();

    let parsed = match command.as_str() {
        "help" => Command::Help,
        "clear" => Command::Clear,
        "compact" => Command::Compact,
        "model" => Command::Model,
        "undo" => Command::Undo,
        "cancel" => Command::Cancel,
        "bug" => Command::Bug,
        "session" => parse_session_command(args),
        "permission" => parse_permission_command(args),
        "tools" => parse_tools_command(args),
        "config" => Command::Config,
        "output-style" => parse_output_style_command(args),
        _ => Command::Custom(command, args),
    };

    Some(parsed)
}

fn parse_session_command(args: Vec<String>) -> Command {
    let mut parts = args.into_iter();
    match parts.next().as_deref() {
        Some("list") => Command::SessionList,
        Some("resume") => {
            let id = parts.next().unwrap_or_default();
            Command::SessionResume(id)
        }
        Some("fork") => Command::SessionFork,
        Some("export") => Command::SessionExport,
        Some(other) => Command::Custom(format!("session {}", other), parts.collect()),
        None => Command::Custom("session".to_string(), Vec::new()),
    }
}

fn parse_permission_command(args: Vec<String>) -> Command {
    let mut parts = args.into_iter();
    match parts.next().as_deref() {
        Some("mode") => {
            let mode = parts.next().unwrap_or_default();
            Command::PermissionMode(mode)
        }
        Some("rules") => Command::PermissionRules,
        Some("allow") => Command::PermissionAllow(parts.collect()),
        Some("deny") => Command::PermissionDeny(parts.collect()),
        Some(other) => Command::Custom(format!("permission {}", other), parts.collect()),
        None => Command::Custom("permission".to_string(), Vec::new()),
    }
}

fn parse_tools_command(args: Vec<String>) -> Command {
    let mut parts = args.into_iter();
    match parts.next().as_deref() {
        Some("enable") => Command::ToolsEnable(parts.collect()),
        Some("disable") => Command::ToolsDisable(parts.collect()),
        None => Command::ToolsList,
        Some(other) => Command::Custom(format!("tools {}", other), parts.collect()),
    }
}

fn parse_output_style_command(args: Vec<String>) -> Command {
    if args.is_empty() {
        Command::OutputStyle(String::new())
    } else {
        Command::OutputStyle(args.join(" "))
    }
}
