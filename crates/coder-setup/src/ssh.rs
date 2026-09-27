//! Run `coder link` on another machine through the system `ssh`.
//!
//! SSH only carries the setup: the remote host still establishes its own
//! owner from a public key and grants rights itself. Secrets never go in
//! the remote command line. An invitation travels on the SSH channel's
//! standard input or output, never in an argument, so neither machine's
//! process list or shell history holds it.

use std::io::Write;
use std::process::{Command, Stdio};

use crate::{Error, Result};

/// The installed `coder` on a machine `link-device.sh` set up.
pub const DEFAULT_REMOTE_CODER: &str = "~/.openagents/bin/coder";

/// A machine reachable with `ssh DESTINATION`.
#[derive(Clone, Debug)]
pub struct Remote {
    pub destination: String,
    /// The remote `coder`, which may start with `~/`.
    pub coder: String,
}

/// What a remote command printed.
#[derive(Debug)]
pub struct Output {
    pub stdout: String,
    pub success: bool,
}

impl Remote {
    /// # Errors
    /// Refuses a destination that `ssh` would read as an option.
    pub fn new(destination: &str, coder: Option<&str>) -> Result<Self> {
        if destination.is_empty() || destination.starts_with('-') {
            return Err(Error::new("--ssh takes a destination such as user@host"));
        }
        Ok(Self {
            destination: destination.to_owned(),
            coder: coder.unwrap_or(DEFAULT_REMOTE_CODER).to_owned(),
        })
    }

    /// The remote shell command for `coder link ARGS`.
    #[must_use]
    pub fn command(&self, args: &[String]) -> String {
        let mut words = vec![shell_path(&self.coder), quote("link")];
        words.extend(args.iter().map(|arg| quote(arg)));
        words.join(" ")
    }

    /// Run `coder link ARGS` there. `input` goes to its standard input;
    /// its standard error passes through to ours. With `tty`, `ssh -t`
    /// gives it a terminal, for QR codes and prompts.
    ///
    /// # Errors
    /// Reports an `ssh` that cannot start.
    pub fn link(&self, args: &[String], input: Option<&str>, tty: bool) -> Result<Output> {
        let mut command = Command::new("ssh");
        if tty {
            command.arg("-t");
        } else {
            command.args(["-o", "BatchMode=yes"]);
        }
        command
            .arg(&self.destination)
            .arg("--")
            .arg(self.command(args))
            .stdin(if input.is_some() {
                Stdio::piped()
            } else if tty {
                Stdio::inherit()
            } else {
                Stdio::null()
            })
            .stdout(if tty {
                Stdio::inherit()
            } else {
                Stdio::piped()
            })
            .stderr(Stdio::inherit());
        let mut child = command.spawn().map_err(|_| Error::new("cannot run ssh"))?;
        if let (Some(input), Some(mut stdin)) = (input, child.stdin.take()) {
            stdin
                .write_all(input.as_bytes())
                .and_then(|()| stdin.write_all(b"\n"))
                .map_err(|_| Error::new("cannot write to ssh"))?;
        }
        let output = child
            .wait_with_output()
            .map_err(|_| Error::new("ssh did not finish"))?;
        Ok(Output {
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            success: output.status.success(),
        })
    }
}

/// POSIX single-quoting.
#[must_use]
pub fn quote(word: &str) -> String {
    format!("'{}'", word.replace('\'', r"'\''"))
}

/// A path that may start with `~/`, quoted so the remote shell expands only
/// the home directory.
fn shell_path(path: &str) -> String {
    match path.strip_prefix("~/") {
        Some(rest) => format!("\"$HOME\"/{}", quote(rest)),
        None => quote(path),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remote_commands_quote_every_word_and_expand_only_home() {
        let remote = Remote::new("me@box", None).unwrap();
        let command = remote.command(&["setup".into(), "--label".into(), "it's; rm -rf /".into()]);
        assert_eq!(
            command,
            r#""$HOME"/'.openagents/bin/coder' 'link' 'setup' '--label' 'it'\''s; rm -rf /'"#
        );
        let absolute = Remote::new("box", Some("/opt/coder")).unwrap();
        assert!(absolute.command(&[]).starts_with("'/opt/coder' 'link'"));
        assert!(Remote::new("-oProxyCommand=x", None).is_err());
    }
}
