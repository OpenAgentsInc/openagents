#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Help,
    Clear,
    Compact,
    Model,
    Undo,
    Cancel,
    SessionList,
    SessionResume(String),
    SessionFork,
    PermissionMode(String),
    PermissionRules,
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

pub trait CommandContext {
    fn open_command_palette(&mut self);
    fn open_model_picker(&mut self);
    fn clear_conversation(&mut self);
    fn undo_last_exchange(&mut self);
    fn interrupt_query(&mut self);
    fn push_system_message(&mut self, message: String);
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
        Some(other) => Command::Custom(format!("permission {}", other), parts.collect()),
        None => Command::Custom("permission".to_string(), Vec::new()),
    }
}

fn parse_tools_command(args: Vec<String>) -> Command {
    let mut parts = args.into_iter();
    match parts.next().as_deref() {
        Some("enable") => Command::ToolsEnable(parts.collect()),
        Some("disable") => Command::ToolsDisable(parts.collect()),
        Some(other) => Command::Custom(format!("tools {}", other), parts.collect()),
        None => Command::Custom("tools".to_string(), Vec::new()),
    }
}

fn parse_output_style_command(args: Vec<String>) -> Command {
    if args.is_empty() {
        Command::OutputStyle(String::new())
    } else {
        Command::OutputStyle(args.join(" "))
    }
}

pub fn execute_command(cmd: Command, ctx: &mut impl CommandContext) {
    match cmd {
        Command::Help => ctx.open_command_palette(),
        Command::Clear => ctx.clear_conversation(),
        Command::Compact => ctx.push_system_message("Compact is not available yet.".to_string()),
        Command::Model => ctx.open_model_picker(),
        Command::Undo => ctx.undo_last_exchange(),
        Command::Cancel => ctx.interrupt_query(),
        Command::SessionList => {
            ctx.push_system_message("Session list is not available yet.".to_string())
        }
        Command::SessionResume(id) => ctx.push_system_message(format!(
            "Session resume is not available yet (id: {}).",
            id
        )),
        Command::SessionFork => {
            ctx.push_system_message("Session fork is not available yet.".to_string())
        }
        Command::PermissionMode(mode) => ctx.push_system_message(format!(
            "Permission mode is not available yet (mode: {}).",
            mode
        )),
        Command::PermissionRules => {
            ctx.push_system_message("Permission rules are not available yet.".to_string())
        }
        Command::ToolsEnable(tools) => ctx.push_system_message(format!(
            "Tool enable is not available yet (tools: {}).",
            tools.join(", ")
        )),
        Command::ToolsDisable(tools) => ctx.push_system_message(format!(
            "Tool disable is not available yet (tools: {}).",
            tools.join(", ")
        )),
        Command::Config => ctx.push_system_message("Config is not available yet.".to_string()),
        Command::OutputStyle(style) => ctx.push_system_message(format!(
            "Output style is not available yet (style: {}).",
            style
        )),
        Command::Custom(name, args) => {
            let mut message = format!("Unknown command: /{}", name);
            if !args.is_empty() {
                message.push(' ');
                message.push_str(&args.join(" "));
            }
            ctx.push_system_message(message);
        }
    }
}
