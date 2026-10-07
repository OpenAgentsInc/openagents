//! The owner's command for private Verse assets
//! (`docs/verse/private-assets.md`).
//!
//! ```text
//! verse-private add SOURCE --name NAME --license ID [--reader HEX]... [--title TEXT]
//!     [--height M] [--near N] [--far N] [--edge PX] [--up AXIS] [--turn DEG]
//!     [--pose standing|seated] [--overlay jacket,heels] [--build DIR] [--dry-run]
//! verse-private place NAME (--at X,Z [--yaw RADIANS] | --seat SEAT) [--scale S]
//!     [--broker URL] [--profile P]
//! verse-private unplace NAME
//! verse-private list
//! verse-private show NAME
//! verse-private grant NAME HEX
//! verse-private revoke NAME HEX
//! verse-private remove NAME
//! verse-private whoami [--profile P]
//! verse-private phones
//! ```
//!
//! `add` converts a licensed character with Blender
//! (`scripts/blender/private_character.py`), compiles the private pack,
//! archives the raw vendor files, and uploads the pack and its manifest to
//! the private bucket with the owner's own `gcloud` login. Everything it
//! writes locally goes under `~/.openagents/verse/private-build/`, outside
//! the repository. Readers default to the `default` Verse profile's key.
//! `--overlay` dresses a seated character in our own garments
//! (`scripts/blender/outfit.py`); the provenance records that script's
//! digest too.
//! `phones` lists the paired phones that asked this computer's host for the
//! placements, with the command that grants each one.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use verse_private::manifest::{self, License, Manifest, Pack, Provenance, Source, Vendor};
use verse_private::placements::{self, Placement, Placements};
use verse_private::{BUCKET, MANIFEST_PREFIX, VENDOR_PREFIX, sha256_hex, valid_digest, valid_name};
use verse_zone_everglade::zones::everglade_pack::compile::private;

const BLENDER: &str = "/Applications/Blender.app/Contents/MacOS/Blender";
const SCRIPT: &str = "scripts/blender/private_character.py";
const OUTFIT: &str = "scripts/blender/outfit.py";
const MAX_VENDOR_FILES: usize = 256;
const LICENSES: [(&str, &str); 1] = [(
    "fab-standard",
    "Fab Standard License: may be used in the licensee's own products, including commercial ones; the raw asset may not be redistributed or resold, alone or in a way that lets others extract it.",
)];

pub fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Err(error) = run(&args) {
        eprintln!("verse-private: {error}");
        std::process::exit(1);
    }
}

fn usage() -> String {
    "usage: verse-private add|place|unplace|list|show|grant|revoke|remove|whoami|phones ... \
     (docs/verse/private-assets.md)"
        .into()
}

/// Flags and positional arguments.
struct Args {
    positional: Vec<String>,
    flags: BTreeMap<String, Vec<String>>,
    switches: Vec<String>,
}

fn parse(args: &[String]) -> Result<Args, String> {
    let mut out = Args {
        positional: Vec::new(),
        flags: BTreeMap::new(),
        switches: Vec::new(),
    };
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        if let Some(name) = arg.strip_prefix("--") {
            if name == "dry-run" {
                out.switches.push(name.into());
                continue;
            }
            let value = rest.next().ok_or(format!("--{name} needs a value"))?;
            out.flags
                .entry(name.into())
                .or_default()
                .push(value.clone());
        } else {
            out.positional.push(arg.clone());
        }
    }
    Ok(out)
}

impl Args {
    fn one(&self, name: &str) -> Option<&str> {
        self.flags
            .get(name)
            .and_then(|v| v.last())
            .map(String::as_str)
    }
    fn all(&self, name: &str) -> &[String] {
        self.flags.get(name).map_or(&[], Vec::as_slice)
    }
    fn number<T: std::str::FromStr>(&self, name: &str, default: T) -> Result<T, String> {
        self.one(name).map_or(Ok(default), |v| {
            v.parse().map_err(|_| format!("--{name} must be a number"))
        })
    }
    fn only(&self, allowed: &[&str]) -> Result<(), String> {
        match self.flags.keys().find(|k| !allowed.contains(&k.as_str())) {
            Some(unknown) => Err(format!("unknown flag --{unknown}")),
            None => Ok(()),
        }
    }
}

fn run(args: &[String]) -> Result<(), String> {
    let (command, rest) = args.split_first().ok_or_else(usage)?;
    let args = parse(rest)?;
    match command.as_str() {
        "add" => add(&args),
        "place" => place(&args),
        "unplace" => unplace(&args),
        "list" => list(),
        "show" => {
            let name = name_arg(&args)?;
            print!(
                "{}",
                String::from_utf8_lossy(&fetch_manifest(&name)?.to_bytes()?)
            );
            Ok(())
        }
        "grant" | "revoke" => readers(&args, command == "grant"),
        "remove" => remove(&args),
        "whoami" => {
            args.only(&["profile"])?;
            println!(
                "{}",
                profile_pubkey(args.one("profile").unwrap_or("default"))?
            );
            Ok(())
        }
        "phones" => {
            args.only(&[])?;
            phones()
        }
        _ => Err(usage()),
    }
}

/// The paired phones that asked for the placements, newest first, and the
/// command that lets each load an asset.
fn phones() -> Result<(), String> {
    let record = verse_private::phones::load(&verse_home()?);
    if record.phones.is_empty() {
        println!(
            "No phone has asked yet. Open the OpenAgents app on a phone paired with this \
             computer, and run this again."
        );
        return Ok(());
    }
    for phone in &record.phones {
        println!(
            "phone {} (last asked {}): verse-private grant NAME {}",
            &phone.device[..12],
            phone.last_seen,
            phone.world_key
        );
    }
    Ok(())
}

fn name_arg(args: &Args) -> Result<String, String> {
    match args.positional.as_slice() {
        [name, ..] if valid_name(name) => Ok(name.clone()),
        _ => Err("name the asset: lowercase letters, digits, and hyphens".into()),
    }
}

/// Verse's home: `VERSE_HOME`, or `~/.openagents/verse`.
fn verse_home() -> Result<PathBuf, String> {
    if let Some(home) = std::env::var_os("VERSE_HOME") {
        return Ok(PathBuf::from(home));
    }
    let home = std::env::var_os("HOME").ok_or("HOME is not set")?;
    Ok(PathBuf::from(home).join(".openagents").join("verse"))
}

/// The public key of a Verse profile's key file, never the secret.
fn profile_pubkey(profile: &str) -> Result<String, String> {
    if profile.is_empty()
        || !profile
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    {
        return Err("a profile is letters, digits, - or _".into());
    }
    let path = verse_home()?.join(format!("{profile}.key"));
    let secret = std::fs::read_to_string(&path)
        .map_err(|_| format!("no Verse key for profile {profile}; run Verse once"))?;
    let signer = nostr::domain::RelaySigner::from_secret_hex(secret.trim())
        .map_err(|_| format!("the {profile} key is not valid"))?;
    Ok(signer.pubkey().to_owned())
}

fn gs(object: &str) -> String {
    format!("gs://{BUCKET}/{object}")
}

/// Runs `gcloud storage ARGS` as the owner.
fn gcloud(args: &[&str]) -> Result<Vec<u8>, String> {
    let output = Command::new("gcloud")
        .arg("storage")
        .args(args)
        .output()
        .map_err(|_| "gcloud is not installed or not on PATH".to_owned())?;
    if !output.status.success() {
        let error = String::from_utf8_lossy(&output.stderr);
        let line = error
            .lines()
            .rev()
            .find(|l| !l.trim().is_empty())
            .unwrap_or("");
        return Err(format!(
            "gcloud storage {}: {line}",
            args.first().unwrap_or(&"")
        ));
    }
    Ok(output.stdout)
}

fn upload(local: &Path, object: &str) -> Result<(), String> {
    let local = local.to_str().ok_or("a local path is not UTF-8")?;
    gcloud(&["cp", "--quiet", local, &gs(object)]).map(|_| ())
}

fn fetch_manifest(name: &str) -> Result<Manifest, String> {
    let bytes = gcloud(&["cat", &gs(&format!("{MANIFEST_PREFIX}{name}.json"))])?;
    Manifest::parse(&bytes)
}

fn put_manifest(manifest: &Manifest, build: &Path) -> Result<(), String> {
    let path = build.join("manifest.json");
    write_private(&path, &manifest.to_bytes()?)?;
    upload(&path, &format!("{MANIFEST_PREFIX}{}.json", manifest.name))
}

fn write_private(path: &Path, bytes: &[u8]) -> Result<(), String> {
    use std::io::Write;
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options
        .open(path)
        .and_then(|mut f| f.write_all(bytes))
        .map_err(|e| format!("{}: {e}", path.display()))
}

/// What a Fab `metadata` file says about its listing.
#[derive(Debug, Default, PartialEq)]
struct Listing {
    title: String,
    uid: String,
    seller: String,
    ai_generated: bool,
}

fn parse_listing(bytes: &[u8]) -> Option<Listing> {
    let value: serde_json::Value = serde_json::from_slice(bytes).ok()?;
    let listing = value.get("listing")?;
    let text = |v: Option<&serde_json::Value>| v.and_then(|v| v.as_str()).unwrap_or("").to_owned();
    Some(Listing {
        title: text(listing.get("title")),
        uid: text(listing.get("uid")),
        seller: text(listing.get("user").and_then(|u| u.get("sellerName"))),
        ai_generated: listing
            .get("isAiGenerated")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false),
    })
}

/// Every regular file under `root`, sorted, at most [`MAX_VENDOR_FILES`].
fn files_under(root: &Path) -> Result<Vec<PathBuf>, String> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let entries = std::fs::read_dir(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        for entry in entries.flatten() {
            let kind = entry.file_type().map_err(|e| e.to_string())?;
            let path = entry.path();
            if kind.is_dir() {
                stack.push(path);
            } else if kind.is_file() && entry.file_name() != ".DS_Store" {
                out.push(path);
            }
        }
        if out.len() > MAX_VENDOR_FILES {
            return Err(format!("{} holds too many files", root.display()));
        }
    }
    out.sort();
    Ok(out)
}

/// The listing folder and the one model in it.
fn locate(source: &Path) -> Result<(PathBuf, PathBuf), String> {
    let is_model = |p: &Path| {
        p.extension().and_then(|e| e.to_str()).is_some_and(|e| {
            matches!(
                e.to_ascii_lowercase().as_str(),
                "glb" | "gltf" | "fbx" | "blend"
            )
        })
    };
    if source.is_file() {
        let folder = source
            .parent()
            .ok_or("the model has no folder")?
            .to_path_buf();
        return Ok((folder, source.to_path_buf()));
    }
    let models: Vec<PathBuf> = files_under(source)?
        .into_iter()
        .filter(|p| is_model(p))
        .collect();
    match models.as_slice() {
        [one] => Ok((source.to_path_buf(), one.clone())),
        [] => Err(format!(
            "no .glb, .gltf, .fbx, or .blend under {}",
            source.display()
        )),
        many => Err(format!(
            "several models under {}; pass one: {}",
            source.display(),
            many.iter()
                .map(|p| p.display().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        )),
    }
}

fn repository() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn add(args: &Args) -> Result<(), String> {
    args.only(&[
        "name", "license", "reader", "title", "height", "near", "far", "edge", "up", "turn",
        "pose", "overlay", "build",
    ])?;
    let source = PathBuf::from(
        args.positional
            .first()
            .ok_or("add needs a SOURCE folder or model")?,
    );
    let name = args.one("name").ok_or("add needs --name")?.to_owned();
    if !valid_name(&name) {
        return Err("--name must be lowercase letters, digits, and hyphens".into());
    }
    let license_id = args
        .one("license")
        .ok_or("add needs --license, such as fab-standard")?;
    let summary = LICENSES
        .iter()
        .find(|(id, _)| *id == license_id)
        .map(|(_, s)| (*s).to_owned())
        .ok_or(format!("unknown license {license_id}; known: fab-standard"))?;
    let mut readers: Vec<String> = args.all("reader").to_vec();
    if readers.is_empty() {
        readers.push(profile_pubkey("default")?);
    }
    if let Some(bad) = readers.iter().find(|r| !valid_digest(r)) {
        return Err(format!(
            "--reader {bad} is not a hex public key; see verse-private whoami"
        ));
    }
    let (folder, model) = locate(&source)?;
    let listing = [folder.as_path(), folder.parent().unwrap_or(&folder)]
        .iter()
        .find_map(|dir| std::fs::read(dir.join("metadata")).ok())
        .and_then(|bytes| parse_listing(&bytes))
        .unwrap_or_default();
    let title = args
        .one("title")
        .map_or(listing.title.clone(), str::to_owned);
    let build = match args.one("build") {
        Some(dir) => PathBuf::from(dir),
        None => verse_home()?.join("private-build").join(&name),
    };
    let repo = repository();
    if build
        .canonicalize()
        .unwrap_or(build.clone())
        .starts_with(repo.canonicalize().unwrap_or(repo.clone()))
    {
        return Err("the build directory must be outside the repository".into());
    }
    let blender_out = build.join("blender");
    let _ = std::fs::remove_dir_all(&blender_out);
    std::fs::create_dir_all(&blender_out).map_err(|e| format!("{}: {e}", build.display()))?;

    // 1. Blender: clean up, levels of detail, bake, rig, idle.
    let mut parameters = BTreeMap::new();
    let mut blender_args: Vec<String> = Vec::new();
    for (flag, default) in [
        ("height", "1.62"),
        ("near", "20000"),
        ("far", "5000"),
        ("edge", "1024"),
        ("up", "auto"),
        ("turn", "0"),
        ("pose", "standing"),
    ] {
        let value = args.one(flag).unwrap_or(default).to_owned();
        blender_args.extend([format!("--{flag}"), value.clone()]);
        parameters.insert(flag.to_owned(), value);
    }
    if let Some(overlay) = args.one("overlay") {
        blender_args.extend(["--overlay".to_owned(), overlay.to_owned()]);
        parameters.insert("overlay".to_owned(), overlay.to_owned());
        let outfit = std::fs::read(repo.join(OUTFIT)).map_err(|e| format!("{OUTFIT}: {e}"))?;
        parameters.insert("outfit_sha256".to_owned(), sha256_hex(&outfit));
    }
    let blender = std::env::var("BLENDER").unwrap_or_else(|_| BLENDER.into());
    let script = repo.join(SCRIPT);
    eprintln!("verse-private: converting {} with Blender", model.display());
    let output = Command::new(&blender)
        .args(["-b", "--factory-startup", "--python"])
        .arg(&script)
        .arg("--")
        .arg(&model)
        .arg(&blender_out)
        .args(&blender_args)
        .output()
        .map_err(|_| format!("Blender is not at {blender}; set BLENDER"))?;
    let report: serde_json::Value = std::fs::read(blender_out.join("report.json"))
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .ok_or_else(|| {
            let log = String::from_utf8_lossy(&output.stdout);
            let tail: Vec<&str> = log.lines().rev().take(12).collect();
            format!(
                "Blender failed:\n{}",
                tail.into_iter().rev().collect::<Vec<_>>().join("\n")
            )
        })?;

    // 2. The pack.
    let compiled = private::compile(&blender_out)?;
    let pack_path = build.join(format!("{}.vtp", compiled.sha256));
    write_private(&pack_path, &compiled.bytes)?;
    let decoded = private::decode(&compiled.bytes)?;

    // 3. The vendor archive's digests.
    let mut vendor_files = BTreeMap::new();
    let vendor = files_under(&folder)?;
    for path in &vendor {
        let relative = path
            .strip_prefix(&folder)
            .map_err(|_| "a vendor file is outside its folder")?
            .to_str()
            .ok_or("a vendor path is not UTF-8")?
            .to_owned();
        let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
        vendor_files.insert(relative, sha256_hex(&bytes));
    }

    let commit = Command::new("git")
        .arg("-C")
        .arg(&repo)
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
        .unwrap_or_default();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let text = |key: &str| {
        report
            .get(key)
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_owned()
    };
    let manifest = Manifest {
        schema: manifest::SCHEMA.into(),
        name: name.clone(),
        title: if title.is_empty() {
            name.clone()
        } else {
            title
        },
        kind: "character".into(),
        source: Source {
            marketplace: if listing.uid.is_empty() {
                "local".into()
            } else {
                "fab".into()
            },
            url: if listing.uid.is_empty() {
                String::new()
            } else {
                format!("https://www.fab.com/listings/{}", listing.uid)
            },
            listing: listing.uid,
            seller: listing.seller,
            ai_generated: listing.ai_generated,
        },
        license: License {
            id: license_id.into(),
            summary,
        },
        readers,
        pack: Pack {
            sha256: compiled.sha256.clone(),
            bytes: compiled.bytes.len() as u64,
            format: "VTP3".into(),
            forms: decoded.forms.iter().map(|f| f.name.clone()).collect(),
            triangles: decoded.forms.iter().map(|f| f.triangles()).collect(),
            texture_edge: decoded
                .textures
                .iter()
                .map(|t| t.width.max(t.height))
                .max()
                .unwrap_or(0),
            height_m: report
                .get("height_m")
                .and_then(serde_json::Value::as_f64)
                .unwrap_or(0.0) as f32,
            rig: text("rig"),
            clips: decoded
                .form(private::NEAR)
                .map(|f| f.clips.iter().map(|c| c.name.clone()).collect())
                .unwrap_or_default(),
        },
        vendor: Vendor {
            prefix: format!("{VENDOR_PREFIX}{name}/"),
            files: vendor_files,
        },
        provenance: Provenance {
            script: SCRIPT.into(),
            script_sha256: sha256_hex(&std::fs::read(&script).map_err(|e| e.to_string())?),
            blender: text("blender"),
            commit,
            parameters,
            compiled_at: verse_private::signed_url::rfc3339(now),
        },
    };
    manifest.check()?;
    eprintln!(
        "verse-private: {} is {} bytes, {:?} triangles, sha256 {}",
        name,
        compiled.bytes.len(),
        manifest.pack.triangles,
        compiled.sha256
    );

    if args.switches.iter().any(|s| s == "dry-run") {
        write_private(&build.join("manifest.json"), &manifest.to_bytes()?)?;
        eprintln!(
            "verse-private: dry run; nothing uploaded. Build: {}",
            build.display()
        );
        return Ok(());
    }
    // 4. Upload: the vendor archive, the pack, and the manifest last, so a
    // reader never sees a manifest whose pack is missing.
    for path in &vendor {
        let relative = path
            .strip_prefix(&folder)
            .map_err(|_| "a vendor file moved")?;
        upload(
            path,
            &format!("{VENDOR_PREFIX}{name}/{}", relative.display()),
        )?;
    }
    upload(
        &pack_path,
        &verse_private::pack_object(&compiled.sha256).ok_or("bad digest")?,
    )?;
    put_manifest(&manifest, &build)?;
    println!(
        "{name}: uploaded {} ({} bytes). Place it with: verse-private place {name} --at X,Z --yaw RADIANS",
        compiled.sha256,
        compiled.bytes.len()
    );
    Ok(())
}

fn coordinates(value: &str) -> Result<[f32; 2], String> {
    let (x, z) = value.split_once(',').ok_or("--at is X,Z")?;
    let parse = |v: &str| {
        v.trim()
            .parse::<f32>()
            .map_err(|_| "--at is X,Z in meters".to_owned())
    };
    Ok([parse(x)?, parse(z)?])
}

fn place(args: &Args) -> Result<(), String> {
    args.only(&["at", "seat", "yaw", "scale", "broker", "profile", "zone"])?;
    let name = name_arg(args)?;
    let manifest = fetch_manifest(&name)?;
    let home = verse_home()?;
    let mut file = match placements::load(&home)? {
        Some(file) => file,
        None => Placements::new(
            args.one("broker")
                .ok_or("the first placement needs --broker https://...")?,
            args.one("profile").unwrap_or("default"),
        ),
    };
    if let Some(broker) = args.one("broker") {
        file.broker = broker.trim_end_matches('/').into();
    }
    if let Some(profile) = args.one("profile") {
        file.profile = profile.into();
    }
    let seat = args.one("seat");
    file.place(Placement {
        asset: name.clone(),
        sha256: manifest.pack.sha256,
        bytes: manifest.pack.bytes,
        zone: args.one("zone").unwrap_or("everglade").into(),
        at: match (args.one("at"), seat) {
            (Some(at), _) => coordinates(at)?,
            (None, Some(_)) => [0.0, 0.0],
            (None, None) => return Err("place needs --at X,Z or --seat NAME".into()),
        },
        yaw: args.number("yaw", 0.0)?,
        scale: args.number("scale", 1.0)?,
        seat: seat.map(str::to_owned),
    });
    placements::save(&home, &file)?;
    println!("{name}: placed in {}", placements::path(&home).display());
    Ok(())
}

fn unplace(args: &Args) -> Result<(), String> {
    let name = name_arg(args)?;
    let home = verse_home()?;
    let Some(mut file) = placements::load(&home)? else {
        return Err("no placements".into());
    };
    file.placements.retain(|p| p.asset != name);
    placements::save(&home, &file)?;
    println!("{name}: unplaced");
    Ok(())
}

fn list() -> Result<(), String> {
    let listing = gcloud(&["ls", &gs(MANIFEST_PREFIX)]).unwrap_or_default();
    for line in String::from_utf8_lossy(&listing).lines() {
        let Some(name) = line
            .rsplit('/')
            .next()
            .and_then(|f| f.strip_suffix(".json"))
        else {
            continue;
        };
        match fetch_manifest(name) {
            Ok(m) => println!(
                "{}\t{}\t{}\t{} bytes\t{} reader(s)\t{}",
                m.name,
                m.title,
                &m.pack.sha256[..12],
                m.pack.bytes,
                m.readers.len(),
                m.license.id
            ),
            Err(error) => println!("{name}\t(unreadable: {error})"),
        }
    }
    Ok(())
}

fn readers(args: &Args, grant: bool) -> Result<(), String> {
    let name = name_arg(args)?;
    let key = args
        .positional
        .get(1)
        .ok_or("name the reader's hex public key")?;
    if !valid_digest(key) {
        return Err("a reader is a hex public key; see verse-private whoami".into());
    }
    let mut manifest = fetch_manifest(&name)?;
    manifest.readers.retain(|r| r != key);
    if grant {
        manifest.readers.push(key.clone());
    }
    let build = verse_home()?.join("private-build").join(&name);
    std::fs::create_dir_all(&build).map_err(|e| e.to_string())?;
    put_manifest(&manifest, &build)?;
    println!(
        "{name}: {} {}; {} reader(s)",
        if grant { "granted" } else { "revoked" },
        &key[..12],
        manifest.readers.len()
    );
    Ok(())
}

fn remove(args: &Args) -> Result<(), String> {
    let name = name_arg(args)?;
    let manifest = fetch_manifest(&name)?;
    // The manifest first, so no reader is granted the pack meanwhile.
    gcloud(&[
        "rm",
        "--quiet",
        &gs(&format!("{MANIFEST_PREFIX}{name}.json")),
    ])?;
    let pack = verse_private::pack_object(&manifest.pack.sha256).ok_or("bad digest")?;
    let _ = gcloud(&["rm", "--quiet", &gs(&pack)]);
    let _ = gcloud(&["rm", "--quiet", "--recursive", &gs(&manifest.vendor.prefix)]);
    println!(
        "{name}: removed; noncurrent copies expire in 30 days (gcloud storage rm --all-versions to purge now)"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fab_listing_names_its_title_seller_and_ai_flag() {
        let metadata = br#"{"listing":{"title":"A Model","uid":"u-1","isAiGenerated":true,
            "user":{"sellerName":"A Seller"}}}"#;
        assert_eq!(
            parse_listing(metadata),
            Some(Listing {
                title: "A Model".into(),
                uid: "u-1".into(),
                seller: "A Seller".into(),
                ai_generated: true,
            })
        );
        assert_eq!(parse_listing(b"not json"), None);
    }

    #[test]
    fn arguments_parse_flags_and_refuse_unknown_ones() {
        let args: Vec<String> = ["name", "--at", "1.5,-2", "--yaw", "0.5", "--dry-run"]
            .iter()
            .map(|s| (*s).to_owned())
            .collect();
        let parsed = parse(&args).unwrap();
        assert_eq!(parsed.positional, vec!["name".to_owned()]);
        assert_eq!(coordinates(parsed.one("at").unwrap()).unwrap(), [1.5, -2.0]);
        assert_eq!(parsed.number("yaw", 0.0f32).unwrap(), 0.5);
        assert!(parsed.only(&["at", "yaw"]).is_ok());
        assert!(parsed.only(&["at"]).is_err());
        assert!(parse(&["--at".to_owned()]).is_err());
    }

    #[test]
    fn locating_a_model_needs_exactly_one() {
        let dir = tempfile::tempdir().unwrap();
        assert!(locate(dir.path()).is_err());
        std::fs::create_dir_all(dir.path().join("source")).unwrap();
        std::fs::write(dir.path().join("source/a.glb"), b"x").unwrap();
        std::fs::write(dir.path().join("metadata"), b"{}").unwrap();
        let (folder, model) = locate(dir.path()).unwrap();
        assert_eq!(folder, dir.path());
        assert_eq!(model, dir.path().join("source/a.glb"));
        std::fs::write(dir.path().join("b.fbx"), b"x").unwrap();
        assert!(locate(dir.path()).is_err());
    }
}
