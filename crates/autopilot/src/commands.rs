#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Help,
    Clear,
    Compact,
    Model,
    Backend,            // Toggle between backends
    BackendSet(String), // Set specific backend
    Undo,
    Cancel,
    Bug,
    SessionList,
    SessionResume(String),
    SessionFork,
    SessionExport,
    WorkspaceList,
    WorkspaceAdd,
    WorkspaceConnect(String),
    WorkspaceRefresh,
    Review(ReviewCommand),
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
    McpLogin(String),
    AccountStatus,
    AccountLoginApiKey(String),
    AccountLoginChatgpt,
    AccountLoginCancel(String),
    AccountLogout,
    AccountRateLimits,
    Agents,
    AgentSelect(String),
    AgentClear,
    AgentReload,
    AgentBackends,
    AgentBackendsRefresh,
    Skills,
    SkillsReload,
    Hooks,
    HooksReload,
    Wallet,
    WalletRefresh,
    Dvm,
    DvmConnect(String),
    DvmKind(u16),
    DvmRefresh,
    LmRouter,
    LmRouterRefresh,
    Nexus,
    NexusConnect(String),
    NexusRefresh,
    SparkWallet,
    SparkWalletRefresh,
    Gateway,
    GatewayRefresh,
    Nip90,
    Nip90Connect(String),
    Nip90Refresh,
    Oanix,
    OanixRefresh,
    Directives,
    DirectivesRefresh,
    Issues,
    IssuesRefresh,
    AutopilotIssues,
    AutopilotIssuesRefresh,
    Rlm,
    RlmRefresh,
    RlmTrace(Option<String>),
    PylonEarnings,
    PylonEarningsRefresh,
    PylonJobs,
    PylonJobsRefresh,
    Dspy,
    DspyRefresh,
    DspyAuto(bool),
    DspyBackground(bool),
    ChainViz(String),
    Nip28,
    Nip28Connect(String),
    Nip28Channel(String),
    Nip28Send(String),
    Nip28Refresh,
    Custom(String, Vec<String>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewCommand {
    pub delivery: ReviewDelivery,
    pub target: ReviewTarget,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewDelivery {
    Inline,
    Detached,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReviewTarget {
    UncommittedChanges,
    BaseBranch { branch: String },
    Commit { sha: String, title: Option<String> },
    Custom { instructions: String },
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
        description: "Select a model",
        requires_args: false,
    },
    CommandSpec {
        usage: "/backend",
        description: "Toggle between backends",
        requires_args: false,
    },
    CommandSpec {
        usage: "/backend codex",
        description: "Switch to a specific backend",
        requires_args: true,
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
        usage: "/workspace list",
        description: "List saved workspaces",
        requires_args: false,
    },
    CommandSpec {
        usage: "/workspace add",
        description: "Add a workspace using the folder picker",
        requires_args: false,
    },
    CommandSpec {
        usage: "/workspace connect <id|name>",
        description: "Connect to a workspace by id or name",
        requires_args: true,
    },
    CommandSpec {
        usage: "/workspace refresh",
        description: "Reload workspace list and refresh threads",
        requires_args: false,
    },
    CommandSpec {
        usage: "/review",
        description: "Review uncommitted changes",
        requires_args: false,
    },
    CommandSpec {
        usage: "/review commit <sha> [title]",
        description: "Review a specific commit",
        requires_args: true,
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
        usage: "/account",
        description: "Show auth status",
        requires_args: false,
    },
    CommandSpec {
        usage: "/account login apikey <key>",
        description: "Login with an API key",
        requires_args: true,
    },
    CommandSpec {
        usage: "/account login chatgpt",
        description: "Login with ChatGPT",
        requires_args: false,
    },
    CommandSpec {
        usage: "/account login cancel <login_id>",
        description: "Cancel a pending ChatGPT login",
        requires_args: true,
    },
    CommandSpec {
        usage: "/account logout",
        description: "Logout of the current account",
        requires_args: false,
    },
    CommandSpec {
        usage: "/account rate-limits",
        description: "Refresh ChatGPT rate limits",
        requires_args: false,
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
        usage: "/mcp login <name>",
        description: "Login to an MCP server via OAuth",
        requires_args: true,
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
        usage: "/agent-backends",
        description: "Open agent backend status",
        requires_args: false,
    },
    CommandSpec {
        usage: "/agent-backends refresh",
        description: "Refresh agent backend status",
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
        usage: "/dvm",
        description: "Open DVM providers",
        requires_args: false,
    },
    CommandSpec {
        usage: "/dvm connect <relay_url>",
        description: "Connect DVM discovery to a relay",
        requires_args: true,
    },
    CommandSpec {
        usage: "/dvm kind <id>",
        description: "Set the job kind to discover",
        requires_args: true,
    },
    CommandSpec {
        usage: "/dvm refresh",
        description: "Refresh DVM provider list",
        requires_args: false,
    },
    CommandSpec {
        usage: "/gateway",
        description: "Open gateway health",
        requires_args: false,
    },
    CommandSpec {
        usage: "/gateway refresh",
        description: "Refresh gateway health",
        requires_args: false,
    },
    CommandSpec {
        usage: "/lm-router",
        description: "Open LM router status",
        requires_args: false,
    },
    CommandSpec {
        usage: "/lm-router refresh",
        description: "Refresh LM router status",
        requires_args: false,
    },
    CommandSpec {
        usage: "/nexus",
        description: "Open Nexus relay stats",
        requires_args: false,
    },
    CommandSpec {
        usage: "/nexus connect <stats_url>",
        description: "Set the Nexus stats URL",
        requires_args: true,
    },
    CommandSpec {
        usage: "/nexus refresh",
        description: "Refresh Nexus relay stats",
        requires_args: false,
    },
    CommandSpec {
        usage: "/spark",
        description: "Open Spark wallet status",
        requires_args: false,
    },
    CommandSpec {
        usage: "/spark refresh",
        description: "Refresh Spark wallet status",
        requires_args: false,
    },
    CommandSpec {
        usage: "/nip90",
        description: "Open NIP-90 job monitor",
        requires_args: false,
    },
    CommandSpec {
        usage: "/nip90 connect <relay_url>",
        description: "Connect NIP-90 monitor to a relay",
        requires_args: true,
    },
    CommandSpec {
        usage: "/nip90 refresh",
        description: "Reconnect NIP-90 monitor",
        requires_args: false,
    },
    CommandSpec {
        usage: "/oanix",
        description: "Open OANIX manifest",
        requires_args: false,
    },
    CommandSpec {
        usage: "/oanix refresh",
        description: "Refresh OANIX manifest",
        requires_args: false,
    },
    CommandSpec {
        usage: "/directives",
        description: "Open workspace directives",
        requires_args: false,
    },
    CommandSpec {
        usage: "/directives refresh",
        description: "Refresh workspace directives",
        requires_args: false,
    },
    CommandSpec {
        usage: "/issue-tracker",
        description: "Open autopilot issue tracker",
        requires_args: false,
    },
    CommandSpec {
        usage: "/issue-tracker refresh",
        description: "Refresh autopilot issue tracker",
        requires_args: false,
    },
    CommandSpec {
        usage: "/rlm",
        description: "Open RLM run history",
        requires_args: false,
    },
    CommandSpec {
        usage: "/rlm refresh",
        description: "Refresh RLM run history",
        requires_args: false,
    },
    CommandSpec {
        usage: "/rlm trace [run_id]",
        description: "Open RLM trace events",
        requires_args: false,
    },
    CommandSpec {
        usage: "/pylon",
        description: "Open Pylon earnings",
        requires_args: false,
    },
    CommandSpec {
        usage: "/pylon earnings",
        description: "Open Pylon earnings",
        requires_args: false,
    },
    CommandSpec {
        usage: "/pylon earnings refresh",
        description: "Refresh Pylon earnings",
        requires_args: false,
    },
    CommandSpec {
        usage: "/pylon refresh",
        description: "Refresh Pylon earnings",
        requires_args: false,
    },
    CommandSpec {
        usage: "/pylon jobs",
        description: "Open Pylon jobs",
        requires_args: false,
    },
    CommandSpec {
        usage: "/pylon jobs refresh",
        description: "Refresh Pylon jobs",
        requires_args: false,
    },
    CommandSpec {
        usage: "/issues",
        description: "Open workspace issues",
        requires_args: false,
    },
    CommandSpec {
        usage: "/issues refresh",
        description: "Refresh workspace issues",
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
        usage: "/chainviz <prompt>",
        description: "Run the DSPy chain visualizer on a prompt",
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
        "backend" => parse_backend_command(args),
        "undo" => Command::Undo,
        "cancel" => Command::Cancel,
        "bug" => Command::Bug,
        "session" => parse_session_command(args),
        "workspace" | "workspaces" => parse_workspace_command(args),
        "review" => parse_review_command(args),
        "permission" => parse_permission_command(args),
        "tools" => parse_tools_command(args),
        "config" => Command::Config,
        "output-style" => parse_output_style_command(args),
        "account" => parse_account_command(args),
        "mcp" => parse_mcp_command(args),
        "agents" => Command::Agents,
        "agent" => parse_agent_command(args),
        "agent-backends" | "agent-backend" | "backends" => parse_agent_backends_command(args),
        "skills" => parse_skills_command(args),
        "hooks" => parse_hooks_command(args),
        "wallet" => parse_wallet_command(args),
        "dvm" => parse_dvm_command(args),
        "gateway" => parse_gateway_command(args),
        "lm-router" | "lmrouter" => parse_lm_router_command(args),
        "nexus" => parse_nexus_command(args),
        "spark" => parse_spark_command(args),
        "nip90" => parse_nip90_command(args),
        "oanix" => parse_oanix_command(args),
        "directives" | "directive" => parse_directives_command(args),
        "issue-tracker" | "autopilot-issues" | "issue-db" => parse_issue_tracker_command(args),
        "rlm" => parse_rlm_command(args),
        "pylon" => parse_pylon_command(args),
        "issues" => parse_issues_command(args),
        "dspy" => parse_dspy_command(args),
        "chainviz" => parse_chainviz_command(args),
        "nip28" => parse_nip28_command(args),
        _ => Command::Custom(command, args),
    };

    Some(parsed)
}

fn parse_chainviz_command(args: Vec<String>) -> Command {
    let prompt = args.join(" ");
    Command::ChainViz(prompt)
}

fn parse_backend_command(args: Vec<String>) -> Command {
    let mut parts = args.into_iter();
    match parts.next().as_deref() {
        None => Command::Backend, // Toggle
        Some("toggle") => Command::Backend,
        Some(name) => Command::BackendSet(name.to_string()),
    }
}

fn parse_wallet_command(args: Vec<String>) -> Command {
    let mut parts = args.into_iter();
    match parts.next().as_deref() {
        Some("refresh") => Command::WalletRefresh,
        Some("status") => Command::Wallet,
        _ => Command::Wallet,
    }
}

fn parse_dvm_command(args: Vec<String>) -> Command {
    let mut parts = args.into_iter();
    match parts.next().as_deref() {
        None => Command::Dvm,
        Some("open") => Command::Dvm,
        Some("refresh") => Command::DvmRefresh,
        Some("connect") => {
            let relay = parts.collect::<Vec<String>>().join(" ");
            Command::DvmConnect(relay)
        }
        Some("kind") => {
            let value = parts.next().unwrap_or_default();
            value
                .parse::<u16>()
                .map(Command::DvmKind)
                .unwrap_or_else(|_| Command::Dvm)
        }
        Some(other) => Command::Custom(format!("dvm {}", other), parts.collect()),
    }
}

fn parse_gateway_command(args: Vec<String>) -> Command {
    let mut parts = args.into_iter();
    match parts.next().as_deref() {
        None => Command::Gateway,
        Some("open") | Some("status") => Command::Gateway,
        Some("refresh") => Command::GatewayRefresh,
        Some(other) => Command::Custom(format!("gateway {}", other), parts.collect()),
    }
}

fn parse_lm_router_command(args: Vec<String>) -> Command {
    let mut parts = args.into_iter();
    match parts.next().as_deref() {
        None => Command::LmRouter,
        Some("open") | Some("status") => Command::LmRouter,
        Some("refresh") => Command::LmRouterRefresh,
        Some(other) => Command::Custom(format!("lm-router {}", other), parts.collect()),
    }
}

fn parse_nexus_command(args: Vec<String>) -> Command {
    let mut parts = args.into_iter();
    match parts.next().as_deref() {
        None => Command::Nexus,
        Some("open") | Some("status") => Command::Nexus,
        Some("refresh") => Command::NexusRefresh,
        Some("connect") => {
            let url = parts.collect::<Vec<String>>().join(" ");
            Command::NexusConnect(url)
        }
        Some(other) => Command::Custom(format!("nexus {}", other), parts.collect()),
    }
}

fn parse_spark_command(args: Vec<String>) -> Command {
    let mut parts = args.into_iter();
    match parts.next().as_deref() {
        None => Command::SparkWallet,
        Some("open") | Some("status") => Command::SparkWallet,
        Some("refresh") => Command::SparkWalletRefresh,
        Some(other) => Command::Custom(format!("spark {}", other), parts.collect()),
    }
}

fn parse_nip90_command(args: Vec<String>) -> Command {
    let mut parts = args.into_iter();
    match parts.next().as_deref() {
        None => Command::Nip90,
        Some("open") => Command::Nip90,
        Some("refresh") => Command::Nip90Refresh,
        Some("connect") => {
            let relay = parts.collect::<Vec<String>>().join(" ");
            Command::Nip90Connect(relay)
        }
        Some(other) => Command::Custom(format!("nip90 {}", other), parts.collect()),
    }
}

fn parse_oanix_command(args: Vec<String>) -> Command {
    let mut parts = args.into_iter();
    match parts.next().as_deref() {
        Some("refresh") => Command::OanixRefresh,
        Some("status") => Command::Oanix,
        _ => Command::Oanix,
    }
}

fn parse_directives_command(args: Vec<String>) -> Command {
    let mut parts = args.into_iter();
    match parts.next().as_deref() {
        Some("refresh") => Command::DirectivesRefresh,
        Some("status") => Command::Directives,
        _ => Command::Directives,
    }
}

fn parse_issue_tracker_command(args: Vec<String>) -> Command {
    let mut parts = args.into_iter();
    match parts.next().as_deref() {
        Some("refresh") => Command::AutopilotIssuesRefresh,
        Some("status") => Command::AutopilotIssues,
        _ => Command::AutopilotIssues,
    }
}

fn parse_rlm_command(args: Vec<String>) -> Command {
    let mut parts = args.into_iter();
    match parts.next().as_deref() {
        Some("refresh") => Command::RlmRefresh,
        Some("trace") | Some("events") => {
            let run_id = parts.collect::<Vec<String>>().join(" ");
            if run_id.trim().is_empty() {
                Command::RlmTrace(None)
            } else {
                Command::RlmTrace(Some(run_id))
            }
        }
        Some("status") => Command::Rlm,
        _ => Command::Rlm,
    }
}

fn parse_pylon_command(args: Vec<String>) -> Command {
    let mut parts = args.into_iter();
    match parts.next().as_deref() {
        None => Command::PylonEarnings,
        Some("refresh") => Command::PylonEarningsRefresh,
        Some("status") => Command::PylonEarnings,
        Some("earnings") | Some("revenue") => match parts.next().as_deref() {
            Some("refresh") => Command::PylonEarningsRefresh,
            _ => Command::PylonEarnings,
        },
        Some("jobs") => match parts.next().as_deref() {
            Some("refresh") => Command::PylonJobsRefresh,
            Some("status") => Command::PylonJobs,
            _ => Command::PylonJobs,
        },
        Some(other) => Command::Custom(format!("pylon {}", other), parts.collect()),
    }
}

fn parse_issues_command(args: Vec<String>) -> Command {
    let mut parts = args.into_iter();
    match parts.next().as_deref() {
        Some("refresh") => Command::IssuesRefresh,
        Some("status") => Command::Issues,
        _ => Command::Issues,
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

fn parse_workspace_command(args: Vec<String>) -> Command {
    let mut parts = args.into_iter();
    match parts.next().as_deref() {
        None | Some("list") => Command::WorkspaceList,
        Some("add") => Command::WorkspaceAdd,
        Some("connect") => {
            let hint = parts.collect::<Vec<String>>().join(" ");
            Command::WorkspaceConnect(hint)
        }
        Some("refresh") => Command::WorkspaceRefresh,
        Some(other) => Command::Custom(format!("workspace {}", other), parts.collect()),
    }
}

fn parse_review_command(args: Vec<String>) -> Command {
    let mut parts = args.into_iter();
    let mut delivery = ReviewDelivery::Inline;
    let mut token = parts.next();

    if let Some(value) = token.as_deref() {
        match value {
            "inline" => {
                delivery = ReviewDelivery::Inline;
                token = parts.next();
            }
            "detached" => {
                delivery = ReviewDelivery::Detached;
                token = parts.next();
            }
            _ => {}
        }
    }

    let target = match token.as_deref() {
        None => ReviewTarget::UncommittedChanges,
        Some("uncommitted") | Some("uncommittedchanges") | Some("changes") => {
            ReviewTarget::UncommittedChanges
        }
        Some("branch") | Some("base") => {
            let branch = parts.next().unwrap_or_default();
            ReviewTarget::BaseBranch { branch }
        }
        Some("commit") => {
            let sha = parts.next().unwrap_or_default();
            let title = {
                let rest = parts.collect::<Vec<String>>();
                if rest.is_empty() {
                    None
                } else {
                    Some(rest.join(" "))
                }
            };
            ReviewTarget::Commit { sha, title }
        }
        Some("custom") => {
            let instructions = parts.collect::<Vec<String>>().join(" ");
            ReviewTarget::Custom { instructions }
        }
        Some(other) => {
            let mut instructions = vec![other.to_string()];
            instructions.extend(parts);
            ReviewTarget::Custom {
                instructions: instructions.join(" "),
            }
        }
    };

    Command::Review(ReviewCommand { delivery, target })
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

fn parse_account_command(args: Vec<String>) -> Command {
    let mut parts = args.into_iter();
    match parts.next().as_deref() {
        None => Command::AccountStatus,
        Some("status") => Command::AccountStatus,
        Some("logout") => Command::AccountLogout,
        Some("rate-limits") | Some("ratelimits") => Command::AccountRateLimits,
        Some("login") => match parts.next().as_deref() {
            Some("apikey") | Some("api-key") | Some("api_key") => {
                let key = parts.collect::<Vec<String>>().join(" ");
                Command::AccountLoginApiKey(key)
            }
            Some("chatgpt") => Command::AccountLoginChatgpt,
            Some("cancel") => Command::AccountLoginCancel(parts.next().unwrap_or_default()),
            Some(other) => Command::Custom(format!("account login {}", other), parts.collect()),
            None => Command::Custom("account login".to_string(), Vec::new()),
        },
        Some(other) => Command::Custom(format!("account {}", other), parts.collect()),
    }
}

fn parse_mcp_command(args: Vec<String>) -> Command {
    let mut parts = args.into_iter();
    match parts.next().as_deref() {
        None => Command::Mcp,
        Some("list") => Command::Mcp,
        Some("reload") => Command::McpReload,
        Some("status") => Command::McpStatus,
        Some("login") => Command::McpLogin(parts.next().unwrap_or_default()),
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

fn parse_agent_backends_command(args: Vec<String>) -> Command {
    let mut parts = args.into_iter();
    match parts.next().as_deref() {
        None => Command::AgentBackends,
        Some("open") | Some("status") => Command::AgentBackends,
        Some("refresh") => Command::AgentBackendsRefresh,
        Some(other) => Command::Custom(format!("agent-backends {}", other), parts.collect()),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_command_requires_leading_slash() {
        assert!(parse_command("help").is_none());
        assert_eq!(parse_command("/").unwrap(), Command::Help);
    }

    #[test]
    fn parse_backend_variants() {
        assert_eq!(parse_command("/backend").unwrap(), Command::Backend);
        assert_eq!(
            parse_command("/backend codex").unwrap(),
            Command::BackendSet("codex".to_string())
        );
    }

    #[test]
    fn parse_review_commit_with_title() {
        let parsed = parse_command("/review detached commit deadbeef Add tests").unwrap();
        match parsed {
            Command::Review(review) => {
                assert_eq!(review.delivery, ReviewDelivery::Detached);
                match review.target {
                    ReviewTarget::Commit { sha, title } => {
                        assert_eq!(sha, "deadbeef");
                        assert_eq!(title, Some("Add tests".to_string()));
                    }
                    _ => panic!("expected commit target"),
                }
            }
            _ => panic!("expected review command"),
        }
    }

    #[test]
    fn parse_permission_allow_list() {
        let parsed = parse_command("/permission allow read write").unwrap();
        assert_eq!(
            parsed,
            Command::PermissionAllow(vec!["read".to_string(), "write".to_string()])
        );
    }

    #[test]
    fn parse_tools_enable_list() {
        let parsed = parse_command("/tools enable ripgrep lsp").unwrap();
        assert_eq!(
            parsed,
            Command::ToolsEnable(vec!["ripgrep".to_string(), "lsp".to_string()])
        );
    }

    #[test]
    fn parse_account_login_apikey() {
        let parsed = parse_command("/account login apikey sk-test").unwrap();
        assert_eq!(parsed, Command::AccountLoginApiKey("sk-test".to_string()));
    }

    #[test]
    fn parse_dvm_kind_fallbacks() {
        let parsed = parse_command("/dvm kind 42").unwrap();
        assert_eq!(parsed, Command::DvmKind(42));

        let parsed = parse_command("/dvm kind nope").unwrap();
        assert_eq!(parsed, Command::Dvm);
    }

    #[test]
    fn parse_dspy_flags() {
        let parsed = parse_command("/dspy auto on").unwrap();
        assert_eq!(parsed, Command::DspyAuto(true));

        let parsed = parse_command("/dspy background off").unwrap();
        assert_eq!(parsed, Command::DspyBackground(false));
    }

    #[test]
    fn parse_chainviz_prompt() {
        let parsed = parse_command("/chainviz summarize readme").unwrap();
        assert_eq!(parsed, Command::ChainViz("summarize readme".to_string()));
    }
}
