use std::path::PathBuf;
use std::sync::Arc;

use clap::{Parser, ValueEnum};
use serde_json::Value;

use fm_bridge_agent::{FmBridgeAgent, FmBridgeAgentConfig};
use gpt_oss_agent::tools::{ToolRequest, ToolResult};
use gpt_oss_agent::{GptOssAgent, GptOssAgentConfig};

const TOOL_CALL_OPEN: &str = "<tool_call>";
const TOOL_CALL_CLOSE: &str = "</tool_call>";
const TOOL_NAMES: [&str; 4] = ["browser", "python", "apply_patch", "ui_pane"];

#[derive(Clone, Copy, Debug, ValueEnum)]
enum Backend {
    #[value(name = "gpt-oss")]
    GptOss,
    #[value(name = "fm-bridge")]
    FmBridge,
}

#[derive(Parser, Debug)]
#[command(name = "local-infer")]
#[command(about = "Local inference runner for GPT-OSS or Apple Foundation Models")]
struct Cli {
    /// Backend to use (gpt-oss or fm-bridge)
    #[arg(long, value_enum, default_value = "gpt-oss")]
    backend: Backend,

    /// Base URL for the backend server
    #[arg(long)]
    url: Option<String>,

    /// Model ID to use
    #[arg(long)]
    model: Option<String>,

    /// Workspace root for file tools
    #[arg(long)]
    workspace: Option<PathBuf>,

    /// Enable tool-call loop with local tools
    #[arg(long)]
    tools: bool,

    /// Send the prompt directly (skip Harmony formatting and tool loop)
    #[arg(long)]
    raw: bool,

    /// Maximum tool-call turns
    #[arg(long, default_value_t = 4)]
    max_tool_turns: u32,

    /// Maximum tokens to generate (raw mode only)
    #[arg(long)]
    max_tokens: Option<usize>,

    /// Sampling temperature (raw mode only)
    #[arg(long)]
    temperature: Option<f32>,

    /// Record rlog trajectory output
    #[arg(long)]
    record: bool,

    /// Prompt to send
    prompt: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.backend {
        Backend::GptOss => run_gpt_oss(cli).await,
        Backend::FmBridge => run_fm_bridge(cli).await,
    }
}

async fn run_gpt_oss(cli: Cli) -> anyhow::Result<()> {
    if cli.raw && cli.tools {
        anyhow::bail!("--raw cannot be combined with --tools");
    }

    let mut config = GptOssAgentConfig::default();
    if let Some(url) = cli.url {
        config.base_url = url;
    }
    if let Some(model) = cli.model {
        config.model = model;
    }
    if let Some(workspace) = cli.workspace {
        config.workspace_root = workspace;
    }
    config.record_trajectory = cli.record;

    if cli.raw {
        let client = gpt_oss::GptOssClient::builder()
            .base_url(&config.base_url)
            .default_model(&config.model)
            .build()?;
        let request = gpt_oss::GptOssRequest {
            model: config.model.clone(),
            prompt: cli.prompt.clone(),
            max_tokens: cli.max_tokens,
            temperature: cli.temperature,
            top_p: None,
            stop: None,
            stream: false,
        };
        let response = client.complete(request).await?;
        println!("{}", response.text.trim());
        return Ok(());
    }

    let agent = GptOssAgent::new(config).await?;
    let session = Arc::new(agent.create_session().await);

    if cli.tools {
        let response = session
            .send_with_tools(&cli.prompt, cli.max_tool_turns)
            .await?;
        println!("{}", response.trim());
        return Ok(());
    }

    let response = session.send(&cli.prompt).await?;
    println!("{}", response.trim());
    Ok(())
}

async fn run_fm_bridge(cli: Cli) -> anyhow::Result<()> {
    let mut config = FmBridgeAgentConfig::default();
    if let Some(url) = cli.url {
        config.base_url = url;
    }
    if let Some(model) = cli.model {
        config.model = model;
    }
    if let Some(workspace) = cli.workspace {
        config.workspace_root = workspace;
    }
    config.record_trajectory = cli.record;

    let agent = FmBridgeAgent::new(config).await?;
    let session = Arc::new(agent.create_session().await);

    let tool_schemas = if cli.tools {
        let mut schemas = Vec::new();
        for name in TOOL_NAMES {
            let schema = agent.get_tool_schema(name).await.unwrap_or(Value::Null);
            schemas.push((name.to_string(), schema));
        }
        schemas
    } else {
        Vec::new()
    };

    let session_for_tools = Arc::clone(&session);
    let session_for_send = Arc::clone(&session);

    run_loop(
        "fm-bridge",
        cli.prompt,
        cli.tools,
        cli.max_tool_turns,
        tool_schemas,
        move |req| {
            let session = Arc::clone(&session_for_tools);
            async move { session.execute_tool(req).await }
        },
        move |message| {
            let session = Arc::clone(&session_for_send);
            async move { session.send(&message).await }
        },
    )
    .await
}

async fn run_loop<
    ExecuteTool,
    ExecuteToolFuture,
    ExecuteToolError,
    SendMessage,
    SendMessageFuture,
    SendMessageError,
>(
    backend: &str,
    prompt: String,
    enable_tools: bool,
    max_tool_turns: u32,
    tool_schemas: Vec<(String, Value)>,
    execute_tool: ExecuteTool,
    send_message: SendMessage,
) -> anyhow::Result<()>
where
    ExecuteTool: Fn(ToolRequest) -> ExecuteToolFuture,
    ExecuteToolFuture: std::future::Future<Output = Result<ToolResult, ExecuteToolError>>,
    ExecuteToolError: std::error::Error + Send + Sync + 'static,
    SendMessage: Fn(String) -> SendMessageFuture,
    SendMessageFuture: std::future::Future<Output = Result<String, SendMessageError>>,
    SendMessageError: std::error::Error + Send + Sync + 'static,
{
    let mut tool_turns = 0;
    let mut message = if enable_tools {
        let tool_prompt = build_tool_prompt(backend, &tool_schemas);
        format!("{tool_prompt}\n\nUser: {prompt}")
    } else {
        prompt
    };

    loop {
        let response = send_message(message).await.map_err(anyhow::Error::msg)?;

        if enable_tools && let Some(request) = extract_tool_call(&response) {
            if tool_turns >= max_tool_turns {
                println!("Max tool turns reached, returning last response.");
                println!("{}", response.trim());
                break;
            }

            tool_turns += 1;
            println!("[tool:{backend}] {}", request.tool);
            let result = execute_tool(request.clone())
                .await
                .map_err(anyhow::Error::msg)?;
            let result_payload = serde_json::json!({
                "tool": request.tool,
                "success": result.success,
                "output": result.output,
                "error": result.error,
            });
            let result_block = format!(
                "Tool result:\n<tool_result>{}</tool_result>",
                serde_json::to_string_pretty(&result_payload)?
            );
            message = result_block;
            continue;
        }

        println!("{}", response.trim());
        break;
    }

    Ok(())
}

fn build_tool_prompt(backend: &str, tool_schemas: &[(String, Value)]) -> String {
    let mut lines = Vec::new();
    lines.push(format!("You are running on backend: {backend}."));
    lines.push("Tool calls are allowed. To call a tool, respond ONLY with:".to_string());
    lines.push(format!(
        "{TOOL_CALL_OPEN}{{\"tool\":\"name\",\"parameters\":{{}}}}{TOOL_CALL_CLOSE}"
    ));
    lines.push("Available tools:".to_string());

    for (name, schema) in tool_schemas {
        let schema_text = serde_json::to_string_pretty(schema).unwrap_or_else(|_| "{}".to_string());
        lines.push(format!("- {name}: {schema_text}"));
    }

    lines.join("\n")
}

fn extract_tool_call(text: &str) -> Option<ToolRequest> {
    let trimmed = text.trim();
    if let Some(start) = trimmed.find(TOOL_CALL_OPEN) {
        let rest = &trimmed[start + TOOL_CALL_OPEN.len()..];
        if let Some(end) = rest.find(TOOL_CALL_CLOSE) {
            let json_str = rest[..end].trim();
            return serde_json::from_str::<ToolRequest>(json_str).ok();
        }
    }

    if trimmed.starts_with('{') && trimmed.ends_with('}') {
        return serde_json::from_str::<ToolRequest>(trimmed).ok();
    }

    None
}
