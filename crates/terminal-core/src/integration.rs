//! Shell hooks for a host's shells: temporary startup files that add the
//! OSC 133, OSC 7, and OSC 777 marks without changing the user's dotfiles.
//!
//! zsh reads them through `ZDOTDIR`, bash through `--rcfile`, and fish
//! through `--init-command`. PowerShell reads a `-File` script after its profiles.
//! Scratch PowerShell uses `-NoProfile` and only its selected home's `profile.ps1`.
//! The user's dotfiles remain unchanged.
//! The files live in a private directory that is removed when the
//! [`Hooks`] value drops, so a host keeps it for as long as it starts
//! shells.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

/// The bash startup file, read through `--rcfile` ([`crate::bash`]).
pub const BASH_RC: &str = "bashrc";
/// The PowerShell startup script, loaded through `-File`.
pub const POWERSHELL_HOOK: &str = "hook.ps1";
/// The fish hooks, sourced through `--init-command` ([`crate::fish`]).
pub const FISH_HOOK: &str = "hook.fish";

/// One host's startup files for one shell.
#[derive(Debug)]
pub struct Hooks {
    root: PathBuf,
}

/// How to start a shell with its hooks: arguments in place of the shell's
/// own, and variables to add.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Start {
    pub args: Vec<String>,
    pub env: Vec<(String, String)>,
}

impl Hooks {
    /// Hooks for `shell` (zsh, bash, or fish, by file name) started as a
    /// login shell, with `home` as the user's own startup directory; `None`
    /// for another shell, or when the files cannot be written.
    #[must_use]
    pub fn for_shell(shell: &Path, home: &Path) -> Option<(Hooks, Start)> {
        match shell.file_name()?.to_str()?.to_ascii_lowercase().as_str() {
            "zsh" => {
                let hooks = Hooks::zsh().ok()?;
                let start = Start {
                    args: vec!["-l".into()],
                    env: hooks.zsh_environment(home).to_vec(),
                };
                Some((hooks, start))
            }
            "bash" => {
                let hooks = Hooks::bash().ok()?;
                // `--rcfile` holds for an interactive shell that is not a
                // login shell, so the file reads the login profile itself.
                let start = hooks.bash_start(true);
                Some((hooks, start))
            }
            "fish" => {
                let hooks = Hooks::fish().ok()?;
                let start = Start {
                    args: hooks.fish_start(),
                    env: Vec::new(),
                };
                Some((hooks, start))
            }
            "pwsh" | "pwsh.exe" | "powershell" | "powershell.exe" => {
                let hooks = Hooks::powershell().ok()?;
                let start = hooks.powershell_start();
                Some((hooks, start))
            }
            _ => None,
        }
    }

    /// A PowerShell script, loaded after the shell's ordinary profiles.
    pub fn powershell() -> Result<Self, String> {
        let hooks = Self {
            root: directory("powershell")?,
        };
        std::fs::write(hooks.root.join(POWERSHELL_HOOK), crate::powershell::HOOK)
            .map_err(|error| error.to_string())?;
        Ok(hooks)
    }
    /// Keep an interactive native shell; respect its configured execution policy.
    #[must_use]
    pub fn powershell_start(&self) -> Start {
        Start {
            args: vec![
                "-NoExit".into(),
                "-File".into(),
                self.root.join(POWERSHELL_HOOK).display().to_string(),
            ],
            env: vec![
                ("POWERSHELL_TELEMETRY_OPTOUT".into(), "1".into()),
                ("DOTNET_CLI_TELEMETRY_OPTOUT".into(), "1".into()),
            ],
        }
    }

    /// A scratch shell never resolves native user-profile folders outside its selected home.
    #[must_use]
    pub fn powershell_isolated_start(&self, home: &Path) -> Start {
        let mut start = self.powershell_start();
        start.args.insert(0, "-NoProfile".into());
        start.env.push((
            "OPENAGENTS_POWERSHELL_PROFILE".into(),
            home.join("profile.ps1").display().to_string(),
        ));
        start
    }

    /// Startup files for bash: one `--rcfile` that sources the user's files
    /// and adds the hooks.
    pub fn bash() -> Result<Self, String> {
        let hooks = Self {
            root: directory("bash")?,
        };
        std::fs::write(hooks.root.join(BASH_RC), crate::bash::rcfile())
            .map_err(|error| error.to_string())?;
        Ok(hooks)
    }

    /// The arguments and environment that start bash on these files, as a
    /// login shell when `login` is set.
    #[must_use]
    pub fn bash_start(&self, login: bool) -> Start {
        let args = vec![
            "--rcfile".into(),
            self.root.join(BASH_RC).display().to_string(),
        ];
        let env = if login {
            vec![("OPENAGENTS_BASH_LOGIN".into(), "1".into())]
        } else {
            Vec::new()
        };
        Start { args, env }
    }

    /// The fish hooks, sourced after the user's configuration.
    pub fn fish() -> Result<Self, String> {
        let hooks = Self {
            root: directory("fish")?,
        };
        std::fs::write(hooks.root.join(FISH_HOOK), crate::fish::HOOK)
            .map_err(|error| error.to_string())?;
        Ok(hooks)
    }

    /// The arguments that start fish, as a login shell, with these hooks.
    #[must_use]
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
    pub fn zsh() -> Result<Self, String> {
        let hooks = Self {
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
            crate::zsh::HOOK
        );
        for (name, text) in [
            (".zshenv", env),
            (".zprofile", profile),
            (".zshrc", rc.as_str()),
        ] {
            std::fs::write(hooks.root.join(name), text).map_err(|error| error.to_string())?;
        }
        Ok(hooks)
    }

    /// The variables that point zsh at these files, with `home` as the
    /// user's own `ZDOTDIR`.
    #[must_use]
    pub fn zsh_environment(&self, home: &Path) -> [(String, String); 2] {
        [
            ("OPENAGENTS_USER_ZDOTDIR".into(), home.display().to_string()),
            ("ZDOTDIR".into(), self.root.display().to_string()),
        ]
    }
}

impl Drop for Hooks {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

/// A new private directory for one host's startup files.
fn directory(shell: &str) -> Result<PathBuf, String> {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.subsec_nanos());
    let root = std::env::temp_dir().join(format!(
        "openagents-{shell}-{}-{}-{nanos}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_supported_shell_gets_files_that_go_with_its_hooks() {
        let home = Path::new("/nonexistent-home");
        let (zsh, start) = Hooks::for_shell(Path::new("/bin/zsh"), home).unwrap();
        assert!(zsh.root.join(".zshrc").is_file());
        assert!(start.env.iter().any(|(name, _)| name == "ZDOTDIR"));
        let (bash, start) = Hooks::for_shell(Path::new("/usr/bin/bash"), home).unwrap();
        assert!(bash.root.join(BASH_RC).is_file());
        assert_eq!(start.args[0], "--rcfile");
        let (fish, start) = Hooks::for_shell(Path::new("/opt/fish"), home).unwrap();
        assert!(fish.root.join(FISH_HOOK).is_file());
        assert!(start.args.contains(&"--init-command".to_owned()));
        assert!(Hooks::for_shell(Path::new("/bin/sh"), home).is_none());
        let (powershell, start) =
            Hooks::for_shell(Path::new("/isolated/PowerShell.EXE"), home).unwrap();
        assert!(powershell.root.join(POWERSHELL_HOOK).is_file());
        assert_eq!(
            start.args,
            vec![
                "-NoExit".to_owned(),
                "-File".to_owned(),
                powershell.root.join(POWERSHELL_HOOK).display().to_string()
            ]
        );
        assert!(
            !start
                .args
                .iter()
                .any(|a| a == "-ExecutionPolicy" || a == "-NoProfile")
        );
        let isolated = powershell.powershell_isolated_start(Path::new("/scratch home/ü"));
        assert_eq!(isolated.args[0], "-NoProfile");
        assert!(
            isolated
                .env
                .iter()
                .any(|(name, value)| name == "OPENAGENTS_POWERSHELL_PROFILE"
                    && value.ends_with("profile.ps1"))
        );
        // The files go when the hooks drop.
        let root = zsh.root.clone();
        drop(zsh);
        assert!(!root.exists());
    }
}
