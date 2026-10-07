//! Native, read-only curated discovery over an explicitly admitted local mirror.

use std::fs::{File, OpenOptions};
use std::io::Read;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;

use discovery::curated::{self, Snapshot, Source};
use nostr::contracts::{ArtifactRef, parse_strict};
use nostr::domain::Event;
use serde_json::{Value, json};

use crate::{Args, Output};

pub(crate) const USAGE: &str = "usage: openagents plugin discover --catalog FILE --mirror DIR
    [--previous FILE] [--query TEXT] [--select KEY:PACKAGE/OPERATION] [--limit N]
  Inspect an explicit bounded curated source set and its current signed
  publisher releases or service heads. Keep the JSON snapshot privately
  and pass it as --previous to retain signed head and revocation knowledge.
  --select names one exact admitted card and only prints its evidence.
  No keys, accounts, reputation service, installation, or payment are used.";

pub(crate) fn run(output: &Output, words: &[String]) -> Option<u8> {
    if words.first().map(String::as_str) != Some("discover") {
        return None;
    }
    if words
        .get(1)
        .is_some_and(|s| matches!(s.as_str(), "--help" | "-h"))
    {
        println!("{USAGE}");
        return Some(0);
    }
    let result = inspect(&words[1..]);
    Some(match result {
        Ok(value) => {
            output.emit(&value, render);
            0
        }
        Err(error) => output.fail("plugin discover", &error),
    })
}

fn inspect(words: &[String]) -> Result<Value, String> {
    let args = Args::parse(words, &[])?;
    let allowed = ["catalog", "mirror", "previous", "query", "select", "limit"];
    if !args.positional().is_empty()
        || args.option_names().iter().any(|n| !allowed.contains(n))
        || allowed.iter().any(|n| args.options(n).len() > 1)
    {
        return Err(USAGE.into());
    }
    let required = |name| {
        args.option(name)
            .filter(|s| !s.is_empty())
            .ok_or_else(|| USAGE.to_owned())
    };
    let bytes = read_file(Path::new(required("catalog")?), curated::MAX_CATALOG_BYTES)?;
    let mirror = Path::new(required("mirror")?);
    let mut source = Mirror::open(mirror)?;
    let previous = match args.option("previous") {
        Some(path) => {
            let bytes = read_file(Path::new(path), 8 * 1024 * 1024)?;
            let mut value =
                parse_strict(&bytes).map_err(|_| "Invalid retained discovery snapshot.")?;
            let object = value
                .as_object_mut()
                .ok_or("Invalid retained discovery snapshot.")?;
            object.remove("mirror");
            object.remove("total_cards");
            object.remove("selected");
            let snapshot: Snapshot = serde_json::from_value(value)
                .map_err(|_| "Invalid retained discovery snapshot.")?;
            if snapshot.schema != curated::SNAPSHOT_SCHEMA {
                return Err("Unsupported retained discovery snapshot.".into());
            }
            snapshot.evidence
        }
        None => Vec::new(),
    };
    let limit = args.number::<usize>("limit", curated::MAX_ITEMS)?;
    if !(1..=curated::MAX_ITEMS).contains(&limit) {
        return Err("--limit must be from 1 through 64.".into());
    }
    let mut snapshot = curated::discover(&bytes, &mut source, &previous, crate::relay::unix_now())?;
    for card in &mut snapshot.cards {
        if card.state != "verified_discovery" {
            continue;
        }
        if let Some(record) = card.component.as_ref() {
            let support = if card.kind == "extension" && card.operation["kind"] == "program" {
                match crate::pay_plugin::packet_from_program(record) {
                    Ok(packet) => {
                        json!({"state":"supported_packet_shape","owner":"openagents-cli/pay_plugin","operation":packet.operation,"profile":match packet.profile { plugin::Profile::Pure => "pure", plugin::Profile::SnapshotRead => "snapshot-read" },"input_scope":"explicit_request_only","workspace_reads":false,"network":false,"fuel":packet.limits.fuel,"memory_bytes":packet.limits.memory_bytes,"output_bytes":packet.limits.output_bytes,"qualification":"Package admission, measured delivery, current total quote, and buyer approval remain separate."})
                    }
                    Err(error) => json!({"state":"unsupported","reason":error.message}),
                }
            } else if card.kind == "service"
                && card.operation["interface"] == "openagents.systemone.v1"
            {
                json!({"state":"supported_interface","owner":"jev","operation":"POST /v1/systemone","qualification":"Advertised interface only. Endpoint availability, model card, account rights, funding, and current quote are checked by the owning customer client."})
            } else {
                json!({"state":"unknown","reason":"No native supported binding is established for this selected operation."})
            };
            card.operation["host_support"] = support;
        }
        let supported = matches!(
            card.operation["host_support"]["state"].as_str(),
            Some("supported_packet_shape" | "supported_interface")
        );
        card.readiness = json!({"state":if supported && card.review["state"] == "current_scoped_review" {"reviewed_quote_candidate"}else{"unqualified"},"native_support_known":supported,"current_scoped_review":card.review["state"] == "current_scoped_review","availability":"not_probed","purchase_authorized":false,"remaining":["Current provider and selected paid lane qualification","Exact current total quote","Current customer authority and explicit quote approval"]});
    }
    curated::search(&mut snapshot, args.option("query").unwrap_or_default());
    let selected = match args.option("select") {
        Some(id) => {
            let found: Vec<_> = snapshot
                .cards
                .iter()
                .filter(|card| format!("{}/{}", card.id, card.selected_operation) == id)
                .collect();
            if found.len() != 1 {
                return Err("--select must name one exact admitted publisher and operation; display names are not identities.".into());
            }
            Some(serde_json::to_value(found[0]).map_err(|_| "Invalid selected discovery card.")?)
        }
        None => None,
    };
    let cards = snapshot.cards.iter().take(limit).collect::<Vec<_>>();
    Ok(
        json!({"schema":snapshot.schema,"catalog_digest":snapshot.catalog_digest,"curator":snapshot.curator,"observed_at":snapshot.observed_at,"mirror":"explicit_local_signed_evidence","cards":cards,"total_cards":snapshot.cards.len(),"selected":selected,"evidence":snapshot.evidence,"limitations":snapshot.limitations}),
    )
}

fn render(value: &Value) -> String {
    let cards = value["selected"]
        .as_object()
        .map(|_| vec![&value["selected"]])
        .unwrap_or_else(|| {
            value["cards"]
                .as_array()
                .map(|a| a.iter().collect())
                .unwrap_or_default()
        });
    let mut lines = vec![
        "Curated discovery. Selection requires separate admission and purchase approval."
            .to_owned(),
    ];
    for card in cards {
        let clean = |v: &Value| {
            v.as_str()
                .unwrap_or("unknown")
                .chars()
                .filter(|c| !c.is_control())
                .take(2048)
                .collect::<String>()
        };
        let fee = card["price"]["publisher_fee_msat"]
            .as_u64()
            .map(|fee| {
                format!(
                    "{fee} msat publisher fee; release {}",
                    clean(&card["price"]["source_release"])
                )
            })
            .unwrap_or_else(|| "unknown publisher fee".into());
        let operation = card["operation"]["host_support"]["operation"]
            .as_str()
            .or_else(|| card["operation"]["door"].as_str())
            .or_else(|| card["operation"]["id"].as_str())
            .unwrap_or("unknown");
        let operation = operation
            .chars()
            .filter(|c| !c.is_control())
            .take(256)
            .collect::<String>();
        lines.push(format!(
            "{} · {}\n  {}/{}\n  Operation: {operation}; support: {}\n  Price: {fee}; total requires a current quote\n  Review: {}; readiness: {}",
            clean(&card["title"]),
            clean(&card["state"]),
            clean(&card["id"]),
            clean(&card["selected_operation"]),
            clean(&card["operation"]["host_support"]["state"]),
            clean(&card["review"]["state"]),
            clean(&card["readiness"]["state"])
        ));
        if let Some(error) = card["error"].as_str() {
            lines.push(
                error
                    .chars()
                    .filter(|c| !c.is_control())
                    .take(2048)
                    .collect(),
            );
        }
    }
    lines.join("\n")
}

/// Only admitted, digest-addressed mirror paths are opened. Locator hints and
/// metadata never cause network requests or reads outside this directory.
pub(crate) struct Mirror {
    root: File,
    bytes: usize,
}

impl Mirror {
    pub(crate) fn open(root: &Path) -> Result<Self, String> {
        let root = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW)
            .open(root)
            .map_err(|_| "Mirror must be an explicit regular directory without a symlink.")?;
        Ok(Self { root, bytes: 0 })
    }

    fn read(&mut self, parts: &[&str], limit: usize) -> Result<Vec<u8>, String> {
        let mut directory = None::<OwnedFd>;
        let mut parent = self.root.as_raw_fd();
        for (index, part) in parts.iter().enumerate() {
            let name = std::ffi::CString::new(*part).map_err(|_| "Invalid mirror path.")?;
            let last = index + 1 == parts.len();
            // SAFETY: parent is a held descriptor, and name is a terminated
            // component. No path or symlink supplied by metadata is traversed.
            let fd = unsafe {
                libc::openat(
                    parent,
                    name.as_ptr(),
                    libc::O_RDONLY
                        | libc::O_CLOEXEC
                        | libc::O_NOFOLLOW
                        | libc::O_NONBLOCK
                        | if last { 0 } else { libc::O_DIRECTORY },
                )
            };
            if fd < 0 {
                return Err("Admitted mirror object is unavailable or contains a symlink.".into());
            }
            // SAFETY: openat returned a new descriptor exclusively owned here.
            let fd = unsafe { OwnedFd::from_raw_fd(fd) };
            if last {
                let bytes = read_opened(File::from(fd), limit)?;
                self.bytes = self
                    .bytes
                    .checked_add(bytes.len())
                    .ok_or("Mirror byte count overflow.")?;
                if self.bytes > 24 * 1024 * 1024 {
                    return Err("Mirror reads exceed 24 MiB.".into());
                }
                return Ok(bytes);
            }
            parent = fd.as_raw_fd();
            directory = Some(fd);
        }
        drop(directory);
        Err("Invalid empty mirror path.".into())
    }
}

impl Source for Mirror {
    fn heads(&mut self, kind: u16, publisher: &str, slug: &str) -> Result<Vec<Event>, String> {
        let bytes = self.read(
            &[
                "heads",
                &kind.to_string(),
                publisher,
                &format!("{slug}.json"),
            ],
            curated::MAX_RECORD_BYTES,
        )?;
        let value = parse_strict(&bytes).map_err(|_| "Invalid signed mirror head.")?;
        match value {
            Value::Array(values) if values.len() <= 64 => values
                .into_iter()
                .map(|v| {
                    serde_json::from_value(v).map_err(|_| "Invalid signed mirror head.".into())
                })
                .collect(),
            Value::Object(_) => Ok(vec![
                serde_json::from_value(value).map_err(|_| "Invalid signed mirror head.")?,
            ]),
            _ => {
                Err("Mirror head must be one signed event or a bounded signed-event array.".into())
            }
        }
    }
    fn event(&mut self, id: &str) -> Result<Event, String> {
        if id.len() != 64
            || !id
                .bytes()
                .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
        {
            return Err("Invalid exact event ID.".into());
        }
        let bytes = self.read(
            &["events", &format!("{id}.json")],
            curated::MAX_RECORD_BYTES,
        )?;
        let value = parse_strict(&bytes).map_err(|_| "Invalid signed mirror event.")?;
        serde_json::from_value(value).map_err(|_| "Invalid signed mirror event.".into())
    }
    fn artifact(&mut self, reference: &ArtifactRef) -> Result<Vec<u8>, String> {
        let digest = reference
            .digest
            .strip_prefix("sha256:")
            .ok_or("Invalid artifact digest.")?;
        if digest.len() != 64
            || !digest
                .bytes()
                .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
        {
            return Err("Invalid artifact digest.".into());
        }
        self.read(
            &["artifacts", "sha256", digest],
            usize::try_from(reference.size)
                .map_err(|_| "Artifact size overflow.")?
                .min(curated::MAX_ARTIFACT_BYTES),
        )
    }
}

pub(crate) fn read_file(path: &Path, limit: usize) -> Result<Vec<u8>, String> {
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
        .map_err(|_| "Explicit discovery file is unavailable or contains a symlink.")?;
    read_opened(file, limit)
}

fn read_opened(file: File, limit: usize) -> Result<Vec<u8>, String> {
    let metadata = file
        .metadata()
        .map_err(|_| "Cannot inspect discovery file.")?;
    if !metadata.is_file() || metadata.len() > limit as u64 {
        return Err("Discovery source must be a bounded regular file.".into());
    }
    let mut bytes = Vec::new();
    file.take(limit as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "Cannot read discovery file.")?;
    if bytes.len() > limit {
        return Err("Discovery source grew beyond its bound.".into());
    }
    Ok(bytes)
}
