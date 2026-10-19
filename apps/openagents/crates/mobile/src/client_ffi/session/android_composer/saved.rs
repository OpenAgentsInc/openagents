use super::*;
use crate::client_ffi::CoreSavedHistory;
use rust_native::{Activation, Axis, TextRole, ValidatedView};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Action {
    Retry,
}
#[derive(Default)]
pub(super) struct Cards {
    pending: Option<ValidatedView<Action>>,
    applied: Option<ValidatedView<Action>>,
}
impl Cards {
    pub(super) fn acknowledge(&mut self, instance: &str, revision: u64) -> CoreResult<()> {
        let view = self.pending.as_ref().ok_or(CoreError::Closed)?;
        if view.view().instance != instance || view.view().revision != revision {
            return Err(CoreError::Closed);
        }
        self.applied = self.pending.take();
        Ok(())
    }
}
fn can_reply(history: &CoreSavedHistory) -> bool {
    history.current_view()
        && history.reader().snapshot().summary.is_some_and(|summary| {
            summary.source == openagents_account::openagents::ChatSource::Web
                && summary.links.continued_by.is_none()
        })
}
pub(super) fn frame(
    history: &CoreSavedHistory,
    model: &Model,
) -> (
    bool,
    bool,
    bool,
    Option<String>,
    String,
    Option<EditorStamp>,
) {
    let snapshot = history.turns.snapshot();
    (
        can_reply(history) && !snapshot.retry,
        snapshot.busy || model.pending_send,
        false,
        snapshot.error,
        model.mirror.state().map(|s| s.text).unwrap_or_default(),
        model.mirror.stamp(),
    )
}
pub(super) fn cards(
    cards: &mut Cards,
    instance: &str,
    revision: u64,
    history: &CoreSavedHistory,
) -> CoreResult<Option<String>> {
    // An old retry button must stop resolving even before its replacement mounts.
    cards.pending = None;
    cards.applied = None;
    let snapshot = history.turns.snapshot();
    let mut children = vec![];
    if let Some(error) = snapshot.error {
        children.push(node(
            "error",
            Element::Text {
                value: error,
                role: TextRole::Body,
            },
        ));
    }
    if snapshot.retry && can_reply(history) {
        children.push(node(
            "retry",
            Element::Button {
                label: "Retry".into(),
                enabled: !snapshot.busy,
                icon: None,
                shortcut: None,
                intent: Action::Retry,
            },
        ));
    }
    if children.is_empty() {
        return Ok(None);
    }
    let view = View::new_v3(
        format!("{instance}-cards"),
        revision,
        node(
            "root",
            Element::Stack {
                axis: Axis::Vertical,
                children,
            },
        ),
    )
    .validate()
    .map_err(|_| CoreError::Closed)?;
    let json = String::from_utf8(view.to_json().map_err(|_| CoreError::Closed)?)
        .map_err(|_| CoreError::Closed)?;
    cards.pending = Some(view);
    Ok(Some(json))
}
fn node(key: &str, element: Element<Action>) -> Node<Action> {
    Node {
        key: key.into(),
        style: Default::default(),
        element,
    }
}
#[allow(clippy::too_many_arguments)]
pub(super) fn submit(
    owner: &CoreAndroidComposer,
    history: &Arc<CoreSavedHistory>,
    id: &str,
    instance: String,
    revision: u64,
    token: String,
    lifetime: u64,
    editor_revision: u64,
    choice: Option<String>,
    text: String,
) -> CoreResult<bool> {
    if !can_reply(history) {
        return Ok(false);
    }
    let mut model = owner.model.lock().map_err(|_| CoreError::Closed)?;
    let stamp = EditorStamp {
        token: token.clone(),
        lifetime,
        revision: editor_revision,
    };
    if model.mirror.stamp().as_ref() != Some(&stamp)
        || model.mirror.state().is_none_or(|s| s.text != text)
    {
        return Err(CoreError::Closed);
    }
    model.registry.composer_busy_policy(
        &instance,
        revision,
        choice.as_deref().unwrap_or(&token),
        &text,
    )?;
    let before = history.turns.snapshot();
    if model.pending_send || before.busy || before.retry {
        return Ok(false);
    }
    if let Some(drafts) = &model.drafts {
        drafts.save(owner.draft_key(), &text)?;
    }
    let Some(completion) = history.turns.try_submit_for(id.into(), text.clone()) else {
        return Ok(false);
    };
    model.pending_send = true;
    let submitted = model
        .mirror
        .submitted(&stamp, &text)
        .map_err(|_| CoreError::Closed);
    let cleared = model.mirror.stamp();
    drop(model);
    finish(
        owner.model.clone(),
        history.clone(),
        id.into(),
        owner.draft_key().into(),
        completion,
        before.accepted,
        text,
        cleared,
        false,
    );
    submitted?;
    Ok(true)
}
pub(super) fn activate(
    owner: &CoreAndroidComposer,
    history: &Arc<CoreSavedHistory>,
    id: &str,
    instance: String,
    revision: u64,
    key: String,
) -> CoreResult<bool> {
    let mut model = owner.model.lock().map_err(|_| CoreError::Closed)?;
    if instance == format!("{}-cards", owner.instance) {
        model
            .saved_cards
            .applied
            .as_ref()
            .ok_or(CoreError::Closed)?
            .activate(&Activation {
                instance,
                revision,
                node: key,
            })
            .map_err(|_| CoreError::Closed)?;
        if model.pending_send || !can_reply(history) {
            return Ok(false);
        }
        let before = history.turns.snapshot();
        if !before.retry || before.busy {
            return Ok(false);
        }
        let completion = history.turns.retry_for(id.into());
        if !history.turns.snapshot().busy {
            return Ok(false);
        }
        model.saved_cards.applied = None;
        model.pending_send = true;
        let stamp = model.mirror.stamp();
        let text = model.mirror.state().map(|s| s.text).unwrap_or_default();
        drop(model);
        finish(
            owner.model.clone(),
            history.clone(),
            id.into(),
            owner.draft_key().into(),
            completion,
            before.accepted,
            text,
            stamp,
            true,
        );
    } else {
        let intent = model.registry.activate(&Activation {
            instance,
            revision,
            node: key,
        })?;
        if !matches!(intent, SessionViewIntent::Interrupt) {
            return Err(CoreError::Closed);
        }
        drop(model);
        openagents_client::runtime::shared().spawn(history.turns.stop_for(id.into()));
    }
    Ok(false)
}
#[allow(clippy::too_many_arguments)]
fn finish(
    model: Arc<Mutex<Model>>,
    history: Arc<CoreSavedHistory>,
    id: String,
    draft_key: String,
    completion: impl std::future::Future<Output = ()> + Send + 'static,
    accepted: u64,
    text: String,
    stamp: Option<EditorStamp>,
    retry: bool,
) {
    openagents_client::runtime::shared().spawn(async move {
        completion.await;
        let snapshot = history.turns.snapshot();
        if let Ok(mut model) = model.lock() {
            model.pending_send = false;
            if history.current_view()
                && snapshot
                    .chat
                    .as_ref()
                    .is_some_and(|chat| chat.id() == id.as_str())
            {
                if snapshot.accepted > accepted {
                    if retry {
                        clear_accepted(&mut model.mirror, stamp.as_ref(), &text);
                    }
                } else if !retry {
                    restore_unsent(&mut model.mirror, stamp.as_ref(), &text);
                }
                if let (Some(drafts), Some(state)) = (&model.drafts, model.mirror.state()) {
                    let _ = drafts.save(&draft_key, &state.text);
                }
            }
            let listener = model.listener.clone();
            drop(model);
            if let Some(listener) = listener {
                listener.on_android_host_changed();
            }
        }
    });
}
fn clear_accepted(mirror: &mut Mirror, stamp: Option<&EditorStamp>, text: &str) {
    if let Some(stamp) = stamp.filter(|stamp| mirror.stamp().as_ref() == Some(*stamp)) {
        let _ = mirror.submitted(stamp, text);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn accepted_retry_clears_only_its_unchanged_editor() {
        let mut mirror = Mirror::default();
        mirror.mount("message", MAX_BYTES, Some("old")).unwrap();
        let stamp = mirror.stamp().unwrap();
        clear_accepted(&mut mirror, Some(&stamp), "old");
        assert!(mirror.state().unwrap().text.is_empty());
        assert_ne!(mirror.stamp(), Some(stamp));
    }
    #[test]
    fn accepted_retry_keeps_later_typing() {
        let mut mirror = Mirror::default();
        mirror.mount("message", MAX_BYTES, Some("old")).unwrap();
        let stamp = mirror.stamp().unwrap();
        mirror
            .apply(
                &stamp,
                rust_native::edit::mirror::Change::Sync {
                    text: "new".into(),
                    selection: [3, 3],
                    marked: None,
                    at_ms: 0,
                },
            )
            .unwrap();
        clear_accepted(&mut mirror, Some(&stamp), "old");
        assert_eq!(mirror.state().unwrap().text, "new");
    }
    #[test]
    fn retry_cards_require_the_mounted_revision() {
        let view = View::new_v3(
            "cards",
            7,
            node(
                "retry",
                Element::Button {
                    label: "Retry".into(),
                    enabled: true,
                    icon: None,
                    shortcut: None,
                    intent: Action::Retry,
                },
            ),
        )
        .validate()
        .unwrap();
        let mut cards = Cards {
            pending: Some(view),
            applied: None,
        };
        assert!(cards.acknowledge("cards", 6).is_err());
        assert!(cards.applied.is_none());
        cards.acknowledge("cards", 7).unwrap();
        assert!(
            cards
                .applied
                .as_ref()
                .unwrap()
                .activate(&Activation {
                    instance: "cards".into(),
                    revision: 6,
                    node: "retry".into(),
                })
                .is_err()
        );
    }
}
