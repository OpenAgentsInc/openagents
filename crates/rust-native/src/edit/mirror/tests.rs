use super::*;

fn mounted(draft: &str) -> (Mirror, State) {
    let mut mirror = Mirror::default();
    let (state, replaced) = mirror.mount("composer-1", 1024, Some(draft)).unwrap();
    assert!(replaced);
    (mirror, state)
}

fn utf16(text: &str) -> usize {
    text.encode_utf16().count()
}

/// The field typed or deleted to `text`, with the caret at `caret` (UTF-16).
fn sync(mirror: &mut Mirror, state: &State, text: &str, caret: usize, at_ms: u64) -> State {
    mirror
        .apply(
            &state.stamp,
            Change::Sync {
                text: text.into(),
                selection: [caret, caret],
                marked: None,
                at_ms,
            },
        )
        .unwrap()
}

#[test]
fn typing_mirrors_the_field_and_undo_coalesces_words() {
    let (mut mirror, mut state) = mounted("");
    let mut text = String::new();
    for (i, ch) in "hi there".chars().enumerate() {
        text.push(ch);
        state = sync(&mut mirror, &state, &text, utf16(&text), i as u64 * 10);
    }
    assert_eq!(state.text, "hi there");
    assert_eq!(state.selection, [8, 8]);
    assert!(state.can_undo);
    state = mirror.apply(&state.stamp, Change::Undo).unwrap();
    assert_eq!(state.text, "hi");
    assert!(state.can_redo);
    state = mirror.apply(&state.stamp, Change::Redo).unwrap();
    assert_eq!(state.text, "hi there");
}

#[test]
fn a_keyboard_deletion_never_splits_a_grapheme() {
    for grapheme in ["👨‍👩‍👧‍👦", "e\u{301}", "🇯🇵", "🧑🏽‍💻"] {
        let draft = format!("a{grapheme}");
        let (mut mirror, state) = mounted(&draft);
        // A keyboard that deletes one UTF-16 unit or one scalar at a time.
        let mut units: Vec<u16> = draft.encode_utf16().collect();
        units.pop();
        let partial = String::from_utf16_lossy(&units);
        let text = if partial.contains('\u{FFFD}') {
            let mut chars: Vec<char> = draft.chars().collect();
            chars.pop();
            chars.into_iter().collect()
        } else {
            partial
        };
        let state = sync(&mut mirror, &state, &text, utf16(&text), 0);
        assert_eq!(state.text, "a", "{grapheme}");
        assert_eq!(state.selection, [1, 1]);
        let state = mirror.apply(&state.stamp, Change::Undo).unwrap();
        assert_eq!(state.text, draft);
    }
}

#[test]
fn the_delete_key_removes_one_whole_grapheme() {
    let (mut mirror, state) = mounted("ok 🧑🏽‍💻");
    let state = mirror
        .apply(
            &state.stamp,
            Change::Delete {
                backwards: true,
                at_ms: 0,
            },
        )
        .unwrap();
    assert_eq!(state.text, "ok ");
}

#[test]
fn an_ime_composition_is_one_undo_step_and_cancels_cleanly() {
    let (mut mirror, state) = mounted("say ");
    // Kana input: the field marks "に", then "にほ", then commits "日本".
    let state = mirror
        .apply(
            &state.stamp,
            Change::Sync {
                text: "say に".into(),
                selection: [5, 5],
                marked: Some([4, 5]),
                at_ms: 0,
            },
        )
        .unwrap();
    assert_eq!(state.marked, Some([4, 5]));
    let state = mirror
        .apply(
            &state.stamp,
            Change::Sync {
                text: "say にほ".into(),
                selection: [6, 6],
                marked: Some([4, 6]),
                at_ms: 10,
            },
        )
        .unwrap();
    assert_eq!(state.marked, Some([4, 6]));
    let state = sync(&mut mirror, &state, "say 日本", 6, 20);
    assert_eq!(state.text, "say 日本");
    assert_eq!(state.marked, None);
    let state = mirror.apply(&state.stamp, Change::Undo).unwrap();
    assert_eq!(state.text, "say ");

    // Undo while composing cancels the composition and restores the text.
    let state = mirror
        .apply(
            &state.stamp,
            Change::Sync {
                text: "say ni".into(),
                selection: [6, 6],
                marked: Some([4, 6]),
                at_ms: 30,
            },
        )
        .unwrap();
    let state = mirror.apply(&state.stamp, Change::Undo).unwrap();
    assert_eq!(state.text, "say ");
    assert_eq!(state.marked, None);
}

#[test]
fn stale_stamps_and_new_tokens_change_nothing() {
    let (mut mirror, first) = mounted("draft");
    let second = sync(&mut mirror, &first, "draft!", 6, 0);
    // A reply to an older state refuses without changing the draft.
    assert_eq!(
        mirror.apply(&first.stamp, Change::Undo),
        Err(MirrorError::Stale)
    );
    assert_eq!(mirror.state().unwrap(), second);
    // A rerender with the same token keeps the draft and its history.
    let (again, replaced) = mirror.mount("composer-1", 1024, Some("other")).unwrap();
    assert!(!replaced);
    assert_eq!(again, second);
    assert_eq!(
        mirror.mount("composer-1", 99, None).map(|(_, r)| r),
        Err(MirrorError::Stale)
    );
    // A new token starts a new lifetime; the old stamp is stale.
    let (next, replaced) = mirror.mount("composer-2", 1024, Some("edit me")).unwrap();
    assert!(replaced);
    assert_eq!(next.text, "edit me");
    assert!(!next.can_undo);
    assert_eq!(
        mirror.apply(&second.stamp, Change::Undo),
        Err(MirrorError::Stale)
    );
    mirror.dispose();
    assert_eq!(
        mirror.apply(&next.stamp, Change::Undo),
        Err(MirrorError::Missing)
    );
}

#[test]
fn the_byte_bound_refuses_and_keeps_the_draft() {
    let mut mirror = Mirror::default();
    let (state, _) = mirror.mount("composer-1", 4, None).unwrap();
    let state = sync(&mut mirror, &state, "abcd", 4, 0);
    assert_eq!(
        mirror.apply(
            &state.stamp,
            Change::Sync {
                text: "abcde".into(),
                selection: [5, 5],
                marked: None,
                at_ms: 1,
            },
        ),
        Err(MirrorError::Edit(EditError::TooLong))
    );
    assert_eq!(mirror.state().unwrap().text, "abcd");
}

#[test]
fn replacements_selection_and_offsets_use_utf16() {
    let (mut mirror, state) = mounted("😀 teh");
    // Autocorrect replaces "teh" with "the".
    let state = sync(&mut mirror, &state, "😀 the", 6, 0);
    assert_eq!(state.text, "😀 the");
    let state = mirror
        .apply(&state.stamp, Change::Select { selection: [0, 2] })
        .unwrap();
    assert_eq!(state.selection, [0, 2]);
    assert_eq!(mirror.editor().unwrap().selected_text(), "😀");
    assert_eq!(
        mirror.apply(&state.stamp, Change::Select { selection: [1, 1] }),
        Err(MirrorError::Edit(EditError::Boundary))
    );
    let state = mirror.apply(&state.stamp, Change::Undo).unwrap();
    assert_eq!(state.text, "😀 teh");
}

#[test]
fn typing_a_repeated_letter_inserts_at_the_caret() {
    let (mut mirror, state) = mounted("aa");
    let state = sync(&mut mirror, &state, "aaa", 1, 0);
    assert_eq!(state.selection, [1, 1]);
    assert_eq!(difference("aa", "aaa", Selection::collapsed(1)), (0, 0, 1));
    assert_eq!(difference("abc", "ac", Selection::collapsed(1)), (1, 2, 1));
    assert_eq!(difference("x", "x", Selection::collapsed(1)), (1, 1, 1));
    let _ = state;
}

#[test]
fn a_submission_clears_only_its_exact_draft() {
    let (mut mirror, state) = mounted("");
    let state = sync(&mut mirror, &state, "send me", 7, 0);
    let sent = state.stamp.clone();
    let later = sync(&mut mirror, &state, "send me!", 8, 10);
    assert!(!mirror.submitted(&sent, "send me").unwrap());
    assert_eq!(mirror.state().unwrap(), later);
    assert!(mirror.submitted(&later.stamp, "send me!").unwrap());
    let cleared = mirror.state().unwrap();
    assert_eq!(cleared.text, "");
    assert!(!cleared.can_undo);
    assert_ne!(cleared.stamp.lifetime, later.stamp.lifetime);
}

#[test]
fn json_requests_answer_with_the_state_or_a_textless_refusal() {
    let mut mirror = Mirror::default();
    let reply: serde_json::Value = serde_json::from_slice(
        &mirror
            .call_json(br#"{"op":"mount","token":"composer-1","max_bytes":64,"draft":"secret"}"#),
    )
    .unwrap();
    assert_eq!(reply["replaced"], true);
    assert_eq!(reply["state"]["text"], "secret");
    let stamp = reply["state"]["stamp"].clone();
    let request = serde_json::json!({"op": "apply", "stamp": stamp,
        "change": {"op": "sync", "text": "secret!", "selection": [7, 7], "at_ms": 0}});
    let reply: serde_json::Value =
        serde_json::from_slice(&mirror.call_json(request.to_string().as_bytes())).unwrap();
    assert_eq!(reply["state"]["text"], "secret!");
    // The same stamp again is stale: the refusal names no draft text beyond
    // the current state it returns for resynchronizing.
    let reply: serde_json::Value =
        serde_json::from_slice(&mirror.call_json(request.to_string().as_bytes())).unwrap();
    assert_eq!(reply["error"], "stale");
    assert_eq!(reply["state"]["text"], "secret!");
    let reply: serde_json::Value =
        serde_json::from_slice(&mirror.call_json(b"{\"op\":\"nope\"}")).unwrap();
    assert_eq!(reply["error"], "unreadable");
    let reply: serde_json::Value =
        serde_json::from_slice(&mirror.call_json(br#"{"op":"dispose"}"#)).unwrap();
    assert!(reply["state"].is_null());
}
