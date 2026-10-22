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

mod publish;

const LIFETIME: Duration = Duration::from_secs(115);
const LOG_CAP: u64 = 32 * 1024 * 1024;

pub(crate) struct Config {
    state: PathBuf,
    relay: String,
    crates: Vec<String>,
    reference: Option<PathBuf>,
    repo_url: String,
    timeout: Duration,
    jobs: u32,
    announce: bool,
}

pub(crate) fn unix() -> u64 {
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
pub(crate) fn stage(config: &Config, job: &Job) -> Result<(), String> {
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
    if let Some(patch) = &job.patch {
        // Apply the signed patch itself, so the check is of what reviewers read.
        let file = config.state.join("apply.patch");
        std::fs::write(&file, &patch.content).map_err(|e| e.to_string())?;
        git(
            &work,
            &["apply", "--whitespace=nowarn", &file.to_string_lossy()],
        )?;
    }
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

/// What one sandboxed command did.
pub(crate) struct Ran {
    pub ending: supervise::Ending,
    pub over: bool,
    pub text: String,
}

/// Run `program args` inside the boundary in the worktree: no network,
/// writes only to the worktree, the target directory, the Cargo home, and
/// `writable`, reads only those, the Cargo caches, and the toolchains, with
/// a cleared environment plus `env`, the wall-clock cap, and the log cap.
/// `progress` gets the log's latest line about every two seconds.
pub(crate) fn run_sandboxed(
    config: &Config,
    program: &str,
    args: &[String],
    env: &[(String, String)],
    writable: &[PathBuf],
    progress: &dyn Fn(String),
) -> Result<Ran, String> {
    let started = Instant::now();
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
    for path in writable {
        spec = spec.writable(path);
    }
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
    let boundary = spec
        .build()
        .map_err(|e| format!("the sandbox can't be enforced here: {e}"))?;
    let mut all = vec!["-n".to_string(), "10".to_string(), program.to_string()];
    all.extend(args.iter().cloned());
    let mut command = boundary
        .command("/usr/bin/nice", &all)
        .map_err(|e| format!("the sandbox refused the command: {e}"))?;
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
    for (name, value) in env {
        if name == "PATH" {
            // Put the caller's directories first.
            let joined = format!("{value}:{}", boundary.search_path(&path).to_string_lossy());
            command.env("PATH", joined);
        } else {
            command.env(name, value);
        }
    }
    let log_path = config.state.join("last.log");
    let log = std::fs::File::create(&log_path).map_err(|e| format!("log: {e}"))?;
    let log2 = log
        .try_clone()
        .map_err(|_| "log: cannot share the file".to_string())?;
    command
        .stdout(log)
        .stderr(log2)
        .stdin(std::process::Stdio::null());
    supervise::blocking::own_group(&mut command);
    let mut child = command
        .spawn()
        .map_err(|e| format!("{program} did not start: {e}"))?;
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
        std::thread::sleep(Duration::from_millis(250));
        if std::fs::metadata(&log_path).is_ok_and(|m| m.len() > LOG_CAP) {
            over.store(true, Ordering::SeqCst);
            // SAFETY: `pid` leads the process group `own_group` made.
            unsafe {
                libc::killpg(pid, libc::SIGKILL);
            }
        }
        if started.elapsed().as_millis() % 2000 < 250
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
    Ok(Ran {
        ending,
        over: over.load(Ordering::SeqCst),
        text: String::from_utf8_lossy(&text).into_owned(),
    })
}

/// Cargo's `args` in the boundary, read as an [`Outcome`].
pub(crate) fn cargo_outcome(
    config: &Config,
    args: &[String],
    started: Instant,
    progress: &dyn Fn(String),
) -> Outcome {
    let mut all = vec![args[0].clone(), "--offline".to_string()];
    all.extend(args[1..].iter().cloned());
    let ran = match run_sandboxed(config, "cargo", &all, &[], &[], progress) {
        Ok(ran) => ran,
        Err(e) => return stopped(started, e),
    };
    let code = ran.ending.code();
    let summary = match &ran.ending {
        supervise::Ending::TimedOut => format!("stopped after {} s", config.timeout.as_secs()),
        supervise::Ending::Failed(why) => format!("could not run: {why}"),
        _ if ran.over => "stopped: the log passed 32 MB".into(),
        _ => remote::summarize(code, &ran.text),
    };
    Outcome {
        v: remote::RESULT_V.into(),
        code,
        summary,
        log: remote::trim_log(&ran.text, remote::MAX_LOG_BYTES),
        ms: started.elapsed().as_millis() as u64,
        check: None,
        published: None,
    }
}

/// An outcome for a run that never got to its command.
pub(crate) fn stopped(started: Instant, summary: String) -> Outcome {
    Outcome {
        v: remote::RESULT_V.into(),
        code: None,
        summary,
        log: String::new(),
        ms: started.elapsed().as_millis() as u64,
        check: None,
        published: None,
    }
}

/// Run Cargo for `job` in the boundary. `progress` gets the latest line.
fn run_cargo(config: &Config, job: &Job, progress: &dyn Fn(String)) -> Outcome {
    let started = Instant::now();
    if let Err(e) = stage(config, job) {
        return stopped(started, format!("could not prepare the workspace: {e}"));
    }
    cargo_outcome(config, &job.args, started, progress)
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
    // `kiln-runner smoke URL`: the self-edit's browser smoke test, run under
    // `openagents browser run`.
    let argv: Vec<String> = std::env::args().collect();
    if argv.get(1).map(String::as_str) == Some("smoke") {
        let url = argv.get(2).cloned().unwrap_or_default();
        match publish::smoke(&url).await {
            Ok(()) => {
                println!("the self-test passed");
                std::process::exit(0);
            }
            Err(e) => {
                println!("{e}");
                std::process::exit(1);
            }
        }
    }
    let mut publish_store = String::new();
    let mut publish_key = PathBuf::new();
    let mut publish_tenant = "openagents".to_string();
    let mut publish_channel = "staging".to_string();
    let mut publish_prefix = "modules/kiln".to_string();
    let mut publish_relay = String::new();
    let mut admins_file = PathBuf::new();
    let mut browser_smoke = true;
    let mut config = Config {
        state: PathBuf::new(),
        relay: "wss://relay.openagents.com".into(),
        crates: vec!["memory-stream".into(), "xp-ledger".into(), "atif".into()],
        reference: None,
        repo_url: "https://github.com/OpenAgentsInc/openagents.git".into(),
        timeout: Duration::from_secs(600),
        jobs: 4,
        announce: true,
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
            "--no-announce" => config.announce = false,
            "--admins" => admins_file = PathBuf::from(value()),
            "--publish-store" => publish_store = value(),
            "--publish-key" => publish_key = PathBuf::from(value()),
            "--publish-tenant" => publish_tenant = value(),
            "--publish-channel" => publish_channel = value(),
            "--publish-prefix" => publish_prefix = value(),
            "--publish-relay" => publish_relay = value(),
            "--no-browser-smoke" => browser_smoke = false,
            _ => {
                eprintln!(
                    "usage: kiln-runner --state DIR [--relay URL] [--crates a,b] [--reference CHECKOUT] [--timeout SECS] [--jobs N] [--no-announce]\n\
                     \x20 self-edit: --admins FILE --publish-store STORE --publish-key FILE [--publish-tenant NAME] [--publish-channel NAME] [--publish-prefix PREFIX] [--publish-relay URL] [--no-browser-smoke]"
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
    let repo = format!(
        "{}:{}:{}",
        kiln::patch::REPO_KIND,
        key.pubkey(),
        kiln::patch::DEMO_REPO_ID
    );
    println!("repository {repo}");
    let _ = std::fs::write(config.state.join("repository"), format!("{repo}\n"));
    // Kiln's self-edit, when the runner is given somewhere to publish.
    let publisher = if publish_store.is_empty() {
        None
    } else {
        let store = livehost::store::Store::parse(&publish_store).unwrap_or_else(|e| {
            eprintln!("kiln-runner: --publish-store: {e:#}");
            std::process::exit(2);
        });
        let signer = livehost::keys::load(&publish_key).unwrap_or_else(|e| {
            eprintln!("kiln-runner: --publish-key: {e:#}");
            std::process::exit(2);
        });
        let admins = publish::read_admins(&admins_file).unwrap_or_else(|e| {
            eprintln!("kiln-runner: --admins: {e}");
            std::process::exit(2);
        });
        if admins.is_empty() {
            eprintln!("kiln-runner: warning: --admins names no key yet, so nobody can publish");
        }
        println!(
            "self-edit: publishes {publish_prefix}/ to {publish_tenant}/{publish_channel} as {} for {} admin keys",
            signer.pubkey(),
            admins.len()
        );
        Some(Arc::new(publish::Publish {
            store,
            signer,
            tenant: publish_tenant,
            channel: publish_channel,
            relay: if publish_relay.is_empty() {
                config.relay.clone()
            } else {
                publish_relay
            },
            prefix: publish_prefix,
            admins: admins_file.clone(),
            browser_smoke,
        }))
    };
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
        if config.announce {
            // The demo's NIP-34 repository announcement, under this runner's
            // key; the owner's `openagents` announcement is the owner's.
            let tags = kiln::patch::announcement_tags(&[key.pubkey()], &config.relay);
            let announcement = key.sign(kiln::patch::REPO_KIND, tags, String::new(), unix());
            let _ = conn.send(json!(["EVENT", announcement])).await;
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
                        .and_then(|job| job.check(&config.crates).map(|()| job))
                        .and_then(|job| match &job.patch {
                            Some(patch) if nostr::domain::open_patch(patch).ok().as_deref() != Some(repo.as_str()) => {
                                Err("the patch names another repository".to_string())
                            }
                            _ => Ok(job),
                        })
                        .and_then(|job| match (&job.action, &publisher) {
                            (None, _) => Ok(job),
                            (Some(_), None) => Err("this runner doesn't publish Kiln".to_string()),
                            (Some(_), Some(p)) if !p.admits(&request.pubkey) => {
                                Err("only an admin's key may publish, revert, or reset Kiln".to_string())
                            }
                            (Some(_), Some(_)) => Ok(job),
                        });
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
                    println!("job {} from {}: {} with {} files at {}", &request.id[..8], &request.pubkey[..8], job.action.clone().unwrap_or_else(|| format!("cargo {}", job.args.join(" "))), job.files.len(), &job.base[..10]);
                    let (key, config, out, busy) = (key.clone(), config.clone(), out_tx.clone(), busy.clone());
                    let publisher = publisher.clone();
                    let relay_url = config.relay.clone();
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
                        let mut outcome = match (job.action.as_deref(), &publisher) {
                            (Some("publish"), Some(p)) => publish::publish(&config, p, &job, &|line| feedback(Some(line))),
                            (Some(action), Some(p)) => publish::restore(&config, p, action),
                            _ => run_cargo(&config, &job, &|line| feedback(Some(line))),
                        };
                        if let Some(patch) = &job.patch {
                            let passed = outcome.code == Some(0);
                            let digest = kiln::sha256_hex(patch.content.as_bytes());
                            let mut tags = kiln::patch::check_tags(&patch.id, &patch.pubkey, &relay_url, passed);
                            tags.push(Tag::new(vec!["kiln-base".into(), job.base.clone()]));
                            tags.push(Tag::new(vec!["kiln-patch-sha256".into(), digest.clone()]));
                            let content = format!(
                                "{} `cargo {}`: {}\n\nApplied this patch (sha256 {digest}) with `git apply` at {} and ran it offline in a sandbox on kiln-runner, in {:.1} s.",
                                if passed { "Passed" } else { "Failed" },
                                job.args.join(" "),
                                outcome.summary,
                                job.base,
                                outcome.ms as f64 / 1000.0
                            );
                            let comment = key.sign(kiln::patch::COMMENT_KIND, tags, content, unix());
                            outcome.check = Some(comment.id.clone());
                            let _ = out.send(comment);
                        }
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
