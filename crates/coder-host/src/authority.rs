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

use std::os::unix::fs::MetadataExt;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant, SystemTime};

use coder_access::host::{Dispatch, Host};
use coder_access::protocol::{DeviceEntry, DeviceState};
use coder_access::{Code, Right, Rights};
use coder_reach::channel::{GrantCheck, GrantRefusal};
use nostr::domain::Event;

/// How long an operation waits for another local process to release the
/// access store.
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
            snapshot: Mutex::new(Snapshot::default()),
        };
        authority.host.owner()?;
        if authority.devices().is_none() {
            return Err(crate::Error::Config(
                "the host access store cannot be read".into(),
            ));
        }
        Ok(authority)
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
        let _serial = lock(&self.serial);
        busy_retry(|| self.host.handle_current(event, relay, dispatch))
    }

    /// Revoke a device locally, as the owner.
    ///
    /// # Errors
    /// Refuses a device without a retained grant.
    pub fn revoke(&self, device: &str) -> coder_access::Result<(u64, Vec<String>)> {
        let _serial = lock(&self.serial);
        let now = coder_access::unix_time()?;
        busy_retry(|| self.host.revoke(device, now))
    }

    /// Record that a direct channel admitted `device` under `grant` now, so
    /// `device.list` reports when the host last saw it. Best effort: a busy
    /// or unreadable store records nothing and admits nothing.
    pub fn touch(&self, device: &str, grant: &str) {
        let _serial = lock(&self.serial);
        let _ = coder_access::unix_time()
            .and_then(|now| busy_retry(|| self.host.touch(device, grant, now)));
    }

    /// The current device list, reloaded when the store changed. `None`
    /// means the snapshot is unusable and every check fails closed.
    pub fn devices(&self) -> Option<Vec<DeviceEntry>> {
        let stamp = self.stamp();
        {
            let snapshot = lock(&self.snapshot);
            if stamp.is_some() && snapshot.stamp == stamp && snapshot.devices.is_some() {
                return snapshot.devices.clone();
            }
        }
        // This process may hold the store while an operation dispatches, and
        // the dispatch may ask for rights. Keep the last snapshot until the
        // operation finishes rather than wait on ourselves.
        let Ok(_serial) = self.serial.try_lock() else {
            return lock(&self.snapshot).devices.clone();
        };
        let loaded = coder_access::unix_time()
            .and_then(|now| busy_retry(|| self.host.devices(now)))
            .ok();
        let mut snapshot = lock(&self.snapshot);
        snapshot.stamp = stamp;
        snapshot.devices.clone_from(&loaded);
        loaded
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

    fn stamp(&self) -> Option<Stamp> {
        let metadata = std::fs::metadata(&self.state).ok()?;
        Some(Stamp {
            inode: metadata.ino(),
            len: metadata.len(),
            modified: metadata.modified().ok(),
        })
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
fn busy_retry<T>(
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
