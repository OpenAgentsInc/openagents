//! A saved layout groups references, never credentials or execution rights.
//! Every member resolves against its own owner and the client's current grant.
use coder_pty::ext::{Layout, Member, SessionRecord};
use serde::{Deserialize, Serialize};
use workbench::{Host, Kind, ResourceRef};

pub const SCHEMA: &str = "openagents.workbench-session.v1";
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Saved {
    pub v: String,
    pub owner: Host,
    pub record: SessionRecord,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MemberRef {
    pub member: u16,
    pub resource: ResourceRef,
}
impl Saved {
    pub fn members(&self) -> Result<Vec<MemberRef>, String> {
        if self.v != SCHEMA {
            return Err("Unsupported workbench session version.".into());
        }
        if self.record.session.is_none() || self.record.revision == 0 {
            return Err("A saved session needs its owner-issued identity and revision.".into());
        }
        self.record.check().map_err(|e| e.detail)?;
        // Validate the layout owner even when every member belongs elsewhere.
        ResourceRef::new(Kind::Thread, self.owner.clone(), "owner-validation")
            .check()
            .map_err(|e| e.detail)?;
        self.record
            .members
            .iter()
            .map(|member| {
                let resource = match member {
                    Member::Terminal { terminal, .. } => ResourceRef::terminal(
                        self.owner.clone(),
                        terminal.generation.clone(),
                        terminal.terminal.clone(),
                    ),
                    Member::Resource { resource, .. } => {
                        serde_json::from_value::<ResourceRef>(resource.clone())
                            .map_err(|e| format!("Invalid typed session reference: {e}"))?
                    }
                };
                resource.check().map_err(|e| e.detail)?;
                Ok(MemberRef {
                    member: member.id(),
                    resource,
                })
            })
            .collect()
    }
    pub fn layout(&self, device: &str, local: Option<&Override>) -> Result<Layout, String> {
        self.members()?;
        match local {
            None => Ok(self.record.layout.clone()),
            Some(local) => {
                if local.device != device
                    || local.session != self.record.session
                    || local.revision != self.record.revision
                {
                    return Err(
                        "The layout override belongs to another device or session revision.".into(),
                    );
                }
                if !digest(device) {
                    return Err("A layout device needs a public identity digest.".into());
                }
                let mut checked = self.record.clone();
                checked.layout = local.layout.clone();
                checked.check().map_err(|e| e.detail)?;
                Ok(local.layout.clone())
            }
        }
    }
    /// Observation only: a resolver cannot dispatch, create, or pay.
    pub fn resolve(
        &self,
        resolver: &impl Resolver,
        consents: &[Consent],
        now: u64,
    ) -> Result<Vec<Pane>, String> {
        self.members()?
            .into_iter()
            .map(|member| {
                let resource = member.resource;
                let mut pane = Pane {
                    member: member.member,
                    resource: resource.clone(),
                    route: None,
                    state: State::Unavailable,
                    input: false,
                };
                let Some(current) = resolver.current(&resource.host) else {
                    return Ok(pane);
                };
                if current.host != resource.host {
                    pane.state = State::IdentityMismatch;
                    return Ok(pane);
                }
                if current
                    .route
                    .as_ref()
                    .is_some_and(|r| r.len() > 128 || r.chars().any(char::is_control))
                    || !digest(&current.disclosure)
                {
                    return Err("Invalid owner route or disclosure binding.".into());
                }
                pane.route = current.route.clone();
                if current.revoked {
                    pane.state = State::Revoked;
                    return Ok(pane);
                }
                if let Some(generation) = &resource.generation {
                    match &current.generation {
                        None => return Ok(pane),
                        Some(observed) if observed != generation => {
                            pane.state = State::Lost;
                            return Ok(pane);
                        }
                        _ => {}
                    }
                }
                if !current.read
                    || current.expires_at <= now
                    || !consents
                        .iter()
                        .any(|c| c.resource == resource && c.disclosure == current.disclosure)
                {
                    pane.state = State::NeedsAdmission;
                    return Ok(pane);
                }
                if !current.capabilities.contains(&resource.kind) {
                    pane.state = State::Unsupported;
                    return Ok(pane);
                }
                if current.route.is_none() {
                    return Ok(pane);
                }
                let probe = resolver.resource(&resource);
                if probe.resource != resource {
                    pane.state = State::IdentityMismatch;
                    return Ok(pane);
                }
                pane.state = probe.state;
                pane.input =
                    current.input && resource.kind == Kind::Terminal && pane.state == State::Ready;
                Ok(pane)
            })
            .collect()
    }
}
/// This device's override never writes the host's default layout.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Override {
    pub device: String,
    pub session: Option<String>,
    pub revision: u64,
    pub layout: Layout,
}
/// A client admission binds one exact resource and disclosure. It carries no credential.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Consent {
    pub resource: ResourceRef,
    pub disclosure: String,
}
impl Consent {
    pub fn new(resource: ResourceRef, disclosure: String) -> Result<Self, String> {
        resource.check().map_err(|e| e.detail)?;
        if !digest(&disclosure) {
            return Err("A disclosure needs its exact digest.".into());
        }
        Ok(Self {
            resource,
            disclosure,
        })
    }
}
#[derive(Clone, Debug)]
pub struct Current {
    pub host: Host,
    pub generation: Option<String>,
    pub route: Option<String>,
    pub disclosure: String,
    pub expires_at: u64,
    pub read: bool,
    pub input: bool,
    pub revoked: bool,
    pub capabilities: Vec<Kind>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum State {
    Ready,
    Unavailable,
    Stale,
    Lost,
    Revoked,
    NeedsAdmission,
    Unsupported,
    IdentityMismatch,
    Missing,
}
#[derive(Clone, Debug)]
pub struct Probe {
    pub resource: ResourceRef,
    pub state: State,
}
pub trait Resolver {
    fn current(&self, host: &Host) -> Option<Current>;
    fn resource(&self, resource: &ResourceRef) -> Probe;
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pane {
    pub member: u16,
    pub resource: ResourceRef,
    pub route: Option<String>,
    pub state: State,
    pub input: bool,
}
impl Pane {
    pub fn title(&self) -> String {
        format!(
            "{:?} {} · {:?} · {} · {:?}",
            self.resource.kind,
            self.resource.id,
            self.resource.host,
            self.route.as_deref().unwrap_or("unavailable"),
            self.state
        )
    }
}
fn digest(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
