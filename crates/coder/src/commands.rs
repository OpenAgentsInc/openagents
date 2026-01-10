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
    Mcp,
    McpAdd { name: String, config: String },
    McpRemove(String),
    McpReload,
    McpStatus,
    Agents,
    AgentSelect(String),
    AgentClear,
    AgentReload,
    Skills,
    SkillsReload,
    Hooks,
    HooksReload,
    Wallet,
    WalletRefresh,
    Dspy,
    DspyRefresh,
    DspyAuto(bool),
    DspyBackground(bool),
    Nip28,
    Nip28Connect(String),
    Nip28Channel(String),
    Nip28Send(String),
    Nip28Refresh,
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
    CommandSpec {
        usage: "/mcp",
        description: "Open MCP configuration",
        requires_args: false,
    },
    CommandSpec {
        usage: "/mcp add <name> <json>",
        description: "Add an MCP server (runtime only)",
        requires_args: true,
    },
    CommandSpec {
        usage: "/mcp remove <name>",
        description: "Disable an MCP server",
        requires_args: true,
    },
    CommandSpec {
        usage: "/mcp reload",
        description: "Reload .mcp.json from project root",
        requires_args: false,
    },
    CommandSpec {
        usage: "/mcp status",
        description: "Refresh MCP server status from the SDK",
        requires_args: false,
    },
    CommandSpec {
        usage: "/agents",
        description: "List available agents",
        requires_args: false,
    },
    CommandSpec {
        usage: "/agent <name>",
        description: "Set the active agent",
        requires_args: true,
    },
    CommandSpec {
        usage: "/agent clear",
        description: "Clear the active agent",
        requires_args: false,
    },
    CommandSpec {
        usage: "/agent reload",
        description: "Reload agents from disk",
        requires_args: false,
    },
    CommandSpec {
        usage: "/skills",
        description: "List available skills",
        requires_args: false,
    },
    CommandSpec {
        usage: "/skills reload",
        description: "Reload skills from disk",
        requires_args: false,
    },
    CommandSpec {
        usage: "/hooks",
        description: "Open hook configuration",
        requires_args: false,
    },
    CommandSpec {
        usage: "/hooks reload",
        description: "Reload hook scripts from disk",
        requires_args: false,
    },
    CommandSpec {
        usage: "/wallet",
        description: "Open wallet status",
        requires_args: false,
    },
    CommandSpec {
        usage: "/wallet refresh",
        description: "Refresh wallet status",
        requires_args: false,
    },
    CommandSpec {
        usage: "/dspy",
        description: "Open DSPy status",
        requires_args: false,
    },
    CommandSpec {
        usage: "/dspy refresh",
        description: "Refresh DSPy status",
        requires_args: false,
    },
    CommandSpec {
        usage: "/dspy auto <on|off>",
        description: "Enable or disable DSPy auto-optimizer",
        requires_args: true,
    },
    CommandSpec {
        usage: "/dspy background <on|off>",
        description: "Enable or disable background optimization",
        requires_args: true,
    },
    CommandSpec {
        usage: "/nip28",
        description: "Open NIP-28 chat",
        requires_args: false,
    },
    CommandSpec {
        usage: "/nip28 connect <relay_url>",
        description: "Connect to a Nostr relay for chat",
        requires_args: true,
    },
    CommandSpec {
        usage: "/nip28 channel <id|name>",
        description: "Join or create a chat channel",
        requires_args: true,
    },
    CommandSpec {
        usage: "/nip28 send <message>",
        description: "Send a chat message",
        requires_args: true,
    },
    CommandSpec {
        usage: "/nip28 refresh",
        description: "Reconnect and resubscribe to chat",
        requires_args: false,
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
        "mcp" => parse_mcp_command(args),
        "agents" => Command::Agents,
        "agent" => parse_agent_command(args),
        "skills" => parse_skills_command(args),
        "hooks" => parse_hooks_command(args),
        "wallet" => parse_wallet_command(args),
        "dspy" => parse_dspy_command(args),
        "nip28" => parse_nip28_command(args),
        _ => Command::Custom(command, args),
    };

    Some(parsed)
}

fn parse_wallet_command(args: Vec<String>) -> Command {
    let mut parts = args.into_iter();
    match parts.next().as_deref() {
        Some("refresh") => Command::WalletRefresh,
        Some("status") => Command::Wallet,
        _ => Command::Wallet,
    }
}

fn parse_dspy_command(args: Vec<String>) -> Command {
    let mut parts = args.into_iter();
    match parts.next().as_deref() {
        None => Command::Dspy,
        Some("status") => Command::Dspy,
        Some("refresh") => Command::DspyRefresh,
        Some("auto") => parse_on_off(parts.next().as_deref())
            .map(Command::DspyAuto)
            .unwrap_or(Command::Dspy),
        Some("background") | Some("bg") => parse_on_off(parts.next().as_deref())
            .map(Command::DspyBackground)
            .unwrap_or(Command::Dspy),
        Some(other) => Command::Custom(format!("dspy {}", other), parts.collect()),
    }
}

fn parse_nip28_command(args: Vec<String>) -> Command {
    let mut parts = args.into_iter();
    match parts.next().as_deref() {
        None => Command::Nip28,
        Some("open") => Command::Nip28,
        Some("refresh") => Command::Nip28Refresh,
        Some("connect") => {
            let relay = parts.collect::<Vec<String>>().join(" ");
            Command::Nip28Connect(relay)
        }
        Some("channel") => {
            let channel = parts.collect::<Vec<String>>().join(" ");
            Command::Nip28Channel(channel)
        }
        Some("send") => {
            let message = parts.collect::<Vec<String>>().join(" ");
            Command::Nip28Send(message)
        }
        Some(other) => Command::Custom(format!("nip28 {}", other), parts.collect()),
    }
}

fn parse_on_off(value: Option<&str>) -> Option<bool> {
    let value = value?.to_ascii_lowercase();
    match value.as_str() {
        "on" | "true" | "enable" | "enabled" => Some(true),
        "off" | "false" | "disable" | "disabled" => Some(false),
        _ => None,
    }
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

fn parse_mcp_command(args: Vec<String>) -> Command {
    let mut parts = args.into_iter();
    match parts.next().as_deref() {
        None => Command::Mcp,
        Some("list") => Command::Mcp,
        Some("reload") => Command::McpReload,
        Some("status") => Command::McpStatus,
        Some("remove") => Command::McpRemove(parts.next().unwrap_or_default()),
        Some("add") => {
            let name = parts.next().unwrap_or_default();
            let config = parts.collect::<Vec<String>>().join(" ");
            Command::McpAdd { name, config }
        }
        Some(other) => Command::Custom(format!("mcp {}", other), parts.collect()),
    }
}

fn parse_agent_command(args: Vec<String>) -> Command {
    if args.is_empty() {
        return Command::Agents;
    }

    let mut parts = args.into_iter();
    match parts.next().as_deref() {
        Some("list") => Command::Agents,
        Some("clear") => Command::AgentClear,
        Some("reload") => Command::AgentReload,
        Some(name) => {
            let mut rest = vec![name.to_string()];
            rest.extend(parts);
            Command::AgentSelect(rest.join(" "))
        }
        None => Command::Agents,
    }
}

fn parse_skills_command(args: Vec<String>) -> Command {
    let mut parts = args.into_iter();
    match parts.next().as_deref() {
        None => Command::Skills,
        Some("list") => Command::Skills,
        Some("reload") => Command::SkillsReload,
        Some(other) => Command::Custom(format!("skills {}", other), parts.collect()),
    }
}

fn parse_hooks_command(args: Vec<String>) -> Command {
    let mut parts = args.into_iter();
    match parts.next().as_deref() {
        None => Command::Hooks,
        Some("list") => Command::Hooks,
        Some("reload") => Command::HooksReload,
        Some(other) => Command::Custom(format!("hooks {}", other), parts.collect()),
    }
}
