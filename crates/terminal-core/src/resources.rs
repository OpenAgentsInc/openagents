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

/// The most product panes a mount keeps open.
pub const PRODUCTS_MAX: usize = 32;

/// A mount's product panes: the adapters and fallbacks it describes them
/// with, the panes open, and which has focus. Navigation, focus, and what a
/// pane shows live here; each pane's store, execution, and the admission of
/// its actions stay with the resource's owner, and opening a pane creates
/// nothing there.
#[derive(Debug)]
pub struct Products {
    pub panes: workbench::pane::Panes,
    pub open: Vec<workbench::pane::PaneDescriptor>,
    pub focus: Option<usize>,
    /// Lines scrolled in the focused read-only sheet.
    pub scroll: usize,
}

impl Default for Products {
    /// No adapter yet, and the fallbacks every mount declares: a thread
    /// reads in `openagents chat`, and a run in `coder task show`.
    fn default() -> Self {
        use workbench::pane::{PaneKind, Panes, View};
        let words = |words: &[&str]| words.iter().map(|word| (*word).to_owned()).collect();
        let panes = Panes::new()
            .fallback(
                PaneKind::Thread,
                View::Tty {
                    command: words(&["openagents", "chat", "read", "--thread", "{id}"]),
                },
            )
            .and_then(|panes| {
                panes.fallback(
                    PaneKind::Run,
                    View::Tty {
                        command: words(&["coder", "task", "show", "{id}"]),
                    },
                )
            })
            .unwrap_or_default();
        Products {
            panes,
            open: Vec::new(),
            focus: None,
            scroll: 0,
        }
    }
}

impl Products {
    /// Opens `subject` as a `pane`, or refreshes and focuses it when it is
    /// already open, and answers its descriptor.
    ///
    /// # Errors
    /// A subject that does not fit the kind, or too many open panes.
    pub fn open(
        &mut self,
        pane: workbench::pane::PaneKind,
        subject: &workbench::pane::Subject,
    ) -> Result<workbench::pane::PaneDescriptor, String> {
        let descriptor = self
            .panes
            .resolve(pane, subject)
            .map_err(|refusal| refusal.to_string())?;
        let index = match self
            .open
            .iter()
            .position(|open| open.pane == pane && same(&open.subject, subject))
        {
            Some(index) => {
                self.open[index] = descriptor.clone();
                index
            }
            None if self.open.len() >= PRODUCTS_MAX => {
                return Err(format!("at most {PRODUCTS_MAX} product panes are open"));
            }
            None => {
                self.open.push(descriptor.clone());
                self.open.len() - 1
            }
        };
        self.focus = Some(index);
        self.scroll = 0;
        Ok(descriptor)
    }

    /// Describes every open pane again, as its owner sees it now.
    pub fn refresh(&mut self) {
        for open in &mut self.open {
            if let Ok(descriptor) = self.panes.resolve(open.pane, &open.subject) {
                *open = descriptor;
            }
        }
    }

    /// Closes pane `index`; its resource is untouched.
    pub fn close(&mut self, index: usize) -> bool {
        if index >= self.open.len() {
            return false;
        }
        self.open.remove(index);
        self.focus = match self.focus {
            Some(focus) if focus == index => None,
            Some(focus) if focus > index => Some(focus - 1),
            other => other,
        };
        true
    }

    /// The open panes and the focus, as JSON for a mount's status.
    #[must_use]
    pub fn status(&self) -> serde_json::Value {
        serde_json::json!({
            "focus": self.focus,
            "panes": self.open,
        })
    }
}

/// Whether two subjects name the same resource or record, at any revision.
fn same(a: &workbench::pane::Subject, b: &workbench::pane::Subject) -> bool {
    use workbench::pane::Subject;
    match (a, b) {
        (Subject::Resource { resource: a }, Subject::Resource { resource: b }) => {
            a.same_resource(b)
        }
        (
            Subject::Record {
                host: ha, id: ia, ..
            },
            Subject::Record {
                host: hb, id: ib, ..
            },
        ) => ha == hb && ia == ib,
        _ => false,
    }
}
