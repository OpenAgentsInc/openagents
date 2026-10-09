//! Writing entries: rendering one to its file, the template `kb add`
//! writes, the in-place edits `kb admit` and `kb withdraw` make, and the
//! `versions/` directory where earlier and pending versions live.
//!
//! An entry's current version is `<dir>/<id>.md`. When it's replaced, the
//! old file moves to `<dir>/versions/<id>.v<N>.md`, so an earlier run's
//! digest can still be explained. A proposed new version of an admitted
//! entry waits in `versions/` as a candidate until `kb admit` promotes it,
//! so a proposal never hides the admitted version.

use std::path::{Path, PathBuf};

use crate::{Entry, Kind, Status, valid_id};
use xp_ledger::entry::block;

/// Marks template text `kb lint` refuses until it's replaced.
pub const PLACEHOLDER: &str = "TODO:";

/// The file `kb add` writes: a candidate with every field present and
/// placeholders the lint refuses until they're filled in.
///
/// # Errors
///
/// A bad ID or an empty title.
pub fn template(id: &str, kind: Kind, title: &str, author: &str) -> Result<String, String> {
    if !valid_id(id) {
        return Err(format!(
            "the id `{id}` must be lowercase letters, digits, dots, and hyphens, starting with a letter"
        ));
    }
    if title.trim().is_empty() {
        return Err("the title is empty".to_string());
    }
    let entry = Entry {
        id: id.to_string(),
        version: 1,
        kind,
        title: title.trim().to_string(),
        summary: format!(
            "{PLACEHOLDER} one or two sentences: what the entry is about and when it applies."
        ),
        tags: Vec::new(),
        applies_when: format!("{PLACEHOLDER} the code or state this entry bears on."),
        status: Status::Candidate,
        author: author.to_string(),
        written_from: vec!["reference".to_string()],
        cites: Vec::new(),
        evidence: Vec::new(),
        answer: None,
        ui: None,
        body: format!(
            "## Details\n\n{PLACEHOLDER} the definitions, formulas, and a worked example.\n\n## How to check\n\n{PLACEHOLDER} a property that tells the right form from the wrong one, with a runnable snippet."
        ),
        digest: String::new(),
    };
    Ok(entry.render())
}

/// `text` with its front matter's top-level `key` line, and any indented
/// lines under it, replaced by `replacement` (a whole line or lines).
fn replace_key(text: &str, key: &str, replacement: &str) -> Result<String, String> {
    let (front, body) = crate::front::split(text)?;
    let mut out: Vec<String> = Vec::new();
    let mut lines = front.lines().peekable();
    let mut found = false;
    while let Some(line) = lines.next() {
        if !found && line.starts_with(&format!("{key}:")) {
            found = true;
            out.push(replacement.trim_end().to_string());
            while lines
                .peek()
                .is_some_and(|l| l.starts_with(' ') || l.trim().is_empty())
            {
                lines.next();
            }
            continue;
        }
        out.push(line.to_string());
    }
    if !found {
        return Err(format!("the front matter has no `{key}`"));
    }
    Ok(format!("---\n{}\n---\n{body}", out.join("\n")))
}

/// The entry file `text` with its status set to `status`.
///
/// # Errors
///
/// When the front matter has no status.
pub fn set_status(text: &str, status: Status) -> Result<String, String> {
    replace_key(text, "status", &format!("status: {status}"))
}

/// The entry file `text` with its evidence list set to `lines`.
///
/// # Errors
///
/// When the front matter has no evidence key.
pub fn set_evidence(text: &str, lines: &[String]) -> Result<String, String> {
    replace_key(text, "evidence", &format!("evidence:{}", block(lines, 2)))
}

/// Today's date in UTC, as `YYYY-MM-DD`.
#[must_use]
pub fn today() -> String {
    let seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    date(seconds)
}

/// The UTC date of a Unix time, as `YYYY-MM-DD`.
#[must_use]
pub fn date(seconds: u64) -> String {
    // Howard Hinnant's days-to-civil algorithm.
    let days = (seconds / 86_400) as i64 + 719_468;
    let era = days.div_euclid(146_097);
    let doe = days.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}")
}

/// `<dir>/versions/<id>.v<version>.md`.
#[must_use]
pub fn version_path(dir: &Path, id: &str, version: u32) -> PathBuf {
    dir.join("versions").join(format!("{id}.v{version}.md"))
}

/// Moves `<dir>/<id>.md` to `versions/`, under the version its file
/// states. Returns where it went.
///
/// # Errors
///
/// When the file can't be read, parsed, or moved.
pub fn archive(dir: &Path, id: &str) -> Result<PathBuf, String> {
    let path = dir.join(format!("{id}.md"));
    let text = std::fs::read_to_string(&path)
        .map_err(|e| format!("can't read {}: {e}", path.display()))?;
    let entry = Entry::parse(&text).map_err(|e| format!("{}: {e}", path.display()))?;
    let to = version_path(dir, id, entry.version);
    if let Some(parent) = to.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("can't make {}: {e}", parent.display()))?;
    }
    std::fs::rename(&path, &to).map_err(|e| format!("can't move {}: {e}", path.display()))?;
    Ok(to)
}

/// The newest version of `id` waiting in `versions/` that is newer than
/// `current`, with its path.
#[must_use]
pub fn pending(dir: &Path, id: &str, current: u32) -> Option<(PathBuf, Entry)> {
    let listing = std::fs::read_dir(dir.join("versions")).ok()?;
    listing
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with(&format!("{id}.v")) && n.ends_with(".md"))
        })
        .filter_map(|p| {
            let entry = Entry::parse(&std::fs::read_to_string(&p).ok()?).ok()?;
            (entry.id == id && entry.version > current).then_some((p, entry))
        })
        .max_by_key(|(_, e)| e.version)
}

#[cfg(test)]
mod tests;
