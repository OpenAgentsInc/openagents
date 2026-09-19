//! Publishes, fetches, and reads the policy snapshot a release serves under.
//!
//! ```text
//! cargo run -p lev --bin lev-policy -- publish --window 24h \
//!     > crates/lev/policy/current.json
//! cargo run -p lev --bin lev-policy -- fetch \
//!     --manifest crates/lev/manifests/lev-base-v1.json
//! cargo run -p lev --bin lev-policy -- show \
//!     --manifest crates/lev/manifests/lev-base-v1.json
//! ```
//!
//! Three commands, and they are the two halves of the mechanism plus the
//! question an operator asks:
//!
//! - `publish` writes a snapshot. `--revoke <release>` marks a release
//!   revoked; `--revoke-base <signature>` marks every release fitted against
//!   a base revoked, which is the shape the operating system treadmill
//!   arrives in.
//! - `fetch` copies the canonical snapshot into the door's cache, after
//!   digesting and checking it. Nothing is written where a door can load it
//!   until it checks out.
//! - `show` reads the cache and says whether the release may still serve, how
//!   long the cached snapshot has left, and what has been revoked. It exits 1
//!   when the release does not serve, so a scheduled check can act on it.

use std::path::{Path, PathBuf};

use lev::manifest::Manifest;
use lev::policy::{
    DEFAULT_WINDOW_SECONDS, Policy, Revocation, Snapshot, SnapshotRef, Standing, stamp, store,
};

fn main() {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let command = arguments.first().cloned().unwrap_or_default();
    let options = Options::read(&arguments[usize::from(!command.is_empty())..]);
    match command.as_str() {
        "publish" => publish(&options),
        "fetch" => fetch(&options),
        "show" => show(&options),
        other => {
            if !other.is_empty() {
                eprintln!("unknown command {other}");
            }
            eprintln!(
                "usage: lev-policy <publish | fetch | show> [--manifest <path>] [--window <24h>] \
                 [--revoke <name@version>] [--revoke-base <signature>] [--families a,b] \
                 [--reason <text>] [--source <path>] [--cache <path>] [--expect <sha256>] \
                 [--release <name@version>] [--base <signature>]"
            );
            std::process::exit(2);
        }
    }
}

/// Everything the three commands take.
#[derive(Default)]
struct Options {
    manifest: Option<String>,
    source: Option<String>,
    cache: Option<String>,
    window: Option<u64>,
    revoke: Vec<String>,
    revoke_base: Vec<String>,
    families: Vec<String>,
    reason: String,
    expect: Option<String>,
    release: Option<String>,
    base: Option<String>,
}

impl Options {
    fn read(arguments: &[String]) -> Self {
        let mut options = Self::default();
        let mut arguments = arguments.iter();
        while let Some(flag) = arguments.next() {
            let mut value = || arguments.next().cloned().unwrap_or_default();
            match flag.as_str() {
                "--manifest" => options.manifest = Some(value()),
                "--source" => options.source = Some(value()),
                "--cache" => options.cache = Some(value()),
                "--window" => options.window = Some(seconds(&value())),
                "--revoke" => options.revoke.push(value()),
                "--revoke-base" => options.revoke_base.push(value()),
                "--families" => {
                    options.families =
                        value().split(',').map(|name| name.trim().to_string()).collect();
                }
                "--reason" => options.reason = value(),
                "--expect" => options.expect = Some(value()),
                "--release" => options.release = Some(value()),
                "--base" => options.base = Some(value()),
                other => {
                    eprintln!("unknown flag {other}");
                    std::process::exit(2);
                }
            }
        }
        options
    }
}

/// `86400`, `24h`, `30m`, or `7d`, in seconds.
fn seconds(value: &str) -> u64 {
    lev::policy::duration(value).unwrap_or_else(|| {
        eprintln!("--window takes a count of seconds, or one of 30m, 24h, 7d");
        std::process::exit(2);
    })
}

/// Writes a snapshot to standard output.
fn publish(options: &Options) {
    let now = lev::policy::Clock::system().now();
    let window = options.window.unwrap_or(DEFAULT_WINDOW_SECONDS);
    let mut snapshot = Snapshot::new(now, window);
    let revoking = options.revoke.len() + options.revoke_base.len();
    if revoking > 0 && options.reason.trim().is_empty() {
        // A revocation with no reason is a refusal nobody can act on, and the
        // reason is what a caller reads on the wire.
        eprintln!("a revocation needs --reason; it is what the refusal says");
        std::process::exit(2);
    }
    let reason = options.reason.clone();
    for release in &options.revoke {
        snapshot = snapshot.revoking(
            Revocation::of(release, reason.clone(), now).for_families(options.families.clone()),
        );
    }
    for signature in &options.revoke_base {
        snapshot = snapshot.revoking(
            Revocation::of_base(signature, reason.clone(), now)
                .for_families(options.families.clone()),
        );
    }
    if let Err(trouble) = snapshot.check() {
        eprintln!("{trouble}");
        std::process::exit(1);
    }
    match snapshot.to_json() {
        Ok(text) => println!("{text}"),
        Err(error) => {
            eprintln!("the snapshot did not encode: {error}");
            std::process::exit(1);
        }
    }
}

/// Copies the canonical snapshot into the door's cache, once it checks out.
fn fetch(options: &Options) {
    let (source, cache) = paths(options);
    let bytes = match std::fs::read(&source) {
        Ok(bytes) => bytes,
        Err(error) => {
            eprintln!("{}: {error}", source.display());
            std::process::exit(1);
        }
    };
    match store(&cache, &bytes, options.expect.as_deref()) {
        Ok(digest) => {
            println!("fetched     {} -> {}", source.display(), cache.display());
            println!("sha256      {digest}");
        }
        Err(trouble) => {
            eprintln!("{trouble}");
            eprintln!("the cache is unchanged, and the door goes stale on schedule");
            std::process::exit(1);
        }
    }
}

/// Says whether the release may still serve.
fn show(options: &Options) {
    let policy = match &options.manifest {
        Some(path) => manifest(path).policy(),
        None => {
            let (source, cache) = paths(options);
            // Without a manifest there is no release and no base signature
            // to aim a revocation at, so a check that names neither covers
            // freshness and nothing else. That is worth saying out loud: a
            // snapshot revoking a base this command was not told about reads
            // as `current`, and it is.
            if options.release.is_none() && options.base.is_none() {
                println!(
                    "scope       freshness only; pass --manifest, or --release and --base, to \
                     check revocations"
                );
            }
            Policy::for_release(
                &SnapshotRef {
                    source: source.display().to_string(),
                    cache: cache.display().to_string(),
                    freshness_window_seconds: options.window.unwrap_or(DEFAULT_WINDOW_SECONDS),
                },
                Path::new("."),
                options.release.clone().unwrap_or_else(|| "an unnamed release".to_string()),
                options.base.clone().unwrap_or_default(),
            )
        }
    };
    let report = policy.report();
    println!("release     {}", policy.release());
    println!("source      {}", report.source);
    println!("cache       {}", report.cache);
    println!("state       {}", report.standing.label());
    if !report.issued.is_empty() {
        println!("issued      {}", report.issued);
        println!("window      {} seconds", report.window_seconds);
        println!("expires     {} seconds from now", report.expires_in_seconds);
        println!("sha256      {}", report.sha256);
    }
    for revocation in &report.revoked {
        println!(
            "revoked     {} {} since {} — {}",
            if revocation.release.is_empty() {
                format!("base {}", revocation.base_signature)
            } else {
                revocation.release.clone()
            },
            revocation.scope(),
            revocation.effective,
            revocation.reason
        );
    }
    match policy.admits("") {
        Ok(()) => println!("serves      yes, until {}", stamp(now_plus(report.expires_in_seconds))),
        Err(refusal) => {
            println!("serves      no");
            println!("reason      {}", refusal.message);
            std::process::exit(1);
        }
    }
    if matches!(report.standing, Standing::Current { .. }) && report.expires_in_seconds <= 0 {
        // Belt and braces: a report that says current and expired at once
        // would mean the two readings disagree, and that is worth a line
        // rather than a silent pass.
        eprintln!("the standing and the expiry disagree, which is a defect");
        std::process::exit(1);
    }
}

fn now_plus(seconds: i64) -> i64 {
    lev::policy::Clock::system().now().saturating_add(seconds)
}

/// The source and cache to act on: the manifest's, or the flags'.
fn paths(options: &Options) -> (PathBuf, PathBuf) {
    if let Some(path) = &options.manifest {
        let policy = manifest(path).policy();
        return (policy.source().to_path_buf(), policy.cache().to_path_buf());
    }
    match (&options.source, &options.cache) {
        (Some(source), Some(cache)) => (PathBuf::from(source), PathBuf::from(cache)),
        _ => {
            eprintln!("pass --manifest <path>, or both --source and --cache");
            std::process::exit(2);
        }
    }
}

fn manifest(path: &str) -> Manifest {
    match Manifest::load(path) {
        Ok(manifest) => manifest,
        Err(fault) => {
            eprintln!("{path}: {fault}");
            std::process::exit(1);
        }
    }
}
