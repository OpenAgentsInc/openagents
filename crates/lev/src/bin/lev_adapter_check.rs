//! Checks a decision model against its manifest, and says which claim failed.
//!
//! ```text
//! cargo run -p lev --bin lev-adapter-check -- crates/lev/manifests/lev-adapted-v1.json
//! cargo run -p lev --bin lev-adapter-check -- ~/code/lev-adapter-work/runs/lev-v1/lev.fmadapter
//! ```
//!
//! The runtime's own error for a bad package is "the adapter asset is
//! invalid", which is true and not useful. This names the rule.
//!
//! Three modes, and the first is the one to use:
//!
//! - A manifest path checks every claim the document makes — the artifact's
//!   digest and size, the rank, the base signature, the declared interface
//!   against the contract this build implements, and each calibration record
//!   the manifest rests its admissions on.
//! - A package path reports what the package itself declares, plus the digest
//!   a manifest would pin it by.
//! - `--emit <name>@<version>` writes a manifest for a package. Computing a
//!   133 MB digest by hand is the error the document exists to prevent, so
//!   the producer lives beside the checker.

use std::path::{Path, PathBuf};

use lev::adapter::{METADATA_FILE, Package, WEIGHTS_FILE};
use lev::bridge::Bridge;
use lev::manifest::{Artifact, Base, EvalRef, Interface, MANIFEST_SCHEMA, Manifest, digest_of};
use lev::policy::{DEFAULT_WINDOW_SECONDS, SnapshotRef};

use gym::calibrate::{EstimatorConfig, Record};

fn main() {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let target = arguments.first().filter(|first| !first.starts_with("--")).cloned();
    let options = Options::read(&arguments[usize::from(target.is_some())..]);

    match (target, &options.emit) {
        (Some(path), _) if path.ends_with(".json") => check_manifest(Path::new(&path)),
        (Some(path), Some(release)) => emit(Some(open(Path::new(&path))), release, &options),
        (Some(path), None) => describe(Path::new(&path)),
        // A base model the operating system ships has no package to read, so
        // it is emitted from flags alone and pins its base by hand.
        (None, Some(release)) => emit(None, release, &options),
        (None, None) => {
            eprintln!(
                "usage: lev-adapter-check <manifest.json | package.fmadapter> \
                 [--emit <name>@<version>]"
            );
            std::process::exit(2);
        }
    }
}

/// What `--emit` needs and the package cannot supply.
#[derive(Default)]
struct Options {
    emit: Option<String>,
    description: String,
    created: String,
    os_build: String,
    families: Vec<String>,
    estimator: String,
    samples: u64,
    seed_base: u64,
    calibration: Option<String>,
    base: String,
    policy_source: String,
    policy_cache: String,
    window: u64,
}

impl Options {
    fn read(arguments: &[String]) -> Self {
        let published = SnapshotRef::published();
        let mut options = Self {
            estimator: "l2".to_string(),
            samples: 8,
            os_build: std::env::var("LEV_OS_BUILD").unwrap_or_default(),
            policy_source: published.source,
            policy_cache: published.cache,
            window: published.freshness_window_seconds,
            ..Self::default()
        };
        let mut arguments = arguments.iter();
        while let Some(flag) = arguments.next() {
            let mut value = || arguments.next().cloned().unwrap_or_default();
            match flag.as_str() {
                "--emit" => options.emit = Some(value()),
                "--description" => options.description = value(),
                "--created" => options.created = value(),
                "--os-build" => options.os_build = value(),
                "--families" => {
                    options.families =
                        value().split(',').map(|name| name.trim().to_string()).collect();
                }
                "--estimator" => options.estimator = value(),
                "--samples" => options.samples = value().parse().unwrap_or(options.samples),
                "--seed-base" => options.seed_base = value().parse().unwrap_or(options.seed_base),
                "--calibration" => options.calibration = Some(value()),
                "--base" => options.base = value(),
                "--policy-source" => options.policy_source = value(),
                "--policy-cache" => options.policy_cache = value(),
                "--window" => {
                    options.window = value().parse().unwrap_or(options.window);
                }
                other => {
                    eprintln!("unknown flag {other}");
                    std::process::exit(2);
                }
            }
        }
        options
    }
}

/// Reports what a package declares, and the digest a manifest would pin.
fn describe(path: &Path) {
    let package = open(path);
    println!("identifier   {}", package.metadata.adapter_identifier);
    println!("base         {}", package.metadata.base_model_signature);
    println!("rank         {}", package.metadata.lora_rank);
    println!("records      {}", package.records.len());
    println!(
        "payload      {} bytes",
        package.records.iter().map(|record| record.length).sum::<u64>()
    );
    println!("draft model  {}", if package.has_draft { "present" } else { "absent" });
    match Artifact::of_package(&package) {
        Ok(artifact) => {
            println!("sha256       {}", artifact.sha256);
            println!("size         {} bytes ({WEIGHTS_FILE})", artifact.size_bytes);
            println!("metadata     {} ({METADATA_FILE})", artifact.metadata_sha256);
        }
        Err(fault) => {
            eprintln!("sha256       not computed: {fault}");
            std::process::exit(1);
        }
    }
    println!("manifest     none — pass a manifest path, or --emit <name>@<version>");
    check_device(&package.metadata.base_model_signature);
}

/// Checks every claim a manifest makes.
fn check_manifest(path: &Path) {
    let manifest = match Manifest::load(path) {
        Ok(manifest) => manifest,
        Err(fault) => {
            eprintln!("{}: {fault}", path.display());
            std::process::exit(1);
        }
    };
    println!("release      {}", manifest.release());
    println!("description  {}", manifest.description);
    println!("interface    {} ok", manifest.interface.contract);
    println!(
        "estimator    {}, {} samples, seed block {}",
        manifest.estimator.estimator, manifest.estimator.samples, manifest.estimator.seed_base
    );

    let signature = match &manifest.artifact {
        Some(artifact) => {
            println!("artifact     {}", artifact.path);
            match manifest.check_artifact() {
                Ok(package) => {
                    println!("sha256       {} ok", artifact.sha256);
                    println!("size         {} bytes ok", artifact.size_bytes);
                    println!("rank         {} ok", artifact.lora_rank);
                    println!("identifier   {} ok", package.metadata.adapter_identifier);
                }
                Err(fault) => {
                    eprintln!("artifact     REFUSED: {fault}");
                    std::process::exit(1);
                }
            }
            manifest.base.signature.clone()
        }
        None => {
            // A base model the operating system ships has no bytes to pin,
            // and `docs/kev/mesh-plan.md` already concluded the floor for one
            // moves from digests to behavior. `evalRef` is the whole grant.
            println!("artifact     none — the operating system ships the weights");
            manifest.base.signature.clone()
        }
    };
    println!("base         {signature}");
    println!("min os build {}", manifest.base.min_os_build);

    if let Err(fault) = manifest.check_eval_refs() {
        eprintln!("evalRef      REFUSED: {fault}");
        std::process::exit(1);
    }
    if manifest.eval_ref.is_empty() {
        println!("evalRef      none — no family admits a probability");
    }
    for reference in &manifest.eval_ref {
        println!(
            "evalRef      {:<9} {:<8} {}",
            reference.family,
            if reference.admitted { "admitted" } else { "refused" },
            reference.verdict
        );
    }
    let admitted = manifest.admitted_families();
    println!(
        "admits       {}",
        if admitted.is_empty() { "nothing".to_string() } else { admitted.join(", ") }
    );

    // A release that checks out and is revoked is still refused, so the
    // policy standing belongs beside the rest rather than in another tool.
    //
    // A revocation fails this check and a stale or absent cache does not.
    // Being revoked is a fact about the release, which is what this tool
    // reads; a cache nobody has fetched on this machine is a fact about the
    // machine, and `lev-policy show` is the command that exits on it.
    let policy = manifest.policy();
    let report = policy.report();
    println!("policy       {} ({})", report.standing.label(), report.source);
    if !report.revoked.is_empty() {
        for revocation in &report.revoked {
            eprintln!("revoked      REFUSED: {} — {}", revocation.scope(), revocation.reason);
        }
        std::process::exit(1);
    }
    match policy.admits("") {
        Ok(()) => {
            println!("serves       yes, for another {} seconds", report.expires_in_seconds);
        }
        Err(refusal) => println!("serves       not from this machine yet — {}", refusal.message),
    }
    check_device(&signature);
}

/// Checks the pinning against the device, when one is reachable.
///
/// This is the failure that costs a training run: an artifact built against a
/// base the device no longer runs.
fn check_device(signature: &str) {
    match Bridge::discover().and_then(|mut bridge| bridge.base_signature_prefix()) {
        Ok(prefix) => {
            if signature.starts_with(&prefix) {
                println!("device       matches (prefix {prefix})");
            } else {
                eprintln!(
                    "device       MISMATCH: this device accepts `{prefix}...` and the model is \
                     pinned to `{signature}`"
                );
                std::process::exit(1);
            }
        }
        Err(refusal) => println!("device       not checked ({})", refusal.message),
    }
}

/// Writes a manifest to standard output.
///
/// With a package, the artifact block is read from it. Without one, the
/// manifest describes a model whose weights the operating system ships, and
/// `--base` pins what it runs on.
fn emit(package: Option<Package>, release: &str, options: &Options) {
    let (name, version) = match release.split_once('@') {
        Some((name, version)) => (name.to_string(), version.parse::<u32>().unwrap_or(0)),
        None => {
            eprintln!("--emit takes <name>@<version>, such as lev-adapted@1");
            std::process::exit(2);
        }
    };
    let artifact = package.as_ref().map(|package| match Artifact::of_package(package) {
        Ok(artifact) => artifact,
        Err(fault) => {
            eprintln!("{fault}");
            std::process::exit(1);
        }
    });
    let signature = match &package {
        Some(package) => package.metadata.base_model_signature.clone(),
        None => options.base.clone(),
    };
    if signature.is_empty() {
        eprintln!("a manifest with no package pins its base with --base <signature>");
        std::process::exit(2);
    }
    let manifest = Manifest {
        schema: MANIFEST_SCHEMA.to_string(),
        name,
        version,
        description: options.description.clone(),
        created: options.created.clone(),
        artifact,
        base: Base {
            signature,
            min_os_build: options.os_build.clone(),
            runtime: "Apple FoundationModels".to_string(),
        },
        policy_snapshot: SnapshotRef {
            source: options.policy_source.clone(),
            cache: options.policy_cache.clone(),
            freshness_window_seconds: if options.window == 0 {
                DEFAULT_WINDOW_SECONDS
            } else {
                options.window
            },
        },
        interface: Interface::of_contract(options.families.clone()),
        estimator: EstimatorConfig::new(&options.estimator, options.samples, options.seed_base),
        eval_ref: eval_refs(options.calibration.as_deref()),
        source: PathBuf::from("."),
    };
    match manifest.to_json() {
        Ok(text) => println!("{text}"),
        Err(error) => {
            eprintln!("the manifest did not encode: {error}");
            std::process::exit(1);
        }
    }
}

/// Describes every committed record in a directory.
///
/// The `record` path each entry carries is the one passed here, so run the
/// command from the directory the manifest will live in and the paths resolve
/// from the file rather than from wherever it happened to be written.
fn eval_refs(dir: Option<&str>) -> Vec<EvalRef> {
    let Some(dir) = dir else { return Vec::new() };
    let records = match Record::load_dir(Path::new(dir)) {
        Ok(records) => records,
        Err(trouble) => {
            eprintln!("{trouble}");
            std::process::exit(1);
        }
    };
    let mut refs = Vec::with_capacity(records.len());
    for (path, record) in records {
        let digest = match digest_of(&path) {
            Ok(digest) => digest,
            Err(fault) => {
                eprintln!("{fault}");
                std::process::exit(1);
            }
        };
        refs.push(EvalRef::of_record(&record, path.display().to_string(), digest));
    }
    refs
}

fn open(path: &Path) -> Package {
    match Package::open(path) {
        Ok(package) => package,
        Err(refusal) => {
            eprintln!("{}: {}", path.display(), refusal.message);
            std::process::exit(1);
        }
    }
}
