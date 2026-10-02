//! The daily Boat template for Coder runs (issue #10219, Boat B5).
//!
//! `boat-template build` creates a `large` sandbox, runs the shared host setup
//! (`scripts/cloud/coder-host-setup.sh --warm`, the same one the GCE image
//! `oa-coder-host` uses, #10224), which clones `origin/main` into
//! `/home/user/openagents` and compiles it into Coder's first target slot,
//! stops the sandbox, saves it as the named snapshot `oa-coder-main-YYYYMMDD`,
//! keeps the newest three, and deletes the build sandbox. Runs then create
//! sandboxes `from` that name and start with a compiled main.
//!
//! `boat-template probe NAME` measures a template: create from it, time to
//! ready, and `cargo build -p openagents-cli` inside it; then deletes it.
//!
//! Runbook: `docs/deployment/boat-template.md`. The key comes from
//! `BOAT_API_KEY`, else Secret Manager `boat-api-key`; it is never printed.

use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use boat::{Client, Error, Nullable, WaitOptions, models::*};
use serde_json::{Value, json};

const PREFIX: &str = "oa-coder-main-";
/// Boat keeps at most this many named snapshots per account.
const NAMED_LIMIT: usize = 10;
const USAGE: &str = "usage:
  boat-template build [--keep N] [--type large] [--name NAME] [--setup-file PATH] [--keep-sandbox]
  boat-template probe NAME [--package openagents-cli]
  boat-template list
  boat-template prune [--keep N]";

type Fallible<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Err(error) = run(args).await {
        eprintln!("boat-template: {error}");
        std::process::exit(1);
    }
}

async fn run(args: Vec<String>) -> Fallible<()> {
    let Some((command, rest)) = args.split_first() else {
        return Err(USAGE.into());
    };
    let flags = Flags::parse(rest)?;
    let client = Client::from_env().await?;
    let summary = match command.as_str() {
        "build" => build(&client, &flags).await?,
        "probe" => {
            let name = flags.positional.first().ok_or(USAGE)?;
            probe(&client, name, &flags).await?
        }
        "list" => {
            json!({ "templates": templates(&client).await?.iter().map(describe).collect::<Vec<_>>() })
        }
        "prune" => json!({ "deleted": prune(&client, flags.keep, None).await? }),
        _ => return Err(USAGE.into()),
    };
    println!("{summary}");
    Ok(())
}

struct Flags {
    keep: usize,
    size: String,
    name: Option<String>,
    setup_file: Option<String>,
    keep_sandbox: bool,
    package: String,
    positional: Vec<String>,
}

impl Flags {
    fn parse(args: &[String]) -> Fallible<Self> {
        let mut flags = Self {
            keep: 3,
            size: "large".into(),
            name: None,
            setup_file: None,
            keep_sandbox: false,
            package: "openagents-cli".into(),
            positional: Vec::new(),
        };
        let mut it = args.iter();
        while let Some(arg) = it.next() {
            let mut value = || it.next().cloned().ok_or(format!("{arg} needs a value"));
            match arg.as_str() {
                "--keep" => flags.keep = value()?.parse()?,
                "--type" => flags.size = value()?,
                "--name" => flags.name = Some(value()?),
                "--setup-file" => flags.setup_file = Some(value()?),
                "--package" => flags.package = value()?,
                "--keep-sandbox" => flags.keep_sandbox = true,
                other if other.starts_with('-') => {
                    return Err(format!("unknown flag {other}\n{USAGE}").into());
                }
                other => flags.positional.push(other.to_string()),
            }
        }
        if flags.keep == 0 {
            return Err("--keep must be at least 1".into());
        }
        Ok(flags)
    }
}

fn log(message: impl AsRef<str>) {
    eprintln!("[boat-template] {}", message.as_ref());
}

fn secs(since: Instant) -> f64 {
    (since.elapsed().as_secs_f64() * 10.0).round() / 10.0
}

/// Today's UTC date as YYYYMMDD (civil-from-days, no date crate needed).
fn today() -> String {
    let days = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() / 86_400)
        .unwrap_or(0) as i64;
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{year:04}{month:02}{day:02}")
}

fn wait(timeout_secs: u64, interval_secs: u64) -> WaitOptions {
    WaitOptions {
        timeout: Duration::from_secs(timeout_secs),
        interval: Duration::from_secs(interval_secs),
        ..Default::default()
    }
}

fn api_code(error: &Error) -> Option<&str> {
    match error {
        Error::Api(api) => api.code(),
        _ => None,
    }
}

async fn starts_left(client: &Client) -> Fallible<i64> {
    let limits = client.limits(&LimitsParams::default()).await?;
    let day = limits.starts.as_ref().and_then(|s| match &s.day {
        Nullable::Value(day) => day.remaining,
        _ => None,
    });
    Ok(day.unwrap_or(i64::MAX))
}

async fn create(
    client: &Client,
    size: &str,
    from: Option<&str>,
    ttl: i64,
    key: String,
) -> Fallible<String> {
    if starts_left(client).await? < 1 {
        return Err("Boat has no starts left today (200 a day on the $20 plan)".into());
    }
    let created = client
        .create(&CreateParams {
            idempotency_key: Some(key),
            body: Some(CreateSandboxRequest {
                type_: Some(size.into()),
                ttl_seconds: Nullable::Value(ttl),
                no_env: Some(true),
                from_: from.map(Into::into),
                ..Default::default()
            }),
            ..Default::default()
        })
        .await?;
    Ok(created.sandbox.id)
}

/// Run a script detached and wait for it; returns (exit code, stdout, stderr).
async fn exec(
    client: &Client,
    id: &str,
    script: String,
    timeout_secs: u64,
) -> Fallible<(i64, String, String)> {
    let started = client
        .exec_detached(
            id,
            CommandRequest {
                command: script,
                ..Default::default()
            },
        )
        .await?;
    let done = client
        .wait_command(id, started.process_id, &wait(timeout_secs, 10))
        .await?;
    Ok((done.exit_code.unwrap_or(-1), done.stdout, done.stderr))
}

async fn stop_and_wait(client: &Client, id: &str) -> Fallible<()> {
    client
        .stop(&StopParams {
            sandbox_id: id.into(),
            ..Default::default()
        })
        .await?;
    let deadline = Instant::now() + Duration::from_secs(1_800);
    loop {
        let state = client
            .get(&GetParams {
                sandbox_id: id.into(),
                ..Default::default()
            })
            .await?
            .sandbox
            .state;
        if matches!(state.as_str(), "stopped" | "archived") {
            return Ok(());
        }
        if state == "error" || Instant::now() > deadline {
            return Err(format!("sandbox {id} did not stop (state {state})").into());
        }
        tokio::time::sleep(Duration::from_secs(5)).await;
    }
}

async fn delete_sandbox(client: &Client, id: &str) -> Fallible<()> {
    let op = client
        .delete_sandbox(&DeleteSandboxParams {
            sandbox_id: id.into(),
            x_ascii_confirm_delete: id.into(),
            ..Default::default()
        })
        .await?;
    client
        .wait_for_deletion(&op.operation.id, &wait(900, 5))
        .await?;
    Ok(())
}

/// Stop, then delete unless asked to keep it; never fails the caller.
async fn teardown(client: &Client, id: &str, delete: bool) {
    if let Err(error) = stop_and_wait(client, id).await {
        log(format!("could not stop {id}: {error}"));
    }
    if delete && let Err(error) = delete_sandbox(client, id).await {
        log(format!("could not delete {id}: {error}"));
    }
}

async fn usage_dollars(client: &Client, id: &str) -> Option<f64> {
    client
        .usage(&UsageParams {
            sandbox_id: id.into(),
            ..Default::default()
        })
        .await
        .ok()
        .map(|u| u.dollars)
}

/// Our templates, newest first (names sort by date).
async fn templates(client: &Client) -> Fallible<Vec<NamedSnapshot>> {
    let mut list: Vec<NamedSnapshot> = client
        .list_named_snapshots()
        .await?
        .snapshots
        .into_iter()
        .filter(|s| s.name.starts_with(PREFIX))
        .collect();
    list.sort_by(|a, b| b.name.cmp(&a.name));
    Ok(list)
}

fn describe(s: &NamedSnapshot) -> Value {
    json!({ "name": s.name, "status": s.status, "sizeBytes": s.size_bytes, "createdAt": s.created_at })
}

/// Delete our templates beyond the newest `keep`, never touching `spare`.
async fn prune(client: &Client, keep: usize, spare: Option<&str>) -> Fallible<Vec<String>> {
    let mut deleted = Vec::new();
    for old in templates(client)
        .await?
        .into_iter()
        .filter(|s| s.status == "ready" || s.status == "failed")
        .skip(keep)
    {
        if Some(old.name.as_str()) == spare {
            continue;
        }
        client
            .delete_named_snapshot(&DeleteNamedSnapshotParams {
                name: old.name.clone(),
                ..Default::default()
            })
            .await?;
        log(format!("deleted {}", old.name));
        deleted.push(old.name);
    }
    Ok(deleted)
}

/// Make room under Boat's 10-name cap by dropping our oldest templates
/// (keeping at least `keep - 1`). Other names are never touched.
async fn make_room(client: &Client, name: &str, keep: usize) -> Fallible<()> {
    let all = client.list_named_snapshots().await?.snapshots;
    if all.iter().any(|s| s.name == name) {
        return Ok(()); // a save under an existing name replaces it
    }
    let mut excess = (all.len() + 1).saturating_sub(NAMED_LIMIT);
    if excess == 0 {
        return Ok(());
    }
    let ours = templates(client).await?;
    for old in ours.iter().skip(keep.saturating_sub(1)).rev() {
        if excess == 0 {
            break;
        }
        client
            .delete_named_snapshot(&DeleteNamedSnapshotParams {
                name: old.name.clone(),
                ..Default::default()
            })
            .await?;
        log(format!(
            "deleted {} to stay under Boat's {NAMED_LIMIT} named snapshots",
            old.name
        ));
        excess -= 1;
    }
    if excess > 0 {
        return Err(format!(
            "the account already keeps {NAMED_LIMIT} named snapshots and too few are ours to drop; remove one (docs/deployment/boat-template.md)"
        )
        .into());
    }
    Ok(())
}

/// The build sandbox's driver: fetch the shared host setup from origin/main
/// (unless `--setup-file` uploaded one), run it with a warm build, and report
/// its phases and sizes as `key=value` lines.
const BUILD_SCRIPT: &str = r#"#!/usr/bin/env bash
set -uo pipefail
cd ~
if [ ! -s /tmp/coder-host-setup.sh ]; then
  [ -d openagents/.git ] || git clone -q https://github.com/OpenAgentsInc/openagents.git openagents || exit 1
  git -C openagents fetch -q origin main || exit 1
  git -C openagents show origin/main:scripts/cloud/coder-host-setup.sh > /tmp/coder-host-setup.sh || exit 1
fi
# The sccache disk cache duplicates the warm target: keep it out of the snapshot.
printf '.cache/sccache/\n' > ~/.boxignore
log=~/.oa-coder-host-setup.log
bash /tmp/coder-host-setup.sh --warm >"$log" 2>&1
rc=$?
grep '^OA_CODER_HOST_SETUP' "$log"
if [ "$rc" != 0 ]; then tail -n 60 "$log"; exit "$rc"; fi
manifest=~/.openagents/coder-host.json
slot=$(jq -r .warm_target.slot "$manifest")
# A build in the default target dir (scripts/boat-run.sh, a hand-run cargo)
# reuses the warm slot too.
[ -e ~/openagents/target ] || ln -s "$slot" ~/openagents/target
echo "rev=$(jq -r .rev "$manifest")"
echo "slot=$slot"
echo "slot_bytes=$(du -sb "$slot" | cut -f1)"
echo "cargo_home_bytes=$(du -sb ~/.cargo | cut -f1)"
echo "rustup_bytes=$(du -sb "${RUSTUP_HOME:-$HOME/.rustup}" | cut -f1)"
echo "sccache_bytes_excluded=$(du -sb ~/.cache/sccache 2>/dev/null | cut -f1)"
echo "home_bytes=$(du -sb --exclude=.cache/sccache ~ | cut -f1)"
"#;

/// Seconds per phase from the setup's `OA_CODER_HOST_SETUP <phase> t=<epoch> begin|end`
/// lines (scripts/cloud/coder-host-setup.sh).
fn phases(output: &str) -> serde_json::Map<String, Value> {
    let mut begun = std::collections::BTreeMap::new();
    let mut done = serde_json::Map::new();
    for line in output.lines() {
        let words: Vec<&str> = line.split_whitespace().collect();
        let [marker, phase, time, edge, ..] = words.as_slice() else {
            continue;
        };
        let Some(t) = time.strip_prefix("t=").and_then(|t| t.parse::<i64>().ok()) else {
            continue;
        };
        if *marker != "OA_CODER_HOST_SETUP" {
            continue;
        }
        match *edge {
            "begin" => {
                begun.insert(phase.to_string(), t);
            }
            "end" => {
                if let Some(start) = begun.remove(*phase) {
                    done.insert(phase.to_string(), Value::from(t - start));
                }
            }
            _ => {}
        }
    }
    done
}

/// `key=value` lines from the setup output.
fn fields(output: &str) -> serde_json::Map<String, Value> {
    output
        .lines()
        .filter_map(|line| line.split_once('='))
        .filter(|(k, _)| k.chars().all(|c| c.is_ascii_lowercase() || c == '_'))
        .map(|(k, v)| {
            let v = v.trim();
            let value = v
                .parse::<i64>()
                .map(Value::from)
                .unwrap_or_else(|_| Value::from(v));
            (k.to_string(), value)
        })
        .collect()
}

async fn build(client: &Client, flags: &Flags) -> Fallible<Value> {
    let name = flags
        .name
        .clone()
        .unwrap_or_else(|| format!("{PREFIX}{}", today()));
    let started = Instant::now();
    let stamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    let id = create(
        client,
        &flags.size,
        None,
        4 * 3600,
        format!("oa-template-{name}-{stamp}"),
    )
    .await?;
    log(format!("building {name} on {id} ({})", flags.size));
    let result = build_on(client, flags, &name, &id, started).await;
    let dollars = usage_dollars(client, &id).await;
    match result {
        Ok(mut summary) => {
            teardown(client, &id, !flags.keep_sandbox).await;
            summary["buildSandboxDollars"] = json!(dollars);
            summary["buildSandboxDeleted"] = json!(!flags.keep_sandbox);
            summary["wallSeconds"] = json!(secs(started));
            Ok(summary)
        }
        Err(error) => {
            log(format!(
                "failed; stopping {id} and keeping it for inspection"
            ));
            teardown(client, &id, false).await;
            Err(error)
        }
    }
}

async fn build_on(
    client: &Client,
    flags: &Flags,
    name: &str,
    id: &str,
    started: Instant,
) -> Fallible<Value> {
    client.wait_until_ready(id, &wait(900, 3)).await?;
    let ready_seconds = secs(started);
    if let Some(path) = &flags.setup_file {
        client
            .write_bytes(id, "/tmp/coder-host-setup.sh", &std::fs::read(path)?)
            .await?;
    }
    let setup_at = Instant::now();
    client
        .write_text(id, "/tmp/oa-template-build.sh", BUILD_SCRIPT)
        .await?;
    let (code, stdout, stderr) = exec(
        client,
        id,
        "bash /tmp/oa-template-build.sh".into(),
        4 * 3600,
    )
    .await?;
    let setup_seconds = secs(setup_at);
    eprint!("{stdout}");
    if code != 0 {
        eprint!("{stderr}");
        return Err(
            format!("setup exited {code} on {id}; log at ~/.oa-coder-host-setup.log").into(),
        );
    }
    let setup = fields(&stdout);
    let setup_phases = phases(&stdout);

    let stop_at = Instant::now();
    stop_and_wait(client, id).await?;
    let stop_seconds = secs(stop_at);

    make_room(client, name, flags.keep).await?;
    let save_at = Instant::now();
    let save = client
        .save_named_snapshot(&SaveNamedSnapshotParams {
            body: NamedSnapshotSaveRequest {
                sandbox_id: id.into(),
                name: name.into(),
                ..Default::default()
            },
            ..Default::default()
        })
        .await;
    if let Err(error) = &save
        && api_code(error) == Some("named_snapshot_limit")
    {
        return Err(
            "Boat refused the save: 10 named snapshots already (docs/deployment/boat-template.md)"
                .into(),
        );
    }
    save?;
    let deadline = Instant::now() + Duration::from_secs(3_600);
    let snapshot = loop {
        tokio::time::sleep(Duration::from_secs(10)).await;
        let info = client
            .get_named_snapshot(&GetNamedSnapshotParams {
                name: name.into(),
                ..Default::default()
            })
            .await?
            .snapshot;
        match info.status.as_str() {
            "ready" => break info,
            "failed" => {
                return Err(
                    format!("saving {name} failed: {}", info.error.unwrap_or_default()).into(),
                );
            }
            _ if Instant::now() > deadline => {
                return Err(format!("saving {name} did not finish in an hour").into());
            }
            _ => {}
        }
    };
    let save_seconds = secs(save_at);
    log(format!(
        "saved {name}: {} bytes in {save_seconds}s",
        snapshot.size_bytes.unwrap_or(0)
    ));
    let deleted = prune(client, flags.keep, Some(name)).await?;
    Ok(json!({
        "template": name,
        "buildSandbox": id,
        "type": flags.size,
        "readySeconds": ready_seconds,
        "setupSeconds": setup_seconds,
        "stopSeconds": stop_seconds,
        "saveSeconds": save_seconds,
        "snapshotBytes": snapshot.size_bytes,
        "setup": setup,
        "setupPhases": setup_phases,
        "pruned": deleted,
    }))
}

async fn probe(client: &Client, name: &str, flags: &Flags) -> Fallible<Value> {
    let started = Instant::now();
    let stamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    let id = create(
        client,
        &flags.size,
        Some(name),
        3600,
        format!("oa-template-probe-{name}-{stamp}"),
    )
    .await?;
    log(format!("probing {name} on {id}"));
    let result = probe_on(client, flags, &id, started).await;
    let dollars = usage_dollars(client, &id).await;
    teardown(client, &id, true).await;
    let mut summary = result?;
    summary["template"] = json!(name);
    summary["sandbox"] = json!(id);
    summary["dollars"] = json!(dollars);
    Ok(summary)
}

async fn probe_on(client: &Client, flags: &Flags, id: &str, started: Instant) -> Fallible<Value> {
    client.wait_until_ready(id, &wait(900, 1)).await?;
    let ready_seconds = secs(started);
    let first_at = Instant::now();
    exec(client, id, "true".into(), 300).await?;
    let first_command_seconds = secs(first_at);
    let package = &flags.package;
    let script = format!(
        r#"set -e
cd ~/openagents
export PATH="$HOME/.cargo/bin:$PATH"
manifest=~/.openagents/coder-host.json
slot=$(jq -r .warm_target.slot "$manifest")
echo "template_rev=$(jq -r .rev "$manifest")"
b() {{
  s=$EPOCHREALTIME
  if ! CARGO_TARGET_DIR="$slot" cargo build -p {package} >/tmp/probe.log 2>&1; then tail -n 30 /tmp/probe.log; exit 1; fi
  awk -v a="$s" -v b="$EPOCHREALTIME" -v k="$1" 'BEGIN {{ printf "%s_build_seconds=%.1f\n", k, b - a }}'
  echo "$1_compiled_crates=$(grep -c '^ *Compiling' /tmp/probe.log || true)"
}}
b noop
git fetch -q origin main
echo "behind_commits=$(git rev-list --count HEAD..origin/main)"
git checkout -q --detach origin/main
b main
"#
    );
    client.write_text(id, "/tmp/oa-probe.sh", &script).await?;
    let script = "bash /tmp/oa-probe.sh".to_string();
    let (code, stdout, stderr) = exec(client, id, script, 3 * 3600).await?;
    eprint!("{stdout}");
    if code != 0 {
        eprint!("{stderr}");
        return Err(format!("probe build exited {code}").into());
    }
    Ok(json!({
        "readySeconds": ready_seconds,
        "firstCommandSeconds": first_command_seconds,
        "package": package,
        "build": fields(&stdout),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn today_is_a_date() {
        let d = today();
        assert_eq!(d.len(), 8);
        assert!(d.starts_with("20"));
    }

    #[test]
    fn fields_reads_key_values() {
        let f = fields("commit=abc\nbuild_seconds=12\nnoise line\n[x] a=b\n");
        assert_eq!(f["commit"], "abc");
        assert_eq!(f["build_seconds"], 12);
        assert!(!f.contains_key("[x] a"));
    }

    #[test]
    fn phases_pairs_begin_and_end() {
        let p = phases(
            "OA_CODER_HOST_SETUP packages t=100 begin\nOA_CODER_HOST_SETUP packages t=130 end\n\
             OA_CODER_HOST_SETUP repo t=130 begin\nOA_CODER_HOST_SETUP repo t=131 end\n\
             OA_CODER_HOST_SETUP finished t=200 manifest=x\n",
        );
        assert_eq!(p["packages"], 30);
        assert_eq!(p["repo"], 1);
        assert!(!p.contains_key("finished"));
    }

    #[test]
    fn flags_parse() {
        let f = Flags::parse(&["probe-name".into(), "--keep".into(), "2".into()]).unwrap();
        assert_eq!(f.keep, 2);
        assert_eq!(f.positional, ["probe-name"]);
        assert!(Flags::parse(&["--keep".into(), "0".into()]).is_err());
    }
}
