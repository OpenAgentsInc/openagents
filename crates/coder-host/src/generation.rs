//! Which NIP-REACH generation `coder host serve` runs as.
//!
//! The host root holds one generation counter that `coder-service` owns
//! (`coder_service::generation`). A standalone host advances it. A host
//! the service launcher starts receives the generation the launcher
//! reserved, in `OPENAGENTS_HOST_GENERATION`, and the launcher's root in
//! `OPENAGENTS_HOST_GENERATION_ROOT`; the host claims that generation there
//! before it serves. An explicit `--generation N` is claimed the same way.
//! A claim below the counter, or of a value already used, refuses, so no
//! mix of standalone and service starts shows clients a lower or repeated
//! generation.

use std::path::{Path, PathBuf};

use crate::{Error, Result};

/// The environment variable naming the root whose counter the launcher
/// advanced.
pub const ROOT_ENV: &str = "OPENAGENTS_HOST_GENERATION_ROOT";

/// Where the generation came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    /// `--generation N` or `OPENAGENTS_HOST_GENERATION`: claim this value.
    Given(u64),
    /// Nothing given: advance the counter.
    Next,
}

/// The counter's root: the launcher's, when it named one, else `root`.
#[must_use]
pub fn counter_root(root: &Path) -> PathBuf {
    std::env::var_os(ROOT_ENV).map_or_else(|| root.to_path_buf(), PathBuf::from)
}

/// Returns the generation this start serves as, recorded durably in the
/// counter under `root` before it returns.
///
/// # Errors
/// Refuses a given generation that is lower than the counter or already
/// used, and a counter that cannot be read or written.
pub fn resolve(root: &Path, source: Source) -> Result<u64> {
    let refused = |error: coder_service::Error| Error::Config(error.to_string());
    match source {
        Source::Given(generation) => {
            coder_service::generation::claim(root, generation).map_err(refused)?;
            Ok(generation)
        }
        Source::Next => coder_service::generation::advance(root).map_err(refused),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_standalone_start_advances_and_a_given_generation_is_claimed_once() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("host");
        let first = resolve(&root, Source::Next).unwrap();
        let second = resolve(&root, Source::Next).unwrap();
        assert!(second > first);
        // A lower or repeated explicit generation would roll clients back.
        assert!(resolve(&root, Source::Given(first)).is_err());
        assert!(resolve(&root, Source::Given(second)).is_err());
        // A launcher reservation is claimed exactly once.
        let reserved = coder_service::generation::reserve(&root, 0).unwrap();
        assert_eq!(resolve(&root, Source::Given(reserved)).unwrap(), reserved);
        assert!(resolve(&root, Source::Given(reserved)).is_err());
        assert!(resolve(&root, Source::Next).unwrap() > reserved);
    }
}
