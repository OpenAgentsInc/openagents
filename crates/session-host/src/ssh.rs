//! At-most-once SSH carriage of the resident protocol. SSH authenticates and
//! encrypts the connection; signed session requests still determine authority.
use crate::{
    Result,
    resident::{MAX_FRAME, Request, Response},
};
use std::{
    io::{Read, Write},
    net::Shutdown,
    os::{
        fd::OwnedFd,
        unix::{net::UnixStream, process::CommandExt},
    },
    path::Path,
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

/// An operator-selected SSH host, remote executable, and private resident socket.
#[derive(Clone, Debug)]
pub struct Endpoint {
    pub destination: String,
    pub binary: String,
    pub socket: String,
}
struct Running(Child);
impl Drop for Running {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
fn quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}
fn remaining(deadline: Instant) -> Result<Duration> {
    deadline
        .checked_duration_since(Instant::now())
        .filter(|d| !d.is_zero())
        .ok_or("unknown: SSH session response timed out; request was not retried".into())
}
impl Endpoint {
    fn check(&self) -> Result<()> {
        if self.destination.is_empty()
            || self.destination.starts_with('-')
            || self.destination.len() > 1024
            || self
                .destination
                .chars()
                .any(|c| !(c.is_ascii_alphanumeric() || "@._-:[]%".contains(c)))
            || [&self.binary, &self.socket]
                .iter()
                .any(|s| s.is_empty() || s.len() > 4096 || s.chars().any(char::is_control))
        {
            return Err("malformed: SSH session endpoint".into());
        }
        Ok(())
    }
    pub fn request(&self, request: &Request) -> Result<Response> {
        self.via(Path::new("ssh"), request, Duration::from_secs(310))
    }
    fn via(&self, program: &Path, request: &Request, timeout: Duration) -> Result<Response> {
        self.check()?;
        let mut bytes = serde_json::to_vec(request).map_err(|e| e.to_string())?;
        if bytes.len() >= MAX_FRAME {
            return Err("limit_exceeded: SSH session request".into());
        }
        bytes.push(b'\n');
        let (mut channel, remote) = UnixStream::pair().map_err(|e| e.to_string())?;
        let input: OwnedFd = remote.try_clone().map_err(|e| e.to_string())?.into();
        let output: OwnedFd = remote.into();
        let remote_command = format!(
            "{} request --socket {} --input -",
            quote(&self.binary),
            quote(&self.socket)
        );
        let mut command = Command::new(program);
        command.args([
            "-T",
            "-o",
            "BatchMode=yes",
            "-o",
            "ConnectionAttempts=1",
            "-o",
            "ConnectTimeout=10",
            "-o",
            "ForwardAgent=no",
            "-o",
            "ClearAllForwardings=yes",
            "-o",
            "ControlMaster=no",
            "-o",
            "ControlPath=none",
            "-o",
            "ServerAliveInterval=15",
            "-o",
            "ServerAliveCountMax=2",
            "--",
            &self.destination,
            &remote_command,
        ]);
        // Authentication may use the local SSH agent; it is never forwarded.
        let path = std::env::var_os("PATH");
        let agent = std::env::var_os("SSH_AUTH_SOCK");
        command.env_clear();
        if let Some(path) = path {
            command.env("PATH", path);
        }
        if let Some(agent) = agent {
            command.env("SSH_AUTH_SOCK", agent);
        }
        let started = Instant::now();
        let deadline = started
            .checked_add(timeout)
            .ok_or("limit_exceeded: SSH timeout")?;
        let mut child = Running(
            command
                .stdin(Stdio::from(input))
                .stdout(Stdio::from(output))
                .stderr(Stdio::null())
                .process_group(0)
                .spawn()
                .map_err(|_| "unavailable: SSH could not start")?,
        );
        // Drop the command's copies of the child descriptors so EOF can arrive.
        drop(command);
        let write_deadline = deadline.min(started + Duration::from_secs(5));
        let mut sent = 0;
        while sent < bytes.len() {
            channel
                .set_write_timeout(Some(remaining(write_deadline)?))
                .map_err(|e| e.to_string())?;
            let count = channel
                .write(&bytes[sent..])
                .map_err(|_| "unknown: SSH request delivery was not confirmed")?;
            if count == 0 {
                return Err("unknown: SSH disconnected during delivery".into());
            }
            sent += count;
        }
        channel
            .shutdown(Shutdown::Write)
            .map_err(|_| "unknown: SSH request delivery was not confirmed")?;
        let mut response = Vec::new();
        let mut chunk = [0; 65_536];
        loop {
            channel
                .set_read_timeout(Some(remaining(deadline)?))
                .map_err(|e| e.to_string())?;
            let count = channel
                .read(&mut chunk)
                .map_err(|_| "unknown: SSH response was not confirmed")?;
            if count == 0 {
                break;
            }
            if response.len() + count > MAX_FRAME {
                return Err("limit_exceeded: SSH session response".into());
            }
            response.extend_from_slice(&chunk[..count]);
        }
        let status = loop {
            remaining(deadline)?;
            if let Some(status) = child
                .0
                .try_wait()
                .map_err(|_| "unknown: SSH process status")?
            {
                break status;
            }
            std::thread::sleep(Duration::from_millis(5));
        };
        // The remote CLI returns 1 for an authenticated protocol refusal too.
        if !matches!(status.code(), Some(0 | 1)) {
            return Err("unknown: SSH session command failed".into());
        }
        let value = nostr::contracts::parse_strict_bounded(&response, MAX_FRAME)
            .map_err(|_| "malformed: SSH session response")?;
        let response: Response =
            serde_json::from_value(value).map_err(|_| "malformed: SSH session response")?;
        if response.id != request.id {
            return Err("identity_mismatch: SSH session response ID".into());
        }
        Ok(response)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resident::{Operation, Outcome};
    use serde_json::{Value, json};
    use std::os::unix::fs::PermissionsExt;
    fn request() -> Request {
        let event = serde_json::from_value(json!({"id":"00".repeat(32),"pubkey":"01".repeat(32),
            "sig":"00".repeat(64),"created_at":1,"kind":3188,"tags":[],"content":"private body"}))
        .unwrap();
        Request::new(Operation::Poll { event })
    }
    fn helper(root: &Path, body: &str) -> std::path::PathBuf {
        let path = root.join("ssh-helper");
        std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
        path
    }
    #[test]
    fn ssh_carries_private_bytes_on_stdin_and_keeps_protocol_refusals() {
        let root = tempfile::tempdir().unwrap();
        let request = request();
        let response = json!({"v":"openagents.session-host-transport.v1","id":request.id,
            "outcome":{"status":"error","code":"not_admitted"}});
        let response_path = root.path().join("response");
        std::fs::write(&response_path, response.to_string()).unwrap();
        let args = root.path().join("args");
        let input = root.path().join("input");
        let program = helper(
            root.path(),
            &format!(
                "printf '%s\n' \"$@\" > {}\ncat > {}\ncat {}\nexit 1",
                quote(args.to_str().unwrap()),
                quote(input.to_str().unwrap()),
                quote(response_path.to_str().unwrap())
            ),
        );
        let endpoint = Endpoint {
            destination: "worker@example.test".into(),
            binary: "/host's tools/session-host".into(),
            socket: "/state/a' $(printf unexpected) socket".into(),
        };
        let reply = endpoint
            .via(&program, &request, Duration::from_secs(5))
            .unwrap();
        assert!(matches!(reply.outcome,Outcome::Error { code } if code=="not_admitted"));
        let received: Value = serde_json::from_slice(&std::fs::read(input).unwrap()).unwrap();
        assert_eq!(received, serde_json::to_value(&request).unwrap());
        let args = std::fs::read_to_string(args).unwrap();
        assert!(!args.contains("private body"));
        assert!(!args.contains(&request.id));
        assert!(args.contains("ConnectionAttempts=1"));
        assert!(args.contains("ForwardAgent=no"));
        let command = args.lines().last().unwrap();
        // Exercise the actual POSIX parsing boundary without executing it.
        let parsed = Command::new("sh")
            .args(["-c", &format!("set -- {}; printf '%s\n' \"$@\"", command)])
            .output()
            .unwrap();
        let parsed = String::from_utf8(parsed.stdout).unwrap();
        assert_eq!(
            parsed.lines().collect::<Vec<_>>(),
            vec![
                endpoint.binary.as_str(),
                "request",
                "--socket",
                endpoint.socket.as_str(),
                "--input",
                "-"
            ]
        );
    }
    #[test]
    fn ssh_lost_delivery_times_out_once_and_refuses_changed_or_duplicate_responses() {
        let root = tempfile::tempdir().unwrap();
        let request = request();
        let endpoint = Endpoint {
            destination: "worker".into(),
            binary: "session-host".into(),
            socket: "/state/socket".into(),
        };
        let count = root.path().join("count");
        let program = helper(
            root.path(),
            &format!(
                "printf x >> {}\nwhile :; do :; done",
                quote(count.to_str().unwrap())
            ),
        );
        let started = Instant::now();
        assert!(
            endpoint
                .via(&program, &request, Duration::from_millis(100))
                .is_err()
        );
        assert!(started.elapsed() < Duration::from_secs(2));
        assert_eq!(std::fs::read(count).unwrap(), b"x");
        for bytes in [
            format!(
                "{{\"v\":\"openagents.session-host-transport.v1\",\"id\":\"wrong\",\"outcome\":{{\"status\":\"error\",\"code\":\"unavailable\"}}}}"
            ),
            format!("{{\"id\":\"{}\",\"id\":\"{}\"}}", request.id, request.id),
        ] {
            let path = root.path().join("response");
            std::fs::write(&path, bytes).unwrap();
            let program = helper(
                root.path(),
                &format!("cat > /dev/null\ncat {}", quote(path.to_str().unwrap())),
            );
            assert!(
                endpoint
                    .via(&program, &request, Duration::from_secs(5))
                    .is_err()
            );
        }
        for destination in [
            "-oProxyCommand=anything",
            "host;cmd",
            "user@host$()",
            "host with space",
        ] {
            let endpoint = Endpoint {
                destination: destination.into(),
                ..endpoint.clone()
            };
            assert!(endpoint.check().is_err());
        }
    }
}
