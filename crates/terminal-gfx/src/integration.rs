//! Temporary startup files inject hooks without changing the user's dotfiles.

use std::path::{Path, PathBuf};

pub struct Integration {
    root: PathBuf,
}

impl Integration {
    pub fn create() -> Result<Self, String> {
        let root = std::env::temp_dir().join(format!(
            "openagents-zsh-{}-{}",
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
        let integration = Self { root };
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
