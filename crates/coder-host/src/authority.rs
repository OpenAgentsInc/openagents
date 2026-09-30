//! The host's grant store, as the channel, terminal, and publication paths
//! see it.
//!
//! `coder-access` owns the grants. Its store takes a short exclusive lock per
//! operation, so this process serializes its own operations through one
//! mutex and retries briefly when another local process, such as
//! `coder host revoke`, holds the lock. Checks read a snapshot of the device
//! list that reloads whenever the store file changes. A local revocation
//! therefore reaches open channels and terminal attachments on their next
//! check. A snapshot that cannot be reloaded fails closed.

#[cfg(unix)]
use std::os::unix::fs::MetadataExt;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard, TryLockError};
use std::thread::ThreadId;
use std::time::{Duration, Instant, SystemTime};

use coder_access::host::{Dispatch, Host};
use coder_access::protocol::{DeviceEntry, DeviceState};
use coder_access::{Code, Right, Rights};
use coder_reach::channel::{GrantCheck, GrantRefusal};
use nostr::domain::Event;

/// How long an operation waits for another local process to release the
/// access store. That includes a child process this host, or anything else
/// in the same process, is starting: a child holds a copy of every open
/// descriptor until it runs its program, so a store lock taken just before
/// a spawn stays held in the child for a moment after this process closes
/// it.
const BUSY_WAIT: Duration = Duration::from_secs(2);

/// Where a device stands at this host now.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Standing {
    /// At least one current grant; the union of their rights.
    Active(Rights),
    /// Every grant the device held is revoked.
    Revoked,
    /// The device's grants expired or name an old epoch.
    Expired,
    /// The host never enrolled this key, or no longer retains its grant.
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Stamp {
    inode: u64,
    len: u64,
    modified: Option<SystemTime>,
}

#[derive(Default)]
struct Snapshot {
    stamp: Option<Stamp>,
    devices: Option<Vec<DeviceEntry>>,
}

/// The host's grant store and a reloading snapshot of its devices.
pub struct Authority {
    host: Host,
    state: PathBuf,
    serial: Mutex<()>,
    /// The thread holding `serial`, so a dispatch that asks for rights
    /// while its own operation holds the store is told apart from another
    /// thread's operation, which a reload waits for.
    holder: Mutex<Option<ThreadId>>,
    snapshot: Mutex<Snapshot>,
}

impl Authority {
    /// Wrap an initialized access store.
    ///
    /// # Errors
    /// Refuses a store that is missing or cannot be read.
    pub fn open(host: Host) -> crate::Result<Self> {
        let authority = Self {
            state: host.state_path(),
            host,
            serial: Mutex::new(()),
            holder: Mutex::new(None),
            snapshot: Mutex::new(Snapshot::default()),
        };
        authority.owner()?;
        if authority.devices().is_none() {
            return Err(crate::Error::Config(
                "the host access store cannot be read".into(),
            ));
        }
        Ok(authority)
    }

    /// The owner key the store was initialized with, waiting briefly for
    /// another holder of the store.
    ///
    /// # Errors
    /// Refuses a store that is missing or cannot be read.
    pub fn owner(&self) -> coder_access::Result<String> {
        let _serial = self.serialize();
        busy_retry(|| self.host.owner())
    }

    /// The host's signing key, waiting briefly for another holder of the
    /// store.
    ///
    /// # Errors
    /// Refuses a store that is missing or cannot be read.
    pub fn signing_key(&self) -> coder_access::Result<secp256k1::SecretKey> {
        let _serial = self.serialize();
        busy_retry(|| self.host.signing_key())
    }

    /// The underlying access store.
    #[must_use]
    pub fn host(&self) -> &Host {
        &self.host
    }

    /// Handle one signed NIP-HOST request with this process's other store
    /// operations serialized and a brief wait for other processes.
    ///
    /// # Errors
    /// An error means no signed reply exists, as in `Host::handle_current`.
    pub fn handle(
        &self,
        event: &Event,
        relay: &str,
        dispatch: &mut dyn Dispatch,
    ) -> coder_access::Result<Event> {
        // A dispatch asks for rights while this operation holds the store,
        // and then reads the snapshot as it stands. Bring it up to date first.
        let _ = self.devices();
        let _serial = self.serialize();
        busy_retry(|| self.host.handle_current(event, relay, dispatch))
    }

    /// Revoke a device locally, as the owner.
    ///
    /// # Errors
    /// Refuses a device without a retained grant.
    pub fn revoke(&self, device: &str) -> coder_access::Result<(u64, Vec<String>)> {
        let _serial = self.serialize();
        let now = coder_access::unix_time()?;
        busy_retry(|| self.host.revoke(device, now))
    }

    /// Renew `device`'s grant when it nears its end (see
    /// `coder_access::host::Host::renew`). Best effort: a busy or
    /// unreadable store renews nothing.
    pub fn renew(&self, device: &str, grant: &str, epoch: u64) -> Option<Event> {
        let _serial = self.serialize();
        coder_access::unix_time()
            .and_then(|now| busy_retry(|| self.host.renew(device, grant, epoch, now)))
            .ok()
            .flatten()
    }

    /// Admit one `enroll.redeem` that arrived off the relays, bound to its
    /// invitation's relay.
    ///
    /// # Errors
    /// An error means no signed reply exists.
    pub fn redeem(&self, event: &Event) -> coder_access::Result<Event> {
        let _serial = self.serialize();
        busy_retry(|| self.host.handle_redemption(event, coder_access::unix_time))
    }

    /// Run a local operator action on the store, with this process's other
    /// store operations serialized and a brief wait for other processes.
    ///
    /// # Errors
    /// The action's own refusal.
    pub fn local<T>(
        &self,
        mut action: impl FnMut(&Host, u64) -> coder_access::Result<T>,
    ) -> coder_access::Result<T> {
        let _serial = self.serialize();
        busy_retry(|| action(&self.host, coder_access::unix_time()?))
    }

    /// Record that a direct channel admitted `device` under `grant` now, so
    /// `device.list` reports when the host last saw it. Best effort: a busy
    /// or unreadable store records nothing and admits nothing.
    pub fn touch(&self, device: &str, grant: &str) {
        let _serial = self.serialize();
        let _ = coder_access::unix_time()
            .and_then(|now| busy_retry(|| self.host.touch(device, grant, now)));
    }

    /// The current device list, reloaded when the store changed. `None`
    /// means the snapshot is unusable and every check fails closed.
    pub fn devices(&self) -> Option<Vec<DeviceEntry>> {
        if let Some(devices) = self.cached() {
            return Some(devices);
        }
        // An operation on this thread may hold the store while it
        // dispatches, and the dispatch may ask for rights. Keep the last
        // snapshot until that operation finishes rather than wait on
        // ourselves. Another thread's operation is waited for like any
        // other: its snapshot would miss a grant it just wrote.
        let _serial = match self.serial.try_lock() {
            Ok(guard) => self.hold(guard),
            Err(TryLockError::Poisoned(poisoned)) => self.hold(poisoned.into_inner()),
            Err(TryLockError::WouldBlock) => {
                if *lock(&self.holder) == Some(std::thread::current().id()) {
                    return lock(&self.snapshot).devices.clone();
                }
                self.serialize()
            }
        };
        // Whoever held the store before may have reloaded already.
        if let Some(devices) = self.cached() {
            return Some(devices);
        }
        let stamp = self.stamp();
        let loaded = coder_access::unix_time()
            .and_then(|now| busy_retry(|| self.host.devices(now)))
            .ok();
        let mut snapshot = lock(&self.snapshot);
        snapshot.stamp = stamp;
        snapshot.devices.clone_from(&loaded);
        loaded
    }

    /// The snapshot's device list while the store file is unchanged.
    fn cached(&self) -> Option<Vec<DeviceEntry>> {
        let stamp = self.stamp()?;
        let snapshot = lock(&self.snapshot);
        if snapshot.stamp == Some(stamp) {
            snapshot.devices.clone()
        } else {
            None
        }
    }

    /// Check one grant at one epoch, the question a direct channel asks.
    ///
    /// # Errors
    /// Returns why the grant cannot open or keep a channel.
    pub fn check(
        &self,
        device: &str,
        grant: &str,
        epoch: u64,
        now: u64,
    ) -> Result<Rights, GrantRefusal> {
        let devices = self.devices().ok_or(GrantRefusal::Unknown)?;
        let entry = devices
            .iter()
            .find(|entry| entry.grant == grant && entry.device == device)
            .ok_or(GrantRefusal::Unknown)?;
        match entry.state {
            DeviceState::Revoked => Err(GrantRefusal::Revoked),
            _ if entry.epoch != epoch => Err(GrantRefusal::EpochMismatch),
            _ if entry.expires_at <= now => Err(GrantRefusal::Expired),
            // The store reports an old epoch as expired.
            DeviceState::Expired => Err(GrantRefusal::EpochMismatch),
            DeviceState::Active => Ok(entry.rights.clone()),
        }
    }

    /// Where a device stands, from every grant the host retains for it.
    pub fn standing(&self, device: &str, now: u64) -> Standing {
        let Some(devices) = self.devices() else {
            return Standing::Unknown;
        };
        let mine: Vec<_> = devices.iter().filter(|e| e.device == device).collect();
        let active: Vec<Right> = mine
            .iter()
            .filter(|e| e.state == DeviceState::Active && e.expires_at > now)
            .flat_map(|e| e.rights.iter())
            .collect();
        if let Ok(rights) = Rights::new(active) {
            return Standing::Active(rights);
        }
        if mine.is_empty() {
            Standing::Unknown
        } else if mine.iter().all(|e| e.state == DeviceState::Revoked) {
            Standing::Revoked
        } else {
            Standing::Expired
        }
    }

    /// Whether a device holds a right now.
    pub fn holds(&self, device: &str, right: Right, now: u64) -> bool {
        matches!(self.standing(device, now), Standing::Active(rights) if rights.contains(right))
    }

    /// Every device with a current grant, and with `right` when one is named.
    pub fn active_devices(&self, right: Option<Right>, now: u64) -> Vec<String> {
        let mut devices: Vec<String> = self
            .devices()
            .unwrap_or_default()
            .into_iter()
            .filter(|e| e.state == DeviceState::Active && e.expires_at > now)
            .filter(|e| right.is_none_or(|r| e.rights.contains(r)))
            .map(|e| e.device)
            .collect();
        devices.sort();
        devices.dedup();
        devices
    }

    /// Serialize this process's store operations, recording the holder.
    fn serialize(&self) -> Serial<'_> {
        self.hold(lock(&self.serial))
    }

    fn hold<'a>(&'a self, guard: MutexGuard<'a, ()>) -> Serial<'a> {
        *lock(&self.holder) = Some(std::thread::current().id());
        Serial {
            holder: &self.holder,
            _guard: guard,
        }
    }

    #[cfg(windows)]
    fn stamp(&self) -> Option<Stamp> {
        // The store replaces its file by a rename, which changes the file
        // index as it changes an inode.
        let (identity, metadata) = private_fs::identity_of(&self.state).ok()?;
        Some(Stamp {
            inode: identity.index,
            len: metadata.len(),
            modified: metadata.modified().ok(),
        })
    }

    #[cfg(unix)]
    fn stamp(&self) -> Option<Stamp> {
        let metadata = std::fs::metadata(&self.state).ok()?;
        Some(Stamp {
            inode: metadata.ino(),
            len: metadata.len(),
            modified: metadata.modified().ok(),
        })
    }
}

/// This process's hold on the access store; releasing it clears the holder
/// before the lock.
struct Serial<'a> {
    holder: &'a Mutex<Option<ThreadId>>,
    _guard: MutexGuard<'a, ()>,
}

impl Drop for Serial<'_> {
    fn drop(&mut self) {
        *lock(self.holder) = None;
    }
}

/// The authority seen through the channel's grant check and the terminal
/// host's rights check.
#[derive(Clone)]
pub struct Grants(pub Arc<Authority>);

impl GrantCheck for Grants {
    fn check(&self, device: &str, grant: &str, epoch: u64, now: u64) -> Result<(), GrantRefusal> {
        self.0.check(device, grant, epoch, now).map(|_| ())
    }
}

impl coder_pty::host::Rights for Grants {
    fn holds(&self, principal: &str, right: coder_pty::host::Right) -> bool {
        let right = match right {
            coder_pty::host::Right::Terminal => Right::Terminal,
            coder_pty::host::Right::Observe => Right::Observe,
        };
        crate::unix_time().is_ok_and(|now| self.0.holds(principal, right, now))
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Retry while another process holds the access store. Opening the store
/// fails before any effect, so a retry cannot repeat one.
///
/// # Errors
/// The operation's own error, or `Conflict` once the store stays busy past
/// the wait.
pub fn busy_retry<T>(
    mut operation: impl FnMut() -> coder_access::Result<T>,
) -> coder_access::Result<T> {
    let started = Instant::now();
    loop {
        match operation() {
            Err(error) if error.code == Code::Conflict && started.elapsed() < BUSY_WAIT => {
                std::thread::sleep(Duration::from_millis(15));
            }
            other => return other,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc;

    use coder_access::RelayPolicy;
    use coder_access::client::prepare_redeem;
    use coder_access::host::Unconnected;
    use coder_access::protocol::HostInvitation;
    use coder_reach::pubkey;
    use secp256k1::SecretKey;

    use super::*;

    const POLICY: RelayPolicy = RelayPolicy::LoopbackTest;
    const RELAY: &str = "ws://127.0.0.1:7777";

    fn key() -> SecretKey {
        SecretKey::new(&mut secp256k1::rand::rng())
    }

    /// Pair `device` on the store `host` names, as a redemption does.
    fn pair(host: &Host, device: &SecretKey, now: u64) {
        let rights = Rights::parse_list("observe").unwrap();
        let code = host.invite(RELAY, rights, now, now + 3600).unwrap().code;
        let invitation = HostInvitation::parse(&code, now, POLICY).unwrap();
        let pending = prepare_redeem(&invitation, device, now, POLICY).unwrap();
        host.handle(&pending.event, RELAY, now, &mut Unconnected)
            .unwrap();
    }

    /// A check on one thread while another thread's operation holds the
    /// store sees what that operation wrote, not the snapshot from before
    /// it (#9981: a phone that had just paired was refused its channel).
    #[test]
    fn a_check_waits_for_another_threads_operation_and_sees_its_write() {
        let temp = tempfile::tempdir().unwrap();
        let access = temp.path().join("access");
        Host::new(&access, POLICY).init(&pubkey(&key())).unwrap();
        let authority = Arc::new(Authority::open(Host::new(&access, POLICY)).unwrap());
        assert_eq!(authority.devices().unwrap().len(), 0);

        let device = key();
        let (holding, held) = mpsc::channel();
        let (checked, check) = mpsc::channel();
        let writer = {
            let authority = authority.clone();
            std::thread::spawn(move || {
                authority
                    .local(|host, now| {
                        pair(host, &device, now);
                        holding.send(()).unwrap();
                        // Hold the store until the check answers, or long
                        // enough that a check which waits has waited.
                        let _ = check.recv_timeout(Duration::from_secs(1));
                        Ok(())
                    })
                    .unwrap();
            })
        };
        held.recv().unwrap();
        let seen = authority.standing(&pubkey(&device), coder_access::unix_time().unwrap());
        let _ = checked.send(());
        writer.join().unwrap();
        assert!(matches!(seen, Standing::Active(_)), "{seen:?}");
    }

    /// A host starts while something else briefly holds its store: a child
    /// process spawned elsewhere in this process keeps a copy of the lock
    /// descriptor until it runs its program (#9991: a test host failed to
    /// start with `conflict` while another test ran `git`). Opening the
    /// authority and reading its keys wait for the holder like every other
    /// store operation.
    #[test]
    fn opening_waits_for_a_brief_holder_of_the_store() {
        let temp = tempfile::tempdir().unwrap();
        let access = temp.path().join("access");
        let owner = pubkey(&key());
        Host::new(&access, POLICY).init(&owner).unwrap();
        let held = std::fs::File::open(access.join("access.lock")).unwrap();
        held.lock().unwrap();
        let holder = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(200));
            drop(held);
        });
        let authority = Authority::open(Host::new(&access, POLICY)).unwrap();
        holder.join().unwrap();
        assert_eq!(authority.owner().unwrap(), owner);

        let held = std::fs::File::open(access.join("access.lock")).unwrap();
        held.lock().unwrap();
        let holder = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(200));
            drop(held);
        });
        let secret = authority.signing_key().unwrap();
        holder.join().unwrap();
        assert_eq!(secret, authority.host().signing_key().unwrap());
    }
}
