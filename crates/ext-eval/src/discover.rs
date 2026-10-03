//! Where a suite lives, and which cases it holds.
//!
//! The eval directory is `evals/` at the extension root; the package
//! record's `eval_dir` overrides it per extension, and `--eval-dir`
//! overrides both per run. Discovery looks only beneath it, skips `.git`,
//! `.openagents`, `node_modules`, and `results`, never recurses into a case
//! directory (one holding `prompt.md`), and orders cases lexicographically
//! by path.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::case::{Case, CaseError, LoadOptions, list_dir};

/// The eval directory when nothing overrides it.
pub const DEFAULT_EVAL_DIR: &str = "evals";
/// Directory names discovery never enters.
pub const SKIPPED: [&str; 4] = [".git", ".openagents", "node_modules", "results"];

/// Resolves the eval directory: the flag over the package record's
/// `eval_dir` over `evals/`.
///
/// # Errors
///
/// Returns [`CaseError::EvalDir`] for a value that isn't one or more plain
/// directory names below the extension root.
pub fn eval_dir(
    root: &Path,
    flag: Option<&str>,
    record: Option<&str>,
) -> Result<PathBuf, CaseError> {
    let value = flag.or(record).unwrap_or(DEFAULT_EVAL_DIR);
    let plain = !value.is_empty()
        && !value.starts_with('/')
        && !value.contains('\\')
        && value.trim_end_matches('/').split('/').all(|segment| {
            !segment.is_empty() && segment != "." && segment != ".." && !segment.contains(':')
        });
    if !plain {
        return Err(CaseError::EvalDir {
            value: value.to_string(),
            detail: "must be one or more plain directory names below the extension root".into(),
        });
    }
    Ok(root.join(value.trim_end_matches('/')))
}

/// Which cases a run keeps.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Filter {
    /// Name globs; a case is kept when any matches. Empty keeps every case.
    pub cases: Vec<String>,
    /// Tags; a case is kept when any of its tags is listed. Empty keeps
    /// every case.
    pub tags: Vec<String>,
}

impl Filter {
    /// Whether the filter keeps `case`.
    #[must_use]
    pub fn keeps(&self, case: &Case) -> bool {
        let by_name = self.cases.is_empty()
            || self
                .cases
                .iter()
                .any(|pattern| crate::glob::matches(pattern, &case.name));
        let by_tag = self.tags.is_empty() || case.tags.iter().any(|tag| self.tags.contains(tag));
        by_name && by_tag
    }
}

/// Every case of one extension, in order.
#[derive(Clone, Debug)]
pub struct Suite {
    /// The eval directory the cases were read from.
    pub dir: PathBuf,
    /// The cases, ordered by path.
    pub cases: Vec<Case>,
}

impl Suite {
    /// Discovers and loads every case beneath `dir`.
    ///
    /// # Errors
    ///
    /// Returns the first [`CaseError`] of any case, and
    /// [`CaseError::DuplicateCase`] when two cases share a name. A missing
    /// eval directory is an error: a suite with nothing in it measures
    /// nothing.
    pub fn load(dir: &Path, options: LoadOptions) -> Result<Self, CaseError> {
        if !dir.is_dir() {
            return Err(CaseError::Io {
                file: dir.display().to_string(),
                detail: "there are no tests yet; write them with `openagents plugin test init`"
                    .into(),
            });
        }
        let paths = case_dirs(dir)?;
        let mut cases = Vec::with_capacity(paths.len());
        let mut names: BTreeMap<String, String> = BTreeMap::new();
        for path in paths {
            let case = Case::load(&dir.join(&path), &path, options)?;
            if let Some(first) = names.insert(case.name.clone(), path.clone()) {
                return Err(CaseError::DuplicateCase {
                    name: case.name,
                    first,
                    second: path,
                });
            }
            cases.push(case);
        }
        if cases.is_empty() {
            return Err(CaseError::Io {
                file: dir.display().to_string(),
                detail: "holds no case; a case is a directory with a prompt.md".into(),
            });
        }
        Ok(Self {
            dir: dir.to_path_buf(),
            cases,
        })
    }

    /// The suite with only the cases `filter` keeps.
    #[must_use]
    pub fn filtered(mut self, filter: &Filter) -> Self {
        self.cases.retain(|case| filter.keeps(case));
        self
    }

    /// The case named `name`.
    #[must_use]
    pub fn case(&self, name: &str) -> Option<&Case> {
        self.cases.iter().find(|case| case.name == name)
    }
}

/// Case directories beneath `dir`, relative and `/`-separated, sorted.
///
/// # Errors
///
/// Returns [`CaseError::Io`] when a directory can't be listed.
pub fn case_dirs(dir: &Path) -> Result<Vec<String>, CaseError> {
    let mut found = Vec::new();
    let mut stack = vec![(dir.to_path_buf(), String::new())];
    while let Some((at, prefix)) = stack.pop() {
        if at.join("prompt.md").exists() && !prefix.is_empty() {
            found.push(prefix.trim_end_matches('/').to_string());
            continue;
        }
        for name in list_dir(&at, &format!("{}/{prefix}", dir.display()))? {
            if SKIPPED.contains(&name.as_str()) {
                continue;
            }
            let full = at.join(&name);
            let is_dir = std::fs::symlink_metadata(&full)
                .map(|metadata| metadata.is_dir())
                .unwrap_or(false);
            if is_dir {
                stack.push((full, format!("{prefix}{name}/")));
            }
        }
    }
    found.sort();
    Ok(found)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_flag_wins_over_the_record_over_the_default() {
        let root = Path::new("/ext");
        assert_eq!(eval_dir(root, None, None).unwrap(), root.join("evals"));
        assert_eq!(
            eval_dir(root, None, Some("tests/evals")).unwrap(),
            root.join("tests/evals")
        );
        assert_eq!(
            eval_dir(root, Some("mine"), Some("tests/evals")).unwrap(),
            root.join("mine")
        );
    }

    #[test]
    fn only_plain_directory_names_are_accepted() {
        let root = Path::new("/ext");
        for bad in [
            "", "/abs", "../up", "a/../b", "./evals", "a//b", "c:\\x", "c:x",
        ] {
            let error = eval_dir(root, Some(bad), None).unwrap_err();
            assert!(
                error.to_string().contains("plain directory names"),
                "{bad}: {error}"
            );
        }
    }
}
