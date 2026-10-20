//! What the running application knows about itself.
//!
//! The repository context block describes the project the user is working
//! in. This block describes Coder: its name and version, the executable
//! that is running, the directory it was launched from, and what its
//! terminal shows. A question about the application is answered from here,
//! so the turn does not search the user's workspace for Coder's own source,
//! which on an installed machine is not there.

use std::path::{Path, PathBuf};

/// Coder's version, from the crate that built it.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Where the application ran: observed at startup and never changed by the
/// turn, so an answer about the working directory is a fact rather than a
/// command.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct About {
    /// The process working directory when the session opened.
    pub working_directory: PathBuf,
    /// The root of the repository the working directory sits in, when it
    /// sits in one.
    pub repository: Option<PathBuf>,
    /// The executable that is running, when the OS can say.
    pub executable: Option<PathBuf>,
}

impl About {
    /// The application as the process finds it: the working directory,
    /// the repository root the caller discovered, and the executable.
    #[must_use]
    pub fn observe(working_directory: &Path, repository: Option<&Path>) -> Self {
        Self {
            working_directory: working_directory.to_path_buf(),
            repository: repository.map(Path::to_path_buf),
            executable: std::env::current_exe().ok(),
        }
    }

    /// Whether the working directory's repository is Coder's own source
    /// tree: the OpenAgents workspace holds `crates/coder/Cargo.toml`.
    #[must_use]
    pub fn in_own_source(&self) -> bool {
        self.repository
            .as_ref()
            .is_some_and(|root| root.join("crates/coder/Cargo.toml").is_file())
    }

    /// The prompt block: what Coder is, where it is, and what its
    /// terminal shows, followed by the rule for telling application
    /// questions from project questions.
    #[must_use]
    pub fn context(&self) -> String {
        let mut block = format!(
            "about this application:\n\
             You are Coder {VERSION}, the OpenAgents terminal agent \
             (github.com/OpenAgentsInc/openagents, crate `coder`).\n\
             working directory: {}\n",
            self.working_directory.display()
        );
        match &self.repository {
            Some(root) => block.push_str(&format!(
                "workspace repository: {} (the project the user is working in)\n",
                root.display()
            )),
            None => block.push_str(
                "workspace repository: none; the working directory is not in a git repository\n",
            ),
        }
        if let Some(executable) = &self.executable {
            block.push_str(&format!("executable: {}\n", executable.display()));
        }
        if self.in_own_source() {
            block.push_str(
                "Coder's own source: this workspace is the OpenAgents checkout, so \
                 `crates/coder` and `crates/coder-terminal` here are the running \
                 application's source at some revision, not necessarily this build's.\n",
            );
        } else {
            block.push_str(
                "Coder's own source: not available on this machine. The workspace is \
                 the user's project, not Coder's implementation; do not search it for \
                 Coder's code or UI, and say the source is unavailable if asked for it.\n",
            );
        }
        block.push_str(
            "terminal: the bottom-right rail shows `input/output`, the generation \
             tokens the door reported for the latest completed turn, summed over \
             that turn's generations; it is not a session total and does not \
             include classifier calls. The composer is the framed box; the \
             intensity ladder is the amber shading.\n\
             Answer questions about Coder, its terminal, its version, or its \
             working directory from this block, without running commands. \
             Answer questions about the user's project from the repo context \
             and the workspace. When a request mixes the two, answer each part \
             from its own source, and answer the working directory from this \
             block even if another part cannot be resolved.",
        );
        block
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn decoy() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("crates/coder-terminal/src")).unwrap();
        std::fs::write(
            dir.path().join("crates/coder-terminal/src/rail.rs"),
            "// a decoy that is not the running application\n",
        )
        .unwrap();
        dir
    }

    #[test]
    fn a_decoy_terminal_directory_is_not_own_source() {
        let dir = decoy();
        let about = About::observe(dir.path(), Some(dir.path()));
        assert!(!about.in_own_source());
        let context = about.context();
        assert!(context.contains(&format!("working directory: {}", dir.path().display())));
        assert!(
            context.contains("not available on this machine"),
            "{context}"
        );
        assert!(context.contains("latest completed turn"), "{context}");
        assert!(context.contains(VERSION));
    }

    #[test]
    fn outside_git_the_block_says_so_and_keeps_the_directory() {
        let dir = tempfile::tempdir().unwrap();
        let about = About::observe(dir.path(), None);
        let context = about.context();
        assert!(context.contains("workspace repository: none"), "{context}");
        assert!(context.contains(&format!("working directory: {}", dir.path().display())));
        assert!(
            context.contains("not available on this machine"),
            "{context}"
        );
    }

    #[test]
    fn the_openagents_checkout_is_own_source() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .unwrap();
        let about = About::observe(root, Some(root));
        assert!(about.in_own_source());
        assert!(
            about
                .context()
                .contains("this workspace is the OpenAgents checkout")
        );
    }

    #[test]
    fn the_block_does_not_claim_classifier_usage() {
        let about = About::observe(Path::new("/tmp"), None);
        assert!(
            about
                .context()
                .contains("does not include classifier calls")
        );
    }
}
