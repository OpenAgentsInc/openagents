//! `voyager` — run an open-ended agent episode in a Minecraft world.
//!
//! ```text
//! voyager run --world worlds/meadow.json [--jar PATH] [--bridge PATH]
//!             [--java PATH] [--runs DIR] [--port N]
//! voyager worlds [DIR]
//! voyager paths
//! ```
//!
//! `run` boots a supervised local server for the world manifest, connects
//! the `mc-bridge` bot, and walks the episode's curriculum while it
//! narrates progress to stderr. Everything the run produces — the world
//! data, `server.log`, and the ATIF trace — stays in the run directory it
//! prints.
//!
//! Prerequisites, in the order the error messages ask for them:
//!
//! 1. `./scripts/build-mc-bridge.sh` — the nightly Rust helper.
//! 2. `./scripts/fetch-mc-server.sh <version>` — the server jar, into the
//!    version cache under `~/.openagents/voyager/minecraft/`.
//! 3. A `java` on `PATH`, `VOYAGER_JAVA`, or Homebrew's keg-only openjdk.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use voyager::episode::{self, Plan};
use voyager::world::{Scenario, World};
use voyager::{Error, Result};

const USAGE: &str = "usage:
  voyager run --world <manifest-or-name> [--scenario quest|war]
              [--jar PATH] [--bridge PATH]
              [--java PATH] [--runs DIR] [--port N]
  voyager worlds [DIR]
  voyager keys <username>...
  voyager evidence <run-dir>
  voyager paths";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("run") => match run(&args[1..]) {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("voyager: {error}");
                ExitCode::FAILURE
            }
        },
        Some("worlds") => match worlds(&args[1..]) {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("voyager: {error}");
                ExitCode::FAILURE
            }
        },
        Some("paths") => match paths() {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("voyager: {error}");
                ExitCode::FAILURE
            }
        },
        Some("keys") => match keys(&args[1..]) {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("voyager: {error}");
                ExitCode::FAILURE
            }
        },
        Some("evidence") => match evidence(&args[1..]) {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("voyager: {error}");
                ExitCode::FAILURE
            }
        },
        _ => {
            eprintln!("{USAGE}");
            ExitCode::from(2)
        }
    }
}

/// `voyager run`: one episode.
fn run(args: &[String]) -> Result<()> {
    let mut world_arg: Option<String> = None;
    let mut jar: Option<PathBuf> = None;
    let mut bridge: Option<PathBuf> = None;
    let mut java: Option<PathBuf> = None;
    let mut runs: Option<PathBuf> = None;
    let mut port: u16 = 25565;
    let mut scenario: Option<Scenario> = None;
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        match arg.as_str() {
            "--world" => world_arg = rest.next().cloned(),
            "--jar" => jar = rest.next().map(PathBuf::from),
            "--bridge" => bridge = rest.next().map(PathBuf::from),
            "--java" => java = rest.next().map(PathBuf::from),
            "--runs" => runs = rest.next().map(PathBuf::from),
            "--scenario" => {
                let name = rest.next().cloned().unwrap_or_default();
                scenario = Some(Scenario::named(&name).ok_or_else(|| {
                    Error::episode(format!("scenario {name:?} is not quest or war"))
                })?);
            }
            "--port" => {
                port = rest
                    .next()
                    .and_then(|value| value.parse().ok())
                    .unwrap_or(25565)
            }
            other if world_arg.is_none() && !other.starts_with("--") => {
                world_arg = Some(other.to_string())
            }
            _ => return Err(Error::episode(format!("unknown argument {arg:?}\n{USAGE}"))),
        }
    }
    let Some(world_arg) = world_arg else {
        return Err(Error::episode(format!("run needs --world\n{USAGE}")));
    };
    let world = World::load(world_path(&world_arg)?)?;
    let jar = jar.map(Ok).unwrap_or_else(|| server_jar(&world))?;
    let java = java.map(Ok).unwrap_or_else(java_path)?;
    let bridge = bridge
        .map(Ok)
        .unwrap_or_else(voyager::bridge::helper_path)?;
    let runs = runs.unwrap_or_else(default_runs);
    let plan = Plan {
        jar,
        java,
        bridge,
        runs,
        port,
        relay_bin: relay_bin(),
        relay_database: std::env::var("VOYAGER_RELAY_DATABASE_URL")
            .unwrap_or_else(|_| "postgres://127.0.0.1:5432/voyager_relay".to_string()),
        repo: std::env::current_dir()?,
        scenario,
    };
    // A world that enrolls agents runs the guild loop; a world with a
    // single `agent` runs the solo curriculum.
    let report = if world.agents.is_empty() {
        episode::run(&world, &plan, |line| eprintln!("voyager: {line}"))?
    } else {
        voyager::ensemble::run_ensemble(&world, &plan, |line| eprintln!("voyager: {line}"))?
    };
    eprintln!();
    for task in &report.tasks {
        eprintln!(
            "  [{}] {} — {}",
            if task.ok { "ok" } else { "failed" },
            task.task,
            task.detail
        );
    }
    eprintln!();
    eprintln!(
        "voyager: {} actions, run dir {}",
        report.actions,
        report.run_dir.display()
    );
    eprintln!("voyager: trace {}", report.trace.display());
    if report.all_ok() {
        Ok(())
    } else {
        Err(Error::episode("one or more tasks failed (see above)"))
    }
}

/// `voyager evidence`: render a run directory into `coverage.json`,
/// `metrics.json`, and `evidence.md` — the demo's D4 record.
fn evidence(args: &[String]) -> Result<()> {
    let Some(dir) = args.first() else {
        return Err(Error::episode(format!(
            "evidence needs a run directory\n{USAGE}"
        )));
    };
    let path = voyager::evidence::render(Path::new(dir))?;
    println!("{}", path.display());
    Ok(())
}

/// `voyager keys`: the Nostr pubkey each enrolled username derives. The
/// manifest records pubkeys; secrets are re-derived at run time and
/// never written.
fn keys(args: &[String]) -> Result<()> {
    if args.is_empty() {
        return Err(Error::episode(format!("keys needs usernames\n{USAGE}")));
    }
    for username in args {
        println!("{}\t{}", username, voyager::keys::agent_pubkey(username)?);
    }
    Ok(())
}

/// `voyager worlds`: the manifests in a directory and their digests.
fn worlds(args: &[String]) -> Result<()> {
    let dir = args
        .first()
        .map(PathBuf::from)
        .unwrap_or_else(|| repo_root().join("worlds"));
    let mut found = Vec::new();
    for entry in std::fs::read_dir(&dir)? {
        let entry = entry?;
        if entry.path().extension().is_some_and(|ext| ext == "json") {
            found.push(entry.path());
        }
    }
    found.sort();
    if found.is_empty() {
        eprintln!("voyager: no world manifests in {}", dir.display());
        return Ok(());
    }
    for path in found {
        match World::load(&path) {
            Ok(world) => println!("{}\t{}\t{}", world.name, world.digest, path.display()),
            Err(error) => println!("?\t{}\t{}", error, path.display()),
        }
    }
    Ok(())
}

/// `voyager paths`: what the defaults resolve to on this machine.
fn paths() -> Result<()> {
    println!("runs\t{}", default_runs().display());
    println!("minecraft\t{}", minecraft_dir().display());
    match java_path() {
        Ok(java) => println!("java\t{}", java.display()),
        Err(error) => println!("java\t{error}"),
    }
    match voyager::bridge::helper_path() {
        Ok(helper) => println!("mc-bridge\t{}", helper.display()),
        Err(error) => println!("mc-bridge\t{error}"),
    }
    Ok(())
}

/// A world name or a path. `meadow` resolves to `worlds/meadow.json`
/// under the repo root; anything containing a path separator or ending
/// in `.json` is read as given.
fn world_path(arg: &str) -> Result<PathBuf> {
    let path = PathBuf::from(arg);
    if path.is_file() {
        return Ok(path);
    }
    if !arg.contains('/') && !arg.ends_with(".json") {
        let named = repo_root().join("worlds").join(format!("{arg}.json"));
        if named.is_file() {
            return Ok(named);
        }
    }
    Err(Error::world(format!(
        "no world manifest at {arg:?}; looked there and under worlds/"
    )))
}

/// The worktree root: `crates/voyager` up two.
fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// `VOYAGER_MC_DIR`, or `~/.openagents/voyager/minecraft` — where the
/// fetch script keeps jars.
fn minecraft_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("VOYAGER_MC_DIR") {
        return PathBuf::from(dir);
    }
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    home.join(".openagents").join("voyager").join("minecraft")
}

/// The `nostr-relay` binary: `VOYAGER_RELAY_BIN`, or the workspace's
/// own debug build.
fn relay_bin() -> PathBuf {
    if let Some(path) = std::env::var_os("VOYAGER_RELAY_BIN") {
        return PathBuf::from(path);
    }
    repo_root().join("target").join("debug").join("nostr-relay")
}

/// `~/.openagents/voyager/runs`, where episodes leave their evidence.
fn default_runs() -> PathBuf {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    home.join(".openagents").join("voyager").join("runs")
}

/// The server jar for a world's Minecraft version.
fn server_jar(world: &World) -> Result<PathBuf> {
    let jar = minecraft_dir()
        .join("versions")
        .join(&world.minecraft.version)
        .join("server.jar");
    if jar.is_file() {
        return Ok(jar);
    }
    Err(Error::episode(format!(
        "no server jar for minecraft {}; run \
         ./scripts/fetch-mc-server.sh {}",
        world.minecraft.version, world.minecraft.version
    )))
}

/// Where `java` is: `VOYAGER_JAVA`, `PATH`, Homebrew's keg-only openjdk,
/// or `/usr/libexec/java_home`. A candidate only counts when
/// `java -version` runs — macOS ships a `/usr/bin/java` stub that exists
/// and then reports "Unable to locate a Java Runtime", which is exactly
/// the answer a presence check is meant to catch.
fn java_path() -> Result<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(java) = std::env::var_os("VOYAGER_JAVA") {
        candidates.push(PathBuf::from(java));
    }
    if let Some(path) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&path) {
            candidates.push(dir.join("java"));
        }
    }
    candidates.push(PathBuf::from("/opt/homebrew/opt/openjdk/bin/java"));
    candidates.push(PathBuf::from("/usr/local/opt/openjdk/bin/java"));
    if let Ok(output) = std::process::Command::new("/usr/libexec/java_home").output()
        && output.status.success()
    {
        let home = String::from_utf8_lossy(&output.stdout).trim().to_string();
        candidates.push(Path::new(&home).join("bin/java"));
    }
    for candidate in candidates {
        if !candidate.is_file() {
            continue;
        }
        let version = std::process::Command::new(&candidate)
            .arg("-version")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();
        if matches!(version, Ok(status) if status.success()) {
            return Ok(candidate);
        }
    }
    Err(Error::Java(
        "no working java found; install a JDK or set VOYAGER_JAVA".to_string(),
    ))
}
