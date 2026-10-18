use rust_native::view::{Activation, FieldChange, ValidatedView, View, ViewError};
use serde::{Deserialize, Serialize};
use std::sync::Mutex;

use super::session::{BusyPolicy, SendOutcome, UserInputAnswer};
use super::types::{CoreError, CoreResult};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum SessionViewIntent {
    SaveDraft,
    Interrupt,
    RespondInput {
        request_id: String,
        answers: Vec<ViewInputAnswer>,
    },
    RequestAttachment,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ViewInputAnswer {
    pub question_id: String,
    pub labels: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum NativeActionResult {
    DraftSaved,
    SendStarted { message_id: String },
    SendSteered { message_id: String },
    SendQueued { queue_id: String },
    Interrupted,
    InputAnswered,
    AttachmentPickerRequested,
}

#[derive(Default)]
pub(crate) struct NativeViewRegistry {
    pending: Option<ValidatedView<SessionViewIntent>>,
    applied: Option<ValidatedView<SessionViewIntent>>,
    disposed: bool,
}

impl NativeViewRegistry {
    pub(crate) fn prepare(&mut self, bytes: &[u8]) -> CoreResult<Vec<u8>> {
        if self.disposed {
            return Err(view_error(ViewError::StaleActivation));
        }
        let view = View::<SessionViewIntent>::from_json(bytes).map_err(view_error)?;
        if let Some(previous) = self.pending.as_ref().or(self.applied.as_ref()) {
            if previous.view().instance != view.view().instance
                || previous.view().revision >= view.view().revision
            {
                return Err(view_error(ViewError::StaleActivation));
            }
        }
        let encoded = view.to_json().map_err(view_error)?;
        self.pending = Some(view);
        Ok(encoded)
    }

    pub(crate) fn acknowledge(&mut self, instance: &str, revision: u64) -> CoreResult<()> {
        let pending = self
            .pending
            .as_ref()
            .ok_or_else(|| view_error(ViewError::StaleActivation))?;
        if self.disposed
            || pending.view().instance != instance
            || pending.view().revision != revision
        {
            return Err(view_error(ViewError::StaleActivation));
        }
        self.applied = self.pending.take();
        Ok(())
    }

    pub(crate) fn activate(&self, event: &Activation) -> CoreResult<SessionViewIntent> {
        if self.disposed {
            return Err(view_error(ViewError::StaleActivation));
        }
        let current = self
            .applied
            .as_ref()
            .ok_or_else(|| view_error(ViewError::StaleActivation))?;
        current.activate(event).cloned().map_err(view_error)
    }

    pub(crate) fn change_field(&self, event: &FieldChange) -> CoreResult<SessionViewIntent> {
        if self.disposed {
            return Err(view_error(ViewError::StaleActivation));
        }
        let current = self
            .applied
            .as_ref()
            .ok_or_else(|| view_error(ViewError::StaleActivation))?;
        current.change_field(event).cloned().map_err(view_error)
    }

    pub(crate) fn accept_composer(&self, token: &str, text: &str) -> CoreResult<()> {
        if self.disposed {
            return Err(view_error(ViewError::StaleActivation));
        }
        let current = self
            .applied
            .as_ref()
            .ok_or_else(|| view_error(ViewError::StaleActivation))?;
        current
            .accept_composer(token, text)
            .map_err(|error| CoreError::InvalidArgument {
                message: error.to_string(),
            })
    }

    pub(crate) fn composer_busy_policy(&self, token: &str, text: &str) -> CoreResult<BusyPolicy> {
        self.accept_composer(token, text)?;
        let current = self
            .applied
            .as_ref()
            .ok_or_else(|| view_error(ViewError::StaleActivation))?;
        let choice = find_composer_token(&current.view().root, token)
            .ok_or_else(|| view_error(ViewError::StaleActivation))?;
        Ok(match choice {
            ComposerToken::Default | ComposerToken::Later => BusyPolicy::Queue,
            ComposerToken::Now => BusyPolicy::Steer,
        })
    }
}

enum ComposerToken {
    Default,
    Later,
    Now,
}

fn find_composer_token<I>(node: &rust_native::view::Node<I>, token: &str) -> Option<ComposerToken> {
    use rust_native::view::Element;
    match &node.element {
        Element::Composer { token: current, .. } if current == token => {
            Some(ComposerToken::Default)
        }
        Element::Composer { choices, .. } => choices
            .iter()
            .find(|choice| choice.token == token)
            .map(|choice| {
                if choice.later {
                    ComposerToken::Later
                } else {
                    ComposerToken::Now
                }
            }),
        Element::Stack { children, .. }
        | Element::List { children, .. }
        | Element::Transcript { children, .. }
        | Element::Message { children, .. }
        | Element::Choice { children, .. }
        | Element::Tool { children, .. } => children
            .iter()
            .find_map(|child| find_composer_token(child, token)),
        Element::Dialog {
            open: true,
            children,
            ..
        } => children
            .iter()
            .find_map(|child| find_composer_token(child, token)),
        _ => None,
    }
}

pub(crate) fn answer_input(
    request_id: String,
    answers: Vec<ViewInputAnswer>,
) -> (String, Vec<UserInputAnswer>) {
    (
        request_id,
        answers
            .into_iter()
            .map(|answer| UserInputAnswer {
                question_id: answer.question_id,
                labels: answer.labels,
            })
            .collect(),
    )
}

pub(crate) fn map_send_outcome(outcome: SendOutcome) -> NativeActionResult {
    match outcome {
        SendOutcome::Started { message_id } => NativeActionResult::SendStarted { message_id },
        SendOutcome::Steered { message_id } => NativeActionResult::SendSteered { message_id },
        SendOutcome::Queued { queue_id } => NativeActionResult::SendQueued { queue_id },
    }
}

pub(crate) fn lock_registry(
    registry: &Mutex<NativeViewRegistry>,
) -> CoreResult<std::sync::MutexGuard<'_, NativeViewRegistry>> {
    registry.lock().map_err(|_| CoreError::Internal {
        message: "native view state is unavailable".into(),
    })
}

fn view_error(error: ViewError) -> CoreError {
    CoreError::InvalidArgument {
        message: error.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_native::view::{Element, Node, Style};

    fn button_view(revision: u64, enabled: bool) -> Vec<u8> {
        View::new_v3(
            "session-surface",
            revision,
            Node {
                key: "stop".into(),
                style: Style::default(),
                element: Element::Button {
                    label: "Stop".into(),
                    enabled,
                    icon: None,
                    shortcut: None,
                    intent: SessionViewIntent::Interrupt,
                },
            },
        )
        .validate()
        .unwrap()
        .to_json()
        .unwrap()
    }

    #[test]
    fn only_the_acknowledged_revision_dispatches() {
        let mut registry = NativeViewRegistry::default();
        let view = button_view(1, true);
        registry.prepare(&view).unwrap();
        registry.acknowledge("session-surface", 1).unwrap();

        let stale = Activation {
            instance: "session-surface".into(),
            revision: 0,
            node: "stop".into(),
        };
        assert!(matches!(
            registry.activate(&stale),
            Err(CoreError::InvalidArgument { .. })
        ));

        let current = Activation {
            instance: "session-surface".into(),
            revision: 1,
            node: "stop".into(),
        };
        assert!(matches!(
            registry.activate(&current),
            Ok(SessionViewIntent::Interrupt)
        ));
    }

    #[test]
    fn disabled_actions_are_rejected_by_validated_view() {
        let mut registry = NativeViewRegistry::default();
        registry.prepare(&button_view(1, false)).unwrap();
        registry.acknowledge("session-surface", 1).unwrap();
        let event = Activation {
            instance: "session-surface".into(),
            revision: 1,
            node: "stop".into(),
        };
        assert!(matches!(
            registry.activate(&event),
            Err(CoreError::InvalidArgument { .. })
        ));
    }

    #[test]
    fn unknown_actions_are_rejected_before_mount() {
        let mut value: serde_json::Value = serde_json::from_slice(&button_view(1, true)).unwrap();
        value["root"]["element"]["intent"]["action"] =
            serde_json::Value::String("approve_project".into());
        let json = serde_json::to_vec(&value).unwrap();
        assert!(matches!(
            NativeViewRegistry::default().prepare(&json),
            Err(CoreError::InvalidArgument { .. })
        ));
    }

    #[test]
    fn field_changes_resolve_only_the_current_draft_action() {
        let view = View::new_v3(
            "session-surface",
            1,
            Node {
                key: "draft".into(),
                style: Style::default(),
                element: Element::Field {
                    label: "Message".into(),
                    value: String::new(),
                    placeholder: String::new(),
                    secret: false,
                    multiline: true,
                    enabled: true,
                    max_bytes: 64 * 1024,
                    on_change: SessionViewIntent::SaveDraft,
                },
            },
        )
        .validate()
        .unwrap()
        .to_json()
        .unwrap();
        let mut registry = NativeViewRegistry::default();
        registry.prepare(&view).unwrap();
        registry.acknowledge("session-surface", 1).unwrap();
        let event = FieldChange {
            activation: Activation {
                instance: "session-surface".into(),
                revision: 1,
                node: "draft".into(),
            },
            value: "café".into(),
        };
        assert!(matches!(
            registry.change_field(&event),
            Ok(SessionViewIntent::SaveDraft)
        ));
    }
}
