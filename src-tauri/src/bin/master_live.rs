use anyhow::{anyhow, Result};
use base64::Engine as _;
use chrono::Utc;
use serde_json::Value;
use std::fs::{self, OpenOptions};
use std::io::Write as _;
use std::path::PathBuf;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

fn codex_home() -> PathBuf {
    if let Ok(val) = std::env::var("CODEX_HOME") { if !val.is_empty() { return PathBuf::from(val); } }
    dirs::home_dir().map(|mut h| { h.push(".codex"); h }).expect("home dir")
}

fn log_path(label: &str) -> PathBuf {
    let mut p = codex_home();
    p.push("master-tasks");
    let ts = Utc::now().format("%Y%m%d-%H%M%S");
    p.push(format!("live-{}-{}.log", label, ts));
    p
}

fn append_log(p: &PathBuf, line: &str) -> Result<()> {
    if let Some(parent) = p.parent() { let _ = fs::create_dir_all(parent); }
    let mut f = OpenOptions::new().create(true).append(true).open(p)?;
    let ts = Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    writeln!(f, "{} | {}", ts, line)?;
    Ok(())
}

#[derive(serde::Deserialize)]
struct AuthJson {
    #[serde(rename = "OPENAI_API_KEY")]
    openai_api_key: Option<String>,
    #[serde(default)]
    tokens: Option<AuthTokens>,
}

#[derive(serde::Deserialize)]
struct AuthTokens { id_token: String }

fn preflight_auth_log(logf: &PathBuf, ch: &PathBuf) -> Result<()> {
    let auth = ch.join("auth.json");
    if !auth.exists() {
        append_log(logf, "auth: auth.json not found; will likely 401")?;
        println!("auth: missing at {}", auth.display());
        return Ok(());
    }
    let text = std::fs::read_to_string(&auth).unwrap_or_default();
    let parsed: AuthJson = serde_json::from_str(&text).unwrap_or(AuthJson { openai_api_key: None, tokens: None });
    let mut parts: Vec<String> = Vec::new();
    if let Some(k) = parsed.openai_api_key {
        if !k.is_empty() { parts.push(format!("api_key=len{}", k.len())); }
    }
    if let Some(t) = parsed.tokens { if !t.id_token.is_empty() { parts.push(format!("id_token=len{}", t.id_token.len())); } }
    if parts.is_empty() { parts.push("none".into()); }
    let line = format!("auth: present [{}]", parts.join(", "));
    append_log(logf, &line)?; println!("{} (from {})", line, auth.display());
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    // Args: <label> [max_seconds] [--prompt "..."] [--path "/abs/path"]
    let mut args_iter = std::env::args().skip(1);
    let label = args_iter.next().unwrap_or_else(|| "default".to_string());
    let mut max_secs: u64 = 60;
    let mut custom_prompt: Option<String> = None;
    let mut abs_path: Option<String> = None;
    let mut rest: Vec<String> = Vec::new();
    if let Some(next) = args_iter.next() {
        if let Ok(n) = next.parse::<u64>() { max_secs = n; rest.extend(args_iter); } else { rest.push(next); rest.extend(args_iter); }
    }
    let mut i = 0usize;
    while i < rest.len() {
        match rest[i].as_str() {
            "--prompt" => { if i+1 < rest.len() { custom_prompt = Some(rest[i+1].clone()); i += 2; } else { i += 1; } }
            "--path" => { if i+1 < rest.len() { abs_path = Some(rest[i+1].clone()); i += 2; } else { i += 1; } }
            _ => { i += 1; }
        }
    }
    let logf = log_path(&label);
    append_log(&logf, "live run starting")?;

    // Spawn protocol
    let cwd = std::env::current_dir()?;
    let codex_dir = cwd.join("codex-rs");
    let mut cmd;
    // Resolve model from env if provided; compiled default is gpt-5
    let model = std::env::var("CODEX_MODEL").unwrap_or_else(|_| "gpt-5".to_string());
    if codex_dir.join("Cargo.toml").exists() {
        cmd = Command::new("cargo");
        cmd.arg("run").arg("-q").arg("-p").arg("codex-cli").arg("--")
            .arg("proto")
            .arg("-c").arg("approval_policy=never")
            .arg("-c").arg("sandbox_mode=read-only")
            .arg("-c").arg(format!("model={}", model))
            .current_dir(&codex_dir);
    } else {
        cmd = Command::new("codex");
        cmd.arg("proto")
            .arg("-c").arg("approval_policy=never")
            .arg("-c").arg("sandbox_mode=read-only")
            .arg("-c").arg(format!("model={}", model));
    }
    cmd.stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    let mut child = cmd.spawn()?;
    let mut stdin = child.stdin.take().ok_or_else(|| anyhow!("no stdin"))?;
    let stdout = child.stdout.take().ok_or_else(|| anyhow!("no stdout"))?;
    let mut lines = BufReader::new(stdout).lines();

    // Writer task pipe
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<Vec<u8>>();
    tokio::spawn(async move {
        use tokio::io::AsyncWriteExt as _;
        while let Some(buf) = rx.recv().await {
            let _ = stdin.write_all(&buf).await;
            let _ = stdin.flush().await;
        }
    });

    let ch = codex_home();
    append_log(&logf, &format!("waiting for session_configured; CODEX_HOME={}; model={}", ch.display(), model))?;
    // Best-effort auth preflight
    let _ = preflight_auth_log(&logf, &ch);
    let start_wait = std::time::Instant::now();
    let mut session_ready = false;
    let mut prompts_sent = 0usize;
    let mut token_in: u64 = 0;
    let mut token_out: u64 = 0;

    // Reader loop with a timebox
    let max_total = std::time::Duration::from_secs(max_secs);
    while start_wait.elapsed() < max_total {
        let remain = max_total.saturating_sub(start_wait.elapsed());
        // Poll the next line with a short timeout so we don't block past the deadline.
        let per_wait = std::cmp::min(remain, std::time::Duration::from_secs(3));
        match tokio::time::timeout(per_wait, lines.next_line()).await {
        Ok(Ok(Some(line))) => {
            append_log(&logf, &format!("raw: {}", line))?;
            let Ok(v) = serde_json::from_str::<Value>(&line) else { continue };
            if let Some(msg) = v.get("msg") {
                let typ = msg.get("type").and_then(|s| s.as_str()).unwrap_or("");
                match typ {
                    "session_configured" => {
                        session_ready = true;
                        append_log(&logf, "event: session_configured")?;
                        // Send a user prompt once
                        if prompts_sent == 0 {
                            let base = custom_prompt.clone().unwrap_or_else(|| "List top-level files; summarize crates".to_string());
                            let pth = abs_path.clone().unwrap_or_default();
                            let prompt = if pth.is_empty() {
                                format!("{}\nConstraints: read-only", base)
                            } else {
                                format!("{}\nConstraints: read-only. Operate on this absolute path only: {}", base, pth)
                            };
                            let id = "1";
                            let payload = serde_json::json!({
                                "id": id,
                                "op": { "type": "user_input", "items": [ { "type": "text", "text": prompt } ] }
                            });
                            let mut buf = serde_json::to_vec(&payload)?; buf.push(b'\n');
                            tx.send(buf).unwrap();
                            prompts_sent = 1;
                            append_log(&logf, "sent: user_input prompt")?;
                        }
                    }
                    "agent_message_delta" => {
                        let delta = msg.get("delta").and_then(|d| d.as_str()).unwrap_or("");
                        append_log(&logf, &format!("assistant: {}", delta))?;
                    }
                    "exec_command_begin" => {
                        append_log(&logf, "tool: begin exec")?;
                    }
                    "exec_command_output_delta" => {
                        let is_stderr = matches!(msg.get("stream").and_then(|s| s.as_str()), Some("stderr"));
                        if let Some(chunk_b64) = msg.get("chunk").and_then(|d| d.as_str()) {
                            if let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(chunk_b64) {
                                if let Ok(text) = String::from_utf8(bytes) {
                                    let tag = if is_stderr { "stderr" } else { "stdout" };
                                    append_log(&logf, &format!("tool: {} {}", tag, text))?;
                                }
                            }
                        }
                    }
                    "token_count" => {
                        let input = msg.get("usage").and_then(|u| u.get("input_tokens")).and_then(|x| x.as_u64()).unwrap_or(0);
                        let output = msg.get("usage").and_then(|u| u.get("output_tokens")).and_then(|x| x.as_u64()).unwrap_or(0);
                        token_in = input; token_out = output;
                        append_log(&logf, &format!("tokens: in={} out={}", input, output))?;
                    }
                    _ => {}
                }
            }
        }
        Ok(Ok(None)) => break,
        Ok(Err(_)) => break,
        Err(_) => {
            // Periodic timeout tick; continue until overall deadline.
            continue;
        }
        }
    }

    if !session_ready {
        append_log(&logf, "error: session_configured not received (auth missing?)")?;
        eprintln!("LIVE run did not configure a session. Check credentials in $CODEX_HOME/auth.json.");
    }

    // Try to stop the child process to avoid lingering runtime.
    let _ = child.kill();
    append_log(&logf, &format!("live run finished; tokens in={} out={}", token_in, token_out))?;
    println!("LOG={}", logf.display());
    Ok(())
}
