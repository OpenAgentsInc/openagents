use super::*;
use rust_native::{Axis, ComposerChoice, View, style::Style};

fn node(token: &str, draft: &str, enabled: bool, busy: bool) -> Node<String> {
    Node {
        key: format!("node-{token}"),
        style: Style::default(),
        element: Element::Composer {
            token: token.into(),
            placeholder: "Message OpenAgents".into(),
            max_bytes: 1024,
            enabled,
            busy,
            stop: busy.then(|| "stop-current-reply".into()),
            choices: vec![ComposerChoice {
                token: format!("{token}-queue"),
                label: "Send after this".into(),
            }],
            draft: Some(draft.into()),
            focus: true,
        },
    }
}

fn view(
    instance: &str,
    revision: u64,
    token: &str,
    draft: &str,
    enabled: bool,
    busy: bool,
) -> ValidatedView<String> {
    View::new(instance, revision, node(token, draft, enabled, busy))
        .validate()
        .unwrap()
}

fn apply(draft: &mut ComposerDraft, input: Input<'_>) {
    let stamp = draft.stamp().unwrap();
    draft.apply(&stamp, input, 0).unwrap();
}

#[test]
fn rerenders_preserve_selection_undo_and_marked_input_without_refocusing() {
    let mut draft = ComposerDraft::default();
    let first = view("chat", 1, "send", "hello", true, false);
    assert_eq!(
        draft.mount(&first, "node-send").unwrap(),
        Mount {
            replaced: true,
            focus: true
        }
    );
    apply(&mut draft, Input::SelectAll);
    apply(
        &mut draft,
        Input::Preedit {
            text: "にほん",
            selection: Selection::collapsed(9),
        },
    );
    let next = view("chat", 2, "send", "old application draft", true, false);
    assert_eq!(
        draft.mount(&next, "node-send").unwrap(),
        Mount {
            replaced: false,
            focus: false
        }
    );
    let editor = draft.editor().unwrap();
    assert_eq!(editor.text(), "にほん");
    assert_eq!(editor.selection(), Selection::collapsed(9));
    assert!(editor.is_composing());
    apply(&mut draft, Input::Commit("日本"));
    apply(&mut draft, Input::Undo);
    assert_eq!(draft.editor().unwrap().text(), "hello");
}

#[test]
fn stale_sequences_and_view_revisions_never_change_the_draft() {
    let mut draft = ComposerDraft::default();
    let first = view("chat", 1, "send", "", true, false);
    draft.mount(&first, "node-send").unwrap();
    let old = draft.stamp().unwrap();
    apply(&mut draft, Input::Text("a"));
    assert_eq!(
        draft.apply(&old, Input::Text("late"), 1),
        Err(ComposerError::Stale)
    );
    let before_render = draft.stamp().unwrap();
    let next = view("chat", 2, "send", "", true, false);
    draft.mount(&next, "node-send").unwrap();
    assert_eq!(
        draft.apply(&before_render, Input::Text("late"), 2),
        Err(ComposerError::Stale)
    );
    assert_eq!(draft.mount(&first, "node-send"), Err(ComposerError::Stale));
    assert_eq!(draft.editor().unwrap().text(), "a");
}

#[test]
fn a_chat_switch_or_disposal_rejects_old_callbacks_even_after_switching_back() {
    let mut draft = ComposerDraft::default();
    let a = view("chat-a", 1, "send", "a", true, false);
    let b = view("chat-b", 1, "send", "b", true, false);
    draft.mount(&a, "node-send").unwrap();
    let old = draft.stamp().unwrap();
    draft.mount(&b, "node-send").unwrap();
    assert_eq!(
        draft.apply(&old, Input::Text("late"), 0),
        Err(ComposerError::Stale)
    );
    draft.mount(&a, "node-send").unwrap();
    assert_eq!(
        draft.apply(&old, Input::Text("late"), 0),
        Err(ComposerError::Stale)
    );
    let before_dispose = draft.stamp().unwrap();
    draft.dispose();
    assert!(draft.editor().is_none());
    draft.mount(&a, "node-send").unwrap();
    assert_eq!(
        draft.apply(&before_dispose, Input::Text("late"), 0),
        Err(ComposerError::Stale)
    );
}

#[test]
fn changing_the_token_starts_a_new_draft_and_a_new_undo_history() {
    let mut draft = ComposerDraft::default();
    draft
        .mount(&view("chat", 1, "one", "old", true, false), "node-one")
        .unwrap();
    apply(&mut draft, Input::Text("!"));
    let old = draft.stamp().unwrap();
    draft
        .mount(
            &view("chat", 2, "two", "replacement", true, false),
            "node-two",
        )
        .unwrap();
    assert_eq!(draft.editor().unwrap().text(), "replacement");
    apply(&mut draft, Input::Undo);
    assert_eq!(draft.editor().unwrap().text(), "replacement");
    assert_eq!(
        draft.apply(&old, Input::Text("late"), 0),
        Err(ComposerError::Stale)
    );
}

#[test]
fn missing_or_changed_bounds_refuse_without_replacing_input() {
    let mut draft = ComposerDraft::default();
    let mut current = view("chat", 1, "send", "kept", true, false).view().clone();
    draft
        .mount(&current.clone().validate().unwrap(), "node-send")
        .unwrap();
    assert_eq!(
        draft.mount(&current.clone().validate().unwrap(), "missing"),
        Err(ComposerError::Missing)
    );
    current.revision = 2;
    if let Element::Composer { max_bytes, .. } = &mut current.root.element {
        *max_bytes = 2;
    }
    // The application does not provide a draft longer than its new bound.
    if let Element::Composer { draft, .. } = &mut current.root.element {
        *draft = None;
    }
    assert_eq!(
        draft.mount(&current.validate().unwrap(), "node-send"),
        Err(ComposerError::BoundChanged)
    );
    assert_eq!(draft.editor().unwrap().text(), "kept");
}

#[test]
fn disabled_composers_allow_selection_but_refuse_edits_and_submissions() {
    let mut draft = ComposerDraft::default();
    let current = view("chat", 1, "send", "copy this", false, false);
    assert!(!draft.mount(&current, "node-send").unwrap().focus);
    apply(&mut draft, Input::SelectAll);
    assert_eq!(draft.editor().unwrap().selected_text(), "copy this");
    let stamp = draft.stamp().unwrap();
    assert_eq!(
        draft.apply(&stamp, Input::Paste("replace"), 0),
        Err(ComposerError::Disabled)
    );
    assert!(matches!(
        draft.submission(&current, &stamp, None),
        Err(ComposerError::Disabled)
    ));
}

#[test]
fn send_refuses_composing_or_empty_text_and_preserves_failed_drafts() {
    let mut draft = ComposerDraft::default();
    let current = view("chat", 1, "send", " \n ", true, false);
    draft.mount(&current, "node-send").unwrap();
    assert!(matches!(
        draft.submission(&current, &draft.stamp().unwrap(), None),
        Err(ComposerError::Empty)
    ));
    apply(
        &mut draft,
        Input::Preedit {
            text: "仮",
            selection: Selection::collapsed(3),
        },
    );
    assert!(matches!(
        draft.submission(&current, &draft.stamp().unwrap(), None),
        Err(ComposerError::Edit(EditError::Composing))
    ));
    apply(&mut draft, Input::Commit("日本"));
    let submission = draft
        .submission(&current, &draft.stamp().unwrap(), None)
        .unwrap();
    assert_eq!(submission.text, " \n 日本");
    // Preparing a submission does not clear text: the application may refuse.
    assert_eq!(draft.editor().unwrap().text(), submission.text);
}

#[test]
fn busy_primary_action_resolves_stop_and_choices_stay_bound_to_their_composer() {
    let mut draft = ComposerDraft::default();
    let current = View::new(
        "chat",
        1,
        Node {
            key: "root".into(),
            style: Style::default(),
            element: Element::Stack {
                axis: Axis::Vertical,
                children: vec![
                    node("send", "message", true, true),
                    node("other", "other", true, false),
                ],
            },
        },
    )
    .validate()
    .unwrap();
    draft.mount(&current, "node-send").unwrap();
    let stamp = draft.stamp().unwrap();
    assert!(matches!(
        draft.submission(&current, &stamp, None),
        Err(ComposerError::Busy)
    ));
    assert_eq!(draft.stop(&current, &stamp).unwrap(), "stop-current-reply");
    assert_eq!(
        draft
            .submission(&current, &stamp, Some("send-queue"))
            .unwrap()
            .token,
        "send-queue"
    );
    assert!(matches!(
        draft.submission(&current, &stamp, Some("other-queue")),
        Err(ComposerError::Stale)
    ));
    let stopped = view("chat", 2, "send", "", true, false);
    draft.mount(&stopped, "node-send").unwrap();
    assert!(matches!(
        draft.stop(&current, &stamp),
        Err(ComposerError::Stale)
    ));
    assert!(matches!(
        draft.stop(&stopped, &draft.stamp().unwrap()),
        Err(ComposerError::NotStop)
    ));
}

#[test]
fn submission_requires_the_current_view_to_have_been_mounted() {
    let mut draft = ComposerDraft::default();
    let current = view("chat", 1, "send", "message", true, false);
    draft.mount(&current, "node-send").unwrap();
    let next = view("chat", 2, "send", "", true, false);
    assert!(matches!(
        draft.submission(&next, &draft.stamp().unwrap(), None),
        Err(ComposerError::Stale)
    ));
}

#[test]
fn acceptance_clears_only_the_submitted_edit_sequence_and_invalidates_callbacks() {
    let mut draft = ComposerDraft::default();
    let current = view("chat", 1, "send", "message", true, false);
    draft.mount(&current, "node-send").unwrap();
    let submitted = draft
        .submission(&current, &draft.stamp().unwrap(), None)
        .unwrap();
    apply(&mut draft, Input::Text(" more"));
    assert!(!draft.accepted(&submitted).unwrap());
    assert_eq!(draft.editor().unwrap().text(), "message more");
    let submitted = draft
        .submission(&current, &draft.stamp().unwrap(), None)
        .unwrap();
    let newer = view("chat", 2, "send", "old application draft", true, true);
    draft.mount(&newer, "node-send").unwrap();
    let stamp = draft.stamp().unwrap();
    assert!(draft.accepted(&submitted).unwrap());
    assert_eq!(draft.editor().unwrap().text(), "");
    assert!(!draft.accepted(&submitted).unwrap());
    assert_eq!(
        draft.apply(&stamp, Input::Text("late"), 0),
        Err(ComposerError::Stale)
    );
    apply(&mut draft, Input::Undo);
    assert_eq!(draft.editor().unwrap().text(), "");
}

#[test]
fn acceptance_in_a_different_chat_preserves_its_draft() {
    let mut draft = ComposerDraft::default();
    let a = view("a", 1, "send", "a message", true, false);
    draft.mount(&a, "node-send").unwrap();
    let submitted = draft.submission(&a, &draft.stamp().unwrap(), None).unwrap();
    draft
        .mount(&view("b", 1, "send", "b message", true, false), "node-send")
        .unwrap();
    assert!(!draft.accepted(&submitted).unwrap());
    assert_eq!(draft.editor().unwrap().text(), "b message");
    assert!(!format!("{submitted:?}").contains("a message"));
}

#[test]
fn multiline_paste_remains_a_single_undo_step_through_the_adapter() {
    let mut draft = ComposerDraft::default();
    let current = view("chat", 1, "send", "", true, false);
    draft.mount(&current, "node-send").unwrap();
    apply(&mut draft, Input::Paste("one\ntwo 😀"));
    let submitted = draft
        .submission(&current, &draft.stamp().unwrap(), None)
        .unwrap();
    assert_eq!(submitted.text, "one\ntwo 😀");
    apply(&mut draft, Input::Undo);
    assert_eq!(draft.editor().unwrap().text(), "");
    apply(&mut draft, Input::Redo);
    assert_eq!(draft.editor().unwrap().text(), submitted.text);
}
