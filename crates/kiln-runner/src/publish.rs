//! Kiln's self-edit (`docs/kiln/self-edit.md`): test Kiln's own crates with
//! an admin's changes, build the page module and tools with the fast
//! staging settings, smoke-test the build in a headless browser, and
//! publish it over the host's copy as `PREFIX/...` in a livehost tenant's
//! manifest under a signed pointer. Also `revert` (point back at the
//! manifest before the last publish) and `reset` (drop the prefix, so the
//! host serves the build it shipped with).
//!
//! The publishing key and the store's credentials stay on this computer;
//! the page only ever sends a sealed job, and only keys on the admin list
//! get one run.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::time::Instant;

use kiln::remote::{self, Job, Outcome};
use serde_json::{Value, json};

use crate::{Config, cargo_outcome, run_sandboxed, stage, stopped};

/// Where and as whom a self-edit publishes.
pub(crate) struct Publish {
    pub store: livehost::store::Store,
    pub signer: nostr::domain::RelaySigner,
    pub tenant: String,
    pub channel: String,
    pub relay: String,
    pub prefix: String,
    /// The admin list: hex keys or `npub`s allowed to ask for `publish`,
    /// `revert`, and `reset`, read again for every job so a key can be
    /// added or removed without a restart.
    pub admins: PathBuf,
    /// Skip the headless browser smoke test (no `openagents` CLI here).
    pub browser_smoke: bool,
}

impl Publish {
    /// Whether `pubkey` is on the admin list right now.
    pub fn admits(&self, pubkey: &str) -> bool {
        read_admins(&self.admins).is_ok_and(|keys| keys.contains(pubkey))
    }
}

/// Read the admin list: one key a line, hex or `npub`; `#` starts a comment.
pub(crate) fn read_admins(path: &Path) -> Result<BTreeSet<String>, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut keys = BTreeSet::new();
    for line in text.lines() {
        let line = line.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        if line.len() == 64 && line.bytes().all(|b| b.is_ascii_hexdigit()) {
            keys.insert(line.to_lowercase());
        } else if let Ok(bytes) = nostr::nip19::decode_npub(line) {
            keys.insert(bytes.iter().map(|b| format!("{b:02x}")).collect());
        } else {
            return Err(format!("{}: {line} is not a key", path.display()));
        }
    }
    Ok(keys)
}

fn history_path(config: &Config) -> PathBuf {
    config.state.join("kiln-history.json")
}

fn history(config: &Config) -> Vec<Value> {
    std::fs::read(history_path(config))
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}

fn save_history(config: &Config, entries: &[Value]) {
    let _ = std::fs::write(
        history_path(config),
        serde_json::to_vec_pretty(entries).unwrap_or_default(),
    );
}

fn collect(root: &Path, dir: &Path, out: &mut BTreeMap<String, Vec<u8>>) -> std::io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            collect(root, &path, out)?;
        } else if let Ok(relative) = path.strip_prefix(root)
            && let Some(relative) = relative.to_str()
        {
            out.insert(relative.replace('\\', "/"), std::fs::read(&path)?);
        }
    }
    Ok(())
}

/// Check the build's shape: every file the page needs, a WebAssembly
/// module that exports the page's entry points, and tools that match the
/// hashes the manifest names.
fn shape(files: &BTreeMap<String, Vec<u8>>) -> Result<(), String> {
    for needed in [
        "index.html",
        "worker.js",
        "pkg/kiln.js",
        "pkg/kiln_bg.wasm",
        "tools/manifest.json",
    ] {
        if !files.contains_key(needed) {
            return Err(format!("the build has no {needed}"));
        }
    }
    let wasm = &files["pkg/kiln_bg.wasm"];
    if !wasm.starts_with(b"\0asm") {
        return Err("pkg/kiln_bg.wasm is not WebAssembly".into());
    }
    let js = String::from_utf8_lossy(&files["pkg/kiln.js"]);
    for export in ["start", "run_task", "start_review"] {
        if !js.contains(&format!("export function {export}")) {
            return Err(format!("pkg/kiln.js doesn't export {export}"));
        }
    }
    let manifest: Value = serde_json::from_slice(&files["tools/manifest.json"])
        .map_err(|e| format!("tools/manifest.json: {e}"))?;
    let tools = manifest["tools"].as_object().ok_or("no tools")?;
    for (name, entry) in tools {
        let hash = entry["sha256"].as_str().unwrap_or_default();
        let file = files
            .get(&format!("tools/{hash}.wasm"))
            .ok_or_else(|| format!("tool {name} is missing"))?;
        if kiln::sha256_hex(file) != hash {
            return Err(format!("tool {name} doesn't match its hash"));
        }
    }
    Ok(())
}

/// Serve `dir` locally and run the page's sandbox self-test in a headless
/// browser through `openagents browser run` and this binary's `smoke`.
fn browser_smoke(dir: &Path) -> Result<(), String> {
    let port = kiln::serve::spawn(kiln::serve::Server {
        root: dir.to_path_buf(),
        config: "{}".into(),
        overlay: None,
        log: false,
    })
    .map_err(|e| format!("smoke server: {e}"))?;
    let me = std::env::current_exe().map_err(|e| e.to_string())?;
    let url = format!("http://127.0.0.1:{port}/?selftest=1");
    let mut child = std::process::Command::new("openagents")
        .args(["browser", "run", "--timeout", "30", "--"])
        .arg(me)
        .args(["smoke", &url])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("openagents browser run: {e}"))?;
    // At most a minute; `openagents browser run` ends its browser when the
    // smoke command ends or is stopped.
    let deadline = Instant::now() + std::time::Duration::from_secs(60);
    let finished = loop {
        if child.try_wait().map_err(|e| e.to_string())?.is_some() {
            break true;
        }
        if Instant::now() > deadline {
            let _ = child.kill();
            break false;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    };
    let out = child.wait_with_output().map_err(|e| e.to_string())?;
    if !finished {
        return Err("the page's self-test didn't finish in a minute".into());
    }
    if out.status.success() {
        Ok(())
    } else {
        let text = String::from_utf8_lossy(&out.stdout).to_string()
            + &String::from_utf8_lossy(&out.stderr);
        Err(format!("the page's self-test failed: {}", text.trim()))
    }
}

/// Test, build, smoke-test, and publish. Blocking; runs on a job thread.
pub(crate) fn publish(
    config: &Config,
    publish: &Publish,
    job: &Job,
    progress: &dyn Fn(String),
) -> Outcome {
    let started = Instant::now();
    let mut phases = serde_json::Map::new();
    let mut mark = Instant::now();
    let mut phase = |name: &str, mark: &mut Instant| {
        phases.insert(name.into(), json!(mark.elapsed().as_millis() as u64));
        *mark = Instant::now();
    };
    progress("preparing the workspace".into());
    if let Err(e) = stage(config, job) {
        return stopped(
            started,
            format!("not published: could not prepare the workspace: {e}"),
        );
    }
    phase("stage", &mut mark);
    progress("cargo test -p kiln -p kiln-tools".into());
    let args: Vec<String> = ["test", "-p", "kiln", "-p", "kiln-tools"]
        .iter()
        .map(|s| (*s).to_string())
        .collect();
    let tested = cargo_outcome(config, &args, Instant::now(), progress);
    phase("test", &mut mark);
    if tested.code != Some(0) {
        let mut out = tested;
        out.summary = format!("not published: {}", out.summary);
        out.ms = started.elapsed().as_millis() as u64;
        return out;
    }
    progress("building the page module and tools".into());
    let out_dir = config.state.join("kiln-build");
    let _ = std::fs::remove_dir_all(&out_dir);
    if let Err(e) = std::fs::create_dir_all(&out_dir) {
        return stopped(started, format!("not published: {e}"));
    }
    let work = config.state.join("work");
    let mut env: Vec<(String, String)> = Vec::new();
    for profile in ["KILN_WEB", "GUEST"] {
        env.push((format!("CARGO_PROFILE_{profile}_LTO"), "false".into()));
        env.push((
            format!("CARGO_PROFILE_{profile}_CODEGEN_UNITS"),
            "16".into(),
        ));
        env.push((
            format!("CARGO_PROFILE_{profile}_INCREMENTAL"),
            "true".into(),
        ));
    }
    env.push(("CARGO_NET_OFFLINE".into(), "true".into()));
    env.push(("OA_STAGING_BUILD".into(), "1".into()));
    let shims = work
        .join("scripts/deploy/staging-shims")
        .display()
        .to_string();
    env.push(("OA_STAGING_SHIMS".into(), shims.clone()));
    env.push(("PATH".into(), shims));
    for (name, tool) in [
        ("CC_wasm32_unknown_unknown", "clang"),
        ("AR_wasm32_unknown_unknown", "llvm-ar"),
    ] {
        let candidate = PathBuf::from("/opt/homebrew/opt/llvm/bin").join(tool);
        if candidate.is_file() {
            env.push((name.into(), candidate.display().to_string()));
        }
    }
    let built = run_sandboxed(
        config,
        "bash",
        &[
            "scripts/build-kiln-web.sh".into(),
            out_dir.display().to_string(),
        ],
        &env,
        &[out_dir.clone()],
        progress,
    );
    phase("build", &mut mark);
    match built {
        Ok(ran) if ran.ending.code() == Some(0) => {}
        Ok(ran) => {
            let mut out = stopped(started, "not published: the build failed".into());
            out.code = ran.ending.code();
            out.log = remote::trim_log(&ran.text, remote::MAX_LOG_BYTES);
            return out;
        }
        Err(e) => return stopped(started, format!("not published: {e}")),
    }
    // Name the build in tools/manifest.json, which the page polls.
    let mut files = BTreeMap::new();
    if let Err(e) = collect(&out_dir, &out_dir, &mut files) {
        return stopped(started, format!("not published: {e}"));
    }
    let wasm = kiln::sha256_hex(files.get("pkg/kiln_bg.wasm").map_or(&[][..], Vec::as_slice));
    let files_digest = kiln::sha256_hex(
        serde_json::to_string(&job.files)
            .unwrap_or_default()
            .as_bytes(),
    );
    let build = format!("{}", &wasm[..12]);
    if let Some(bytes) = files.get_mut("tools/manifest.json")
        && let Ok(mut manifest) = serde_json::from_slice::<Value>(bytes)
    {
        manifest["kiln"] = json!({
            "build": build,
            "wasm": wasm,
            "base": job.base,
            "files": files_digest,
            "published_at_ms": std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_millis() as u64),
        });
        *bytes = serde_json::to_vec(&manifest).unwrap_or_default();
        let _ = std::fs::write(out_dir.join("tools/manifest.json"), &*bytes);
    }
    // The compressed copies of the old manifest would shadow the new one.
    files.retain(|path, _| !path.starts_with("tools/manifest.json."));
    progress("smoke test".into());
    if let Err(e) = shape(&files) {
        return stopped(started, format!("not published: {e}"));
    }
    if publish.browser_smoke
        && let Err(e) = browser_smoke(&out_dir)
    {
        return stopped(started, format!("not published: {e}"));
    }
    phase("smoke", &mut mark);
    progress("uploading and signing the pointer".into());
    let target = livehost::deploy::Target {
        store: &publish.store,
        relay: &publish.relay,
        signer: &publish.signer,
        channel: &publish.channel,
    };
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(e) => return stopped(started, format!("not published: {e}")),
    };
    let report = runtime.block_on(livehost::deploy::overlay(
        &publish.tenant,
        &publish.prefix,
        &files,
        &target,
    ));
    let report = match report {
        Ok(report) => report,
        Err(e) => return stopped(started, format!("not published: {e:#}")),
    };
    phase("publish", &mut mark);
    let livehost_phases: Vec<(String, f64)> = report.phases.clone();
    for (name, ms) in &livehost_phases {
        phases.insert(format!("livehost_{name}"), json!(*ms as u64));
    }
    let manifest = report.manifest.clone().unwrap_or_default();
    let mut entries = history(config);
    entries.push(json!({
        "manifest": manifest,
        "previous": report.previous,
        "build": build,
        "base": job.base,
        "at": crate::unix(),
    }));
    save_history(config, &entries);
    let mut out = stopped(
        started,
        format!(
            "published Kiln build {build} as {}/{} manifest {}",
            publish.tenant,
            publish.channel,
            &manifest[..manifest.len().min(12)]
        ),
    );
    out.code = Some(0);
    out.log = tested.log;
    out.published = Some(json!({
        "manifest": manifest,
        "previous": report.previous,
        "build": build,
        "uploaded_bytes": report.bytes_uploaded,
        "phases_ms": phases,
    }));
    out
}

/// `revert` or `reset`.
pub(crate) fn restore(config: &Config, publish: &Publish, action: &str) -> Outcome {
    let started = Instant::now();
    let target = livehost::deploy::Target {
        store: &publish.store,
        relay: &publish.relay,
        signer: &publish.signer,
        channel: &publish.channel,
    };
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(e) => return stopped(started, e.to_string()),
    };
    let mut entries = history(config);
    let result = if action == "reset" {
        runtime
            .block_on(livehost::deploy::overlay(
                &publish.tenant,
                &publish.prefix,
                &BTreeMap::new(),
                &target,
            ))
            .map(|r| {
                entries.push(json!({"manifest": r.manifest, "previous": r.previous, "build": "image", "at": crate::unix()}));
                format!("reset: the host serves its own Kiln build (manifest {})", r.manifest.unwrap_or_default())
            })
    } else {
        let Some(last) = entries.pop() else {
            return stopped(
                started,
                "nothing to revert; reset serves the host's own build".into(),
            );
        };
        let Some(previous) = last["previous"].as_str() else {
            return stopped(
                started,
                "the last publish had nothing before it; use reset".into(),
            );
        };
        runtime
            .block_on(livehost::deploy::rollback(
                &publish.tenant,
                &target,
                Some(previous),
            ))
            .map(|_| {
                format!(
                    "reverted build {} to manifest {previous}",
                    last["build"].as_str().unwrap_or("?")
                )
            })
    };
    match result {
        Ok(summary) => {
            save_history(config, &entries);
            let mut out = stopped(started, summary);
            out.code = Some(0);
            out
        }
        Err(e) => stopped(started, format!("{action} failed: {e:#}")),
    }
}

/// `kiln-runner smoke URL`, under `openagents browser run`: open URL in
/// the browser it started and wait up to 25 s for `window.kilnSelftest`.
pub(crate) async fn smoke(url: &str) -> Result<(), String> {
    tokio::time::timeout(std::time::Duration::from_secs(40), smoke_inner(url))
        .await
        .map_err(|_| "the smoke test timed out".to_string())?
}

async fn smoke_inner(url: &str) -> Result<(), String> {
    use futures_util::{SinkExt, StreamExt};
    use std::io::{Read, Write};
    let port = std::env::var("OPENAGENTS_CHROME_PORT").map_err(|_| "no OPENAGENTS_CHROME_PORT")?;
    let encoded: String = url
        .bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect();
    let mut stream = std::net::TcpStream::connect((
        "127.0.0.1",
        port.parse::<u16>().map_err(|e| e.to_string())?,
    ))
    .map_err(|e| e.to_string())?;
    write!(
        stream,
        "PUT /json/new?{encoded} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\nContent-Length: 0\r\n\r\n"
    )
    .map_err(|e| e.to_string())?;
    // Chrome keeps the connection open, so read until the body parses.
    stream
        .set_read_timeout(Some(std::time::Duration::from_secs(10)))
        .map_err(|e| e.to_string())?;
    let mut answer = Vec::new();
    let mut chunk = [0u8; 4096];
    let target: Value = loop {
        let n = stream
            .read(&mut chunk)
            .map_err(|e| format!("devtools: {e}"))?;
        if n == 0 {
            return Err("devtools closed the connection".into());
        }
        answer.extend_from_slice(&chunk[..n]);
        let text = String::from_utf8_lossy(&answer);
        if let Some(body) = text.split("\r\n\r\n").nth(1)
            && let Ok(value) = serde_json::from_str::<Value>(body)
        {
            break value;
        }
    };
    let ws_url = target["webSocketDebuggerUrl"]
        .as_str()
        .ok_or("no debugger url")?;
    // Chrome may name `localhost`, which can resolve to an address it
    // doesn't listen on; use the port it gave.
    let path = ws_url
        .split("/devtools/")
        .nth(1)
        .ok_or("odd debugger url")?;
    let ws_url = format!("ws://127.0.0.1:{port}/devtools/{path}");
    let (mut ws, _) = tokio_tungstenite::connect_async(ws_url.as_str())
        .await
        .map_err(|e| e.to_string())?;
    let deadline = Instant::now() + std::time::Duration::from_secs(25);
    let mut id = 0;
    while Instant::now() < deadline {
        id += 1;
        let ask = json!({"id": id, "method": "Runtime.evaluate", "params": {"expression": "window.kilnSelftest", "returnByValue": true}});
        ws.send(tokio_tungstenite::tungstenite::Message::text(
            ask.to_string(),
        ))
        .await
        .map_err(|e| e.to_string())?;
        while let Some(Ok(message)) = ws.next().await {
            let Ok(text) = message.into_text() else {
                continue;
            };
            let Ok(value) = serde_json::from_str::<Value>(&text) else {
                continue;
            };
            if value["id"] == id {
                match value["result"]["result"]["value"].as_bool() {
                    Some(true) => return Ok(()),
                    Some(false) => return Err("the self-test failed".into()),
                    None => break,
                }
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(250)).await;
    }
    Err("the page didn't finish its self-test in 25 s".into())
}
