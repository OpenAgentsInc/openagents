//! Temporary startup files inject hooks without changing the user's dotfiles.

use std::path::{Path, PathBuf};

pub struct Integration {
    root: PathBuf,
}

/// A private directory for one host's startup files.
fn directory(shell: &str) -> Result<PathBuf, String> {
    let root = std::env::temp_dir().join(format!(
        "openagents-{shell}-{}-{}",
        std::process::id(),
        super::pty::request()
    ));
    let mut builder = std::fs::DirBuilder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(&root).map_err(|error| error.to_string())?;
    Ok(root)
}

/// The bash startup file, read through `--rcfile` (`terminal_core::bash`).
pub const BASH_RC: &str = "bashrc";
/// The fish hooks, sourced through `--init-command` (`terminal_core::fish`).
pub const FISH_HOOK: &str = "hook.fish";

impl Integration {
    /// Startup files for bash: one `--rcfile` that sources the user's files
    /// and adds the hooks.
    pub fn bash() -> Result<Self, String> {
        let integration = Self {
            root: directory("bash")?,
        };
        std::fs::write(
            integration.root.join(BASH_RC),
            terminal_core::bash::rcfile(),
        )
        .map_err(|error| error.to_string())?;
        Ok(integration)
    }

    /// The arguments and environment that start bash on these files, as a
    /// login shell when `login` is set.
    pub fn bash_start(&self, login: bool) -> (Vec<String>, Vec<(String, String)>) {
        let args = vec![
            "--rcfile".into(),
            self.root.join(BASH_RC).display().to_string(),
        ];
        let env = if login {
            vec![("OPENAGENTS_BASH_LOGIN".into(), "1".into())]
        } else {
            Vec::new()
        };
        (args, env)
    }

    /// The fish hooks, sourced after the user's configuration.
    pub fn fish() -> Result<Self, String> {
        let integration = Self {
            root: directory("fish")?,
        };
        std::fs::write(integration.root.join(FISH_HOOK), terminal_core::fish::HOOK)
            .map_err(|error| error.to_string())?;
        Ok(integration)
    }

    /// The arguments that start fish, as a login shell, with these hooks.
    pub fn fish_start(&self) -> Vec<String> {
        let hook = self.root.join(FISH_HOOK).display().to_string();
        vec![
            "-l".into(),
            "--init-command".into(),
            format!(
                "source '{}'",
                hook.replace('\\', "\\\\").replace('\'', "\\'")
            ),
        ]
    }

    /// Startup files for zsh, through `ZDOTDIR`.
    pub fn create() -> Result<Self, String> {
        let integration = Self {
            root: directory("zsh")?,
        };
        let env = r#"
typeset -g _openagents_hook_dir=$ZDOTDIR
typeset -g _openagents_user_dir=${OPENAGENTS_USER_ZDOTDIR:-$HOME}
ZDOTDIR=$_openagents_user_dir
[[ -r $ZDOTDIR/.zshenv ]] && source "$ZDOTDIR/.zshenv"
_openagents_user_dir=${ZDOTDIR:-$HOME}
ZDOTDIR=$_openagents_hook_dir
"#;
        let profile = r#"
ZDOTDIR=$_openagents_user_dir
[[ -r $ZDOTDIR/.zprofile ]] && source "$ZDOTDIR/.zprofile"
_openagents_user_dir=${ZDOTDIR:-$HOME}
ZDOTDIR=$_openagents_hook_dir
"#;
        let rc = format!(
            "{}\n{}",
            r#"
ZDOTDIR=$_openagents_user_dir
[[ -r $ZDOTDIR/.zshrc ]] && source "$ZDOTDIR/.zshrc"
"#,
            terminal_core::zsh::HOOK
        );
        for (name, text) in [
            (".zshenv", env),
            (".zprofile", profile),
            (".zshrc", rc.as_str()),
        ] {
            std::fs::write(integration.root.join(name), text).map_err(|error| error.to_string())?;
        }
        Ok(integration)
    }

    pub fn environment(&self, home: &Path) -> [(String, String); 2] {
        [
            ("OPENAGENTS_USER_ZDOTDIR".into(), home.display().to_string()),
            ("ZDOTDIR".into(), self.root.display().to_string()),
        ]
    }
}

impl Drop for Integration {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}
