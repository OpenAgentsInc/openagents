//! A draft's tests as case files, and back.
//!
//! Each draft test is the exact bytes of its `prompt.md` and each
//! `graders/<name>.md`, so writing a draft out and reading it back is
//! byte-identical, and the directory it writes is a suite the runner loads.
//! The hosted runner and a connected computer write a chat draft out the
//! same way before running it.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use nostr::cj_conversation::DraftCase;

use crate::case::{CaseError, LoadOptions};
use crate::discover::{Suite, case_dirs};

use super::render::wire_kind;

/// Each file of `cases` as `(path under the eval directory, bytes)`, in
/// order.
#[must_use]
pub fn case_files(cases: &[DraftCase]) -> Vec<(String, Vec<u8>)> {
    let mut files = Vec::new();
    for case in cases {
        files.push((
            format!("{}/prompt.md", case.id),
            case.prompt.clone().into_bytes(),
        ));
        for (name, text) in &case.graders {
            files.push((
                format!("{}/graders/{name}.md", case.id),
                text.clone().into_bytes(),
            ));
        }
    }
    files
}

/// Writes `cases` under `eval_dir`, creating it. A case directory that
/// already exists is refused, so a write never overwrites a test.
///
/// # Errors
///
/// The first file that can't be written, and an existing case directory.
pub fn write(eval_dir: &Path, cases: &[DraftCase]) -> io::Result<Vec<PathBuf>> {
    for case in cases {
        let dir = eval_dir.join(&case.id);
        if dir.exists() {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                format!(
                    "{} already exists; we never overwrite a test",
                    dir.display()
                ),
            ));
        }
    }
    let mut written = Vec::new();
    for (path, bytes) in case_files(cases) {
        let path = eval_dir.join(path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&path, bytes)?;
        written.push(path);
    }
    Ok(written)
}

/// Reads the draft tests under `eval_dir`: every case directory's exact
/// `prompt.md` and `graders/*.md` bytes. The engine loads the suite first,
/// so a directory it refuses is refused here too.
///
/// # Errors
///
/// The engine's refusal, or a file that isn't UTF-8.
pub fn read(eval_dir: &Path) -> Result<Vec<DraftCase>, CaseError> {
    let suite = Suite::load(eval_dir, LoadOptions::default())?;
    let mut cases = Vec::new();
    for path in case_dirs(eval_dir)? {
        let Some(case) = suite.cases.iter().find(|c| c.path == path) else {
            continue;
        };
        let text = |bytes: &[u8], file: String| {
            String::from_utf8(bytes.to_vec()).map_err(|_| CaseError::Invalid {
                file,
                detail: "is not UTF-8".into(),
            })
        };
        let graders = case
            .files
            .graders
            .iter()
            .map(|(name, bytes)| {
                Ok((
                    name.strip_suffix(".md").unwrap_or(name).to_string(),
                    text(bytes, format!("{path}/graders/{name}"))?,
                ))
            })
            .collect::<Result<Vec<_>, CaseError>>()?;
        cases.push(DraftCase {
            id: path.clone(),
            kind: wire_kind(case.kind),
            prompt: text(&case.files.prompt, format!("{path}/prompt.md"))?,
            graders,
        });
    }
    Ok(cases)
}
