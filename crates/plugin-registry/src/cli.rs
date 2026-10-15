//! Explicit registry trust and inert package selection.
use crate::{
    CATALOG_LIMIT, Result,
    catalog::{self, Catalog, Id},
    publish::{self, Previous},
    source::Source,
    store::{Policy, Store},
};
use nostr::domain::{Event, RelaySigner};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub const USAGE: &str = "openagents registry add ID|NADDR --sources FILE [--policy FILE] [--release EVENT|NEVENT] [--state DIR]\nopenagents registry list|compare [--state DIR]\nopenagents registry refresh|browse|remove ID [--cached] [--state DIR]\nopenagents registry select|pin ID --package PUBLISHER:SLUG [--state DIR]\nopenagents registry publish CATALOG.json --mirror DIR --license LICENSE --key-file FILE";

pub async fn run(words: Vec<String>) -> Result<Option<Value>> {
    if words.is_empty() || words.iter().any(|s| matches!(s.as_str(), "--help" | "-h")) {
        println!("{USAGE}");
        return Ok(None);
    }
    let command = words[0].as_str();
    let mut positionals = Vec::new();
    let mut options = BTreeMap::new();
    let mut i = 1;
    while i < words.len() {
        if !words[i].starts_with('-') {
            positionals.push(words[i].clone());
            i += 1;
            continue;
        }
        let name = words[i].as_str();
        if !matches!(
            name,
            "--sources"
                | "--policy"
                | "--release"
                | "--state"
                | "--package"
                | "--mirror"
                | "--license"
                | "--key-file"
                | "--cached"
        ) {
            return Err("unknown registry option; use --help".into());
        }
        let value = if name == "--cached" {
            "true".to_string()
        } else {
            i += 1;
            words
                .get(i)
                .filter(|s| !s.starts_with("--"))
                .ok_or("registry option needs a value")?
                .clone()
        };
        if options.insert(name.to_string(), value).is_some() {
            return Err("registry option repeated".into());
        }
        i += 1;
    }
    let allowed: &[&str] = match command {
        "add" => &["--sources", "--policy", "--release", "--state"],
        "list" | "compare" => &["--state"],
        "refresh" | "remove" => &["--state"],
        "browse" => &["--state", "--cached"],
        "select" | "pin" => &["--state", "--package"],
        "publish" => &["--mirror", "--license", "--key-file"],
        _ => return Err(USAGE.into()),
    };
    if options.keys().any(|k| !allowed.contains(&k.as_str())) {
        return Err("option does not apply to this registry command".into());
    }
    let option = |key: &str| options.get(key).map(String::as_str);
    let input = if matches!(command, "list" | "compare") {
        if !positionals.is_empty() {
            return Err(USAGE.into());
        }
        None
    } else {
        if positionals.len() != 1 {
            return Err(USAGE.into());
        }
        positionals.first().map(String::as_str)
    };
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| "clock is before Unix time")?
        .as_secs();
    if command == "publish" {
        let catalog = Catalog::parse(read_json(Path::new(input.ok_or(USAGE)?))?)?;
        let signer = signer(Path::new(
            option("--key-file").ok_or("publication needs --key-file")?,
        ))?;
        let mirror = Path::new(option("--mirror").ok_or("publication needs --mirror")?);
        let previous = previous(mirror, catalog.id())?;
        let publication = publish::build(
            &signer,
            &catalog,
            option("--license").ok_or("publication needs --license")?,
            now,
            previous,
        )?;
        publish::mirror(mirror, &publication)?;
        return Ok(Some(
            json!({"registry":catalog.id().as_str(),"revision":catalog.revision(),"listing":publication.listing.id,"mirror":mirror}),
        ));
    }
    let root = match option("--state") {
        Some(path) => PathBuf::from(path),
        None => user_home::home_dir()
            .map_err(|e| e.to_string())?
            .join(".openagents/registries"),
    };
    let store = Store::open(root)?;
    let id = input.map(registry_id).transpose()?;
    let required = || id.as_ref().ok_or(USAGE.to_string());
    let value = match command {
        "add" => {
            let id = required()?.clone();
            let sources: Vec<Source> = serde_json::from_value(read_json(Path::new(
                option("--sources").ok_or("enrollment needs --sources FILE")?,
            ))?)
            .map_err(|_| "invalid registry sources")?;
            let policy = option("--policy")
                .map(|p| read_json(Path::new(p)))
                .transpose()?
                .map(serde_json::from_value::<Policy>)
                .transpose()
                .map_err(|_| "invalid registry policy")?
                .unwrap_or_default();
            let pin = option("--release").map(release_id).transpose()?;
            store.enroll_with_pin(id.clone(), sources, policy, pin)?;
            json!({"registry":id.as_str(),"added":true})
        }
        "list" => serde_json::to_value(store.list()?).map_err(|e| e.to_string())?,
        "compare" => store.compare(now)?,
        "refresh" => store.refresh(required()?, now).await?,
        "browse" => store.browse(required()?, now, options.contains_key("--cached"))?,
        "remove" => {
            store.remove(required()?)?;
            json!({"registry":required()?.as_str(),"removed":true})
        }
        "select" | "pin" => {
            let package = Id::parse(option("--package").ok_or("selection needs --package")?)?;
            let selection = if command == "select" {
                store.select(required()?, &package, now).await?
            } else {
                store.prior_selection(required()?, &package, now)?
            };
            serde_json::to_value(selection).map_err(|e| e.to_string())?
        }
        _ => return Err(USAGE.into()),
    };
    Ok(Some(value))
}
fn read_json(path: &Path) -> Result<Value> {
    nostr::contracts::parse_strict(&publish::read(path, CATALOG_LIMIT)?).map_err(|e| e.to_string())
}
pub fn registry_id(value: &str) -> Result<Id> {
    if value.starts_with("naddr1") {
        let address = nostr::nip19::decode_naddr(value).map_err(|_| "invalid registry address")?;
        if address.kind != 30184 {
            return Err("registry address must refer to an EXT listing".into());
        }
        let key = address
            .pubkey
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        Id::parse(&format!("{key}:{}", address.identifier))
    } else {
        Id::parse(value)
    }
}
pub fn release_id(value: &str) -> Result<String> {
    if !value.starts_with("nevent1") {
        catalog::hex(value)?;
        return Ok(value.into());
    }
    let (prefix, bytes) =
        nostr::nip19::decode(value).map_err(|_| "invalid catalog event pointer")?;
    if prefix != "nevent" {
        return Err("expected catalog event pointer".into());
    }
    let mut i = 0;
    let mut id = None;
    let mut author = false;
    let mut kind = false;
    while i < bytes.len() {
        if i + 2 > bytes.len() {
            return Err("truncated event pointer".into());
        }
        let tag = bytes[i];
        let size = bytes[i + 1] as usize;
        i += 2;
        let data = bytes.get(i..i + size).ok_or("truncated event pointer")?;
        i += size;
        match tag {
            0 => {
                if size != 32 || id.is_some() {
                    return Err("invalid event pointer identity".into());
                }
                id = Some(data.iter().map(|b| format!("{b:02x}")).collect());
            }
            1 => {
                std::str::from_utf8(data).map_err(|_| "invalid relay hint")?;
            }
            2 => {
                if size != 32 || author {
                    return Err("invalid event pointer author".into());
                }
                author = true;
            }
            3 => {
                if data != 3184u32.to_be_bytes() || kind {
                    return Err("catalog pointer has another event kind".into());
                }
                kind = true;
            }
            _ => return Err("unsupported event pointer field".into()),
        }
    }
    id.ok_or("event pointer has no identity".into())
}
fn signer(path: &Path) -> Result<RelaySigner> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if std::fs::metadata(path)
            .map_err(|_| "key file is unavailable")?
            .permissions()
            .mode()
            & 0o077
            != 0
        {
            return Err("key file must be readable only by its owner".into());
        }
    }
    let secret = String::from_utf8(publish::read(path, 4096)?).map_err(|_| "invalid key file")?;
    let hex = if secret.trim().starts_with("nsec1") {
        nostr::nip19::decode_nsec(secret.trim())
            .map_err(|_| "invalid signing key")?
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect()
    } else {
        secret.trim().to_string()
    };
    RelaySigner::from_secret_hex(&hex).map_err(|_| "invalid signing key".into())
}
fn previous(root: &Path, id: &Id) -> Result<Previous> {
    let load = |kind| -> Result<Option<Event>> {
        let path = root.join(format!(
            "heads/{kind}/{}/{}.json",
            id.publisher(),
            id.slug()
        ));
        if !path.exists() {
            return Ok(None);
        }
        serde_json::from_value(read_json(&path)?)
            .map(Some)
            .map_err(|_| "mirror head is malformed".into())
    };
    let listing = load(30184)?;
    let checkpoint = load(30185)?;
    let mut result = Previous {
        listing,
        checkpoint,
        ..Default::default()
    };
    if let Some(listing) = &result.listing {
        let body = nostr::ext::parse_record(listing).map_err(|e| e.to_string())?;
        if body["state"] == "published" {
            let event = catalog::event_ref(&body["release"], 3184, Some(id.publisher()))?;
            result.release = Some(
                serde_json::from_value(read_json(&root.join(format!("events/{event}.json")))?)
                    .map_err(|_| "mirror release is malformed")?,
            );
        }
    }
    if let Some(checkpoint) = &result.checkpoint {
        let body = nostr::ext::parse_record(checkpoint).map_err(|e| e.to_string())?;
        for id in body["revocations"]
            .as_array()
            .ok_or("local publisher needs an expanded revocation list")?
        {
            let id = id.as_str().ok_or("revocation ID")?;
            catalog::hex(id)?;
            result.revocations.push(
                serde_json::from_value(read_json(&root.join(format!("events/{id}.json")))?)
                    .map_err(|_| "mirror revocation is malformed")?,
            );
        }
    }
    Ok(result)
}
/// JSON escapes control characters in untrusted titles and errors.
pub fn display(value: &Value) -> String {
    value.to_string()
}
