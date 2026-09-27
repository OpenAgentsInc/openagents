//! A local loopback port forwarded to a remote host's loopback port.
//!
//! A tunnel is only a route. Closing it, dropping it, or losing it to a
//! network failure ends this `ssh -N` process and nothing on the remote
//! machine: the host keeps running until someone explicitly removes it.

use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use supervise::blocking;

use crate::askpass::Askpass;
use crate::error::Error;
use crate::ssh::{Forwarding, Ssh};

/// How long a closing tunnel has to exit on `SIGTERM`.
const GRACE: Duration = Duration::from_millis(250);

/// A running `ssh -N -L` process that forwards one loopback port.
pub struct Tunnel {
    child: Child,
    local_port: u16,
    remote_port: u16,
    askpass: Option<Askpass>,
}

impl std::fmt::Debug for Tunnel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Tunnel")
            .field("pid", &self.child.id())
            .field("local_port", &self.local_port)
            .field("remote_port", &self.remote_port)
            .finish()
    }
}

/// Reserves a free local loopback port and returns it.
///
/// The operating system picks the port; the listener is released
/// immediately before `ssh` binds it. Another local program could take the
/// port in that window, in which case `ssh` exits on the forwarding failure
/// and [`Tunnel::ready`] reports it.
fn reserve() -> Result<u16, Error> {
    let listener = TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0)))?;
    Ok(listener.local_addr()?.port())
}

impl Tunnel {
    pub(crate) fn open(ssh: &Ssh, remote_port: u16) -> Result<Self, Error> {
        let local_port = reserve()?;
        let (mut command, askpass): (Command, Option<Askpass>) = ssh.command(
            &Forwarding::Local {
                local: local_port,
                remote: remote_port,
            },
            None,
        )?;
        command
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        blocking::own_group(&mut command);
        let child = command
            .spawn()
            .map_err(|error| Error::Spawn(format!("{}: {error}", ssh.program.display())))?;
        Ok(Tunnel {
            child,
            local_port,
            remote_port,
            askpass,
        })
    }

    /// The local loopback port that reaches the host.
    #[must_use]
    pub fn local_port(&self) -> u16 {
        self.local_port
    }

    /// The host's loopback port on the remote machine.
    #[must_use]
    pub fn remote_port(&self) -> u16 {
        self.remote_port
    }

    /// The process identifier of the local `ssh` process.
    #[must_use]
    pub fn pid(&self) -> u32 {
        self.child.id()
    }

    /// Whether the local `ssh` process is still running. A dead tunnel says
    /// nothing about the host, which keeps running.
    pub fn alive(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(None))
    }

    /// Waits until the local port accepts a connection, at most `wait`.
    ///
    /// On success, the askpass helper is removed: authentication is over.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Ssh`] when the `ssh` process exits first, and
    /// [`Error::TimedOut`] when the port does not open in time.
    pub fn ready(&mut self, wait: Duration) -> Result<(), Error> {
        let deadline = Instant::now() + wait;
        let address = SocketAddr::from((Ipv4Addr::LOCALHOST, self.local_port));
        loop {
            if let Ok(Some(status)) = self.child.try_wait() {
                return Err(Error::Ssh {
                    code: status.code(),
                    detail: "the tunnel exited before its port opened".into(),
                });
            }
            if TcpStream::connect_timeout(&address, Duration::from_millis(200)).is_ok() {
                self.askpass = None;
                return Ok(());
            }
            if Instant::now() >= deadline {
                return Err(Error::TimedOut("the tunnel"));
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    /// Ends the tunnel. The remote host keeps running.
    pub fn close(mut self) {
        self.end();
    }

    fn end(&mut self) {
        if matches!(self.child.try_wait(), Ok(Some(_))) {
            self.askpass = None;
            return;
        }
        let Ok(group) = i32::try_from(self.child.id()) else {
            let _ = self.child.kill();
            let _ = self.child.wait();
            return;
        };
        // SAFETY: `killpg` has no memory-safety preconditions. The group is
        // the tunnel's own, created by `own_group` at spawn.
        unsafe { libc::killpg(group, libc::SIGTERM) };
        let deadline = Instant::now() + GRACE;
        while Instant::now() < deadline {
            if matches!(self.child.try_wait(), Ok(Some(_)) | Err(_)) {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        // SAFETY: as above.
        unsafe { libc::killpg(group, libc::SIGKILL) };
        let _ = self.child.wait();
        self.askpass = None;
    }
}

impl Drop for Tunnel {
    fn drop(&mut self) {
        self.end();
    }
}
