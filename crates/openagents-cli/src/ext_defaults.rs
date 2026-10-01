//! `openagents plugin defaults` (also `openagents ext defaults`): how a `coder-defaults` release reaches the
//! Coder on this computer (`packages/coder-defaults/policy.md`, "How a
//! release reaches runtimes").
//!
//! `sync` reads the package root's NIP-EXT releases from the relay and
//! the documents they pin (the manifest and each admission, from the
//! directory's own cache first and then the NIP-94 locators the adopter
//! published, each checked against its digest), asks the ledger's reader
//! (`xp_ledger::defaults::current`) which dependencies a live admission
//! admits, and resolves each admitted extension against the directories
//! given with `--catalog` and the extensions installed under
//! `~/.openagents/extensions`, matching the release's manifest digest to
//! the manifest the local bytes would release as. It then writes the
//! defaults directory Coder reads (`coder::defaults`): `lock.json`, the
//! admitted programs, and their skills. An admitted extension that isn't
//! held here is named and admits nothing. `show` prints the lock as
//! written.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use knowledge::xp::defaults::{self, Defaults};
use knowledge::xp::eval::Documents;
use knowledge::xp::{adopt, npub};
use nostr::domain::Event;
use serde_json::json;

use crate::ext_eval::{Target, fetch, openagents_home, resolve};
use crate::relay::{Client, relay_url, signer_for};
use crate::{Args, Output};

pub(crate) const USAGE: &str = "usage: openagents plugin defaults COMMAND [OPTIONS]
  sync [--relay URL] [--root PUBKEY] [--catalog DIR]... [--into DIR] [--as PROFILE]
        Read the newest coder-defaults release and the admissions it cites
        from the relay, resolve each admitted plugin against --catalog
        directories and the plugins installed here, and write the
        defaults directory Coder admits programs from (default
        ~/.openagents/coder-defaults, or CODER_DEFAULTS).
  show [--into DIR]
        Print the lock the last sync wrote, and what it admits.
--root is the coder-defaults package's root key (default: the package
record's). Nothing here signs or publishes; the lock is what Coder reads.";

const NAME: &str = "plugin defaults";

pub fn run(output: &Output, words: &[String]) -> u8 {
    let Some((command, rest)) = words.split_first() else {
        return output.usage(NAME, "a command is required", USAGE);
    };
    if matches!(command.as_str(), "--help" | "-h" | "help") {
        println!("{USAGE}");
        return 0;
    }
    let args = match Args::parse(rest, &[]) {
        Ok(args) => args,
        Err(message) => return output.usage(NAME, &message, USAGE),
    };
    match command.as_str() {
        "sync" => sync(output, &args),
        "show" => show(output, &args),
        other => output.usage(NAME, &format!("unknown command `{other}`"), USAGE),
    }
}

/// The defaults directory: `--into`, `CODER_DEFAULTS`, else
/// `~/.openagents/coder-defaults`.
fn directory(args: &Args) -> PathBuf {
    args.option("into").map_or_else(
        || coder::defaults::directory().unwrap_or_else(|| openagents_home().join("coder-defaults")),
        PathBuf::from,
    )
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// Every document under `dir/documents`, by digest.
fn cached_documents(dir: &Path) -> Documents {
    let Ok(entries) = std::fs::read_dir(dir.join("documents")) else {
        return Documents::new();
    };
    knowledge::xp::eval::documents(
        entries
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.is_file())
            .filter_map(|p| std::fs::read(p).ok()),
    )
}

/// Fetches one document a locator names over HTTPS, checked against its
/// digest.
fn fetch_document(url: &str, digest: &str) -> Option<Vec<u8>> {
    if !url.starts_with("https://") {
        return None;
    }
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .ok()?;
    let response = client.get(url).send().ok()?;
    if !response.status().is_success() {
        return None;
    }
    let bytes = response.bytes().ok()?;
    (bytes.len() <= knowledge::xp::eval::MAX_DOCUMENT_BYTES
        && nostr::contracts::digest_bytes(&bytes) == digest)
        .then(|| bytes.to_vec())
}

/// The documents `releases` pin: the cache, then the relay's locators.
fn documents_for(client: &mut Client, releases: &[Event], dir: &Path) -> Result<Documents, String> {
    let mut documents = cached_documents(dir);
    for _ in 0..2 {
        let missing: BTreeSet<String> = releases
            .iter()
            .flat_map(|r| defaults::wanted_digests(r, &documents))
            .filter(|d| !documents.contains_key(d))
            .collect();
        if missing.is_empty() {
            break;
        }
        let hexes: Vec<&str> = missing
            .iter()
            .map(|d| d.trim_start_matches("sha256:"))
            .collect();
        let locators = fetch(
            client,
            json!({"kinds": [nostr::ext::LOCATOR_KIND], "#x": hexes, "limit": 100}),
        )?;
        for locator in locators {
            let (Some(url), Some(hex)) = (
                locator.tag_values("url").next(),
                locator.tag_values("x").next(),
            ) else {
                continue;
            };
            let digest = format!("sha256:{hex}");
            if documents.contains_key(&digest) || !missing.contains(&digest) {
                continue;
            }
            if let Some(bytes) = fetch_document(url, &digest) {
                documents.insert(digest, bytes);
            }
        }
    }
    Ok(documents)
}

/// The extension directories to resolve admitted subjects against:
/// `--catalog` directories, then every installed
/// `~/.openagents/extensions/<key>/<slug>/<version>`.
fn candidate_dirs(args: &Args) -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = args
        .options("catalog")
        .into_iter()
        .map(PathBuf::from)
        .collect();
    let installed = openagents_home().join("extensions");
    if let Ok(keys) = std::fs::read_dir(&installed) {
        for key in keys.filter_map(Result::ok) {
            let Ok(slugs) = std::fs::read_dir(key.path()) else {
                continue;
            };
            for slug in slugs.filter_map(Result::ok) {
                let Ok(versions) = std::fs::read_dir(slug.path()) else {
                    continue;
                };
                for version in versions.filter_map(Result::ok) {
                    if version.path().join("package.json").is_file() {
                        dirs.push(version.path());
                    }
                }
            }
        }
    }
    dirs
}

/// The manifest digest `dir` would release as, under its package record's
/// publisher: what `ext_eval::publish::extension_release` builds, which is
/// how the hosted runner released each catalog tool.
fn would_release_as(target: &Target) -> Option<String> {
    let record = std::fs::read(target.root.join("package.json")).ok()?;
    let release = ext_eval::publish::extension_release(
        &target.package.publisher,
        &target.subject,
        &target.package.version,
        &record,
    )
    .ok()?;
    Some(nostr::contracts::digest_bytes(&release.manifest))
}

/// The manifest digest a release event pins.
fn manifest_digest(release: &Event) -> Option<String> {
    let body = nostr::ext::parse_record(release).ok()?;
    nostr::contracts::parse_artifact(&body["manifest"])
        .ok()
        .map(|a| a.digest)
}

/// One admitted extension resolved on this computer.
struct Held {
    subject: String,
    name: String,
    target: Target,
}

fn sync(output: &Output, args: &Args) -> u8 {
    let dir = directory(args);
    let root = match args.option("root") {
        Some(root) if root.len() == 64 && root.bytes().all(|b| b.is_ascii_hexdigit()) => {
            root.to_string()
        }
        Some(_) => return output.usage(NAME, "--root takes a 64-hex public key", USAGE),
        None => adopt::root(),
    };
    let package = adopt::package_of(&root);
    let signer = match signer_for(args.option("as")) {
        Ok(signer) => signer,
        Err(message) => return output.fail(NAME, &message),
    };
    let relay = relay_url(args.option("relay"));
    let mut client = Client::connect(&relay, signer);
    let releases = match fetch(
        &mut client,
        json!({"kinds": [nostr::ext::RELEASE_KIND], "authors": [root], "limit": 100}),
    ) {
        Ok(found) => found,
        Err(message) => return output.fail(NAME, &message),
    };
    let documents = match documents_for(&mut client, &releases, &dir) {
        Ok(documents) => documents,
        Err(message) => return output.fail(NAME, &message),
    };
    let at = now();
    let Some(current) = defaults::current(&releases, &package, &documents, at) else {
        client.close();
        return output.fail(
            NAME,
            &format!(
                "{relay} holds no {package} release whose manifest is available; nothing admitted, \
and the defaults directory is left as it was"
            ),
        );
    };
    // The admitted subjects' releases, to match against local bytes.
    let ids: Vec<String> = current.subjects();
    let subject_releases = if ids.is_empty() {
        Vec::new()
    } else {
        match fetch(
            &mut client,
            json!({"ids": ids, "kinds": [nostr::ext::RELEASE_KIND]}),
        ) {
            Ok(found) => found,
            Err(message) => return output.fail(NAME, &message),
        }
    };
    client.close();
    let candidates: Vec<Target> = candidate_dirs(args)
        .iter()
        .filter_map(|path| resolve(&path.display().to_string()).ok())
        .collect();
    let mut held = Vec::new();
    let mut missing = Vec::new();
    for admitted in &current.admitted {
        let wanted = subject_releases
            .iter()
            .find(|r| r.id == admitted.subject.id)
            .and_then(manifest_digest);
        let found = candidates
            .iter()
            .position(|target| wanted.is_some() && would_release_as(target) == wanted);
        match found {
            Some(at) => {
                let target = resolve(&candidates[at].root.display().to_string())
                    .expect("it resolved a moment ago");
                held.push(Held {
                    subject: admitted.subject.id.clone(),
                    name: target.package.name.clone(),
                    target,
                });
            }
            None => missing.push(admitted.subject.id.clone()),
        }
    }
    if let Err(message) = write_directory(&dir, &current, &documents, &held) {
        return output.fail(NAME, &message);
    }
    output.emit(
        &json!({
            "relay": relay,
            "package": package,
            "directory": dir.display().to_string(),
            "release": {"id": current.release.id, "pubkey": current.release.pubkey, "kind": current.release.kind},
            "version": current.version,
            "lock": nostr::contracts::digest_bytes(&defaults::lock_document(&current)),
            "admitted": current.admitted.iter().map(|a| json!({
                "subject": a.subject.id,
                "definition": a.definition,
                "admission": a.admission,
                "expires_at": a.expires_at,
                "held": held.iter().any(|h| h.subject == a.subject.id),
            })).collect::<Vec<_>>(),
            "missing": missing,
            "lapsed": current.lapsed,
        }),
        |value| {
            let admitted = value["admitted"].as_array().map_or(0, Vec::len);
            let held_names: Vec<String> = held.iter().map(|h| h.name.clone()).collect();
            format!(
                "coder-defaults {} (release {}) admits {} plugin(s); {} held here{}{}; wrote {}",
                value["version"].as_str().unwrap_or_default(),
                &value["release"]["id"].as_str().unwrap_or_default()[..12],
                admitted,
                if held_names.is_empty() {
                    "none".to_string()
                } else {
                    held_names.join(", ")
                },
                if missing.is_empty() {
                    String::new()
                } else {
                    format!("; not held here: {}", missing.len())
                },
                if current.lapsed.is_empty() {
                    String::new()
                } else {
                    format!("; lapsed: {}", current.lapsed.len())
                },
                value["directory"].as_str().unwrap_or_default()
            )
        },
    );
    0
}

/// Writes the defaults directory: the lock, the cached documents, and
/// each held extension's programs and skills, replacing what was there.
fn write_directory(
    dir: &Path,
    current: &Defaults,
    documents: &Documents,
    held: &[Held],
) -> Result<(), String> {
    let io = |path: &Path, error: std::io::Error| format!("{}: {error}", path.display());
    for sub in [coder::defaults::PROGRAMS_DIR, coder::defaults::SKILLS_DIR] {
        let path = dir.join(sub);
        if path.is_dir() {
            std::fs::remove_dir_all(&path).map_err(|e| io(&path, e))?;
        }
        std::fs::create_dir_all(&path).map_err(|e| io(&path, e))?;
    }
    let docs = dir.join("documents");
    std::fs::create_dir_all(&docs).map_err(|e| io(&docs, e))?;
    for (digest, bytes) in documents {
        let path = docs.join(format!("{}.json", digest.trim_start_matches("sha256:")));
        if !path.is_file() {
            std::fs::write(&path, bytes).map_err(|e| io(&path, e))?;
        }
    }
    for extension in held {
        for program in &extension.target.subject.programs {
            let path = dir
                .join(coder::defaults::PROGRAMS_DIR)
                .join(format!("{}.json", program.slug));
            std::fs::write(&path, &program.bytes).map_err(|e| io(&path, e))?;
        }
        for skill in &extension.target.subject.skills {
            let path = dir
                .join(coder::defaults::SKILLS_DIR)
                .join(format!("{}.md", skill.name));
            std::fs::write(&path, &skill.bytes).map_err(|e| io(&path, e))?;
        }
    }
    let lock = dir.join(coder::defaults::LOCK_FILE);
    std::fs::write(&lock, defaults::lock_document(current)).map_err(|e| io(&lock, e))
}

fn show(output: &Output, args: &Args) -> u8 {
    let dir = directory(args);
    match coder::defaults::read(&dir, now()) {
        Ok(Some(admitted)) => {
            output.emit(
                &json!({
                    "directory": dir.display().to_string(),
                    "release": {"id": admitted.defaults.release.id, "npub": npub(&admitted.defaults.release.pubkey)},
                    "version": admitted.defaults.version,
                    "lock": admitted.digest,
                    "programs": admitted.programs,
                    "missing": admitted.missing,
                    "skills": admitted.skills.iter().map(|s| s.name.clone()).collect::<Vec<_>>(),
                    "lapsed": admitted.lapsed,
                    "admitted": admitted.defaults.admitted,
                }),
                |_| admitted.line(),
            );
            0
        }
        Ok(None) => output.fail(
            NAME,
            &format!(
                "{} holds no lock; run `openagents plugin defaults sync`",
                dir.display()
            ),
        ),
        Err(message) => output.fail(NAME, &message),
    }
}
