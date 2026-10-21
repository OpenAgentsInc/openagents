//! `livehost deploy` and `livehost rollback`.
//!
//! A deploy finds which tier changed (content, a browser module, or a
//! handler component), builds only that, uploads the new objects, writes a
//! manifest, and signs a pointer to it.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, bail};
use nostr::domain::RelaySigner;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};

use crate::manifest::{Built, HOST_VERSION, Manifest, Route, SCHEMA};
use crate::pointer::{self, Pointer};
use crate::relay::Relay;
use crate::store::Store;
use crate::{ms_since, sha256_hex};

/// A tenant's source directory. Every file under it becomes an object by
/// its path. An optional `site.json` at its root adds routes, guest builds,
/// and metadata.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Site {
    #[serde(default)]
    pub tenant: Option<String>,
    #[serde(default)]
    pub routes: Vec<Route>,
    /// Free-form manifest metadata, such as a game zone's description.
    #[serde(default)]
    pub meta: Option<serde_json::Value>,
    /// The Cargo workspace with the site's guest crates.
    #[serde(default)]
    pub guests: Option<String>,
    /// Further paths whose contents feed every guest build, such as a WIT
    /// directory outside the guest workspace.
    #[serde(default)]
    pub guest_inputs: Vec<String>,
    /// Browser modules: name to Cargo package (wasm32-unknown-unknown).
    #[serde(default)]
    pub modules: BTreeMap<String, String>,
    /// Handler components: name to Cargo package (wasm32-wasip2).
    #[serde(default)]
    pub handlers: BTreeMap<String, String>,
}

pub struct Target<'a> {
    pub store: &'a Store,
    pub relay: &'a str,
    pub signer: &'a RelaySigner,
    pub channel: &'a str,
}

/// What a pre-publish check sees.
pub struct CheckInput<'a> {
    pub tenant: &'a str,
    pub channel: &'a str,
    /// Every object by path, each a SHA-256.
    pub objects: &'a BTreeMap<String, String>,
    /// The bytes of every file in the directory, by path.
    pub files: &'a BTreeMap<String, Vec<u8>>,
    pub meta: Option<&'a serde_json::Value>,
}

/// A tenant's pre-publish check, such as a zone's first-play budget. A
/// failure blocks the `live` channel unless overridden, and only warns on
/// any other channel.
pub type Check = dyn Fn(&CheckInput<'_>) -> Result<()> + Send + Sync;

/// The channel a failing check blocks.
pub const LIVE: &str = "live";

#[derive(Default)]
pub struct Options<'a> {
    /// The tenant; required unless `site.json` names it.
    pub tenant: Option<String>,
    /// Rebuild every guest and publish even when nothing changed.
    pub force: bool,
    /// Replaces `site.json`'s metadata.
    pub meta: Option<serde_json::Value>,
    pub check: Option<&'a Check>,
    /// Publish to `live` even when the check fails.
    pub override_check: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct Report {
    pub tenant: String,
    pub channel: String,
    /// The tiers that changed: `content`, `module`, `handler`.
    pub tiers: Vec<String>,
    pub manifest: Option<String>,
    pub previous: Option<String>,
    pub objects_uploaded: usize,
    pub bytes_uploaded: usize,
    /// Check failures that didn't block this channel.
    pub warnings: Vec<String>,
    /// Milliseconds per phase, in order.
    pub phases: Vec<(String, f64)>,
    pub total_ms: f64,
}

impl Report {
    pub fn print(&self) {
        for warning in &self.warnings {
            println!("warning: {warning}");
        }
        let phases: Vec<String> = self
            .phases
            .iter()
            .map(|(name, ms)| format!("{name} {ms:.0} ms"))
            .collect();
        match &self.manifest {
            Some(manifest) => println!(
                "{}/{}: {} -> {} [{}] uploaded {} objects ({} bytes)\n  {}; total {:.0} ms",
                self.tenant,
                self.channel,
                self.previous.as_deref().map_or("none", |hash| &hash[..12]),
                &manifest[..12],
                self.tiers.join(", "),
                self.objects_uploaded,
                self.bytes_uploaded,
                phases.join(", "),
                self.total_ms
            ),
            None => println!(
                "{}/{}: nothing changed ({}; total {:.0} ms)",
                self.tenant,
                self.channel,
                phases.join(", "),
                self.total_ms
            ),
        }
    }
}

/// Reads `site.json` when the directory has one, and settles the tenant.
pub fn read_site(root: &Path, tenant: Option<&str>) -> Result<(Site, String)> {
    let path = root.join("site.json");
    let site: Site = match std::fs::read(&path) {
        Ok(bytes) => {
            serde_json::from_slice(&bytes).with_context(|| format!("parsing {}", path.display()))?
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Site::default(),
        Err(error) => return Err(error).with_context(|| format!("reading {}", path.display())),
    };
    let name = match (tenant, site.tenant.as_deref()) {
        (Some(given), Some(named)) if given != named => {
            bail!("--tenant {given} doesn't match site.json's tenant {named}")
        }
        (Some(name), _) | (None, Some(name)) => name.to_owned(),
        (None, None) => bail!("name the tenant with --tenant or in site.json"),
    };
    pointer::check_name("tenant", &name)?;
    Ok((site, name))
}

/// The newest pointer the signer published for a tenant's channel, and the
/// manifest it names.
pub async fn current(
    relay: &mut Relay,
    store: &Store,
    signer: &RelaySigner,
    tenant: &str,
    channel: &str,
) -> Result<Option<(Pointer, Manifest)>> {
    let events = relay
        .query(json!({
            "kinds": [pointer::KIND],
            "authors": [signer.pubkey()],
            "#d": [pointer::d_tag(tenant, channel)],
            "limit": 1,
        }))
        .await?;
    let newest = events
        .iter()
        .filter_map(|event| pointer::verify(event, signer.pubkey(), tenant, channel).ok())
        .max_by(|a, b| (a.created_at, &a.id).cmp(&(b.created_at, &b.id)));
    let Some(pointer) = newest else {
        return Ok(None);
    };
    let bytes = match &pointer.body {
        Some(body) => body.clone(),
        None => store.get(&pointer.manifest).await?,
    };
    let manifest = Manifest::open(&bytes, &pointer.manifest)?;
    Ok(Some((pointer, manifest)))
}

pub async fn deploy(root: &Path, target: &Target<'_>, options: &Options<'_>) -> Result<Report> {
    let force = options.force;
    let started = Instant::now();
    let mut phases = Vec::new();
    let mut mark = Instant::now();
    let mut phase = |name: &str, mark: &mut Instant| {
        phases.push((name.to_owned(), ms_since(*mark)));
        *mark = Instant::now();
    };

    let (site, tenant) = read_site(root, options.tenant.as_deref())?;
    let meta = options.meta.clone().or_else(|| site.meta.clone());
    let mut relay = Relay::connect(target.relay).await?;
    let previous = current(
        &mut relay,
        target.store,
        target.signer,
        &tenant,
        target.channel,
    )
    .await?;
    phase("current", &mut mark);

    // Detect: hash content files and guest inputs.
    let mut files = BTreeMap::new();
    collect(root, root, &mut files)?;
    let objects: BTreeMap<String, String> = files
        .iter()
        .map(|(path, bytes)| (path.clone(), sha256_hex(bytes)))
        .collect();
    let guests = site.guests.as_ref().map(|dir| root.join(dir));
    let mut tiers = Vec::new();
    let old = previous.as_ref().map(|(_, manifest)| manifest);
    if old.is_none_or(|old| old.objects != objects || old.routes != site.routes || old.meta != meta)
    {
        tiers.push("content".to_owned());
    }
    let mut builds = Vec::new();
    let mut modules = BTreeMap::new();
    let mut handlers = BTreeMap::new();
    for (kind, packages, built, wasm_target) in [
        (
            "module",
            &site.modules,
            &mut modules,
            "wasm32-unknown-unknown",
        ),
        ("handler", &site.handlers, &mut handlers, "wasm32-wasip2"),
    ] {
        for (name, package) in packages {
            let guests = guests
                .as_ref()
                .context("site.json names guest packages but no guests")?;
            let inputs = guest_inputs(root, guests, package, &site.guest_inputs)?;
            let reuse = old
                .and_then(|old| {
                    if kind == "module" {
                        old.modules.get(name)
                    } else {
                        old.handlers.get(name)
                    }
                })
                .filter(|previous| previous.inputs == inputs && !force);
            match reuse {
                Some(previous) => {
                    built.insert(name.clone(), previous.clone());
                }
                None => {
                    if !tiers.iter().any(|tier| tier == kind) {
                        tiers.push(kind.to_owned());
                    }
                    builds.push((kind, name.clone(), package.clone(), wasm_target, inputs));
                }
            }
        }
    }
    let mut warnings = Vec::new();
    if let Some(check) = options.check {
        let input = CheckInput {
            tenant: &tenant,
            channel: target.channel,
            objects: &objects,
            files: &files,
            meta: meta.as_ref(),
        };
        if let Err(error) = check(&input) {
            if target.channel == LIVE && !options.override_check {
                bail!("the pre-publish check failed, so {LIVE} is unchanged: {error:#}");
            }
            warnings.push(format!("pre-publish check: {error:#}"));
        }
    }
    phase("detect", &mut mark);

    // Build only what changed.
    let mut outputs = Vec::new();
    for (kind, name, package, wasm_target, inputs) in builds {
        let guests = guests.as_ref().expect("checked above");
        let bytes = build(guests, &package, wasm_target).await?;
        outputs.push((kind, name, inputs, bytes));
    }

    // Upload new objects, with compressed copies of the ones worth it.
    let known = old.map(Manifest::object_hashes).unwrap_or_default();
    let mut uploaded = 0;
    let mut uploaded_bytes = 0;
    let mut fresh = Vec::new();
    for (path, hash) in &objects {
        if !known.contains(hash) {
            fresh.push((path.clone(), files[path].clone()));
        }
    }
    for (kind, name, inputs, bytes) in outputs {
        let hash = sha256_hex(&bytes);
        let entry = Built {
            object: hash.clone(),
            inputs,
        };
        if kind == "module" {
            modules.insert(name, entry);
        } else {
            handlers.insert(name, entry);
        }
        if !known.contains(&hash) {
            fresh.push(("build.wasm".into(), bytes));
        }
    }
    phase("build", &mut mark);
    let mut encodings: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
    // Unchanged objects keep the copies they already have.
    if let Some(old) = old {
        let current: std::collections::BTreeSet<&String> = objects
            .values()
            .chain(modules.values().map(|built| &built.object))
            .chain(handlers.values().map(|built| &built.object))
            .collect();
        for (hash, copies) in &old.encodings {
            if current.contains(hash) {
                encodings.insert(hash.clone(), copies.clone());
            }
        }
    }
    let compressed = futures_util::future::try_join_all(fresh.iter().map(|(path, bytes)| {
        let (path, bytes) = (path.clone(), bytes.clone());
        tokio::task::spawn_blocking(move || compress(&path, &bytes))
    }))
    .await?;
    let mut uploads = Vec::new();
    for ((_, bytes), copies) in fresh.into_iter().zip(compressed) {
        let hash = sha256_hex(&bytes);
        uploads.push(bytes);
        for (encoding, copy) in copies {
            encodings
                .entry(hash.clone())
                .or_default()
                .insert(encoding.to_owned(), sha256_hex(&copy));
            uploads.push(copy);
        }
    }
    phase("compress", &mut mark);
    let manifest = Manifest {
        schema: SCHEMA.into(),
        tenant: tenant.clone(),
        host_min: HOST_VERSION,
        parent: previous
            .as_ref()
            .map(|(pointer, _)| pointer.manifest.clone()),
        objects,
        routes: site.routes.clone(),
        modules,
        handlers,
        encodings,
        meta,
    };
    let unchanged =
        old.is_some_and(|old| old.objects == manifest.objects && manifest.same_code(old));
    if unchanged && !force {
        return Ok(Report {
            tenant,
            channel: target.channel.to_owned(),
            tiers: Vec::new(),
            manifest: None,
            previous: previous.map(|(pointer, _)| pointer.manifest),
            objects_uploaded: 0,
            bytes_uploaded: 0,
            warnings,
            phases,
            total_ms: ms_since(started),
        });
    }
    // The manifest names objects by hash, so it uploads with them, and the
    // pointer carries it too.
    let bytes = manifest.to_bytes();
    let hash = crate::sha256_hex(&bytes);
    uploads.push(bytes.clone());

    let results =
        futures_util::future::try_join_all(uploads.iter().map(|bytes| target.store.put(bytes)))
            .await?;
    for ((written_hash, written), bytes) in results.iter().zip(&uploads) {
        if *written_hash == hash {
            continue;
        }
        if *written {
            uploaded += 1;
            uploaded_bytes += bytes.len();
        }
    }
    phase("upload", &mut mark);

    let prev_pointer = previous.as_ref().map(|(pointer, _)| pointer);
    publish(
        &mut relay,
        target,
        &tenant,
        &hash,
        prev_pointer,
        Some(&bytes),
    )
    .await?;
    phase("publish", &mut mark);

    Ok(Report {
        tenant,
        channel: target.channel.to_owned(),
        tiers,
        manifest: Some(hash),
        previous: prev_pointer.map(|pointer| pointer.manifest.clone()),
        objects_uploaded: uploaded,
        bytes_uploaded: uploaded_bytes,
        warnings,
        phases,
        total_ms: ms_since(started),
    })
}

/// Points the channel back at the previous manifest, or at `to`.
pub async fn rollback(tenant: &str, target: &Target<'_>, to: Option<&str>) -> Result<Report> {
    let started = Instant::now();
    let mut relay = Relay::connect(target.relay).await?;
    let Some((pointer, manifest)) = current(
        &mut relay,
        target.store,
        target.signer,
        tenant,
        target.channel,
    )
    .await?
    else {
        bail!("{tenant}/{} has no deploy to roll back", target.channel);
    };
    let found = ms_since(started);
    let destination = match to {
        Some(hash) => hash.to_owned(),
        None => manifest
            .parent
            .clone()
            .context("the current manifest has no parent")?,
    };
    // Check that the destination exists and belongs to this tenant.
    let older = Manifest::open(&target.store.get(&destination).await?, &destination)?;
    if older.tenant != tenant {
        bail!("manifest {destination} belongs to tenant {}", older.tenant);
    }
    let mark = Instant::now();
    publish(
        &mut relay,
        target,
        tenant,
        &destination,
        Some(&pointer),
        None,
    )
    .await?;
    Ok(Report {
        tenant: tenant.to_owned(),
        channel: target.channel.to_owned(),
        tiers: vec!["rollback".into()],
        manifest: Some(destination),
        previous: Some(pointer.manifest),
        objects_uploaded: 0,
        bytes_uploaded: 0,
        warnings: Vec::new(),
        phases: vec![
            ("current".into(), found),
            ("publish".into(), ms_since(mark)),
        ],
        total_ms: ms_since(started),
    })
}

/// Points `target`'s channel (signed by its own key) at the manifest that
/// `from_channel` serves, as signed by `from_pubkey`. `confirm` must name
/// that manifest: promotion moves only a build someone has checked, and
/// never what the source channel moved to since.
pub async fn promote(
    tenant: &str,
    target: &Target<'_>,
    from_channel: &str,
    from_pubkey: &str,
    confirm: &str,
) -> Result<Report> {
    let started = Instant::now();
    if from_pubkey == target.signer.pubkey() {
        bail!("promotion needs the destination channel's own key, not the source's");
    }
    let mut relay = Relay::connect(target.relay).await?;
    let events = relay
        .query(json!({
            "kinds": [pointer::KIND],
            "authors": [from_pubkey],
            "#d": [pointer::d_tag(tenant, from_channel)],
            "limit": 1,
        }))
        .await?;
    let source = events
        .iter()
        .filter_map(|event| pointer::verify(event, from_pubkey, tenant, from_channel).ok())
        .max_by(|a, b| (a.created_at, &a.id).cmp(&(b.created_at, &b.id)))
        .with_context(|| format!("{tenant}/{from_channel} has no deploy"))?;
    if source.manifest != confirm {
        bail!(
            "{tenant}/{from_channel} serves {}, not the confirmed {confirm}",
            source.manifest
        );
    }
    let bytes = match &source.body {
        Some(body) => body.clone(),
        None => target.store.get(&source.manifest).await?,
    };
    let manifest = Manifest::open(&bytes, &source.manifest)?;
    if manifest.tenant != tenant {
        bail!("manifest {confirm} belongs to tenant {}", manifest.tenant);
    }
    let previous = current(
        &mut relay,
        target.store,
        target.signer,
        tenant,
        target.channel,
    )
    .await?;
    let found = ms_since(started);
    let mark = Instant::now();
    let prev_pointer = previous.as_ref().map(|(pointer, _)| pointer);
    publish(
        &mut relay,
        target,
        tenant,
        confirm,
        prev_pointer,
        Some(&bytes),
    )
    .await?;
    Ok(Report {
        tenant: tenant.to_owned(),
        channel: target.channel.to_owned(),
        tiers: vec![format!("promote from {from_channel}")],
        manifest: Some(confirm.to_owned()),
        previous: prev_pointer.map(|pointer| pointer.manifest.clone()),
        objects_uploaded: 0,
        bytes_uploaded: 0,
        warnings: Vec::new(),
        phases: vec![("check".into(), found), ("publish".into(), ms_since(mark))],
        total_ms: ms_since(started),
    })
}

async fn publish(
    relay: &mut Relay,
    target: &Target<'_>,
    tenant: &str,
    manifest: &str,
    previous: Option<&Pointer>,
    body: Option<&[u8]>,
) -> Result<()> {
    let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    // A relay keeps the newest pointer by created_at, in whole seconds, so
    // a second deploy within the same second must still sort after the
    // first.
    let created_at = previous.map_or(now, |pointer| now.max(pointer.created_at + 1));
    let event = pointer::sign_with_body(
        target.signer,
        tenant,
        target.channel,
        manifest,
        previous.map(|pointer| pointer.manifest.as_str()),
        created_at,
        body,
    )?;
    relay.publish(&event).await
}

/// Objects at least this large get compressed copies.
const COMPRESS_MIN: usize = 1024;

/// Brotli and gzip copies of a compressible object, when they are smaller.
fn compress(path: &str, bytes: &[u8]) -> Vec<(&'static str, Vec<u8>)> {
    use std::io::Write;
    if bytes.len() < COMPRESS_MIN || !crate::manifest::compressible(path) {
        return Vec::new();
    }
    let mut copies = Vec::new();
    let mut br = Vec::new();
    {
        // Quality 9 keeps a multi-megabyte module under a second.
        let mut writer = brotli::CompressorWriter::new(&mut br, 1 << 16, 9, 22);
        if writer.write_all(bytes).is_ok() && writer.flush().is_ok() {
            drop(writer);
            if br.len() < bytes.len() {
                copies.push(("br", br));
            }
        }
    }
    let mut gzip = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::new(6));
    if gzip.write_all(bytes).is_ok()
        && let Ok(gzip) = gzip.finish()
        && gzip.len() < bytes.len()
    {
        copies.push(("gzip", gzip));
    }
    copies
}

fn collect(root: &Path, dir: &Path, files: &mut BTreeMap<String, Vec<u8>>) -> Result<()> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Ok(());
    };
    for entry in entries {
        let path = entry?.path();
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("");
        if name.starts_with('.') || name == "target" || name == "site.json" {
            continue;
        }
        if path.is_dir() {
            collect(root, &path, files)?;
        } else {
            let relative = path
                .strip_prefix(root)?
                .to_str()
                .context("non-UTF-8 path")?
                .replace('\\', "/");
            files.insert(relative, std::fs::read(&path)?);
        }
    }
    Ok(())
}

/// Digests what a guest package's build reads: its own directory, the
/// guest workspace's manifest and lock, and the site's extra inputs.
fn guest_inputs(root: &Path, guests: &Path, package: &str, extra: &[String]) -> Result<String> {
    let mut files = BTreeMap::new();
    let package_dir = package_dir(guests, package)?;
    collect(guests, &package_dir, &mut files)?;
    for name in ["Cargo.toml", "Cargo.lock"] {
        if let Ok(bytes) = std::fs::read(guests.join(name)) {
            files.insert(name.to_owned(), bytes);
        }
    }
    for path in extra {
        let mut more = BTreeMap::new();
        collect(root, &root.join(path), &mut more)?;
        files.extend(
            more.into_iter()
                .map(|(path, bytes)| (format!("extra:{path}"), bytes)),
        );
    }
    let mut digest = Sha256::new();
    digest.update(package.as_bytes());
    for (path, bytes) in files {
        digest.update((path.len() as u64).to_le_bytes());
        digest.update(path.as_bytes());
        digest.update((bytes.len() as u64).to_le_bytes());
        digest.update(&bytes);
    }
    Ok(hex::encode(digest.finalize()))
}

/// A package's directory: `guests/<name>` or `guests/<name without the
/// livehost- prefix>`.
fn package_dir(guests: &Path, package: &str) -> Result<PathBuf> {
    for candidate in [package, package.trim_start_matches("livehost-")] {
        let dir = guests.join(candidate);
        if dir.join("Cargo.toml").is_file() {
            return Ok(dir);
        }
    }
    bail!(
        "no directory for guest package {package} in {}",
        guests.display()
    )
}

/// Builds one guest package incrementally and returns its `.wasm`.
async fn build(guests: &Path, package: &str, target: &str) -> Result<Vec<u8>> {
    let mut command = tokio::process::Command::new("cargo");
    // The guest workspace gets a target of its own, so a deploy never
    // waits on another build's lock in a shared target.
    if let Ok(dir) = std::env::var("LIVEHOST_GUEST_TARGET") {
        command
            .env("CARGO_TARGET_DIR", dir)
            .env_remove("CARGO_BUILD_BUILD_DIR");
    }
    let output = command
        .current_dir(guests)
        .args(["build", "--release", "--target", target, "-p", package])
        .args(["--message-format", "json-render-diagnostics"])
        .stderr(std::process::Stdio::inherit())
        .output()
        .await
        .context("running cargo")?;
    if !output.status.success() {
        bail!("building {package} for {target} failed");
    }
    let mut wasm = None;
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        let Ok(message) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        if message["reason"] != "compiler-artifact"
            || message["target"]["name"] != package.replace('-', "_").as_str()
                && message["target"]["name"] != package
        {
            continue;
        }
        if let Some(files) = message["filenames"].as_array() {
            wasm = files
                .iter()
                .filter_map(|file| file.as_str())
                .find(|file| file.ends_with(".wasm"))
                .map(PathBuf::from)
                .or(wasm);
        }
    }
    let wasm = wasm.with_context(|| format!("cargo built no .wasm for {package}"))?;
    Ok(tokio::fs::read(&wasm).await?)
}
