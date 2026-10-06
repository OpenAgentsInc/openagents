//! A mount as the owner of its local panes, in the workbench contract
//! (`crates/workbench`, `docs/terminal/workbench-resources.md`).
//!
//! The standalone window and Verse's overlay both mount [`Application`],
//! so both name their panes and answer intents the same way. A local pane
//! lives only as long as its process: the mount's instance ID is also its
//! generation, so a reference kept from an earlier run is `lost`, and
//! nothing opens a new shell in its place.

use sha2::{Digest, Sha256};
use workbench::{
    Action, Capability, Directory, Host, Intent, Kind, Operation, Outcome, Owner, ResourceRef,
    State,
};

use crate::Application;
use crate::layout::PaneId;

/// A fresh instance ID: a digest of the process, the time, and a counter,
/// distinct for every mount this process starts.
pub(crate) fn instance() -> String {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let mut digest = Sha256::new();
    digest.update(std::process::id().to_le_bytes());
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    digest.update(now.as_nanos().to_le_bytes());
    digest.update(
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            .to_le_bytes(),
    );
    digest.update(format!("{:p}", &NEXT).as_bytes());
    digest
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

impl Application {
    fn host(&self) -> Host {
        Host::Local {
            instance: self.instance.clone(),
        }
    }

    /// The workbench reference for pane `id`, while it exists.
    #[must_use]
    pub fn resource(&self, id: PaneId) -> Option<ResourceRef> {
        self.panes
            .contains_key(&id)
            .then(|| ResourceRef::terminal(self.host(), self.instance.clone(), id.to_string()))
    }
}

impl Owner for Application {
    fn directory(&self) -> Directory {
        Directory {
            v: workbench::DIRECTORY.into(),
            host: self.host(),
            generation: Some(self.instance.clone()),
            capabilities: vec![Capability {
                kind: Kind::Terminal,
                operations: vec![Operation::Open],
                intents: Vec::new(),
                fallback: None,
            }],
        }
    }

    /// Opens a local pane: shows the overlay with that pane focused. A
    /// pane that ended is `closed`; one from another run is `lost`. It
    /// never starts a program.
    fn resolve(&mut self, intent: &Intent) -> Outcome {
        let target = &intent.target;
        let state = if target.generation.as_deref() != Some(self.instance.as_str()) {
            State::Lost
        } else {
            match (target.kind, &intent.action, target.id.parse::<PaneId>()) {
                (Kind::Terminal, Action::Open, Ok(id)) if self.show_pane(id) => State::Opened,
                (Kind::Terminal, Action::Open, _) => State::Closed,
                _ => State::Unsupported,
            }
        };
        Outcome::new(intent, state)
    }
}
