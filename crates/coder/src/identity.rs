//! Which Coder this is: the repository, the commit, and whether the tree
//! it was built from was dirty.
//!
//! Two programs have answered to `coder` on the same machine, so the
//! binary says which one it is. `build.rs` stamps the commit and the tree
//! state; `scripts/install-coder.sh` passes both explicitly.

/// The repository this Coder is built from.
pub const REPOSITORY: &str = "OpenAgentsInc/openagents";

/// The commit this binary was built from, or `unknown`.
pub const COMMIT: &str = env!("CODER_GIT_COMMIT");

/// `clean`, `dirty`, or `unknown`: the tree the build came from.
pub const TREE: &str = env!("CODER_GIT_TREE");

/// The crate version.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The commit's first ten characters, or the whole of it when shorter.
#[must_use]
pub fn short_commit() -> &'static str {
    COMMIT.get(..10).unwrap_or(COMMIT)
}

/// The build as a trace's session header records it, such as
/// `0.1.0+5a64cdf04f` or `0.1.0+5a64cdf04f.dirty`.
#[must_use]
pub fn build() -> String {
    match TREE {
        "clean" => format!("{VERSION}+{}", short_commit()),
        other => format!("{VERSION}+{}.{other}", short_commit()),
    }
}

/// What `coder --version` prints.
#[must_use]
pub fn line() -> String {
    format!("coder {VERSION} ({REPOSITORY} {} {TREE})", short_commit())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_version_names_the_repository_the_commit_and_the_tree() {
        let line = line();
        assert!(line.starts_with("coder "), "{line}");
        assert!(line.contains(REPOSITORY), "{line}");
        assert!(line.contains(short_commit()), "{line}");
        assert!(["clean", "dirty", "unknown"].contains(&TREE), "{TREE}");
        assert!(build().starts_with(VERSION));
    }
}
