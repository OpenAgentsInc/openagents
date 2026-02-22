use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use chrono::{Duration, SecondsFormat, Utc};
use clap::{Args, Parser, Subcommand};
use hmac::{Hmac, Mac};
use openagents_control_service::auth::AuthService;
use openagents_control_service::config::Config;
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use zip::read::ZipArchive;

#[derive(Parser)]
#[command(name = "openagents-control-ops")]
#[command(about = "Rust replacements for openagents.com operator Artisan commands")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    #[command(name = "demo:l402")]
    DemoL402(DemoL402Args),
    #[command(name = "khala:import-chat")]
    KhalaImportChat(KhalaImportChatArgs),
    #[command(name = "ops:test-login-link")]
    OpsTestLoginLink(OpsTestLoginLinkArgs),
    #[command(name = "runtime:tools:invoke-api")]
    RuntimeToolsInvokeApi(RuntimeToolsInvokeApiArgs),
    #[command(name = "ops:create-api-token")]
    OpsCreateApiToken(OpsCreateApiTokenArgs),
}

#[derive(Args)]
struct DemoL402Args {
    #[arg(long, default_value = "http://127.0.0.1:8787")]
    api_base: String,
    #[arg(long)]
    token: String,
    #[arg(long, default_value = "fake")]
    preset: String,
    #[arg(long, default_value_t = 100)]
    max_spend_sats: u64,
}

#[derive(Args)]
struct KhalaImportChatArgs {
    source: PathBuf,
    #[arg(long)]
    replace: bool,
    #[arg(long)]
    dry_run: bool,
    #[arg(long)]
    resolve_workos_users: bool,
    #[arg(long)]
    skip_blueprints: bool,
    #[arg(long)]
    auth_store: Option<PathBuf>,
    #[arg(long)]
    codex_thread_store: Option<PathBuf>,
}

#[derive(Args)]
struct OpsTestLoginLinkArgs {
    email: String,
    #[arg(long, default_value_t = 30)]
    minutes: i64,
    #[arg(long)]
    name: Option<String>,
    #[arg(long)]
    base_url: Option<String>,
    #[arg(long)]
    signing_key: Option<String>,
}

#[derive(Args)]
struct RuntimeToolsInvokeApiArgs {
    #[arg(long, default_value = "http://127.0.0.1:8787")]
    api_base: String,
    #[arg(long)]
    token: String,
    #[arg(long, default_value = "coding.v1")]
    tool_pack: String,
    #[arg(long, default_value = "replay")]
    mode: String,
    #[arg(long, default_value = "get_issue")]
    operation: String,
    #[arg(long, default_value = "OpenAgentsInc/openagents")]
    repository: String,
    #[arg(long, default_value_t = 1)]
    issue_number: u64,
    #[arg(long, default_value_t = 1)]
    pull_number: u64,
    #[arg(long)]
    comment_body: Option<String>,
    #[arg(long, default_value = "run_cli_tools")]
    run_id: String,
    #[arg(long, default_value = "thread_cli_tools")]
    thread_id: String,
    #[arg(long, default_value_t = false)]
    write_approved: bool,
}

#[derive(Args)]
struct OpsCreateApiTokenArgs {
    email: String,
    #[arg(default_value = "ops-cli")]
    name: String,
    #[arg(long = "abilities", value_delimiter = ',', default_values_t = vec!["*".to_string()])]
    abilities: Vec<String>,
    #[arg(long)]
    expires_days: Option<u64>,
    #[arg(long)]
    auth_store: Option<PathBuf>,
}

type HmacSha256 = Hmac<Sha256>;

#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("error: {error}");
        std::process::exit(1);
    }
}

async fn run() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::DemoL402(args) => run_demo_l402(args).await,
        Commands::KhalaImportChat(args) => run_khala_import_chat(args),
        Commands::OpsTestLoginLink(args) => run_ops_test_login_link(args),
        Commands::RuntimeToolsInvokeApi(args) => run_runtime_tools_invoke_api(args).await,
        Commands::OpsCreateApiToken(args) => run_ops_create_api_token(args).await,
    }
}

async fn run_demo_l402(args: DemoL402Args) -> Result<()> {
    let token = non_empty(&args.token).context("--token is required")?;
    let api_base = normalized_api_base(&args.api_base)?;
    let wallet_url = format!("{api_base}/api/l402/wallet");
    let transactions_url = format!("{api_base}/api/l402/transactions");

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .context("failed to build HTTP client")?;

    let wallet = send_json(
        client
            .get(wallet_url)
            .bearer_auth(&token)
            .header("accept", "application/json"),
    )
    .await
    .context("failed to fetch /api/l402/wallet")?;

    let transactions = send_json(
        client
            .get(transactions_url)
            .bearer_auth(&token)
            .header("accept", "application/json"),
    )
    .await
    .context("failed to fetch /api/l402/transactions")?;

    let latest_transaction = transactions
        .get("data")
        .and_then(Value::as_array)
        .and_then(|rows| rows.first().cloned());

    print_json(&json!({
        "preset": args.preset,
        "max_spend_sats": args.max_spend_sats,
        "wallet": wallet.get("data").cloned().unwrap_or(Value::Null),
        "latest_transaction": latest_transaction,
    }))?;

    Ok(())
}

async fn run_runtime_tools_invoke_api(args: RuntimeToolsInvokeApiArgs) -> Result<()> {
    let token = non_empty(&args.token).context("--token is required")?;
    let api_base = normalized_api_base(&args.api_base)?;
    let operation = non_empty(&args.operation).context("--operation cannot be empty")?;
    let repository = non_empty(&args.repository).context("--repository cannot be empty")?;
    let run_id = non_empty(&args.run_id).unwrap_or_else(|| "run_cli_tools".to_string());
    let thread_id = non_empty(&args.thread_id).unwrap_or_else(|| "thread_cli_tools".to_string());

    let mut request_payload = json!({
        "integration_id": "github.primary",
        "operation": operation,
        "repository": repository,
        "run_id": run_id,
        "thread_id": thread_id,
        "tool_call_id": format!("tool_call_cli_{}", Utc::now().timestamp()),
    });

    if operation == "get_pull_request" {
        request_payload["pull_number"] = Value::from(args.pull_number.max(1));
    } else {
        request_payload["issue_number"] = Value::from(args.issue_number.max(1));
    }

    if operation == "add_issue_comment" {
        let body = non_empty(args.comment_body.as_deref().unwrap_or_default())
            .context("--comment-body is required for add_issue_comment")?;
        request_payload["body"] = Value::from(body);
    }

    let payload = json!({
        "tool_pack": args.tool_pack,
        "mode": non_empty(&args.mode).unwrap_or_else(|| "replay".to_string()),
        "run_id": run_id,
        "thread_id": thread_id,
        "manifest": {
            "manifest_version": "coding.integration.v1",
            "integration_id": "github.primary",
            "provider": "github",
            "status": "active",
            "tool_pack": "coding.v1",
            "capabilities": ["get_issue", "get_pull_request", "add_issue_comment"],
            "secrets_ref": {
                "provider": "laravel",
                "key_id": "intsec_github_1"
            },
            "policy": {
                "write_operations_mode": "enforce",
                "max_requests_per_minute": 240,
                "default_repository": repository,
            }
        },
        "request": request_payload,
        "policy": {
            "authorization_id": "auth_cli_demo",
            "authorization_mode": "delegated_budget",
            "write_approved": args.write_approved,
            "budget": {
                "max_total_sats": 10_000,
                "max_per_call_sats": 2_000
            }
        }
    });

    let url = format!("{api_base}/api/runtime/tools/execute");
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .context("failed to build HTTP client")?;

    let response = client
        .post(url)
        .bearer_auth(token)
        .header("accept", "application/json")
        .json(&payload)
        .send()
        .await
        .context("runtime tools invoke request failed")?;

    let status = response.status();
    let body = response
        .text()
        .await
        .context("failed to read runtime tools response")?;

    println!("Runtime tools API status: {}", status.as_u16());

    let parsed: Value = serde_json::from_str(&body).unwrap_or_else(|_| Value::String(body.clone()));
    print_json(&parsed)?;

    if !status.is_success() {
        bail!(
            "runtime tools invoke request returned status {}",
            status.as_u16()
        );
    }

    Ok(())
}

fn run_ops_test_login_link(args: OpsTestLoginLinkArgs) -> Result<()> {
    let email = normalized_email(&args.email).context("Invalid email address")?;
    if args.minutes < 1 || args.minutes > 1_440 {
        bail!("--minutes must be between 1 and 1440");
    }

    let config = Config::from_env().context("failed to load service config from environment")?;
    let signing_key = resolve_signing_key(args.signing_key, &config)?;

    let expires = Utc::now() + Duration::minutes(args.minutes);
    let expires_unix = expires.timestamp();

    let mut unsigned = format!(
        "/internal/test-login?email={}&expires={expires_unix}",
        percent_encode(&email)
    );
    if let Some(name) = non_empty(args.name.as_deref().unwrap_or_default()) {
        unsigned.push_str("&name=");
        unsigned.push_str(&percent_encode(&name));
    }

    let signature = hmac_sha256_hex(signing_key.as_bytes(), unsigned.as_bytes())?;
    let signed_path = format!("{unsigned}&signature={signature}");

    let final_url = match non_empty(args.base_url.as_deref().unwrap_or_default()) {
        Some(base) => {
            let trimmed = base.trim_end_matches('/');
            format!("{trimmed}{signed_path}")
        }
        None => signed_path,
    };

    println!("Signed maintenance test-login URL:");
    println!("{final_url}");

    if !config.auth_local_test_login_enabled {
        eprintln!(
            "warning: OA_AUTH_LOCAL_TEST_LOGIN_ENABLED is disabled; this URL will not work until enabled"
        );
    }

    Ok(())
}

async fn run_ops_create_api_token(args: OpsCreateApiTokenArgs) -> Result<()> {
    let email = normalized_email(&args.email).context("Invalid email address")?;
    let token_name = non_empty(&args.name).context("Token name cannot be empty")?;

    let mut config =
        Config::from_env().context("failed to load service config from environment")?;
    if let Some(path) = args.auth_store {
        config.auth_store_path = Some(path);
    }
    let auth = AuthService::from_config(&config);

    let user = auth
        .user_by_email(&email)
        .await
        .context("failed to resolve user by email")?
        .context(format!("User not found for email: {email}"))?;

    let abilities = normalized_abilities(args.abilities);

    let ttl_seconds = if let Some(days) = args.expires_days {
        if !(1..=3_650).contains(&days) {
            bail!("--expires-days must be between 1 and 3650 when provided");
        }
        let seconds = days
            .checked_mul(24)
            .and_then(|value| value.checked_mul(60))
            .and_then(|value| value.checked_mul(60))
            .context("--expires-days overflow")?;
        Some(seconds)
    } else {
        None
    };

    let issued = auth
        .issue_personal_access_token(&user.id, token_name.clone(), abilities.clone(), ttl_seconds)
        .await
        .context("failed to issue personal access token")?;

    println!("Token created successfully. Copy now; it will not be shown again:");
    println!("{}", issued.plain_text_token);
    println!();
    println!("metadata:");
    println!("  user_id={}", user.id);
    println!("  email={}", user.email);
    println!("  name={token_name}");
    println!("  abilities={}", abilities.join(","));
    println!(
        "  expires_at={}",
        issued
            .token
            .expires_at
            .map(|value| value.to_rfc3339())
            .unwrap_or_else(|| "null".to_string())
    );

    Ok(())
}

#[derive(Debug, Default)]
struct KhalaImportStats {
    users_seen: usize,
    users_inserted: usize,
    threads_seen: usize,
    threads_upserted: usize,
    messages_seen: usize,
    messages_imported: usize,
    runs_seen: usize,
    receipts_seen: usize,
    blueprints_seen: usize,
}

fn run_khala_import_chat(args: KhalaImportChatArgs) -> Result<()> {
    let source = KhalaSource::from_path(&args.source)?;

    let users_rows = source.read_table("users")?;
    let threads_rows = source.read_table("threads")?;
    let messages_rows = source.read_table("messages")?;
    let runs_rows = source.read_table("runs")?;
    let receipts_rows = source.read_table("receipts")?;
    let blueprints_rows = if args.skip_blueprints {
        Vec::new()
    } else {
        source.read_table("blueprints")?
    };

    let mut stats = KhalaImportStats {
        users_seen: users_rows.len(),
        threads_seen: threads_rows.len(),
        messages_seen: messages_rows.len(),
        runs_seen: runs_rows.len(),
        receipts_seen: receipts_rows.len(),
        blueprints_seen: blueprints_rows.len(),
        ..KhalaImportStats::default()
    };

    let mut config =
        Config::from_env().context("failed to load service config from environment")?;
    if let Some(path) = args.auth_store {
        config.auth_store_path = Some(path);
    }
    if let Some(path) = args.codex_thread_store {
        config.codex_thread_store_path = Some(path);
    }

    let auth_store_path = config
        .auth_store_path
        .clone()
        .context("missing auth store path (set OA_AUTH_STORE_PATH or --auth-store)")?;
    let codex_store_path = config.codex_thread_store_path.clone().context(
        "missing codex thread store path (set OA_CODEX_THREAD_STORE_PATH or --codex-thread-store)",
    )?;

    let mut auth_store = load_store_json(&auth_store_path, default_auth_store_state())?;
    let mut codex_store = load_store_json(&codex_store_path, default_codex_store_state())?;

    let mut legacy_to_local_user = HashMap::new();

    for row in &users_rows {
        let legacy_user_id = non_empty(value_string(row, "userId").as_deref().unwrap_or_default())
            .or_else(|| non_empty(value_string(row, "_id").as_deref().unwrap_or_default()))
            .unwrap_or_else(|| format!("legacy_user_{}", legacy_to_local_user.len() + 1));

        let email = value_string(row, "email")
            .and_then(|value| normalized_email(&value).ok())
            .unwrap_or_else(|| {
                format!(
                    "khala+{}@import.invalid",
                    sanitize_identifier(&legacy_user_id)
                )
            });

        let local_user_id = if let Some(existing) = {
            let users_by_email = ensure_object_field(&mut auth_store, "users_by_email")?;
            users_by_email
                .get(&email)
                .and_then(Value::as_str)
                .map(ToOwned::to_owned)
        } {
            existing
        } else {
            let candidate = {
                let users_by_id = ensure_object_field(&mut auth_store, "users_by_id")?;
                if users_by_id.contains_key(&legacy_user_id) {
                    allocate_unique_user_id(&legacy_user_id, users_by_id)
                } else {
                    legacy_user_id.clone()
                }
            };

            let display_name =
                value_string(row, "name").unwrap_or_else(|| default_name_from_email(&email));
            let workos_id = format!("test_local_{}", sanitize_identifier(&candidate));
            {
                let users_by_id = ensure_object_field(&mut auth_store, "users_by_id")?;
                users_by_id.insert(
                    candidate.clone(),
                    json!({
                        "id": candidate,
                        "email": email,
                        "name": display_name,
                        "workos_user_id": workos_id,
                        "memberships": [{
                            "org_id": "org:openagents",
                            "org_slug": "openagents",
                            "role": "owner",
                            "role_scopes": ["*"],
                            "default_org": true,
                        }]
                    }),
                );
            }
            stats.users_inserted = stats.users_inserted.saturating_add(1);
            {
                let users_by_email = ensure_object_field(&mut auth_store, "users_by_email")?;
                users_by_email.insert(email.clone(), Value::String(candidate.clone()));
            }
            {
                let users_by_workos_id =
                    ensure_object_field(&mut auth_store, "users_by_workos_id")?;
                users_by_workos_id.insert(
                    format!("test_local_{}", sanitize_identifier(&candidate)),
                    Value::String(candidate.clone()),
                );
            }
            candidate
        };

        legacy_to_local_user.insert(legacy_user_id, local_user_id);
    }

    if args.replace {
        ensure_object_field(&mut codex_store, "threads")?.clear();
        ensure_object_field(&mut codex_store, "messages_by_thread")?.clear();
    }

    let mut thread_owner_map = HashMap::new();
    for row in &threads_rows {
        let Some(thread_id) = value_string(row, "threadId") else {
            continue;
        };
        let owner_legacy = value_string(row, "ownerId").unwrap_or_default();
        thread_owner_map.insert(thread_id, owner_legacy);
    }

    for (index, row) in messages_rows.iter().enumerate() {
        let Some(thread_id) = value_string(row, "threadId") else {
            continue;
        };

        let message_id =
            value_string(row, "messageId").unwrap_or_else(|| format!("khala_msg_{}", index + 1));
        let role = value_string(row, "role").unwrap_or_else(|| "user".to_string());
        let text = value_string(row, "text").unwrap_or_default();

        let owner_legacy = thread_owner_map
            .get(&thread_id)
            .cloned()
            .unwrap_or_default();
        let user_id = legacy_to_local_user
            .get(&owner_legacy)
            .cloned()
            .or_else(|| legacy_to_local_user.values().next().cloned())
            .unwrap_or_else(|| "khala-import-user".to_string());

        let created_at = timestamp_from_ms(value_i64(row, "createdAtMs"));

        let entry = ensure_object_field(&mut codex_store, "messages_by_thread")?
            .entry(thread_id.clone())
            .or_insert_with(|| Value::Array(Vec::new()));

        let Some(message_rows) = entry.as_array_mut() else {
            bail!("codex store messages_by_thread.{thread_id} is not an array");
        };

        if message_rows.iter().any(|value| {
            value
                .get("message_id")
                .and_then(Value::as_str)
                .map(|existing| existing == message_id)
                .unwrap_or(false)
        }) {
            continue;
        }

        message_rows.push(json!({
            "message_id": message_id,
            "thread_id": thread_id,
            "user_id": user_id,
            "role": role,
            "text": text,
            "created_at": created_at,
        }));
        stats.messages_imported = stats.messages_imported.saturating_add(1);
    }

    {
        let messages_by_thread = ensure_object_field(&mut codex_store, "messages_by_thread")?;
        for row in messages_by_thread.values_mut() {
            if let Some(messages) = row.as_array_mut() {
                messages.sort_by(|left, right| {
                    let left_ts = left
                        .get("created_at")
                        .and_then(Value::as_str)
                        .unwrap_or_default();
                    let right_ts = right
                        .get("created_at")
                        .and_then(Value::as_str)
                        .unwrap_or_default();
                    left_ts.cmp(right_ts)
                });
            }
        }
    }

    for row in &threads_rows {
        let Some(thread_id) = value_string(row, "threadId") else {
            continue;
        };

        let owner_legacy = value_string(row, "ownerId").unwrap_or_default();
        let user_id = legacy_to_local_user
            .get(&owner_legacy)
            .cloned()
            .or_else(|| legacy_to_local_user.values().next().cloned())
            .unwrap_or_else(|| "khala-import-user".to_string());

        let created_at = timestamp_from_ms(value_i64(row, "createdAtMs"));
        let updated_candidate = timestamp_from_ms(value_i64(row, "updatedAtMs"));

        let (message_count, last_message_at) = {
            let messages_by_thread = ensure_object_field(&mut codex_store, "messages_by_thread")?;
            thread_message_summary(messages_by_thread, &thread_id)?
        };
        let updated_at = if let Some(last) = last_message_at.as_deref() {
            if last > updated_candidate.as_str() {
                last.to_string()
            } else {
                updated_candidate.clone()
            }
        } else {
            updated_candidate.clone()
        };

        ensure_object_field(&mut codex_store, "threads")?.insert(
            thread_id.clone(),
            json!({
                "thread_id": thread_id,
                "user_id": user_id,
                "org_id": "org:openagents",
                "autopilot_id": Value::Null,
                "title": Value::Null,
                "created_at": created_at,
                "updated_at": updated_at,
                "message_count": message_count,
                "last_message_at": last_message_at,
            }),
        );
        stats.threads_upserted = stats.threads_upserted.saturating_add(1);
    }

    if !args.dry_run {
        write_store_json(&auth_store_path, &auth_store)?;
        write_store_json(&codex_store_path, &codex_store)?;
    }

    print_import_summary(&stats, args.dry_run);
    if args.resolve_workos_users {
        eprintln!(
            "note: --resolve-workos-users requested; Rust command currently uses local placeholder identities for missing emails"
        );
    }

    Ok(())
}

fn thread_message_summary(
    messages_by_thread: &Map<String, Value>,
    thread_id: &str,
) -> Result<(u32, Option<String>)> {
    let Some(rows) = messages_by_thread.get(thread_id) else {
        return Ok((0, None));
    };
    let Some(rows) = rows.as_array() else {
        bail!("codex store messages_by_thread.{thread_id} is not an array");
    };

    let count = u32::try_from(rows.len()).unwrap_or(u32::MAX);
    let last_message_at = rows
        .iter()
        .filter_map(|row| row.get("created_at").and_then(Value::as_str))
        .max()
        .map(ToOwned::to_owned);

    Ok((count, last_message_at))
}

fn print_import_summary(stats: &KhalaImportStats, dry_run: bool) {
    println!("Khala chat import summary:");
    println!("  users_seen: {}", stats.users_seen);
    println!("  users_inserted: {}", stats.users_inserted);
    println!("  threads_seen: {}", stats.threads_seen);
    println!("  threads_upserted: {}", stats.threads_upserted);
    println!("  messages_seen: {}", stats.messages_seen);
    println!("  messages_imported: {}", stats.messages_imported);
    println!("  runs_seen: {}", stats.runs_seen);
    println!("  receipts_seen: {}", stats.receipts_seen);
    println!("  blueprints_seen: {}", stats.blueprints_seen);
    if dry_run {
        println!("Dry-run completed: no store writes were performed.");
    }
}

enum KhalaSource {
    Directory(PathBuf),
    Zip(PathBuf),
}

impl KhalaSource {
    fn from_path(path: &Path) -> Result<Self> {
        if path.is_dir() {
            return Ok(Self::Directory(path.to_path_buf()));
        }

        if path
            .extension()
            .and_then(|value| value.to_str())
            .map(|value| value.eq_ignore_ascii_case("zip"))
            .unwrap_or(false)
            && path.is_file()
        {
            return Ok(Self::Zip(path.to_path_buf()));
        }

        bail!(
            "source must be a directory or a .zip file (received: {})",
            path.display()
        )
    }

    fn read_table(&self, table: &str) -> Result<Vec<Value>> {
        match self {
            Self::Directory(path) => read_jsonl_file(&path.join(table).join("documents.jsonl")),
            Self::Zip(path) => read_jsonl_zip_entry(path, &format!("{table}/documents.jsonl")),
        }
    }
}

fn read_jsonl_file(path: &Path) -> Result<Vec<Value>> {
    if !path.exists() {
        return Ok(Vec::new());
    }

    let content = fs::read_to_string(path)
        .with_context(|| format!("failed to read JSONL file {}", path.display()))?;
    parse_jsonl_lines(&content, &path.display().to_string())
}

fn read_jsonl_zip_entry(path: &Path, entry_name: &str) -> Result<Vec<Value>> {
    let file = fs::File::open(path)
        .with_context(|| format!("failed to open zip source {}", path.display()))?;
    let mut archive = ZipArchive::new(file)
        .with_context(|| format!("failed to read zip archive {}", path.display()))?;

    let Ok(mut entry) = archive.by_name(entry_name) else {
        return Ok(Vec::new());
    };

    let mut content = String::new();
    entry
        .read_to_string(&mut content)
        .with_context(|| format!("failed to read zip entry {entry_name}"))?;

    parse_jsonl_lines(&content, entry_name)
}

fn parse_jsonl_lines(content: &str, source: &str) -> Result<Vec<Value>> {
    let mut rows = Vec::new();
    for (index, raw_line) in content.lines().enumerate() {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }
        let parsed: Value = serde_json::from_str(line).with_context(|| {
            format!(
                "failed to parse JSONL row at {source}:{}",
                index.saturating_add(1)
            )
        })?;
        rows.push(parsed);
    }
    Ok(rows)
}

fn default_auth_store_state() -> Value {
    json!({
        "users_by_id": {},
        "users_by_email": {},
        "users_by_workos_id": {},
        "challenges": {},
        "sessions": {},
        "access_index": {},
        "refresh_index": {},
        "revoked_refresh_tokens": {},
        "revoked_refresh_token_ids": {},
        "personal_access_tokens": {},
    })
}

fn default_codex_store_state() -> Value {
    json!({
        "threads": {},
        "messages_by_thread": {},
    })
}

fn load_store_json(path: &Path, default_value: Value) -> Result<Value> {
    if !path.exists() {
        return Ok(default_value);
    }
    let bytes =
        fs::read(path).with_context(|| format!("failed to read store JSON {}", path.display()))?;
    serde_json::from_slice::<Value>(&bytes)
        .with_context(|| format!("failed to parse store JSON {}", path.display()))
}

fn write_store_json(path: &Path, value: &Value) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| {
            format!(
                "failed to create store parent directory {}",
                parent.display()
            )
        })?;
    }

    let payload = serde_json::to_vec_pretty(value).context("failed to encode store JSON")?;
    fs::write(path, payload)
        .with_context(|| format!("failed to write store JSON {}", path.display()))
}

fn ensure_object_field<'a>(
    value: &'a mut Value,
    field: &str,
) -> Result<&'a mut Map<String, Value>> {
    if !value.is_object() {
        bail!("expected object root while resolving field: {field}");
    }

    let root = value
        .as_object_mut()
        .context("store root is not an object")?;
    if !root.contains_key(field) {
        root.insert(field.to_string(), Value::Object(Map::new()));
    }

    root.get_mut(field)
        .and_then(Value::as_object_mut)
        .context(format!("field {field} is not an object"))
}

fn value_string(row: &Value, field: &str) -> Option<String> {
    row.get(field)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn value_i64(row: &Value, field: &str) -> Option<i64> {
    row.get(field).and_then(Value::as_i64).or_else(|| {
        row.get(field)
            .and_then(Value::as_u64)
            .map(|value| value as i64)
    })
}

fn timestamp_from_ms(raw_ms: Option<i64>) -> String {
    let now = Utc::now();
    let timestamp = raw_ms
        .and_then(chrono::DateTime::<Utc>::from_timestamp_millis)
        .unwrap_or(now);
    timestamp.to_rfc3339_opts(SecondsFormat::Millis, true)
}

fn normalized_api_base(raw: &str) -> Result<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        bail!("--api-base must be a valid URL");
    }
    Ok(trimmed.trim_end_matches('/').to_string())
}

fn normalized_email(raw: &str) -> Result<String> {
    let email = raw.trim().to_lowercase();
    if !is_valid_email(&email) {
        bail!("invalid email address");
    }
    Ok(email)
}

fn is_valid_email(email: &str) -> bool {
    if email.is_empty() || email.contains(char::is_whitespace) {
        return false;
    }
    let mut parts = email.split('@');
    let local = parts.next().unwrap_or_default();
    let domain = parts.next().unwrap_or_default();
    if parts.next().is_some() {
        return false;
    }
    !local.is_empty() && domain.contains('.') && !domain.starts_with('.') && !domain.ends_with('.')
}

fn non_empty(raw: &str) -> Option<String> {
    let value = raw.trim();
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

fn resolve_signing_key(cli_key: Option<String>, config: &Config) -> Result<String> {
    if let Some(value) = cli_key.and_then(|key| non_empty(&key)) {
        return Ok(value);
    }

    config
        .auth_local_test_login_signing_key
        .clone()
        .and_then(|value| non_empty(&value))
        .context(
            "missing signing key (provide --signing-key or set OA_AUTH_LOCAL_TEST_LOGIN_SIGNING_KEY)",
        )
}

fn hmac_sha256_hex(key: &[u8], payload: &[u8]) -> Result<String> {
    let mut mac = HmacSha256::new_from_slice(key).context("invalid HMAC signing key")?;
    mac.update(payload);
    Ok(sha256_bytes_hex(&mac.finalize().into_bytes()))
}

fn sha256_bytes_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let digest = hasher.finalize();
    let mut output = String::with_capacity(digest.len() * 2);
    for byte in digest {
        output.push(hex_char((byte >> 4) & 0x0f));
        output.push(hex_char(byte & 0x0f));
    }
    output
}

fn hex_char(nibble: u8) -> char {
    match nibble {
        0..=9 => (b'0' + nibble) as char,
        10..=15 => (b'a' + (nibble - 10)) as char,
        _ => '0',
    }
}

fn percent_encode(raw: &str) -> String {
    let mut output = String::new();
    for byte in raw.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            output.push(char::from(byte));
            continue;
        }
        output.push('%');
        output.push(hex_char((byte >> 4) & 0x0f).to_ascii_uppercase());
        output.push(hex_char(byte & 0x0f).to_ascii_uppercase());
    }
    output
}

fn default_name_from_email(email: &str) -> String {
    email
        .split('@')
        .next()
        .map(|value| value.replace('.', " "))
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "Khala Import User".to_string())
}

fn sanitize_identifier(raw: &str) -> String {
    let mut output = String::new();
    for ch in raw.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
            output.push(ch);
        } else {
            output.push('_');
        }
    }
    let trimmed = output.trim_matches('_').to_string();
    if trimmed.is_empty() {
        "id".to_string()
    } else {
        trimmed
    }
}

fn allocate_unique_user_id(candidate: &str, users_by_id: &Map<String, Value>) -> String {
    let base = sanitize_identifier(candidate);
    if !users_by_id.contains_key(&base) {
        return base;
    }

    for suffix in 2..=10_000u32 {
        let next = format!("{base}_{suffix}");
        if !users_by_id.contains_key(&next) {
            return next;
        }
    }

    format!("{}_{}", base, Utc::now().timestamp())
}

fn normalized_abilities(abilities: Vec<String>) -> Vec<String> {
    let mut normalized = HashSet::new();
    for ability in abilities {
        for value in ability.split(',') {
            if let Some(trimmed) = non_empty(value) {
                normalized.insert(trimmed);
            }
        }
    }

    if normalized.is_empty() {
        return vec!["*".to_string()];
    }

    let mut sorted: Vec<String> = normalized.into_iter().collect();
    sorted.sort();
    sorted
}

async fn send_json(builder: reqwest::RequestBuilder) -> Result<Value> {
    let response = builder.send().await.context("request execution failed")?;
    let status = response.status();
    let body = response
        .text()
        .await
        .context("failed to read response body")?;

    let parsed: Value = serde_json::from_str(&body).unwrap_or_else(|_| Value::String(body.clone()));
    if !status.is_success() {
        let body_preview = match parsed {
            Value::String(_) => body,
            _ => serde_json::to_string(&parsed)
                .unwrap_or_else(|_| "<non-serializable body>".to_string()),
        };
        bail!(
            "request failed with status {}: {}",
            status.as_u16(),
            body_preview
        );
    }

    Ok(parsed)
}

fn print_json(value: &Value) -> Result<()> {
    let rendered = serde_json::to_string_pretty(value).context("failed to render JSON output")?;
    println!("{rendered}");
    Ok(())
}
