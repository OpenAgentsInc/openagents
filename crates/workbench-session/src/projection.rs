//! A read-only session view shared by browser and native workbench mounts.
//! Original references and owner states remain distinct from local presentation.
//! A displayed input flag is evidence from a current mount, never authorization.

use coder_pty::ext::{Layout, Member as NativeMember, MemberState};
use serde::{Deserialize, Serialize};
use workbench::pane::{PaneDescriptor, PaneKind, PaneState, Panes, Subject, View};
use workbench::{Host, Kind, ResourceRef};

use crate::{Consent, Resolver, Saved, State};

/// The projection schema. It carries no credential, command, or terminal output.
pub const SCHEMA: &str = "openagents.workbench-session-projection.v1";
/// The maximum serialized projection, including owner descriptions.
pub const MAX_BYTES: usize = 64 * 1024;

/// Where a saved member stands after its current owner was checked.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Resolution {
    Ready,
    Closed,
    Lost,
    Revoked,
    NeedsAdmission,
    Unsupported,
    IdentityMismatch,
    Missing,
    Stale,
    Unavailable,
    /// The session read did not include a native terminal member state.
    Unknown,
}

impl From<State> for Resolution {
    fn from(state: State) -> Self {
        match state {
            State::Ready => Self::Ready,
            State::Unavailable => Self::Unavailable,
            State::Stale => Self::Stale,
            State::Lost => Self::Lost,
            State::Revoked => Self::Revoked,
            State::NeedsAdmission => Self::NeedsAdmission,
            State::Unsupported => Self::Unsupported,
            State::IdentityMismatch => Self::IdentityMismatch,
            State::Missing => Self::Missing,
        }
    }
}

/// Evidence supplied by a live mount after applying the owner's snapshot and
/// typist frame. It cannot be deserialized from a navigation response.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InputProof {
    pub resource: ResourceRef,
    pub current_snapshot: bool,
    pub current_attachment: Option<String>,
    pub is_current_typist: bool,
}

impl InputProof {
    fn check(&self) -> Result<(), String> {
        self.resource
            .check()
            .map_err(|_| "The input evidence has an invalid resource reference.")?;
        if self.resource.kind != Kind::Terminal
            || self
                .current_attachment
                .as_ref()
                .is_some_and(|id| !workbench::is_common_id(id))
        {
            return Err("The input evidence has an invalid terminal attachment.".into());
        }
        Ok(())
    }

    fn admits(&self, resource: &ResourceRef) -> bool {
        self.resource == *resource
            && self.current_snapshot
            && self.current_attachment.is_some()
            && self.is_current_typist
    }
}

/// One member's exact reference and current presentation. Opening this view
/// never creates a terminal, task, thread, or replacement for a missing member.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Member {
    pub member: u16,
    pub resource: ResourceRef,
    pub route: Option<String>,
    pub resolution: Resolution,
    pub native_terminal_state: Option<MemberState>,
    pub input: bool,
    pub pane: Option<PaneDescriptor>,
}

/// A session projection keeps the owner's session and layout identity whole.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Projection {
    pub v: String,
    pub owner: Host,
    pub session: String,
    pub revision: u64,
    pub name: String,
    pub layout: Layout,
    pub members: Vec<Member>,
}

impl Projection {
    /// Checks the display shape against the original saved record. This is not
    /// an admission check; every native operation still needs its current grant.
    pub fn check(&self, saved: &Saved) -> Result<(), String> {
        let originals = saved
            .members()
            .map_err(|_| "The saved session has invalid member references.")?;
        if self.v != SCHEMA
            || self.owner != saved.owner
            || Some(&self.session) != saved.record.session.as_ref()
            || self.revision != saved.record.revision
            || self.name != saved.record.name
            || self.layout != saved.record.layout
            || self.members.len() != originals.len()
        {
            return Err("The session projection does not match its owner record.".into());
        }
        for (index, (member, original)) in self.members.iter().zip(originals).enumerate() {
            if member.member != original.member || member.resource != original.resource {
                return Err("The session projection substituted a member reference.".into());
            }
            if member.native_terminal_state != saved.record.members[index].state()
                || member.route.as_ref().is_some_and(|route| {
                    route.is_empty() || route.len() > 128 || route.chars().any(char::is_control)
                })
            {
                return Err("The session projection has invalid owner state.".into());
            }
            if member.input
                && (member.resource.kind != Kind::Terminal
                    || member.resolution != Resolution::Ready
                    || member.route.is_none()
                    || member.native_terminal_state != Some(MemberState::Live))
            {
                return Err("Only a current live terminal can display input evidence.".into());
            }
            if member.resolution == Resolution::Ready
                && (matches!(
                    member.native_terminal_state,
                    Some(MemberState::Closed | MemberState::Lost)
                ) || (matches!(
                    saved.record.members[index],
                    NativeMember::Terminal { state: None, .. }
                )))
            {
                return Err("The terminal projection has no current live owner state.".into());
            }
            if member.pane.as_ref().map(|pane| pane.pane) != pane_kind(member.resource.kind) {
                return Err("The pane kind does not match its original resource.".into());
            }
            if let Some(pane) = &member.pane {
                pane.check()
                    .map_err(|_| "The session projection has an invalid pane description.")?;
                if pane.subject
                    != (Subject::Resource {
                        resource: member.resource.clone(),
                    })
                    || !pane.actions.is_empty()
                    || matches!(
                        pane.state,
                        PaneState::Fallback {
                            view: View::Tty { .. }
                        }
                    )
                    || (member.resolution != Resolution::Ready && pane.state == PaneState::Ready)
                {
                    return Err("The pane does not match its read-only member presentation.".into());
                }
            }
        }
        if serde_json::to_vec(self).map_or(true, |bytes| bytes.len() > MAX_BYTES) {
            return Err("The session projection exceeds its display bound.".into());
        }
        Ok(())
    }
}

/// Resolves each saved member through its current owner. Pane descriptions use
/// only adapters registered for that exact owner. A browser receives labels
/// instead of TTY commands; no action or input queue exists in this projection.
pub fn project(
    saved: &Saved,
    resolver: &impl Resolver,
    consents: &[Consent],
    now: u64,
    panes: &Panes,
    input: &[InputProof],
) -> Result<Projection, String> {
    if input.len() > coder_pty::ext::MEMBERS_MAX {
        return Err("Too many terminal input evidence records.".into());
    }
    for (index, proof) in input.iter().enumerate() {
        proof.check()?;
        if input[..index]
            .iter()
            .any(|previous| previous.resource == proof.resource)
        {
            return Err("The input evidence repeats a terminal reference.".into());
        }
    }
    let resolved = saved
        .resolve(resolver, consents, now)
        .map_err(|_| "The saved session cannot be resolved with the current owner state.")?;
    let mut members = Vec::with_capacity(resolved.len());
    for (resolved, native) in resolved.into_iter().zip(&saved.record.members) {
        let mut resolution = Resolution::from(resolved.state);
        let native_terminal_state = native.state();
        if matches!(native, NativeMember::Terminal { .. })
            && matches!(resolution, Resolution::Ready | Resolution::Missing)
        {
            resolution = match native_terminal_state {
                Some(MemberState::Live) => resolution,
                Some(MemberState::Closed) => Resolution::Closed,
                Some(MemberState::Lost) => Resolution::Lost,
                None if resolution == Resolution::Ready => Resolution::Unknown,
                None => resolution,
            };
        }
        let can_input = resolution == Resolution::Ready
            && native_terminal_state == Some(MemberState::Live)
            && resolved.input
            && input.iter().any(|proof| proof.admits(&resolved.resource));
        let pane = pane_kind(resolved.resource.kind)
            .map(|kind| {
                let subject = Subject::Resource {
                    resource: resolved.resource.clone(),
                };
                let mut described = if resolution == Resolution::Ready {
                    panes
                        .resolve_for_host(kind, &subject)
                        .map_err(|_| "The owner pane cannot be described.")?
                } else {
                    PaneDescriptor {
                        v: workbench::pane::PANE.into(),
                        pane: kind,
                        subject,
                        title: format!("{} resource", kind.label()),
                        detail: String::new(),
                        actions: Vec::new(),
                        state: pane_state(resolution),
                    }
                };
                described.actions.clear();
                if matches!(
                    described.state,
                    PaneState::Fallback {
                        view: View::Tty { .. }
                    }
                ) {
                    described.state = PaneState::Fallback { view: View::Label };
                }
                resolution = match &described.state {
                    PaneState::Ready => resolution,
                    PaneState::Missing => Resolution::Missing,
                    PaneState::Stale { .. } => Resolution::Stale,
                    PaneState::Revoked => Resolution::Revoked,
                    PaneState::Unavailable if resolution == Resolution::Ready => {
                        Resolution::Unavailable
                    }
                    PaneState::Fallback { .. } if resolution == Resolution::Ready => {
                        Resolution::Unsupported
                    }
                    _ => resolution,
                };
                Ok::<_, String>(described)
            })
            .transpose()?;
        members.push(Member {
            member: resolved.member,
            resource: resolved.resource,
            route: resolved.route,
            resolution,
            native_terminal_state,
            input: can_input,
            pane,
        });
    }
    let projection = Projection {
        v: SCHEMA.into(),
        owner: saved.owner.clone(),
        session: saved
            .record
            .session
            .clone()
            .ok_or("The saved session has no identity.")?,
        revision: saved.record.revision,
        name: saved.record.name.clone(),
        layout: saved.record.layout.clone(),
        members,
    };
    projection.check(saved)?;
    Ok(projection)
}

fn pane_kind(kind: Kind) -> Option<PaneKind> {
    match kind {
        Kind::Thread => Some(PaneKind::Thread),
        Kind::Run => Some(PaneKind::Run),
        Kind::Studio => Some(PaneKind::Studio),
        Kind::Artifact => Some(PaneKind::Artifact),
        Kind::File => Some(PaneKind::File),
        Kind::Evidence => Some(PaneKind::Evaluation),
        Kind::Terminal | Kind::Tool => None,
    }
}

fn pane_state(resolution: Resolution) -> PaneState {
    match resolution {
        Resolution::Missing | Resolution::Closed => PaneState::Missing,
        Resolution::Stale => PaneState::Stale { current: None },
        Resolution::Revoked => PaneState::Revoked,
        Resolution::Unsupported => PaneState::Fallback { view: View::Label },
        _ => PaneState::Unavailable,
    }
}
