//! Workspace-scoped, identity-bound computer operations over NIP-HOST.

use crate::{Code, Error, Result, Right, Rights};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

pub const FEATURE: &str = "openagents.workspace-scope.v1";
pub const MAX_OUTPUT: usize = 64 * 1024;
pub const MAX_READ: usize = 8 * 1024;
pub const MAX_TIMEOUT: u64 = 300;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Root {
    pub workspace: String,
    pub path: String,
    pub identity: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Scope {
    pub account: String,
    pub roots: Vec<Root>,
}

impl Scope {
    pub fn validate(&self, rights: &Rights) -> Result<()> {
        text(&self.account, 128)?;
        if self.roots.is_empty()
            || self.roots.len() > 16
            || rights
                .iter()
                .any(|r| !matches!(r, Right::Observe | Right::Operate | Right::Review))
        {
            return Err(Error::new(
                Code::Malformed,
                "The workspace permissions are invalid.",
            ));
        }
        let mut labels = std::collections::BTreeSet::new();
        for root in &self.roots {
            text(&root.workspace, 128)?;
            text(&root.path, crate::computer::MAX_PATH)?;
            coder_connect::protocol::identity(&root.identity).map_err(Error::from)?;
            if !root.path.starts_with('/') || root.path == "/" || !labels.insert(&root.workspace) {
                return Err(Error::new(
                    Code::Malformed,
                    "The workspace folder is invalid.",
                ));
            }
        }
        Ok(())
    }

    /// Construct the scope from roots selected by the local host operator.
    #[cfg(all(feature = "host", unix))]
    pub fn local(account: &str, roots: &[(String, std::path::PathBuf)]) -> Result<Self> {
        let roots = roots
            .iter()
            .map(|(workspace, path)| {
                let path = path.canonicalize().map_err(|_| {
                    Error::new(Code::Unavailable, "The workspace folder cannot be opened.")
                })?;
                Ok(Root {
                    workspace: workspace.clone(),
                    identity: root_identity(&path)?,
                    path: path
                        .to_str()
                        .ok_or_else(|| {
                            Error::new(Code::Malformed, "The workspace folder name is invalid.")
                        })?
                        .into(),
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let scope = Self {
            account: account.into(),
            roots,
        };
        scope.validate(&Rights::new(vec![Right::Observe, Right::Operate])?)?;
        Ok(scope)
    }
}

#[cfg(unix)]
pub fn root_identity(path: &std::path::Path) -> Result<String> {
    use std::os::unix::fs::MetadataExt;
    let metadata = std::fs::symlink_metadata(path)
        .map_err(|_| Error::new(Code::Unavailable, "The workspace folder cannot be opened."))?;
    if !metadata.is_dir() {
        return Err(Error::new(Code::Forbidden, "The workspace folder changed."));
    }
    Ok(hash(
        &[metadata.dev().to_be_bytes(), metadata.ino().to_be_bytes()].concat(),
    ))
}

pub fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Call {
    pub account: String,
    pub action: Action,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Action {
    Attach {
        computer: String,
        workspace: String,
    },
    Observe {
        computer: String,
    },
    Detach {
        computer: String,
    },
    File {
        computer: String,
        file: crate::computer::Request,
    },
    Start {
        computer: String,
        command: Command,
    },
    Read {
        computer: String,
        command: String,
        stdout: u64,
        stderr: u64,
        max_bytes: usize,
    },
    Stop {
        computer: String,
        command: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Command {
    pub id: String,
    pub script: String,
    pub cwd: String,
    pub env: BTreeMap<String, String>,
    pub timeout_seconds: u64,
    pub digest: String,
    pub source_digest: String,
}

impl Command {
    pub fn content_hash(&self) -> String {
        hash(
            &serde_json::to_vec(&(
                &self.id,
                &self.script,
                &self.cwd,
                &self.env,
                self.timeout_seconds,
                &self.source_digest,
            ))
            .expect("command values serialize"),
        )
    }
    pub fn validate(&self) -> Result<()> {
        id(&self.id)?;
        id(&self.source_digest)?;
        if self.script.is_empty()
            || self.script.len() > 32 * 1024
            || self.script.contains('\0')
            || self.timeout_seconds == 0
            || self.timeout_seconds > MAX_TIMEOUT
            || self.digest != self.content_hash()
            || self.env.len() > 16
        {
            return Err(Error::new(
                Code::Malformed,
                "The command settings are invalid.",
            ));
        }
        relative(&self.cwd)?;
        for (name, value) in &self.env {
            if name.is_empty()
                || name.len() > 64
                || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
                || ["PATH", "HOME", "ENV", "BASH_ENV", "SSH_AUTH_SOCK"].contains(&name.as_str())
                || name.starts_with("LD_")
                || name.starts_with("DYLD_")
                || name.starts_with("OPENAGENTS")
                || value.len() > 4096
                || value.contains('\0')
            {
                return Err(Error::new(
                    Code::Forbidden,
                    "That environment setting is not allowed.",
                ));
            }
        }
        Ok(())
    }
}

impl Call {
    pub fn validate(&self) -> Result<()> {
        text(&self.account, 128)?;
        id(self.action.computer())?;
        match &self.action {
            Action::Attach { workspace, .. } => text(workspace, 128),
            Action::File { file, .. } => file.validate(),
            Action::Start { command, .. } => command.validate(),
            Action::Read {
                command,
                stdout,
                stderr,
                max_bytes,
                ..
            } => {
                id(command)?;
                if *stdout > MAX_OUTPUT as u64
                    || *stderr > MAX_OUTPUT as u64
                    || *max_bytes == 0
                    || *max_bytes > MAX_READ
                {
                    return Err(Error::new(Code::Bounds, "The output request is too large."));
                }
                Ok(())
            }
            Action::Stop { command, .. } => id(command),
            _ => Ok(()),
        }
    }
    pub fn reads_only(&self) -> bool {
        matches!(
            &self.action,
            Action::Observe { .. }
                | Action::Read { .. }
                | Action::File {
                    file: crate::computer::Request::Read { .. }
                        | crate::computer::Request::Stat { .. },
                    ..
                }
        )
    }
    pub fn required(&self) -> Right {
        if matches!(self.action, Action::Attach { .. } | Action::Detach { .. }) || self.reads_only()
        {
            Right::Observe
        } else {
            Right::Operate
        }
    }
}

impl Action {
    pub fn computer(&self) -> &str {
        match self {
            Self::Attach { computer, .. }
            | Self::Observe { computer }
            | Self::Detach { computer }
            | Self::File { computer, .. }
            | Self::Start { computer, .. }
            | Self::Read { computer, .. }
            | Self::Stop { computer, .. } => computer,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum Progress {
    Running,
    Exited { code: i64, timed_out: bool },
    Lost,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Answer {
    Computer {
        computer: String,
        attached: bool,
    },
    File {
        file: crate::computer::Answer,
    },
    Command {
        command: String,
        progress: Progress,
        digest: String,
        stdout: Vec<u8>,
        stderr: Vec<u8>,
    },
}

impl Answer {
    pub fn validate(&self) -> Result<()> {
        match self {
            Self::Computer { computer, .. } => id(computer),
            Self::File { file } => file.validate(),
            Self::Command {
                command,
                digest,
                stdout,
                stderr,
                ..
            } => {
                id(command)?;
                id(digest)?;
                if stdout.len() + stderr.len() > MAX_READ {
                    return Err(Error::new(Code::Bounds, "The command output is too large."));
                }
                Ok(())
            }
        }
    }
    pub fn answers(&self, call: &Call) -> bool {
        match (&call.action, self) {
            (
                Action::Attach { computer, .. }
                | Action::Observe { computer }
                | Action::Detach { computer },
                Self::Computer {
                    computer: answered, ..
                },
            ) => computer == answered,
            (Action::File { file, .. }, Self::File { file: answered }) => answered.answers(file),
            (
                Action::Start { command, .. },
                Self::Command {
                    command: answered,
                    digest,
                    ..
                },
            ) => command.id == *answered && command.source_digest == *digest,
            (
                Action::Read { command, .. } | Action::Stop { command, .. },
                Self::Command {
                    command: answered, ..
                },
            ) => command == answered,
            _ => false,
        }
    }
}

pub fn relative(path: &str) -> Result<()> {
    if path.len() > crate::computer::MAX_PATH
        || path.starts_with('/')
        || path.contains(['\\', '\0', ':'])
        || path.split('/').any(|p| p == "..")
    {
        return Err(Error::new(
            Code::Forbidden,
            "The path leaves the workspace.",
        ));
    }
    Ok(())
}
fn id(value: &str) -> Result<()> {
    coder_connect::protocol::identity(value).map_err(Error::from)
}
fn text(value: &str, max: usize) -> Result<()> {
    if value.is_empty() || value.len() > max || value.chars().any(char::is_control) {
        Err(Error::new(
            Code::Malformed,
            "The computer settings are invalid.",
        ))
    } else {
        Ok(())
    }
}
