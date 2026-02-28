use std::path::PathBuf;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, anyhow, bail};
use codex_client::{
    AppServerChannels, AppServerClient, AppServerConfig, AppServerNotification, AppServerRequest,
    ClientInfo, InitializeCapabilities, InitializeParams, ThreadListParams, ThreadReadParams,
    ThreadSnapshot, ThreadSortKey, ThreadStartParams, TurnStartParams, UserInput,
};
use serde_json::Value;

#[derive(Debug)]
struct HarnessArgs {
    cwd: PathBuf,
    model: String,
    prompt: Option<String>,
    list_limit: u32,
    drain_ms: u64,
    timeout_ms: u64,
    max_events: usize,
}

impl Default for HarnessArgs {
    fn default() -> Self {
        Self {
            cwd: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            model: "gpt-5-codex".to_string(),
            prompt: None,
            list_limit: 20,
            drain_ms: 700,
            timeout_ms: 4_000,
            max_events: 24,
        }
    }
}

impl HarnessArgs {
    fn from_env() -> Result<Self> {
        let mut args = Self::default();
        let mut input = std::env::args().skip(1);

        while let Some(flag) = input.next() {
            match flag.as_str() {
                "--cwd" => {
                    let value = input
                        .next()
                        .ok_or_else(|| anyhow!("missing value for --cwd"))?;
                    args.cwd = PathBuf::from(value);
                }
                "--model" => {
                    let value = input
                        .next()
                        .ok_or_else(|| anyhow!("missing value for --model"))?;
                    if value.trim().is_empty() {
                        bail!("--model cannot be empty");
                    }
                    args.model = value;
                }
                "--prompt" => {
                    let value = input
                        .next()
                        .ok_or_else(|| anyhow!("missing value for --prompt"))?;
                    if value.trim().is_empty() {
                        args.prompt = None;
                    } else {
                        args.prompt = Some(value);
                    }
                }
                "--list-limit" => {
                    let value = input
                        .next()
                        .ok_or_else(|| anyhow!("missing value for --list-limit"))?;
                    args.list_limit = value
                        .parse::<u32>()
                        .map_err(|error| anyhow!("invalid --list-limit '{}': {error}", value))?;
                    if args.list_limit == 0 {
                        bail!("--list-limit must be greater than 0");
                    }
                }
                "--drain-ms" => {
                    let value = input
                        .next()
                        .ok_or_else(|| anyhow!("missing value for --drain-ms"))?;
                    args.drain_ms = value
                        .parse::<u64>()
                        .map_err(|error| anyhow!("invalid --drain-ms '{}': {error}", value))?;
                }
                "--timeout-ms" => {
                    let value = input
                        .next()
                        .ok_or_else(|| anyhow!("missing value for --timeout-ms"))?;
                    args.timeout_ms = value
                        .parse::<u64>()
                        .map_err(|error| anyhow!("invalid --timeout-ms '{}': {error}", value))?;
                    if args.timeout_ms == 0 {
                        bail!("--timeout-ms must be greater than 0");
                    }
                }
                "--max-events" => {
                    let value = input
                        .next()
                        .ok_or_else(|| anyhow!("missing value for --max-events"))?;
                    args.max_events = value
                        .parse::<usize>()
                        .map_err(|error| anyhow!("invalid --max-events '{}': {error}", value))?;
                }
                "--help" | "-h" => {
                    print_usage();
                    std::process::exit(0);
                }
                _ => {
                    bail!(
                        "unknown argument '{}'\n\n{}",
                        flag,
                        usage_text()
                    );
                }
            }
        }

        Ok(args)
    }
}

#[derive(Default)]
struct ChannelEventBatch {
    notifications: Vec<String>,
    requests: Vec<String>,
}

fn main() -> Result<()> {
    let args = HarnessArgs::from_env()?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("failed to create tokio runtime")?;
    runtime.block_on(run(args))
}

async fn run(args: HarnessArgs) -> Result<()> {
    println!("codex live harness");
    println!("cwd={}", args.cwd.display());
    println!("model={}", args.model);
    println!("list_limit={}", args.list_limit);
    println!(
        "prompt={}",
        args.prompt
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or("<none>")
    );
    println!();

    let (client, mut channels) = AppServerClient::spawn(AppServerConfig {
        cwd: Some(args.cwd.clone()),
        ..Default::default()
    })
    .await
    .context("failed to spawn codex app-server client")?;

    client
        .initialize(InitializeParams {
            client_info: ClientInfo {
                name: "openagents-codex-live-harness".to_string(),
                title: Some("OpenAgents Codex Live Harness".to_string()),
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
            capabilities: Some(InitializeCapabilities {
                experimental_api: true,
                opt_out_notification_methods: None,
            }),
        })
        .await
        .context("codex initialize failed")?;

    let init_events = collect_channel_events(
        &mut channels,
        Duration::from_millis(args.drain_ms),
        Duration::from_millis(args.timeout_ms),
    )
    .await;
    print_events("post-initialize", init_events, args.max_events);

    println!("simulate_click chat.refresh_threads -> thread/list");
    let list = client
        .thread_list(ThreadListParams {
            cursor: None,
            limit: Some(args.list_limit),
            sort_key: Some(ThreadSortKey::UpdatedAt),
            model_providers: None,
            source_kinds: None,
            archived: Some(false),
            cwd: Some(args.cwd.display().to_string()),
            search_term: None,
        })
        .await
        .context("thread/list failed")?;
    println!("thread/list returned {} entries", list.data.len());
    let previous_thread_id = list.data.first().map(|thread| thread.id.clone());
    if let Some(thread_id) = previous_thread_id.as_deref() {
        let previous_read = client
            .thread_read(ThreadReadParams {
                thread_id: thread_id.to_string(),
                include_turns: true,
            })
            .await
            .with_context(|| format!("thread/read failed for prior thread {}", thread_id))?;
        println!(
            "thread/read prior id={} turns={} transcript_messages={}",
            thread_id,
            previous_read.thread.turns.len(),
            transcript_message_count(&previous_read.thread)
        );
    } else {
        println!("thread/read prior skipped (no prior thread available)");
    }
    let refresh_events = collect_channel_events(
        &mut channels,
        Duration::from_millis(args.drain_ms),
        Duration::from_millis(args.timeout_ms),
    )
    .await;
    print_events("post-refresh", refresh_events, args.max_events);
    println!();

    println!("simulate_click chat.new_thread -> thread/start");
    let started = client
        .thread_start(ThreadStartParams {
            model: Some(args.model.clone()),
            model_provider: None,
            cwd: Some(args.cwd.display().to_string()),
            approval_policy: None,
            sandbox: None,
        })
        .await
        .context("thread/start failed")?;
    let new_thread_id = started.thread.id;
    println!("thread/start new_thread_id={new_thread_id}");

    match client
        .thread_read(ThreadReadParams {
            thread_id: new_thread_id.clone(),
            include_turns: true,
        })
        .await
    {
        Ok(new_read) => {
            println!(
                "thread/read new id={} turns={} transcript_messages={}",
                new_thread_id,
                new_read.thread.turns.len(),
                transcript_message_count(&new_read.thread)
            );
        }
        Err(error) => {
            let error_text = error.to_string();
            if error_text.contains("not materialized yet")
                && error_text.contains("includeTurns is unavailable")
            {
                println!(
                    "thread/read new id={} unavailable pre-materialization: {}",
                    new_thread_id, error_text
                );
            } else {
                return Err(error)
                    .with_context(|| format!("thread/read failed for new thread {}", new_thread_id));
            }
        }
    }

    if let Some(previous_thread_id) = previous_thread_id.as_deref() {
        let same_thread = previous_thread_id == new_thread_id;
        println!(
            "thread-switch-check previous={} new={} changed={}",
            previous_thread_id, new_thread_id, !same_thread
        );
    }
    let new_chat_events = collect_channel_events(
        &mut channels,
        Duration::from_millis(args.drain_ms),
        Duration::from_millis(args.timeout_ms),
    )
    .await;
    print_events("post-new-chat", new_chat_events, args.max_events);

    if let Some(prompt) = args
        .prompt
        .as_ref()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
    {
        println!();
        println!("simulate_click chat.send -> turn/start");
        println!("prompt={prompt}");
        let turn = client
            .turn_start(TurnStartParams {
                thread_id: new_thread_id.clone(),
                input: vec![UserInput::Text {
                    text: prompt,
                    text_elements: Vec::new(),
                }],
                cwd: None,
                approval_policy: None,
                sandbox_policy: None,
                model: Some(args.model),
                effort: None,
                summary: None,
                personality: None,
                output_schema: None,
                collaboration_mode: None,
            })
            .await
            .context("turn/start failed")?;
        println!("turn/start turn_id={}", turn.turn.id);
        let turn_events = collect_channel_events(
            &mut channels,
            Duration::from_millis(args.drain_ms),
            Duration::from_millis(args.timeout_ms),
        )
        .await;
        print_events("post-send", turn_events, args.max_events);

        let after_turn = client
            .thread_read(ThreadReadParams {
                thread_id: new_thread_id.clone(),
                include_turns: true,
            })
            .await
            .with_context(|| format!("thread/read failed for post-send {}", new_thread_id))?;
        println!(
            "thread/read after-send id={} turns={} transcript_messages={}",
            new_thread_id,
            after_turn.thread.turns.len(),
            transcript_message_count(&after_turn.thread)
        );
    }

    Ok(())
}

fn collect_message_text(value: &Value) -> Option<&str> {
    if let Some(text) = value.get("text").and_then(Value::as_str) {
        return Some(text);
    }
    if let Some(message) = value.get("message").and_then(Value::as_str) {
        return Some(message);
    }
    if let Some(content_items) = value.get("content").and_then(Value::as_array) {
        for item in content_items {
            if let Some(text) = item.get("text").and_then(Value::as_str) {
                return Some(text);
            }
            if let Some(text) = item
                .get("annotations")
                .and_then(Value::as_array)
                .and_then(|annotations| annotations.first())
                .and_then(|annotation| annotation.get("text"))
                .and_then(Value::as_str)
            {
                return Some(text);
            }
        }
    }
    None
}

fn transcript_message_count(thread: &ThreadSnapshot) -> usize {
    thread
        .turns
        .iter()
        .map(|turn| {
            turn.items
                .iter()
                .filter(|item| collect_message_text(item).is_some())
                .count()
        })
        .sum()
}

async fn collect_channel_events(
    channels: &mut AppServerChannels,
    idle_break: Duration,
    max_wait: Duration,
) -> ChannelEventBatch {
    let start = Instant::now();
    let deadline = start + max_wait;
    let mut last_event = start;
    let mut batch = ChannelEventBatch::default();

    loop {
        let now = Instant::now();
        if now >= deadline {
            break;
        }
        if (!batch.notifications.is_empty() || !batch.requests.is_empty())
            && now.duration_since(last_event) >= idle_break
        {
            break;
        }

        let sleep_for = deadline
            .saturating_duration_since(now)
            .min(Duration::from_millis(150));

        tokio::select! {
            maybe = channels.notifications.recv() => {
                let Some(notification) = maybe else {
                    break;
                };
                batch.notifications.push(notification_summary(&notification));
                last_event = Instant::now();
            }
            maybe = channels.requests.recv() => {
                let Some(request) = maybe else {
                    break;
                };
                batch.requests.push(request_summary(&request));
                last_event = Instant::now();
            }
            _ = tokio::time::sleep(sleep_for) => {}
        }
    }

    batch
}

fn notification_summary(notification: &AppServerNotification) -> String {
    format!(
        "notify method={} params={}",
        notification.method,
        compact_json(notification.params.as_ref())
    )
}

fn request_summary(request: &AppServerRequest) -> String {
    format!(
        "request method={} id={} params={}",
        request.method,
        request_id_summary(&request.id),
        compact_json(request.params.as_ref())
    )
}

fn request_id_summary(id: &codex_client::AppServerRequestId) -> String {
    match id {
        codex_client::AppServerRequestId::String(value) => value.clone(),
        codex_client::AppServerRequestId::Integer(value) => value.to_string(),
    }
}

fn compact_json(value: Option<&Value>) -> String {
    let Some(value) = value else {
        return "null".to_string();
    };
    let serialized = serde_json::to_string(value).unwrap_or_else(|_| "<invalid-json>".to_string());
    const LIMIT: usize = 180;
    if serialized.chars().count() <= LIMIT {
        serialized
    } else {
        let mut truncated = serialized.chars().take(LIMIT).collect::<String>();
        truncated.push_str("...");
        truncated
    }
}

fn print_events(label: &str, events: ChannelEventBatch, max_events: usize) {
    println!(
        "{} events: notifications={} requests={}",
        label,
        events.notifications.len(),
        events.requests.len()
    );
    let mut notification_lines = events.notifications;
    let notification_omitted = notification_lines.len().saturating_sub(max_events);
    notification_lines.truncate(max_events);
    for line in notification_lines {
        println!("  {line}");
    }
    if notification_omitted > 0 {
        println!("  ... {} additional notifications omitted", notification_omitted);
    }
    let mut request_lines = events.requests;
    let request_omitted = request_lines.len().saturating_sub(max_events);
    request_lines.truncate(max_events);
    for line in request_lines {
        println!("  {line}");
    }
    if request_omitted > 0 {
        println!("  ... {} additional requests omitted", request_omitted);
    }
}

fn usage_text() -> String {
    [
        "Usage:",
        "  cargo run -p autopilot-desktop --bin codex-live-harness -- [options]",
        "",
        "Options:",
        "  --cwd <path>           Workspace cwd to send to app-server",
        "  --model <id>           Model to use for thread/start and turn/start",
        "  --prompt <text>        Optional prompt to send after new thread starts",
        "  --list-limit <n>       thread/list limit (default: 20)",
        "  --drain-ms <n>         Idle settle period for channel drains (default: 700)",
        "  --timeout-ms <n>       Max wait per channel drain phase (default: 4000)",
        "  --max-events <n>       Max notifications/requests to print per phase (default: 24)",
        "  --help                 Show this help",
    ]
    .join("\n")
}

fn print_usage() {
    println!("{}", usage_text());
}
