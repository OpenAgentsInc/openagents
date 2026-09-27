//! The client operations the Computers screens call.
//!
//! [`ComputersService`] is the application-level seam between these screens
//! and a client that owns grants, connections, and relay traffic. The
//! live service in `crate::live` implements it over the resident host client
//! (`coder_host::client`), `coder-access`, `coder-reach`, and a `coder-link`
//! registry. [`Unavailable`] keeps every screen honest in a build without
//! that client, and [`crate::synthetic::Synthetic`] drives tests and
//! simulator checks.
//!
//! Every method is an effect request. The host still authorizes each one
//! against its own grant records; the screens' checks only avoid offering a
//! control that cannot work.
use crate::model::{CreatedInvitation, ListingChange, LocalHost, ServiceState, Snapshot};
use coder_access::{Code, Error, Rights};

pub type Result<T> = std::result::Result<T, Error>;

/// Client operations behind the Computers screens. Calls block until the
/// operation finishes or fails; a caller that must not block runs them on a
/// worker, as the mobile bridge does.
pub trait ComputersService {
    /// Read the current projection input.
    fn snapshot(&mut self) -> Result<Snapshot>;
    /// Switch a host on or off. Switching off keeps its grant and label.
    fn set_enabled(&mut self, host: &str, enabled: bool) -> Result<()>;
    /// Ask the host's supervisor to try now instead of waiting.
    fn retry_now(&mut self, host: &str) -> Result<()>;
    /// Stop connecting and drop the host from this device's list. The host
    /// keeps this device's grant until someone revokes it.
    fn forget(&mut self, host: &str) -> Result<()>;
    /// Redeem a scanned or pasted `coder-host:` invitation. Returns the host
    /// key. The service parses, checks expiry, and verifies the grant.
    fn redeem_invitation(&mut self, invitation: &str) -> Result<String>;
    /// Approve a headless host's enrollment request with the code shown on
    /// that host, admitting this device with `rights`.
    fn approve_enrollment(
        &mut self,
        host: &str,
        enrollment: &str,
        code: &str,
        rights: &Rights,
        grant_expires_at: u64,
    ) -> Result<()>;
    fn deny_enrollment(&mut self, host: &str, enrollment: &str) -> Result<()>;
    /// Start an SSH setup: install or reuse the host release on
    /// `destination`, start or adopt its host, and redeem its invitation.
    /// Desktop and terminal only. It may return before the setup finishes;
    /// [`Snapshot::ssh`] reports progress, prompts, and the result.
    fn connect_ssh(&mut self, destination: &str) -> Result<()>;
    /// Record that this machine runs no local host.
    fn run_without_local_host(&mut self) -> Result<()>;
    /// Fetch the host's device list (`device.list`).
    fn refresh_devices(&mut self, host: &str) -> Result<()>;
    /// Create a single-use invitation (`invite.create`).
    fn create_invitation(
        &mut self,
        host: &str,
        rights: &Rights,
        grant_expires_at: u64,
    ) -> Result<CreatedInvitation>;
    /// Cancel an unredeemed invitation (`invite.cancel`).
    fn cancel_invitation(&mut self, host: &str, invitation: &str) -> Result<()>;
    /// Revoke a device (`device.revoke`).
    fn revoke(&mut self, host: &str, device: &str) -> Result<()>;
    /// Record that first run finished.
    fn complete_first_run(&mut self) -> Result<()>;
    /// The application became active (`true`) or moved to the background
    /// (`false`). Each host supervisor probes its connection after a short
    /// absence and replaces it after a long one.
    fn application(&mut self, active: bool) -> Result<()> {
        let _ = active;
        Ok(())
    }
    /// Answer the SSH prompt `id` that [`Snapshot::ssh`] shows, or refuse
    /// it with `None`. The answer is a password or passphrase: never log it.
    fn answer_ssh_prompt(&mut self, id: u64, answer: Option<&str>) -> Result<()> {
        let _ = (id, answer);
        Err(Error::new(Code::Unavailable, "no SSH setup is waiting"))
    }
    /// Hold the owner key on this device so it can read and update the owner
    /// directory. `secret` is a hex or `nsec` secret key; the service accepts
    /// it only when its public key is the owner a held grant names.
    fn import_owner_key(&mut self, secret: &str) -> Result<()> {
        let _ = secret;
        Err(Error::new(
            Code::Unavailable,
            "this client can't hold an owner key",
        ))
    }
    /// Publish the next directory revision with `host` listed under `label`.
    /// An owner action.
    fn list_in_directory(&mut self, host: &str, label: &str) -> Result<()> {
        let _ = (host, label);
        Err(Error::new(
            Code::Unavailable,
            "this client can't change the directory",
        ))
    }
    /// Publish the next directory revision with `host`'s entry changed. An
    /// owner action. `revision` is the revision the screen showed: the
    /// service refuses the edit as stale when the directory it holds has
    /// another revision.
    fn edit_listing(&mut self, host: &str, revision: u64, change: &ListingChange) -> Result<()> {
        let _ = (host, revision, change);
        Err(Error::new(
            Code::Unavailable,
            "this client can't change the directory",
        ))
    }
    /// Publish the next directory revision without `host`. An owner action,
    /// bound to `revision` as [`ComputersService::edit_listing`] is. A grant
    /// this device holds keeps the host reachable; it leaves placement.
    fn remove_from_directory(&mut self, host: &str, revision: u64) -> Result<()> {
        let _ = (host, revision);
        Err(Error::new(
            Code::Unavailable,
            "this client can't change the directory",
        ))
    }
    /// End a directory conflict at `revision` by publishing the version this
    /// device last trusted above it. An owner action.
    fn keep_directory(&mut self, revision: u64) -> Result<()> {
        let _ = revision;
        Err(Error::new(
            Code::Unavailable,
            "this client can't change the directory",
        ))
    }
    /// Start `coder-ssh`'s explicit remove on the destination `host` was set
    /// up through: it stops a host the setup started and detaches from one
    /// that was already running. When it finishes, the service forgets the
    /// computer. Desktop and terminal only. It may return before the remove
    /// finishes; [`Snapshot::ssh`] reports prompts and the outcome.
    fn remove_ssh(&mut self, host: &str) -> Result<()> {
        let _ = host;
        Err(Error::new(
            Code::Unavailable,
            "this client does not reach hosts over SSH",
        ))
    }
}

/// A service for a build with no host client. It reports an empty list and
/// refuses every effect as `unavailable` with a user-facing reason.
pub struct Unavailable {
    device: String,
    reason: String,
    local_host: LocalHost,
    now: fn() -> u64,
}

impl Unavailable {
    pub fn new(device: impl Into<String>, local_host: LocalHost, now: fn() -> u64) -> Self {
        Self {
            device: device.into(),
            reason: "This build can't reach computers yet. Chats pairing still works.".into(),
            local_host,
            now,
        }
    }

    fn refuse<T>(&self) -> Result<T> {
        Err(Error::new(Code::Unavailable, self.reason.clone()))
    }
}

impl ComputersService for Unavailable {
    fn snapshot(&mut self) -> Result<Snapshot> {
        Ok(Snapshot {
            now: (self.now)(),
            device: self.device.clone(),
            owner: false,
            service: ServiceState::Unavailable {
                reason: self.reason.clone(),
            },
            local_host: self.local_host.clone(),
            first_run_complete: false,
            hosts: Vec::new(),
            activity: Vec::new(),
            directory: crate::model::DirectoryState::NoOwnerKey,
            ssh_ready: false,
            ssh: None,
        })
    }
    fn set_enabled(&mut self, _: &str, _: bool) -> Result<()> {
        self.refuse()
    }
    fn retry_now(&mut self, _: &str) -> Result<()> {
        self.refuse()
    }
    fn forget(&mut self, _: &str) -> Result<()> {
        self.refuse()
    }
    fn redeem_invitation(&mut self, _: &str) -> Result<String> {
        self.refuse()
    }
    fn approve_enrollment(&mut self, _: &str, _: &str, _: &str, _: &Rights, _: u64) -> Result<()> {
        self.refuse()
    }
    fn deny_enrollment(&mut self, _: &str, _: &str) -> Result<()> {
        self.refuse()
    }
    fn connect_ssh(&mut self, _: &str) -> Result<()> {
        self.refuse()
    }
    fn run_without_local_host(&mut self) -> Result<()> {
        self.refuse()
    }
    fn refresh_devices(&mut self, _: &str) -> Result<()> {
        self.refuse()
    }
    fn create_invitation(&mut self, _: &str, _: &Rights, _: u64) -> Result<CreatedInvitation> {
        self.refuse()
    }
    fn cancel_invitation(&mut self, _: &str, _: &str) -> Result<()> {
        self.refuse()
    }
    fn revoke(&mut self, _: &str, _: &str) -> Result<()> {
        self.refuse()
    }
    fn complete_first_run(&mut self) -> Result<()> {
        self.refuse()
    }
}
