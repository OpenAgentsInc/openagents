//! Product panes: what a workbench surface shows for a resource beside its
//! shells, and the adapters that describe each kind
//! (`openagents.workbench-pane.v1`).
//!
//! A pane descriptor is display state, not the record. Its subject names
//! the resource exactly as the surface asked for it, and only an adapter of
//! the resource's own domain describes it: a title, a bounded detail, the
//! actions the owner offers, and whether the pane is ready, missing, stale,
//! revoked, or unavailable. A surface without an adapter for a kind shows
//! the kind's declared fallback, a TTY view or a link, under a label that
//! says so. Stores, execution, and the admission of every action stay with
//! the owner; [`Panes::resolve`] creates nothing.
//!
//! The kinds are closed. Kinds that version-1 references name take a
//! [`ResourceRef`]; the others name the owner's own record. A new kind is
//! a new schema version.

use serde::{Deserialize, Serialize};

use crate::{
    Host, Kind, Reason, Refusal, ResourceRef, Revision, SUMMARY_MAX, TOKEN_MAX, link, slug, token,
    version,
};

/// `v` of a pane descriptor.
pub const PANE: &str = "openagents.workbench-pane.v1";
/// The longest pane title.
pub const TITLE_MAX: usize = 128;
/// The most actions a pane offers.
pub const ACTIONS_MAX: usize = 8;
/// The most words in a TTY fallback's command.
pub const COMMAND_MAX: usize = 16;

/// What a pane shows. Closed: a new kind is a new schema version.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PaneKind {
    /// A chat thread.
    Thread,
    /// An agent run or durable task.
    Run,
    /// An Agent Studio record.
    Studio,
    /// A file at an exact revision.
    File,
    /// A file's change against its exact revision.
    Diff,
    /// A retained artifact.
    Artifact,
    /// A rendered view of an artifact, such as a page or an image.
    Preview,
    /// A knowledge entry.
    Knowledge,
    /// Gym or check evidence.
    Evaluation,
    /// A background job.
    Background,
    /// An account's balance and usage, from its owner.
    Account,
    /// A receipt the owner keeps for spent work.
    Receipt,
}

impl PaneKind {
    /// The version-1 resource kind this pane shows, when references name
    /// it; `None` for kinds whose subject is the owner's own record.
    #[must_use]
    pub fn resource(self) -> Option<Kind> {
        match self {
            PaneKind::Thread => Some(Kind::Thread),
            PaneKind::Run => Some(Kind::Run),
            PaneKind::Studio => Some(Kind::Studio),
            PaneKind::File | PaneKind::Diff => Some(Kind::File),
            PaneKind::Artifact | PaneKind::Preview => Some(Kind::Artifact),
            PaneKind::Evaluation => Some(Kind::Evidence),
            PaneKind::Knowledge | PaneKind::Background | PaneKind::Account | PaneKind::Receipt => {
                None
            }
        }
    }

    /// The kind in words, for labels.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            PaneKind::Thread => "thread",
            PaneKind::Run => "run",
            PaneKind::Studio => "studio record",
            PaneKind::File => "file",
            PaneKind::Diff => "diff",
            PaneKind::Artifact => "artifact",
            PaneKind::Preview => "preview",
            PaneKind::Knowledge => "knowledge entry",
            PaneKind::Evaluation => "evaluation",
            PaneKind::Background => "background job",
            PaneKind::Account => "account",
            PaneKind::Receipt => "receipt",
        }
    }
}

/// The resource a pane shows.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Subject {
    /// A resource version-1 references name.
    Resource { resource: ResourceRef },
    /// An owner's own record, for kinds references do not name: its host,
    /// its ID, and its revision when the owner keeps one.
    Record {
        host: Host,
        id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        revision: Option<Revision>,
    },
}

impl Subject {
    /// Checks the subject's shape and that it fits a pane of `pane`.
    pub fn check(&self, pane: PaneKind) -> Result<(), Refusal> {
        match (self, pane.resource()) {
            (Subject::Resource { resource }, Some(kind)) => {
                resource.check()?;
                if resource.kind != kind {
                    return Err(Refusal::malformed(format!(
                        "a {} pane shows a {kind:?} resource",
                        pane.label()
                    )));
                }
                Ok(())
            }
            (Subject::Record { host, id, revision }, None) => {
                host.check()?;
                token(id, TOKEN_MAX, "record")?;
                if let Some(revision) = revision {
                    revision.check()?;
                }
                Ok(())
            }
            (Subject::Resource { .. }, None) => Err(Refusal::malformed(format!(
                "a {} pane names the owner's record",
                pane.label()
            ))),
            (Subject::Record { .. }, Some(_)) => Err(Refusal::malformed(format!(
                "a {} pane names a resource reference",
                pane.label()
            ))),
        }
    }

    /// The owner the subject belongs to.
    #[must_use]
    pub fn host(&self) -> &Host {
        match self {
            Subject::Resource { resource } => &resource.host,
            Subject::Record { host, .. } => host,
        }
    }

    /// The owner's ID for it.
    #[must_use]
    pub fn id(&self) -> &str {
        match self {
            Subject::Resource { resource } => &resource.id,
            Subject::Record { id, .. } => id,
        }
    }

    /// The revision the surface asked for.
    #[must_use]
    pub fn revision(&self) -> Option<&Revision> {
        match self {
            Subject::Resource { resource } => resource.revision.as_ref(),
            Subject::Record { revision, .. } => revision.as_ref(),
        }
    }
}

/// What a surface shows for a kind it has no adapter for.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum View {
    /// Run this command in a terminal pane: a program and its arguments,
    /// `{id}` replaced with the subject's ID.
    Tty { command: Vec<String> },
    /// Open this `https` address, `{id}` replaced with the subject's ID.
    Link { url: String },
    /// Only the label: nothing else can show the kind here.
    Label,
}

impl View {
    fn check(&self) -> Result<(), Refusal> {
        match self {
            View::Tty { command } => {
                if command.is_empty() || command.len() > COMMAND_MAX {
                    return Err(Refusal::malformed("a TTY view runs 1 to 16 words"));
                }
                if command.iter().any(|word| {
                    word.is_empty() || word.len() > 1024 || word.chars().any(char::is_control)
                }) {
                    return Err(Refusal::malformed("a TTY view's words are plain text"));
                }
                Ok(())
            }
            View::Link { url } => link(url),
            View::Label => Ok(()),
        }
    }

    fn for_subject(&self, id: &str) -> View {
        match self {
            View::Tty { command } => View::Tty {
                command: command
                    .iter()
                    .map(|word| word.replace("{id}", id))
                    .collect(),
            },
            View::Link { url } => View::Link {
                url: url.replace("{id}", id),
            },
            View::Label => View::Label,
        }
    }
}

/// Where a pane stands.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum PaneState {
    /// The owner describes the resource; its actions are offered.
    Ready,
    /// The owner keeps no such record. Nothing is created in its place.
    Missing,
    /// The surface asked for another revision; the owner names the current
    /// one, and the surface asks again only when the person chooses.
    Stale { current: Option<Revision> },
    /// This device no longer holds the right to it.
    Revoked,
    /// The owner cannot answer now.
    Unavailable,
    /// No adapter here: the kind's declared fallback.
    Fallback { view: View },
}

/// A pane, as a surface draws it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PaneDescriptor {
    pub v: String,
    pub pane: PaneKind,
    /// Exactly what the surface asked for.
    pub subject: Subject,
    pub title: String,
    /// Bounded plain text the owner writes.
    pub detail: String,
    /// The owner intents the pane offers, by name. Empty for a read-only
    /// pane and for every state but `ready`.
    pub actions: Vec<String>,
    pub state: PaneState,
}

impl PaneDescriptor {
    pub fn check(&self) -> Result<(), Refusal> {
        version(&self.v, PANE)?;
        self.subject.check(self.pane)?;
        plain(&self.title, TITLE_MAX, "title")?;
        if self.detail.len() > SUMMARY_MAX
            || self.detail.chars().any(|c| c.is_control() && c != '\n')
        {
            return Err(Refusal::malformed(
                "a detail is plain text up to 2048 bytes",
            ));
        }
        if self.actions.len() > ACTIONS_MAX {
            return Err(Refusal::limit("a pane offers at most 8 actions"));
        }
        for action in &self.actions {
            slug(action, 32, "action")?;
        }
        if self.state != PaneState::Ready && !self.actions.is_empty() {
            return Err(Refusal::malformed("only a ready pane offers actions"));
        }
        match &self.state {
            PaneState::Fallback { view } => view.check(),
            PaneState::Stale {
                current: Some(current),
            } => {
                current.check()?;
                if self.subject.revision() == Some(current) {
                    return Err(Refusal::malformed("a stale pane names the same revision"));
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }
}

/// What an adapter says about one subject.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Description {
    pub state: PaneState,
    pub title: String,
    pub detail: String,
    /// The owner intents it offers; [`Panes::resolve`] drops them for any
    /// state but ready.
    pub actions: Vec<String>,
}

impl Description {
    /// A description in `state` with a title and nothing else.
    #[must_use]
    pub fn only(state: PaneState, title: impl Into<String>) -> Self {
        Description {
            state,
            title: title.into(),
            detail: String::new(),
            actions: Vec::new(),
        }
    }
}

/// One domain's view of its kind. It reads its own store and describes;
/// it never creates, starts, or changes anything, and it is never asked
/// about another kind.
pub trait PaneAdapter: Send {
    fn kind(&self) -> PaneKind;
    fn describe(&self, subject: &Subject) -> Description;
}

/// A surface's adapters and declared fallbacks.
#[derive(Default)]
pub struct Panes {
    adapters: Vec<Box<dyn PaneAdapter>>,
    fallbacks: Vec<(PaneKind, View)>,
}

impl std::fmt::Debug for Panes {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let kinds: Vec<PaneKind> = self.adapters.iter().map(|a| a.kind()).collect();
        f.debug_struct("Panes")
            .field("adapters", &kinds)
            .field("fallbacks", &self.fallbacks)
            .finish()
    }
}

impl Panes {
    #[must_use]
    pub fn new() -> Self {
        Panes::default()
    }

    /// Adds an adapter; a later one for the same kind replaces it.
    #[must_use]
    pub fn adapter(mut self, adapter: Box<dyn PaneAdapter>) -> Self {
        self.adapters.retain(|known| known.kind() != adapter.kind());
        self.adapters.push(adapter);
        self
    }

    /// Declares what shows for `kind` when no adapter describes it.
    ///
    /// # Errors
    /// A malformed view.
    pub fn fallback(mut self, kind: PaneKind, view: View) -> Result<Self, Refusal> {
        view.check()?;
        self.fallbacks.retain(|(known, _)| *known != kind);
        self.fallbacks.push((kind, view));
        Ok(self)
    }

    /// The kinds an adapter describes here.
    #[must_use]
    pub fn kinds(&self) -> Vec<PaneKind> {
        self.adapters.iter().map(|adapter| adapter.kind()).collect()
    }

    /// The pane for `subject` shown as `pane`: its adapter's description,
    /// or the kind's labeled fallback. The subject is always the one asked
    /// for, a pane not ready offers no action, and an adapter's malformed
    /// answer shows as unavailable.
    ///
    /// # Errors
    /// A subject that does not fit the kind.
    pub fn resolve(&self, pane: PaneKind, subject: &Subject) -> Result<PaneDescriptor, Refusal> {
        subject.check(pane)?;
        let Some(adapter) = self.adapters.iter().find(|adapter| adapter.kind() == pane) else {
            let view = self
                .fallbacks
                .iter()
                .find(|(kind, _)| *kind == pane)
                .map_or(View::Label, |(_, view)| view.for_subject(subject.id()));
            return Ok(descriptor(
                pane,
                subject,
                Description::only(
                    PaneState::Fallback { view },
                    format!("No {} viewer here", pane.label()),
                ),
            ));
        };
        let mut description = adapter.describe(subject);
        if description.state != PaneState::Ready {
            description.actions.clear();
        }
        let described = descriptor(pane, subject, description);
        if described.check().is_ok() {
            return Ok(described);
        }
        Ok(descriptor(
            pane,
            subject,
            Description::only(
                PaneState::Unavailable,
                format!(
                    "The {} viewer answered with something it may not show",
                    pane.label()
                ),
            ),
        ))
    }
}

fn descriptor(pane: PaneKind, subject: &Subject, description: Description) -> PaneDescriptor {
    PaneDescriptor {
        v: PANE.into(),
        pane,
        subject: subject.clone(),
        title: description.title,
        detail: description.detail,
        actions: description.actions,
        state: description.state,
    }
}

fn plain(text: &str, max: usize, what: &str) -> Result<(), Refusal> {
    if text.is_empty() || text.len() > max || text.chars().any(char::is_control) {
        return Err(Refusal::new(
            Reason::Malformed,
            format!("{what} is 1 to {max} bytes of plain text"),
        ));
    }
    Ok(())
}
