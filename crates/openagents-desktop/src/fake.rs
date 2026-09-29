//! An in-process host that answers the control operations, for tests and
//! for `openagents-desktop --fake-host`, which shows the window's screens
//! without a running host.
//!
//! It keeps NIP-HOST's rules that the window relies on: a code redeems
//! once, a cancelled or expired code redeems never, and a redemption adds a
//! device with `observe` and `operate`, plus `terminal` only when the code
//! was made with it. Its codes follow the `openagents-connect:` layout with
//! random keys, so a QR drawn from one has the real size.

use crate::control::{
    Autostart, ControlError, ControlResult, Device, HostControl, Invite, Project, Status,
};
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, MutexGuard};

/// How long a host invitation lives, in seconds (NIP-HOST).
pub const LIFETIME: u64 = 300;

/// One invitation the fake host issued.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Issued {
    pub invitation: String,
    pub terminal: bool,
    pub issued_at: u64,
    pub cancelled_at: Option<u64>,
    pub redeemed_by: Option<String>,
}

impl Issued {
    /// Whether a phone could still redeem it at `now`.
    pub fn open(&self, now: u64) -> bool {
        self.cancelled_at.is_none() && self.redeemed_by.is_none() && now < self.issued_at + LIFETIME
    }
}

#[derive(Debug, Default)]
struct State {
    now: u64,
    online: bool,
    down: bool,
    label: String,
    issued: Vec<Issued>,
    devices: BTreeMap<String, Device>,
    projects: Vec<Project>,
    autostart: Option<Autostart>,
    counter: u64,
}

/// The fake host. Clones share one state, so a test can hold one and give
/// another to the model.
#[derive(Clone, Debug)]
pub struct FakeHost(Arc<Mutex<State>>);

impl Default for FakeHost {
    fn default() -> Self {
        Self::new("Studio Mac", 1_790_000_000)
    }
}

fn random32(counter: u64) -> [u8; 32] {
    use std::collections::hash_map::RandomState;
    use std::hash::{BuildHasher, Hasher};
    let mut out = [0u8; 32];
    for (index, chunk) in out.chunks_mut(8).enumerate() {
        let mut hasher = RandomState::new().build_hasher();
        hasher.write_u64(counter);
        hasher.write_usize(index);
        chunk.copy_from_slice(&hasher.finish().to_be_bytes());
    }
    out
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

impl FakeHost {
    /// A host named `label` whose clock reads `now` (Unix seconds).
    pub fn new(label: &str, now: u64) -> FakeHost {
        FakeHost(Arc::new(Mutex::new(State {
            now,
            online: true,
            label: label.into(),
            ..State::default()
        })))
    }

    fn state(&self) -> MutexGuard<'_, State> {
        self.0.lock().unwrap_or_else(|poison| poison.into_inner())
    }

    /// Sets the host's clock.
    pub fn set_now(&self, now: u64) {
        self.state().now = now;
    }

    /// Stops answering, as a host that is not running.
    pub fn set_down(&self, down: bool) {
        self.state().down = down;
    }

    /// Every invitation issued so far.
    pub fn issued(&self) -> Vec<Issued> {
        self.state().issued.clone()
    }

    /// The invitations a phone could redeem now.
    pub fn open(&self) -> Vec<Issued> {
        let state = self.state();
        state
            .issued
            .iter()
            .filter(|issued| issued.open(state.now))
            .cloned()
            .collect()
    }

    /// A phone named `label` redeems `invitation`: `forbidden` when it was
    /// used, `revoked` when cancelled, `expired` when past its life.
    pub fn redeem(&self, invitation: &str, label: &str) -> ControlResult<Device> {
        let mut state = self.state();
        let now = state.now;
        state.counter += 1;
        let counter = state.counter;
        let refuse = |code: &str| ControlError::Refused {
            code: code.into(),
            message: "the code does not admit this phone".into(),
        };
        let issued = state
            .issued
            .iter_mut()
            .find(|issued| issued.invitation == invitation)
            .ok_or_else(|| refuse("forbidden"))?;
        if issued.redeemed_by.is_some() {
            return Err(refuse("forbidden"));
        }
        if issued.cancelled_at.is_some() {
            return Err(refuse("revoked"));
        }
        if now >= issued.issued_at + LIFETIME {
            return Err(refuse("expired"));
        }
        let device = hex(&random32(counter));
        issued.redeemed_by = Some(device.clone());
        let mut rights = vec!["observe".to_string(), "operate".to_string()];
        if issued.terminal {
            rights.push("terminal".into());
        }
        let record = Device {
            device: device.clone(),
            label: label.into(),
            rights,
            grant: hex(&random32(counter + 1)),
            epoch: 0,
            enrolled_at: now,
            last_seen: Some(now),
            revoked: false,
        };
        state.devices.insert(device, record.clone());
        Ok(record)
    }

    /// An `openagents-connect:` code in the spec's layout.
    fn code(label: &str, counter: u64, invitation: &[u8; 32], issued_at: u64) -> String {
        let mut bytes = vec![1u8];
        bytes.extend(random32(counter + 10));
        bytes.extend(random32(counter + 11));
        bytes.extend(invitation);
        bytes.extend(random32(counter + 12));
        bytes.extend(issued_at.to_be_bytes());
        bytes.extend((issued_at + LIFETIME).to_be_bytes());
        let relay = b"https://iroh.openagents.com/";
        bytes.push(relay.len() as u8);
        bytes.extend(relay);
        bytes.push(2);
        bytes.extend([4, 192, 168, 1, 23, 0xb8, 0x2d]);
        bytes.extend([
            6, 0xfd, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x17, 0xb8, 0x2d,
        ]);
        let label = &label.as_bytes()[..label.len().min(48)];
        bytes.push(label.len() as u8);
        bytes.extend(label);
        format!("openagents-connect:{}", URL_SAFE_NO_PAD.encode(bytes))
    }
}

impl HostControl for FakeHost {
    fn status(&mut self) -> ControlResult<Status> {
        let state = self.state();
        if state.down {
            return Err(ControlError::Unreachable);
        }
        let now = state.now;
        Ok(Status {
            host: hex(&random32(0)),
            endpoint: hex(&random32(1)),
            label: state.label.clone(),
            online: state.online,
            relay: Some("https://iroh.openagents.com/".into()),
            devices: state.devices.values().filter(|d| !d.revoked).count() as u32,
            outstanding_invitations: state.issued.iter().filter(|i| i.open(now)).count() as u32,
            version: "fake".into(),
        })
    }

    fn invite(&mut self, terminal: bool) -> ControlResult<Invite> {
        let mut state = self.state();
        if state.down {
            return Err(ControlError::Unreachable);
        }
        state.counter += 1;
        let counter = state.counter;
        let invitation = random32(counter);
        let now = state.now;
        let code = Self::code(&state.label, counter, &invitation, now);
        let invitation = hex(&invitation);
        state.issued.push(Issued {
            invitation: invitation.clone(),
            terminal,
            issued_at: now,
            cancelled_at: None,
            redeemed_by: None,
        });
        let mut rights = vec!["observe".to_string(), "operate".to_string()];
        if terminal {
            rights.push("terminal".into());
        }
        Ok(Invite {
            invitation,
            code,
            expires_at: now + LIFETIME,
            rights,
        })
    }

    fn cancel(&mut self, invitation: &str) -> ControlResult<u32> {
        let mut state = self.state();
        if state.down {
            return Err(ControlError::Unreachable);
        }
        let now = state.now;
        let mut count = 0;
        for issued in &mut state.issued {
            if issued.invitation == invitation && issued.open(now) {
                issued.cancelled_at = Some(now);
                count += 1;
            }
        }
        Ok(count)
    }

    fn cancel_all(&mut self) -> ControlResult<u32> {
        let mut state = self.state();
        if state.down {
            return Err(ControlError::Unreachable);
        }
        let now = state.now;
        let mut count = 0;
        for issued in &mut state.issued {
            if issued.open(now) {
                issued.cancelled_at = Some(now);
                count += 1;
            }
        }
        Ok(count)
    }

    fn devices(&mut self) -> ControlResult<Vec<Device>> {
        let state = self.state();
        if state.down {
            return Err(ControlError::Unreachable);
        }
        Ok(state.devices.values().cloned().collect())
    }

    fn revoke(&mut self, device: &str) -> ControlResult<()> {
        let mut state = self.state();
        match state.devices.get_mut(device) {
            Some(record) => {
                record.revoked = true;
                record.epoch += 1;
                Ok(())
            }
            None => Err(ControlError::Refused {
                code: "not_found".into(),
                message: "no such device".into(),
            }),
        }
    }

    fn autostart(&mut self) -> ControlResult<Autostart> {
        Ok(self.state().autostart.clone().unwrap_or(Autostart {
            enabled: false,
            projects: vec![],
            max_running: 1,
        }))
    }

    fn set_autostart(&mut self, policy: Autostart) -> ControlResult<Autostart> {
        self.state().autostart = Some(policy.clone());
        Ok(policy)
    }

    fn projects(&mut self) -> ControlResult<Vec<Project>> {
        Ok(self.state().projects.clone())
    }

    fn add_project(&mut self, path: &str) -> ControlResult<Vec<Project>> {
        let mut state = self.state();
        let label = std::path::Path::new(path).file_name().map_or_else(
            || path.to_string(),
            |name| name.to_string_lossy().into_owned(),
        );
        if !state.projects.iter().any(|project| project.path == path) {
            state.projects.push(Project {
                label,
                path: path.into(),
            });
        }
        Ok(state.projects.clone())
    }
}
