//! Owner-issued configuration offers and explicit session start semantics.

use super::Selection;
use crate::{Digest, digest_of};
use serde::{Deserialize, Serialize};

pub const SCHEMA: &str = "openagents.coder.session-control.v1";
pub const NATIVE_SCHEMA: &str = "openagents.coder.native-session.v1";
pub const NATIVE_NOTE: &str = "coder_native_session";

/// A provider conversation's lifecycle, independent of the durable task ID.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    New,
    Continue,
    NativeResume,
    ContextTransfer,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Native {
    pub schema: String,
    pub host: String,
    pub task: String,
    pub turn: usize,
    pub session: String,
    pub provider: String,
    pub model: String,
    pub adapter: String,
    pub checkout: String,
    pub target: String,
    pub account: Option<String>,
}
impl Native {
    pub fn id(&self) -> Digest {
        digest_of(self)
    }
    pub fn valid(&self) -> bool {
        self.schema == NATIVE_SCHEMA
            && self.turn > 0
            && bounded(&self.host, 256)
            && bounded(&self.task, 256)
            && bounded(&self.session, 256)
            && !self.session.chars().any(char::is_control)
            && bounded(&self.provider, 128)
            && bounded(&self.model, 256)
            && bounded(&self.adapter, 128)
            && bounded(&self.checkout, 4096)
            && bounded(&self.target, 256)
            && self
                .account
                .as_ref()
                .is_none_or(|account| bounded(account, 256))
    }
    pub fn reference(&self) -> Resume {
        Resume {
            id: self.id(),
            turn: self.turn,
            provider: self.provider.clone(),
            model: self.model.clone(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Resume {
    pub id: Digest,
    pub turn: usize,
    pub provider: String,
    pub model: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Model {
    pub selection: Selection,
    pub adapter: String,
    /// Explicitly supported overrides; an empty list keeps the configured value.
    pub efforts: Vec<String>,
    pub available: bool,
    #[serde(default)]
    pub modes: Vec<Mode>,
    pub account: Option<String>,
    pub reason: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Offer {
    pub schema: String,
    pub id: Digest,
    pub task: Option<String>,
    pub revision: u64,
    pub turn: usize,
    pub authority: Digest,
    pub checkout: String,
    pub target: String,
    pub base: String,
    pub models: Vec<Model>,
    pub resume: Option<Resume>,
}
impl Offer {
    fn identity(&self) -> Digest {
        digest_of(&(
            &self.schema,
            &self.task,
            self.revision,
            self.turn,
            &self.authority,
            &self.checkout,
            &self.target,
            &self.base,
            &self.models,
            &self.resume,
        ))
    }
    pub fn seal(&mut self) {
        self.id = self.identity();
    }
    pub fn valid(&self) -> bool {
        self.schema == SCHEMA
            && self.id == self.identity()
            && bounded(&self.checkout, 4096)
            && bounded(&self.target, 256)
            && self
                .task
                .as_ref()
                .map_or(self.turn == 0, |task| bounded(task, 256) && self.turn > 0)
            && self.models.len() <= 32
            && self.models.iter().all(|m| {
                selection_valid(&m.selection)
                    && bounded(&m.adapter, 128)
                    && m.efforts.len() <= 8
                    && m.modes.len() <= 4
                    && m.efforts.iter().all(|effort| bounded(effort, 64))
                    && m.reason.as_ref().is_none_or(|reason| reason.len() <= 1024)
            })
            && self
                .resume
                .as_ref()
                .is_none_or(|r| r.turn > 0 && bounded(&r.provider, 128) && bounded(&r.model, 256))
    }
    pub fn accepts(&self, request: &Request) -> bool {
        if !self.valid()
            || !request.valid()
            || request.offer != self.id
            || request.checkout != self.checkout
            || request.target != self.target
        {
            return false;
        }
        let Some(model) = self.models.iter().find(|model| {
            model.available
                && model.adapter == request.adapter
                && model.selection.provider == request.selection.provider
                && model.selection.model == request.selection.model
        }) else {
            return false;
        };
        if !model.modes.contains(&request.mode) {
            return false;
        }
        if model.selection.effort != request.selection.effort
            && !request
                .selection
                .effort
                .as_ref()
                .is_some_and(|effort| model.efforts.contains(effort))
        {
            return false;
        }
        match request.mode {
            Mode::NativeResume => {
                self.task.is_some()
                    && model.adapter == "claude-sdk"
                    && self.resume.as_ref().is_some_and(|resume| {
                        Some(&resume.id) == request.resume.as_ref()
                            && resume.provider == request.selection.provider
                            && resume.model == request.selection.model
                    })
            }
            Mode::Continue => self.task.is_some(),
            Mode::New => self.task.is_none() || model.adapter == "claude-sdk",
            Mode::ContextTransfer => self.task.is_some() && model.adapter == "claude-sdk",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Request {
    pub schema: String,
    pub offer: Digest,
    pub adapter: String,
    pub checkout: String,
    pub target: String,
    pub selection: Selection,
    pub mode: Mode,
    pub resume: Option<Digest>,
}
impl Request {
    pub fn valid(&self) -> bool {
        self.schema == SCHEMA
            && selection_valid(&self.selection)
            && bounded(&self.adapter, 128)
            && bounded(&self.checkout, 4096)
            && bounded(&self.target, 256)
            && (self.mode == Mode::NativeResume) == self.resume.is_some()
    }
}

/// A bounded summary selected by the owner, separate from the person's message.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Context {
    pub task: String,
    pub turn: usize,
    pub text: String,
}
impl Context {
    pub fn valid(&self) -> bool {
        bounded(&self.task, 256) && self.turn > 0 && bounded(&self.text, 14 * 1024)
    }
}

/// The owner's resolved start, included in the execution grant.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Start {
    /// None preserves automatic routing and the task's existing continuation.
    pub request: Option<Request>,
    pub selection: Selection,
    pub account: Option<String>,
    /// The task's original model when an explicit later turn selects another.
    pub task_model: Option<String>,
    pub context: Option<Context>,
    pub native: Option<Native>,
}
impl Start {
    pub fn mode(&self) -> Mode {
        self.request
            .as_ref()
            .map_or(Mode::Continue, |request| request.mode)
    }
    pub fn valid_for(&self, provider: &str, model: &str, effort: Option<&str>) -> bool {
        selection_valid(&self.selection)
            && self
                .task_model
                .as_ref()
                .is_none_or(|model| self.request.is_some() && bounded(model, 256))
            && if self.mode() == Mode::ContextTransfer {
                self.context.as_ref().is_some_and(Context::valid)
            } else {
                self.context.is_none()
            }
            && self.selection.provider == provider
            && self.selection.model == model
            && self.selection.effort.as_deref() == effort
            && self
                .request
                .as_ref()
                .is_none_or(|request| request.valid() && request.selection == self.selection)
            && if self.mode() == Mode::NativeResume {
                self.native.as_ref().is_some_and(|native| {
                    native.valid()
                        && Some(native.id())
                            == self
                                .request
                                .as_ref()
                                .and_then(|request| request.resume.clone())
                        && native.adapter == "claude-sdk"
                        && native.provider == provider
                        && native.model == model
                        && native.account.is_some()
                        && native.account == self.account
                })
            } else {
                self.native.is_none()
            }
    }
    pub fn fresh(&self) -> bool {
        matches!(self.mode(), Mode::New | Mode::ContextTransfer)
    }
    pub fn strict_resume(&self) -> bool {
        self.mode() == Mode::NativeResume
    }
}

fn selection_valid(selection: &Selection) -> bool {
    bounded(&selection.provider, 128)
        && bounded(&selection.model, 256)
        && selection
            .effort
            .as_ref()
            .is_none_or(|effort| bounded(effort, 64))
}
fn bounded(value: &str, max: usize) -> bool {
    !value.trim().is_empty() && value.len() <= max
}

#[cfg(test)]
mod tests {
    use super::*;
    fn offer() -> Offer {
        let mut offer = Offer {
            schema: SCHEMA.into(),
            id: digest_of(&()),
            task: Some("task".into()),
            revision: 3,
            turn: 1,
            authority: digest_of(&"rights"),
            checkout: "/fixture".into(),
            target: "inode:1".into(),
            base: "head".into(),
            models: vec![Model {
                selection: Selection {
                    provider: "claude".into(),
                    model: "model".into(),
                    effort: None,
                },
                adapter: "claude-sdk".into(),
                efforts: vec!["high".into()],
                available: true,
                modes: vec![
                    Mode::Continue,
                    Mode::New,
                    Mode::ContextTransfer,
                    Mode::NativeResume,
                ],
                account: Some("login".into()),
                reason: None,
            }],
            resume: Some(Resume {
                id: digest_of(&"source"),
                turn: 1,
                provider: "claude".into(),
                model: "model".into(),
            }),
        };
        offer.seal();
        offer
    }
    fn request(offer: &Offer) -> Request {
        Request {
            schema: SCHEMA.into(),
            offer: offer.id.clone(),
            adapter: "claude-sdk".into(),
            checkout: offer.checkout.clone(),
            target: offer.target.clone(),
            selection: offer.models[0].selection.clone(),
            mode: Mode::Continue,
            resume: None,
        }
    }
    #[test]
    fn selection_binds_adapter_effort_target_and_current_offer() {
        let mut offer = offer();
        let mut cli = offer.models[0].clone();
        cli.adapter = "claude-cli".into();
        cli.modes = vec![Mode::Continue];
        offer.models.insert(0, cli);
        offer.seal();
        let mut request = request(&offer);
        assert!(offer.accepts(&request));
        request.selection.effort = Some("high".into());
        assert!(offer.accepts(&request));
        request.selection.effort = Some("invented".into());
        assert!(!offer.accepts(&request));
        request.selection.effort = None;
        request.adapter = "unknown".into();
        assert!(!offer.accepts(&request));
        request.adapter = "claude-sdk".into();
        request.target = "replacement".into();
        assert!(!offer.accepts(&request));
        request.target = offer.target.clone();
        offer.revision += 1;
        offer.seal();
        assert!(!offer.accepts(&request));
    }
    #[test]
    fn native_resume_requires_exact_source_and_supported_adapter() {
        let offer = offer();
        let mut request = request(&offer);
        request.mode = Mode::NativeResume;
        assert!(!offer.accepts(&request));
        request.resume = offer.resume.as_ref().map(|r| r.id.clone());
        assert!(offer.accepts(&request));
        request.resume = Some(digest_of(&"other"));
        assert!(!offer.accepts(&request));
        request.mode = Mode::New;
        assert!(!offer.accepts(&request));
        request.resume = None;
        assert!(offer.accepts(&request));
    }
    #[test]
    fn resolved_start_refuses_cross_login_resume_and_unbounded_context() {
        let offer = offer();
        let mut request = request(&offer);
        request.mode = Mode::NativeResume;
        let native = Native {
            schema: NATIVE_SCHEMA.into(),
            host: "local".into(),
            task: "task".into(),
            turn: 1,
            session: "provider-session".into(),
            provider: "claude".into(),
            model: "model".into(),
            adapter: "claude-sdk".into(),
            checkout: "/fixture".into(),
            target: "inode:1".into(),
            account: Some("login".into()),
        };
        request.resume = Some(native.id());
        let mut start = Start {
            request: Some(request),
            selection: offer.models[0].selection.clone(),
            account: Some("login".into()),
            task_model: None,
            context: None,
            native: Some(native),
        };
        assert!(start.valid_for("claude", "model", None));
        start.account = Some("different".into());
        assert!(!start.valid_for("claude", "model", None));
        start.request.as_mut().unwrap().mode = Mode::ContextTransfer;
        start.request.as_mut().unwrap().resume = None;
        start.native = None;
        start.context = Some(Context {
            task: "task".into(),
            turn: 1,
            text: "x".repeat(14 * 1024 + 1),
        });
        assert!(!start.valid_for("claude", "model", None));
        start.context.as_mut().unwrap().text = "Summary".into();
        assert!(start.valid_for("claude", "model", None));
        assert!(start.fresh());
        assert!(!start.strict_resume());
    }
}
