//! `kiln-runner`: runs Kiln's remote `cargo` jobs on this computer.
//!
//! ```text
//! kiln-runner --state DIR [--relay URL] [--crates a,b] [--reference CHECKOUT]
//!             [--timeout SECS] [--jobs N]
//! ```
//!
//! The runner holds its own key in `DIR/key`, listens on the relay for
//! NIP-CJ conversation requests (`25900`) addressed to it, and treats each
//! request's task as a [`kiln::remote::Job`]: changed files at a commit and
//! a `cargo test -p CRATE` or `cargo check -p CRATE` command. For each job
//! it:
//!
//! 1. fetches the commit into its own shallow clone (`DIR/repo`) when it
//!    lacks it;
//! 2. checks out the commit in its one worktree (`DIR/work`), removes
//!    every change and untracked file, and writes the job's files;
//! 3. runs Cargo `--offline` inside `coder-boundary`: no network, writes
//!    only to the worktree, `DIR/target`, and `DIR/cargo-home`, reads only
//!    the toolchains, the worktree, and the Cargo caches; with a cleared
//!    environment, a wall-clock cap, and a log cap;
//! 4. sends `processing` feedback with the latest log line every two
//!    seconds, and the result: exit code, test summary, and the log's end.
//!
//! It runs one job at a time and answers a second with `busy`. It publishes
//! no beacon. Read `docs/kiln/rust-tests.md`.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use kiln::cj::{self, Key};
use kiln::remote::{self, Job, Outcome};
use nostr::domain::{Event, Tag};
use serde_json::{Value, json};
use tokio::sync::mpsc;

const LIFETIME: Duration = Duration::from_secs(115);
const LOG_CAP: u64 = 32 * 1024 * 1024;

struct Config {
    state: PathBuf,
    relay: String,
    crates: Vec<String>,
    reference: Option<PathBuf>,
    repo_url: String,
    timeout: Duration,
    jobs: u32,
}

fn unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

fn nonce() -> [u8; 32] {
    secp256k1::rand::random()
}

fn load_key(path: &Path) -> Result<Key, String> {
    if let Ok(text) = std::fs::read_to_string(path) {
        let text = text.trim();
        let mut bytes = [0u8; 32];
        if text.len() != 64 {
            return Err(format!("{} does not hold a 64-hex key", path.display()));
        }
        for (i, b) in bytes.iter_mut().enumerate() {
            *b = u8::from_str_radix(&text[2 * i..2 * i + 2], 16).map_err(|e| e.to_string())?;
        }
        return Key::from_bytes(bytes);
    }
    let bytes: [u8; 32] = secp256k1::rand::random();
    let key = Key::from_bytes(bytes)?;
    let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .map_err(|e| e.to_string())?;
    writeln!(file, "{hex}").map_err(|e| e.to_string())?;
    Ok(key)
}

fn git(dir: &Path, args: &[&str]) -> Result<String, String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .map_err(|e| format!("git: {e}"))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    } else {
        Err(format!(
            "git {}: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        ))
    }
}

/// Make the clone, the worktree, and the Cargo home once.
fn prepare(config: &Config) -> Result<(), String> {
    let repo = config.state.join("repo");
    if !repo.join("HEAD").exists() && !repo.join(".git").exists() {
        let mut clone = Command::new("git");
        clone.args(["clone", "--bare", "--quiet", "--depth", "1"]);
        if let Some(reference) = &config.reference {
            // Borrow objects from a local checkout, read only.
            clone.arg("--reference").arg(reference);
        }
        let status = clone
            .arg(&config.repo_url)
            .arg(&repo)
            .status()
            .map_err(|e| e.to_string())?;
        if !status.success() {
            return Err("git clone failed".into());
        }
    }
    let work = config.state.join("work");
    if !work.join(".git").exists() {
        let head = git(&repo, &["rev-parse", "HEAD"])?;
        git(
            &repo,
            &[
                "worktree",
                "add",
                "--detach",
                "--quiet",
                &work.to_string_lossy(),
                head.trim(),
            ],
        )?;
    }
    let home = config.state.join("cargo-home");
    std::fs::create_dir_all(&home).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(config.state.join("target")).map_err(|e| e.to_string())?;
    let user_cargo = std::env::var_os("CARGO_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cargo")))
        .ok_or("no Cargo home")?;
    for cache in ["registry", "git"] {
        let link = home.join(cache);
        if !link.exists() {
            std::os::unix::fs::symlink(user_cargo.join(cache), &link)
                .map_err(|e| format!("linking the Cargo {cache} cache: {e}"))?;
        }
    }
    Ok(())
}

/// Check out `job.base` cleanly and write the job's files.
fn stage(config: &Config, job: &Job) -> Result<(), String> {
    let repo = config.state.join("repo");
    if git(
        &repo,
        &["cat-file", "-e", &format!("{}^{{commit}}", job.base)],
    )
    .is_err()
    {
        git(
            &repo,
            &["fetch", "--quiet", "--depth", "1", "origin", &job.base],
        )?;
    }
    let work = config.state.join("work");
    git(
        &work,
        &["checkout", "--quiet", "--detach", "--force", &job.base],
    )?;
    git(&work, &["clean", "-fdxq"])?;
    for (path, content) in &job.files {
        let file = work.join(path);
        if let Some(parent) = file.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        std::fs::write(&file, content).map_err(|e| format!("{path}: {e}"))?;
    }
    for path in &job.deleted {
        let _ = std::fs::remove_file(work.join(path));
    }
    Ok(())
}

/// The last log line worth showing.
fn last_line(log: &Path) -> Option<String> {
    let text = std::fs::read(log).ok()?;
    let start = text.len().saturating_sub(4096);
    String::from_utf8_lossy(&text[start..])
        .lines()
        .rev()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .map(|l| l.chars().take(160).collect())
}

/// The Cargo configuration files in the directories above `dir`.
fn user_cargo_configs(dir: &Path) -> Vec<PathBuf> {
    dir.ancestors()
        .skip(1)
        .flat_map(|a| [a.join(".cargo/config.toml"), a.join(".cargo/config")])
        .filter(|p| p.is_file())
        .collect()
}

/// Run Cargo for `job` in the boundary. `progress` gets the latest line.
fn run_cargo(config: &Config, job: &Job, progress: &dyn Fn(String)) -> Outcome {
    let started = Instant::now();
    let stopped = |summary: String| Outcome {
        v: remote::RESULT_V.into(),
        code: None,
        summary,
        log: String::new(),
        ms: started.elapsed().as_millis() as u64,
    };
    if let Err(e) = stage(config, job) {
        return stopped(format!("could not prepare the workspace: {e}"));
    }
    let work = config.state.join("work");
    let target = config.state.join("target");
    let home = config.state.join("cargo-home");
    let toolchains =
        coder_boundary::Toolchains::derive(&coder_boundary::toolchains::Host::this_computer());
    let mut spec = coder_boundary::Boundary::writing(&work)
        .readable(&work)
        .writable(&target)
        .writable(&home)
        .owned_scratch_under(std::env::temp_dir())
        .offline();
    for read in &toolchains.reads {
        spec = spec.readable(&read.path);
    }
    for cache in ["registry", "git"] {
        if let Ok(real) = home.join(cache).canonicalize() {
            spec = spec.readable(real);
        }
    }
    // Cargo reads the configuration in every directory above the worktree,
    // which includes the person's `~/.cargo/config.toml` when the state
    // directory is under home. Those files hold build settings, not
    // credentials (`credentials.toml` stays unreadable); the wrapper they
    // may name is switched off below.
    for config in user_cargo_configs(&work) {
        spec = spec.readable(config);
    }
    let boundary = match spec.build() {
        Ok(boundary) => boundary,
        Err(e) => return stopped(format!("the sandbox can't be enforced here: {e}")),
    };
    let mut args = vec![
        "-n".to_string(),
        "10".to_string(),
        "cargo".to_string(),
        job.args[0].clone(),
        "--offline".to_string(),
    ];
    args.extend(job.args[1..].iter().cloned());
    let mut command = match boundary.command("/usr/bin/nice", &args) {
        Ok(command) => command,
        Err(e) => return stopped(format!("the sandbox refused the command: {e}")),
    };
    let path = std::env::join_paths(&toolchains.path).unwrap_or_default();
    command
        .current_dir(&work)
        .env_clear()
        .env("PATH", boundary.search_path(&path))
        .env("HOME", boundary.scratch().unwrap_or(&work))
        .env("CARGO_HOME", &home)
        .env("CARGO_TARGET_DIR", &target)
        .env("CARGO_BUILD_JOBS", config.jobs.to_string())
        .env("CARGO_TERM_COLOR", "never")
        .env("RUSTC_WRAPPER", "")
        .env("CARGO_BUILD_RUSTC_WRAPPER", "")
        .env("TMPDIR", boundary.scratch().unwrap_or(&work));
    for (name, value) in &toolchains.environment {
        if name != "CARGO_HOME" {
            command.env(name, value);
        }
    }
    let log_path = config.state.join("last.log");
    let log = match std::fs::File::create(&log_path) {
        Ok(log) => log,
        Err(e) => return stopped(format!("log: {e}")),
    };
    let Ok(log2) = log.try_clone() else {
        return stopped("log: cannot share the file".into());
    };
    command
        .stdout(log)
        .stderr(log2)
        .stdin(std::process::Stdio::null());
    supervise::blocking::own_group(&mut command);
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(e) => return stopped(format!("cargo did not start: {e}")),
    };
    let pid = child.id() as i32;
    let done = Arc::new(AtomicBool::new(false));
    let over = Arc::new(AtomicBool::new(false));
    let wall = config.timeout;
    let waiter = {
        let done = done.clone();
        std::thread::spawn(move || {
            let ending = supervise::blocking::wait(&mut child, wall);
            done.store(true, Ordering::SeqCst);
            ending
        })
    };
    let mut shown = String::new();
    while !done.load(Ordering::SeqCst) {
        std::thread::sleep(Duration::from_millis(500));
        if std::fs::metadata(&log_path).is_ok_and(|m| m.len() > LOG_CAP) {
            over.store(true, Ordering::SeqCst);
            // SAFETY: `pid` leads the process group `own_group` made.
            unsafe {
                libc::killpg(pid, libc::SIGKILL);
            }
        }
        if started.elapsed().as_millis() % 2000 < 500
            && let Some(line) = last_line(&log_path)
            && line != shown
        {
            shown = line.clone();
            progress(line);
        }
    }
    let ending = waiter
        .join()
        .unwrap_or(supervise::Ending::Failed("the wait panicked".into()));
    drop(boundary);
    let text = std::fs::read(&log_path).unwrap_or_default();
    let text = String::from_utf8_lossy(&text);
    let code = ending.code();
    let summary = match ending {
        supervise::Ending::TimedOut => format!("stopped after {} s", wall.as_secs()),
        supervise::Ending::Failed(why) => format!("could not run: {why}"),
        _ if over.load(Ordering::SeqCst) => "stopped: the log passed 32 MB".into(),
        _ => remote::summarize(code, &text),
    };
    Outcome {
        v: remote::RESULT_V.into(),
        code,
        summary,
        log: remote::trim_log(&text, remote::MAX_LOG_BYTES),
        ms: started.elapsed().as_millis() as u64,
    }
}

fn reply(key: &Key, request: &Event, kind: u16, body: &Value) -> Option<Event> {
    let tags = vec![
        Tag::new(vec!["p".into(), request.pubkey.clone()]),
        Tag::new(vec!["e".into(), request.id.clone()]),
    ];
    key.seal(&request.pubkey, kind, tags, body, nonce(), unix())
        .ok()
}

fn error_body(code: &str, message: &str) -> Value {
    json!({"v": 1, "requires": [], "type": "status", "status": "error", "code": code, "message": message})
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let mut config = Config {
        state: PathBuf::new(),
        relay: "wss://relay.openagents.com".into(),
        crates: vec!["memory-stream".into(), "xp-ledger".into(), "atif".into()],
        reference: None,
        repo_url: "https://github.com/OpenAgentsInc/openagents.git".into(),
        timeout: Duration::from_secs(600),
        jobs: 4,
    };
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        let mut value = || args.next().unwrap_or_default();
        match arg.as_str() {
            "--state" => config.state = PathBuf::from(value()),
            "--relay" => config.relay = value(),
            "--crates" => config.crates = value().split(',').map(str::to_string).collect(),
            "--reference" => config.reference = Some(PathBuf::from(value())),
            "--repo" => config.repo_url = value(),
            "--timeout" => config.timeout = Duration::from_secs(value().parse().unwrap_or(600)),
            "--jobs" => config.jobs = value().parse().unwrap_or(4).clamp(1, 6),
            _ => {
                eprintln!(
                    "usage: kiln-runner --state DIR [--relay URL] [--crates a,b] [--reference CHECKOUT] [--timeout SECS] [--jobs N]"
                );
                std::process::exit(2);
            }
        }
    }
    if config.state.as_os_str().is_empty() {
        eprintln!("kiln-runner: --state DIR is required");
        std::process::exit(2);
    }
    if let Err(e) = std::fs::create_dir_all(&config.state) {
        eprintln!("kiln-runner: {}: {e}", config.state.display());
        std::process::exit(1);
    }
    let key = load_key(&config.state.join("key")).unwrap_or_else(|e| {
        eprintln!("kiln-runner: {e}");
        std::process::exit(1);
    });
    let _ = std::fs::write(config.state.join("pubkey"), format!("{}\n", key.pubkey()));
    println!("kiln-runner {} ({})", key.npub(), key.pubkey());
    println!("preparing {}", config.state.display());
    if let Err(e) = prepare(&config) {
        eprintln!("kiln-runner: {e}");
        std::process::exit(1);
    }
    println!(
        "ready: cargo test or check for {} on {}",
        config.crates.join(", "),
        config.relay
    );
    let config = Arc::new(config);
    let (out_tx, mut out_rx) = mpsc::unbounded_channel::<Event>();
    let busy = Arc::new(AtomicBool::new(false));
    let mut seen = HashSet::new();
    let mut pending: Vec<Event> = Vec::new();
    let secret = secp256k1::SecretKey::from_byte_array({
        let text = std::fs::read_to_string(config.state.join("key")).unwrap_or_default();
        let mut bytes = [0u8; 32];
        for (i, b) in bytes.iter_mut().enumerate() {
            *b = u8::from_str_radix(text.trim().get(2 * i..2 * i + 2).unwrap_or("00"), 16)
                .unwrap_or(0);
        }
        bytes
    })
    .unwrap_or_else(|_| std::process::exit(1));
    loop {
        let mut conn =
            match nostr_transport::Connection::connect(&config.relay, &secret, LIFETIME).await {
                Ok(conn) => conn,
                Err(e) => {
                    eprintln!("relay: {e}; retrying");
                    tokio::time::sleep(Duration::from_secs(3)).await;
                    continue;
                }
            };
        let filter = json!({"kinds": [cj::REQUEST_KIND], "#p": [key.pubkey()], "since": unix().saturating_sub(30)});
        if conn.send(json!(["REQ", "jobs", filter])).await.is_err() {
            continue;
        }
        for event in pending.drain(..) {
            let _ = conn.send(json!(["EVENT", event])).await;
        }
        loop {
            tokio::select! {
                frame = conn.next() => {
                    let Ok(frame) = frame else { break };
                    if frame[0] != "EVENT" || frame[1] != "jobs" {
                        continue;
                    }
                    let Ok(request) = serde_json::from_value::<Event>(frame[2].clone()) else { continue };
                    if request.kind != cj::REQUEST_KIND
                        || request.validate_crypto().is_err()
                        || !seen.insert(request.id.clone())
                    {
                        continue;
                    }
                    let job = key
                        .open(&request)
                        .ok()
                        .and_then(|plain| serde_json::from_str::<Value>(&plain).ok())
                        .and_then(|body| body["task"].as_str().map(str::to_string))
                        .ok_or_else(|| "the request does not open".to_string())
                        .and_then(|task| serde_json::from_str::<Job>(&task).map_err(|e| format!("the task is not a job: {e}")))
                        .and_then(|job| job.check(&config.crates).map(|()| job));
                    let job = match job {
                        Ok(job) => job,
                        Err(why) => {
                            println!("refused {}: {why}", &request.id[..8]);
                            if let Some(event) = reply(&key, &request, cj::FEEDBACK_KIND, &error_body("refused", &why)) {
                                let _ = out_tx.send(event);
                            }
                            continue;
                        }
                    };
                    if busy.swap(true, Ordering::SeqCst) {
                        if let Some(event) = reply(&key, &request, cj::FEEDBACK_KIND, &error_body("busy", "another run is in progress; try again in a minute")) {
                            let _ = out_tx.send(event);
                        }
                        continue;
                    }
                    println!("job {} from {}: cargo {} with {} files at {}", &request.id[..8], &request.pubkey[..8], job.args.join(" "), job.files.len(), &job.base[..10]);
                    let (key, config, out, busy) = (key.clone(), config.clone(), out_tx.clone(), busy.clone());
                    std::thread::spawn(move || {
                        let feedback = |note: Option<String>| {
                            let mut body = json!({"v": 1, "requires": [], "type": "status", "status": "processing"});
                            if let Some(note) = note {
                                body["progress"] = json!(note);
                            }
                            if let Some(event) = reply(&key, &request, cj::FEEDBACK_KIND, &body) {
                                let _ = out.send(event);
                            }
                        };
                        feedback(Some("starting".into()));
                        let outcome = run_cargo(&config, &job, &|line| feedback(Some(line)));
                        println!("job {}: {} in {} ms", &request.id[..8], outcome.summary, outcome.ms);
                        let text = serde_json::to_string(&outcome).unwrap_or_default();
                        let body = json!({"v": 1, "requires": [], "type": "result", "text": text, "model": "kiln/cargo"});
                        if let Some(event) = reply(&key, &request, cj::RESULT_KIND, &body) {
                            let _ = out.send(event);
                        }
                        busy.store(false, Ordering::SeqCst);
                    });
                }
                event = out_rx.recv() => {
                    let Some(event) = event else { return };
                    if conn.send(json!(["EVENT", event.clone()])).await.is_err() {
                        pending.push(event);
                        break;
                    }
                }
            }
        }
    }
}
