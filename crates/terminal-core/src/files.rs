//! The run's files page: what a Coder run changed and the bytes it retained,
//! drawn natively beside the run (#10661).
//!
//! The list is the run's own artifact manifest, as the task owner's view
//! carries it: each change (created, modified, removed, renamed, retyped)
//! and the state of its retained bytes. Opening a file reads its retained
//! bytes through the task owner (`openagents --json task artifact ID --path
//! P`), which reads only by manifest path. The page checks the bytes
//! against the digest the manifest names before it shows a line, so a
//! changed blob is shown as changed, never as the run's result. Text is
//! shown literally, line-numbered and in ASCII; binary content is described,
//! never interpreted. Nothing here runs, renders, or exports content, and a
//! read never changes the task or its review.

use serde_json::Value;
use sha2::{Digest, Sha256};
use std::sync::mpsc::Receiver;

/// The most bytes of helper output the page reads: a retained file is at
/// most 1 MiB, and its JSON byte array is at most four bytes a byte.
pub const READ_MAX: usize = 5 * 1024 * 1024;

/// The most bytes a retained file may hold; the owner's own bound.
pub const FILE_MAX: usize = 1024 * 1024;

/// The most lines of a file the page shows.
pub const LINES_MAX: usize = 20_000;

/// One change the run made, with what was retained of it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Change {
    /// `created`, `modified`, `removed`, `renamed`, or `retyped`.
    pub kind: String,
    /// The path after the change, relative to the workspace.
    pub path: String,
    /// The path before a rename.
    pub from: Option<String>,
    /// The retained bytes' state: `retained`, `removed`, `symlink`,
    /// `directory`, `unavailable_or_over_limit`, or `unavailable`.
    pub state: String,
    pub digest: Option<String>,
    pub bytes: Option<usize>,
}

/// The manifest a run's view carries.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Manifest {
    pub source: String,
    pub candidate: Option<String>,
    pub complete: bool,
    pub omitted: usize,
    pub changes: Vec<Change>,
}

fn text(value: &Value) -> Option<String> {
    value.as_str().map(crate::ascii::ascii)
}

impl Manifest {
    /// Reads the manifest out of a run view's `artifacts`, or `None` when
    /// it is not one.
    #[must_use]
    pub fn from_view(artifacts: &Value) -> Option<Manifest> {
        let entries = artifacts["entries"].as_array()?;
        let mut changes = Vec::new();
        for change in artifacts["changes"].as_array()? {
            let kind = text(&change["change"])?;
            let (path, from) = match kind.as_str() {
                "renamed" => (text(&change["to"])?, text(&change["from"])),
                _ => (text(&change["path"])?, None),
            };
            let entry = entries
                .iter()
                .find(|entry| text(&entry["path"]).as_deref() == Some(path.as_str()));
            changes.push(Change {
                kind,
                path,
                from,
                state: entry
                    .and_then(|entry| text(&entry["state"]))
                    .unwrap_or_else(|| "unavailable".into()),
                digest: entry.and_then(|entry| text(&entry["digest"])),
                bytes: entry
                    .and_then(|entry| entry["bytes"].as_u64())
                    .and_then(|bytes| usize::try_from(bytes).ok()),
            });
        }
        Some(Manifest {
            source: text(&artifacts["source_snapshot"]).unwrap_or_default(),
            candidate: text(&artifacts["candidate_snapshot"]),
            complete: artifacts["complete"].as_bool().unwrap_or(false),
            omitted: artifacts["omitted_changes"]
                .as_u64()
                .and_then(|n| usize::try_from(n).ok())
                .unwrap_or(0),
            changes,
        })
    }
}

/// A file's bytes as the page may show them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Shown {
    /// Text, its digest checked.
    Text(String),
    /// Bytes that are not UTF-8 text: described, never shown.
    Binary(usize),
}

/// Why a file could not be shown.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Unread {
    /// The task store keeps no such retained file.
    Missing,
    /// The bytes differ from the digest the manifest names.
    Changed,
    /// The owner refused the read: an unsafe path or another refusal.
    Forbidden(String),
    /// The owner cannot answer now.
    Unavailable(String),
}

pub type Read = Result<Shown, Unread>;

/// `sha256:` and the hex digest of `bytes`, as the task owner writes it.
#[must_use]
pub fn digest(bytes: &[u8]) -> String {
    let mut out = String::from("sha256:");
    for byte in Sha256::digest(bytes) {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

/// Decodes the owner's answer to reading retained file `path`, whose
/// manifest names `expected`: the bytes, checked against that digest.
#[must_use]
pub fn decode(stdout: &[u8], stderr: &[u8], path: &str, expected: &str) -> Read {
    if stdout.len() > READ_MAX {
        return Err(Unread::Unavailable("the file is too large to show".into()));
    }
    let last = |bytes: &[u8]| -> Option<Value> {
        let text = String::from_utf8_lossy(bytes);
        let line = text.lines().rev().find(|line| !line.trim().is_empty())?;
        serde_json::from_str(line).ok()
    };
    if let Some(value) = last(stdout)
        && value.get("error").is_none()
    {
        if value["path"].as_str() != Some(path) {
            return Err(Unread::Unavailable(
                "the task owner answered for another file".into(),
            ));
        }
        let Some(array) = value["bytes"].as_array() else {
            return Err(Unread::Unavailable(
                "the task owner's answer held no bytes".into(),
            ));
        };
        if array.len() > FILE_MAX {
            return Err(Unread::Unavailable(
                "the file is over the 1 MiB bound".into(),
            ));
        }
        let mut bytes = Vec::with_capacity(array.len());
        for byte in array {
            match byte.as_u64().and_then(|b| u8::try_from(b).ok()) {
                Some(byte) => bytes.push(byte),
                None => {
                    return Err(Unread::Unavailable(
                        "the task owner's answer was not bytes".into(),
                    ));
                }
            }
        }
        if digest(&bytes) != expected || value["digest"].as_str() != Some(expected) {
            return Err(Unread::Changed);
        }
        return Ok(match String::from_utf8(bytes) {
            Ok(text) => Shown::Text(text),
            Err(error) => Shown::Binary(error.into_bytes().len()),
        });
    }
    let error = last(stderr).or_else(|| last(stdout));
    let error = error.as_ref().map(|value| &value["error"]);
    let code = error.and_then(|e| e["code"].as_str()).unwrap_or_default();
    let message = error
        .and_then(|e| e["message"].as_str())
        .map(crate::ascii::ascii)
        .unwrap_or_else(|| "the task owner's answer was not readable".into());
    Err(match code {
        "not_found" => Unread::Missing,
        "corrupt_store" if message.contains("digest") => Unread::Changed,
        "unsafe_path" => Unread::Forbidden(message),
        "" => Unread::Unavailable(message),
        code => Unread::Unavailable(format!("{message} ({code})")),
    })
}

/// The files page's state.
#[derive(Default)]
pub struct Page {
    pub open: bool,
    /// The run whose files are listed.
    pub task: Option<String>,
    pub manifest: Option<Manifest>,
    /// The change the arrows pick.
    pub selected: usize,
    /// The file shown, by index into the changes, and its read.
    pub viewing: Option<usize>,
    pub shown: Option<Read>,
    pub reading: Option<Receiver<Read>>,
    pub scroll: usize,
    pub reads: u64,
}

impl Page {
    /// Lists the files of run `task` from its `manifest`; the same run keeps
    /// what was picked.
    pub fn show(&mut self, task: &str, manifest: Option<Manifest>) {
        if self.task.as_deref() != Some(task) {
            *self = Page {
                task: Some(task.to_owned()),
                reads: self.reads,
                ..Page::default()
            };
        }
        let count = manifest.as_ref().map_or(0, |m| m.changes.len());
        self.selected = self.selected.min(count.saturating_sub(1));
        self.manifest = manifest;
        self.open = true;
    }

    /// The picked change, when there is one.
    #[must_use]
    pub fn picked(&self) -> Option<&Change> {
        self.manifest.as_ref()?.changes.get(self.selected)
    }
}

fn word(text: &str) -> String {
    text.replace('_', " ")
}

fn short(digest: &str) -> String {
    let hex = digest.strip_prefix("sha256:").unwrap_or(digest);
    format!("sha256:{}", &hex[..hex.len().min(12)])
}

/// What opening a change would show without a read, when nothing can be
/// read: the reason, or `None` when its bytes are retained.
#[must_use]
pub fn unreadable(change: &Change) -> Option<String> {
    match change.state.as_str() {
        "retained" if change.digest.is_some() => None,
        "removed" => Some("The run removed this file; there are no bytes to show.".into()),
        "symlink" => Some("A symbolic link; links are listed, never followed.".into()),
        "directory" => Some("A directory; its files are listed separately.".into()),
        "unavailable_or_over_limit" => Some(
            "Over the retained bound (1 MiB a file, 8 MiB a run), or unreadable when the run ended; not retained."
                .into(),
        ),
        _ => Some("Its bytes were not retained.".into()),
    }
}

/// The page's text before wrapping.
#[must_use]
pub fn lines(page: &Page) -> Vec<(String, crate::paper::Tone)> {
    use crate::ascii::ascii;
    use crate::paper::Tone;
    let mut out = Vec::new();
    let task = ascii(page.task.as_deref().unwrap_or("-"));
    let Some(manifest) = &page.manifest else {
        out.push((format!("FILES of run {task}  [none retained]"), Tone::Loud));
        out.push((
            "The run retained no changed files: it has not ended, changed nothing, or kept no manifest."
                .into(),
            Tone::Present,
        ));
        return out;
    };
    if let Some(index) = page.viewing
        && let Some(change) = manifest.changes.get(index)
    {
        let state = match (&page.shown, page.reading.is_some()) {
            _ if unreadable(change).is_some() => word(&change.state),
            (None, _) | (_, true) => "reading".into(),
            (Some(Ok(Shown::Text(_))), _) => "digest checked".into(),
            (Some(Ok(Shown::Binary(_))), _) => "binary".into(),
            (Some(Err(Unread::Missing)), _) => "missing".into(),
            (Some(Err(Unread::Changed)), _) => "changed".into(),
            (Some(Err(Unread::Forbidden(_))), _) => "forbidden".into(),
            (Some(Err(Unread::Unavailable(_))), _) => "unavailable".into(),
        };
        out.push((format!("FILE {}  [{state}]", change.path), Tone::Loud));
        let mut about = format!("CHANGE {}", word(&change.kind));
        if let Some(from) = &change.from {
            about.push_str(&format!(" from {from}"));
        }
        if let Some(digest) = &change.digest {
            about.push_str(&format!("  {}", short(digest)));
        }
        if let Some(bytes) = change.bytes {
            about.push_str(&format!("  {bytes} bytes"));
        }
        about.push_str(&format!("  RUN {task}"));
        out.push((about, Tone::Present));
        out.push((
            "The run's result as retained; the source's bytes are not kept here. Nothing on this page runs."
                .into(),
            Tone::Quiet,
        ));
        out.push((String::new(), Tone::Quiet));
        if let Some(why) = unreadable(change) {
            out.push((why, Tone::Present));
            return out;
        }
        match &page.shown {
            None => out.push(("Reading the retained bytes...".into(), Tone::Quiet)),
            Some(Ok(Shown::Text(text))) => {
                let count = text.lines().count();
                let width = count.max(1).to_string().len();
                for (number, line) in text.lines().take(LINES_MAX).enumerate() {
                    out.push((
                        format!(
                            "{:>width$} | {}",
                            number + 1,
                            ascii(&line.replace('\t', "    "))
                        ),
                        Tone::Present,
                    ));
                }
                if count > LINES_MAX {
                    out.push((
                        format!("[{} more lines are not shown]", count - LINES_MAX),
                        Tone::Quiet,
                    ));
                }
                if text.is_empty() {
                    out.push(("[an empty file]".into(), Tone::Quiet));
                }
            }
            Some(Ok(Shown::Binary(bytes))) => out.push((
                format!("Binary content, {bytes} bytes; this page shows text only."),
                Tone::Present,
            )),
            Some(Err(Unread::Missing)) => out.push((
                "The task store no longer keeps these bytes.".into(),
                Tone::Present,
            )),
            Some(Err(Unread::Changed)) => out.push((
                "The retained bytes differ from the digest the run recorded; they are not shown."
                    .into(),
                Tone::Present,
            )),
            Some(Err(Unread::Forbidden(why))) => {
                out.push((
                    format!("The task owner refused the read: {why}."),
                    Tone::Present,
                ));
            }
            Some(Err(Unread::Unavailable(why))) => out.push((
                format!("The file can't be read now: {why}. ENTER reads it again."),
                Tone::Present,
            )),
        }
        return out;
    }
    let candidate = manifest
        .candidate
        .as_deref()
        .map_or_else(|| "not complete".to_owned(), short);
    out.push((
        format!(
            "FILES of run {task}  {} changes{}",
            manifest.changes.len(),
            if manifest.complete {
                ""
            } else {
                "  [incomplete]"
            }
        ),
        Tone::Loud,
    ));
    out.push((
        format!(
            "SOURCE {}  RESULT {candidate}",
            if manifest.source.is_empty() {
                "unknown".into()
            } else {
                short(&manifest.source)
            }
        ),
        Tone::Present,
    ));
    if manifest.omitted > 0 {
        out.push((
            format!("{} more changes were not recorded", manifest.omitted),
            Tone::Quiet,
        ));
    }
    out.push((String::new(), Tone::Quiet));
    if manifest.changes.is_empty() {
        out.push(("The run changed no files.".into(), Tone::Present));
    }
    for (index, change) in manifest.changes.iter().enumerate() {
        let picked = index == page.selected;
        let size = change
            .bytes
            .map(|bytes| format!(" {bytes} bytes"))
            .unwrap_or_default();
        let from = change
            .from
            .as_deref()
            .map(|from| format!(" (from {from})"))
            .unwrap_or_default();
        out.push((
            format!(
                "{} {:<8} {}{from}  {}{size}",
                if picked { ">" } else { " " },
                word(&change.kind),
                change.path,
                word(&change.state)
            ),
            if picked { Tone::Loud } else { Tone::Present },
        ));
    }
    out
}
