//! `icecream-live`: The Ice Cream Man as a live-deployed zone on livehost
//! (`docs/verse/ice-cream-man-live.md`).
//!
//! ```text
//! icecream-live serve    [--listen 127.0.0.1:8790]      the host, both tenants
//! icecream-live shell                                   build the page's shell (rare)
//! icecream-live deploy   [--channel live] [--override] [--wait SECONDS]
//! icecream-live rollback [--channel live] [--wait SECONDS]
//! icecream-live grid     DIR                            deploy a Grid build as tenant `grid`
//! ```
//!
//! Every command takes `--work DIR` (default `~/.openagents/icecream-live`),
//! `--relay URL` (default `ws://127.0.0.1:7448`, from
//! `crates/livehost/scripts/relay.sh`), and `--host URL` (default
//! `http://127.0.0.1:8790`). `deploy` builds the logic module only when its
//! sources changed, bakes the zone, and publishes through livehost with the
//! first-play budget as the pre-publish check. With `--wait` it then waits
//! for a page in the zone to apply the new version and prints the time from
//! the start of the deploy to that page's ack. It prints one JSON line.

mod bake;
mod cdp;
mod grid;
mod host;
mod measure;

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, bail};
use sha2::{Digest, Sha256};

pub struct Args {
    rest: Vec<String>,
}

impl Args {
    pub fn value(&mut self, name: &str) -> Option<String> {
        let at = self.rest.iter().position(|a| a == name)?;
        self.rest.remove(at);
        (at < self.rest.len()).then(|| self.rest.remove(at))
    }

    pub fn flag(&mut self, name: &str) -> bool {
        let at = self.rest.iter().position(|a| a == name);
        at.map(|at| self.rest.remove(at)).is_some()
    }

    pub fn work(&mut self) -> host::Work {
        let dir = self.value("--work").map_or_else(
            || {
                let home =
                    std::env::var_os("HOME").map_or_else(|| PathBuf::from("."), PathBuf::from);
                home.join(".openagents/icecream-live")
            },
            PathBuf::from,
        );
        let relay = self
            .value("--relay")
            .unwrap_or_else(|| "ws://127.0.0.1:7448".into());
        host::Work { dir, relay }
    }

    pub fn host(&mut self) -> String {
        self.value("--host")
            .unwrap_or_else(|| "http://127.0.0.1:8790".into())
            .trim_end_matches('/')
            .to_owned()
    }

    fn wait(&mut self) -> Result<Option<Duration>> {
        self.value("--wait")
            .map(|s| Ok(Duration::from_secs_f64(s.parse()?)))
            .transpose()
    }
}

pub fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

pub fn epoch_ms() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0.0, |d| d.as_secs_f64() * 1000.0)
}

fn ms(start: Instant) -> f64 {
    (start.elapsed().as_secs_f64() * 10_000.0).round() / 10.0
}

fn target_dir() -> PathBuf {
    std::env::var_os("CARGO_TARGET_DIR").map_or_else(|| root().join("target"), PathBuf::from)
}

/// A digest of every file under `dirs`, to skip a build whose inputs
/// didn't change.
fn digest(dirs: &[PathBuf]) -> Result<String> {
    fn walk(dir: &Path, hasher: &mut Sha256) -> Result<()> {
        let mut entries: Vec<_> = std::fs::read_dir(dir)?.collect::<Result<_, _>>()?;
        entries.sort_by_key(std::fs::DirEntry::file_name);
        for entry in entries {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, hasher)?;
            } else {
                hasher.update(path.to_string_lossy().as_bytes());
                hasher.update(Sha256::digest(std::fs::read(&path)?));
            }
        }
        Ok(())
    }
    let mut hasher = Sha256::new();
    for dir in dirs {
        walk(dir, &mut hasher)?;
    }
    Ok(hex::encode(hasher.finalize()))
}

/// Builds the logic module when its sources changed; the module's path and
/// whether it built.
fn logic(work: &Path) -> Result<(PathBuf, bool)> {
    let root = root();
    let inputs = digest(&[
        root.join("crates/icecream-rules/src"),
        root.join("crates/icecream-logic/src"),
    ])?;
    let built = target_dir().join("wasm32-unknown-unknown/live-wasm/icecream_logic.wasm");
    let stamp = work.join("logic.inputs");
    if built.exists() && std::fs::read_to_string(&stamp).ok().as_deref() == Some(inputs.as_str()) {
        return Ok((built, false));
    }
    let status = Command::new("cargo")
        .current_dir(&root)
        .args([
            "build",
            "--quiet",
            "--profile",
            "live-wasm",
            "--target",
            "wasm32-unknown-unknown",
            "-p",
            "icecream-logic",
        ])
        .status()
        .context("running cargo")?;
    if !status.success() {
        bail!("the logic module didn't build");
    }
    std::fs::create_dir_all(work)?;
    std::fs::write(&stamp, inputs)?;
    Ok((built, true))
}

pub fn runtime() -> Result<tokio::runtime::Runtime> {
    Ok(tokio::runtime::Runtime::new()?)
}

/// What one deploy needs.
pub struct DeployArgs {
    pub channel: String,
    pub force: bool,
    /// Wait this long for this many pages to ack the new version.
    pub wait: Option<(Duration, usize)>,
}

/// Waits on `acks` for `pages` swap acks of `manifest` and adds them to
/// `line`, with each page's time from `started` (an epoch time, ms).
pub async fn add_acks(
    acks: &mut host::Acks,
    line: &mut serde_json::Value,
    manifest: &str,
    started: f64,
    (wait, pages): (Duration, usize),
) -> Result<()> {
    let mut seen = Vec::new();
    let mut times = Vec::new();
    while seen.len() < pages {
        let want = serde_json::json!({
            "tenant": icecream_zone::TENANT,
            "manifest": manifest,
            "kind": "swap",
        });
        let ack = acks.wait(&want, wait).await?;
        let at = ack["at_ms"].as_f64().unwrap_or(0.0);
        times.push(((at - started) * 10.0).round() / 10.0);
        seen.push(ack);
    }
    line["edit_to_live_ms"] = times.iter().copied().fold(0.0, f64::max).into();
    line["acks"] = seen.into();
    Ok(())
}

/// Builds what changed, bakes, deploys, and (with `wait`) waits for pages
/// to apply it. `started` is when the edit was saved (epoch ms).
async fn deploy_once(
    work: &host::Work,
    host: &str,
    args: &DeployArgs,
    started: f64,
) -> Result<serde_json::Value> {
    let start = Instant::now();
    let shell = work.dir.join("shell");
    let tenant = work.dir.join("tenant");
    let mut acks = match args.wait {
        // Listen before publishing, so a fast page isn't missed.
        Some(_) => Some(host::Acks::open(host).await?),
        None => None,
    };
    let phase = Instant::now();
    let (logic, built) = logic(&work.dir)?;
    let build_ms = ms(phase);
    let phase = Instant::now();
    let baked = bake::bake(&bake::Sources::checked_in(), &shell, &logic, &tenant)?;
    let bake_ms = ms(phase);
    let report = work
        .deploy(icecream_zone::TENANT, &args.channel, &tenant, args.force)
        .await?;
    let manifest = report.manifest.clone().unwrap_or_default();
    let mut line = serde_json::json!({
        "manifest": manifest,
        "previous": report.previous,
        "channel": args.channel,
        "tiers": report.tiers,
        "logic_built": built,
        "build_ms": build_ms,
        "bake_ms": bake_ms,
        "publish_ms": (report.total_ms * 10.0).round() / 10.0,
        "deploy_ms": ms(start),
        "objects_uploaded": report.objects_uploaded,
        "bytes_uploaded": report.bytes_uploaded,
        "warnings": report.warnings,
        "chunks": baked.zone.chunks.len(),
        "url": format!("{host}/t/{}/", icecream_zone::TENANT),
    });
    if let (Some(wait), Some(acks)) = (args.wait, acks.as_mut())
        && !manifest.is_empty()
        && report.manifest != report.previous
    {
        add_acks(acks, &mut line, &manifest, started, wait).await?;
    }
    Ok(line)
}

fn deploy(mut args: Args) -> Result<()> {
    let started = epoch_ms();
    let work = args.work();
    let host = args.host();
    let wait = args.wait()?;
    let pages: usize = args.value("--pages").map_or(Ok(1), |p| p.parse())?;
    let options = DeployArgs {
        channel: args.value("--channel").unwrap_or_else(|| "live".into()),
        force: args.flag("--override"),
        wait: wait.map(|w| (w, pages)),
    };
    let line = runtime()?.block_on(deploy_once(&work, &host, &options, started))?;
    println!("{line}");
    Ok(())
}

async fn rollback_once(
    work: &host::Work,
    host: &str,
    wait: Option<(Duration, usize)>,
) -> Result<serde_json::Value> {
    let started = epoch_ms();
    let mut acks = match wait {
        Some(_) => Some(host::Acks::open(host).await?),
        None => None,
    };
    let report = work.rollback(icecream_zone::TENANT, "live").await?;
    let manifest = report.manifest.clone().unwrap_or_default();
    let mut line = serde_json::json!({
        "manifest": manifest,
        "previous": report.previous,
        "publish_ms": (report.total_ms * 10.0).round() / 10.0,
    });
    if let (Some(wait), Some(acks)) = (wait, acks.as_mut()) {
        add_acks(acks, &mut line, &manifest, started, wait).await?;
    }
    Ok(line)
}

fn rollback(mut args: Args) -> Result<()> {
    let work = args.work();
    let host = args.host();
    let wait = args.wait()?;
    let line = runtime()?.block_on(rollback_once(&work, &host, wait.map(|w| (w, 1))))?;
    println!("{line}");
    Ok(())
}

/// Deploys a Grid build (`scripts/build-everglade-web.sh` output plus its
/// page) as the `grid` tenant, so the Grid and the zone share an origin.
fn grid(mut args: Args) -> Result<()> {
    let work = args.work();
    let Some(dir) = args.rest.first().cloned() else {
        bail!("usage: icecream-live grid DIR");
    };
    let report = runtime()?.block_on(work.deploy("grid", "live", Path::new(&dir), false))?;
    report.print();
    Ok(())
}

/// Builds the page's shell with `scripts/build-icecream-web.sh`.
fn shell(mut args: Args) -> Result<()> {
    let out = args.work().dir.join("shell");
    let start = Instant::now();
    let status = Command::new(root().join("scripts/build-icecream-web.sh"))
        .current_dir(root())
        .arg(&out)
        .status()?;
    if !status.success() {
        bail!("the shell didn't build");
    }
    eprintln!(
        "icecream-live: shell built in {:.1} s",
        start.elapsed().as_secs_f64()
    );
    Ok(())
}

fn serve(mut args: Args) -> Result<()> {
    let work = args.work();
    let listen = args
        .value("--listen")
        .unwrap_or_else(|| "127.0.0.1:8790".into())
        .parse()?;
    let config = work.config()?;
    let store = work.store()?;
    runtime()?.block_on(livehost::host::serve(
        config,
        store,
        work.relay.clone(),
        listen,
    ))
}

/// Publishes a Game Lab version's numbers to the zone: the version from
/// `--version`, else `GAME_LAB_VERSION` (set by Game Lab's on-save hook),
/// else the latest. It writes them to `zone/tuning.json` and deploys, so a
/// chat's edit reaches players in the zone in seconds. Run it from Game Lab
/// with `GAME_LAB_ON_SAVE="icecream-live lab"`.
fn lab(mut args: Args) -> Result<()> {
    use game_lab::{doc::ICE_CREAM_MAN, lab::Lab};
    let started = epoch_ms();
    let work = args.work();
    let host = args.host();
    let version = args
        .value("--version")
        .or_else(|| std::env::var("GAME_LAB_VERSION").ok());
    let lab = Lab::open(Lab::default_root(), root()).map_err(anyhow::Error::msg)?;
    let n = match version {
        Some(v) => v.parse()?,
        None => lab
            .latest(ICE_CREAM_MAN)
            .context("Game Lab has no version yet")?,
    };
    let (document, _) = lab
        .load(ICE_CREAM_MAN, n)
        .with_context(|| format!("Game Lab has no version {n}"))?;
    let game = game_lab::doc::read(&document).map_err(|p| anyhow::anyhow!(p.join(" ")))?;
    let defaults = icecream_rules::Tuning::default();
    let mut changed = serde_json::Map::new();
    for field in icecream_rules::tuning::FIELDS {
        let value = game.tuning.get(field.name);
        if value != defaults.get(field.name)
            && let Some(value) = value
        {
            changed.insert(field.name.into(), value.into());
        }
    }
    let path = bake::Sources::checked_in().dir.join("tuning.json");
    std::fs::write(&path, serde_json::to_string_pretty(&changed)? + "\n")?;
    let options = DeployArgs {
        channel: "live".into(),
        force: false,
        wait: None,
    };
    let mut line = runtime()?.block_on(deploy_once(&work, &host, &options, started))?;
    line["game_lab_version"] = n.into();
    line["settings"] = changed.len().into();
    println!("{line}");
    Ok(())
}

/// Opens `URL` in the browser `openagents browser run` started, waits
/// `--after` seconds, and prints the value of `EXPR` in it.
fn probe(mut args: Args) -> Result<()> {
    let after: f64 = args.value("--after").map_or(Ok(3.0), |v| v.parse())?;
    let [url, expr] = args.rest.as_slice() else {
        bail!("usage: icecream-live probe [--after SECONDS] URL EXPR");
    };
    let port = cdp::port().context("run this under `openagents browser run`")?;
    runtime()?.block_on(async {
        let mut page = cdp::Page::open(port, url).await?;
        tokio::time::sleep(Duration::from_secs_f64(after)).await;
        println!("{}", page.eval(expr).await?);
        Ok(())
    })
}

fn main() -> Result<()> {
    let mut raw: Vec<String> = std::env::args().skip(1).collect();
    if raw.is_empty() {
        bail!("usage: icecream-live serve|shell|deploy|rollback|grid");
    }
    let command = raw.remove(0);
    let args = Args { rest: raw };
    match command.as_str() {
        "serve" => serve(args),
        "shell" => shell(args),
        "deploy" => deploy(args),
        "rollback" => rollback(args),
        "grid" => grid(args),
        "measure" => measure::run(args),
        "probe" => probe(args),
        "lab" => lab(args),
        "grid-check" => grid::run(args),
        other => bail!("unknown command {other}"),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use livehost::deploy::CheckInput;

    #[test]
    fn the_budget_check_reads_the_deploys_files() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("zone");
        std::fs::create_dir_all(&dir).unwrap();
        for (name, text) in [
            (
                "level.json",
                r#"{"seed":7,"near_m":16,"first_play_bytes":10,"first_play_ms":1500}"#,
            ),
            ("tuning.json", "{}"),
            ("palette.json", "{}"),
        ] {
            std::fs::write(dir.join(name), text).unwrap();
        }
        let shell = tmp.path().join("shell");
        std::fs::create_dir_all(&shell).unwrap();
        for name in &crate::bake::SHELL[1..] {
            std::fs::write(shell.join(name), "x").unwrap();
        }
        let logic = tmp.path().join("logic.wasm");
        std::fs::write(&logic, "x").unwrap();
        let out = tmp.path().join("out");
        crate::bake::bake(&crate::bake::Sources { dir }, &shell, &logic, &out).unwrap();
        let files: BTreeMap<String, Vec<u8>> = walk(&out);
        let objects = BTreeMap::new();
        let input = CheckInput {
            tenant: "icecream",
            channel: "live",
            objects: &objects,
            files: &files,
            meta: None,
        };
        let refused = crate::host::budget(&input).unwrap_err().to_string();
        assert!(refused.contains("over the first-play budget"), "{refused}");
    }

    fn walk(dir: &std::path::Path) -> BTreeMap<String, Vec<u8>> {
        let mut out = BTreeMap::new();
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                for (k, v) in walk(&path) {
                    out.insert(
                        format!("{}/{k}", path.file_name().unwrap().to_string_lossy()),
                        v,
                    );
                }
            } else {
                out.insert(
                    path.file_name().unwrap().to_string_lossy().into_owned(),
                    std::fs::read(&path).unwrap(),
                );
            }
        }
        out
    }
}
