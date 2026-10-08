use super::*;

fn key(s: &mut DemoState, c: KeyCode) {
    assert!(s.key(Key::new(c)));
}

#[test]
fn original_five_chats_keep_full_messages_cursor_and_top_scroll() {
    let mut s = DemoState::default();
    assert_eq!(s.scroll, 0);
    s.paste("main👩‍💻界");
    key(&mut s, KeyCode::Left);
    let cursor = s.draft.cursor;
    s.scroll = 7;
    key(&mut s, KeyCode::Down);
    s.paste("first");
    key(&mut s, KeyCode::Enter);
    s.paste("second");
    key(&mut s, KeyCode::Enter);
    s.paste("child draft");
    key(&mut s, KeyCode::Left);
    key(&mut s, KeyCode::Esc);
    assert_eq!(s.draft.text, "main👩‍💻界");
    assert_eq!(s.draft.cursor, cursor);
    assert_eq!(s.scroll, 7);
    key(&mut s, KeyCode::Down);
    assert_eq!(s.messages, ["first", "second"]);
    assert_eq!(s.draft.text, "child draft");
    assert_eq!(s.draft.cursor, "child draf".len());
    for _ in 0..9 {
        key(&mut s, KeyCode::Down)
    }
    assert_eq!(s.selected_agent, Some(3));
    for _ in 0..9 {
        key(&mut s, KeyCode::Up)
    }
    assert_eq!(s.selected_agent, None);
}
#[test]
fn original_graphemes_paste_alt_enter_and_releases_remain_local() {
    let mut s = DemoState::default();
    s.paste("a👩‍💻e\u{301}界");
    key(&mut s, KeyCode::Left);
    key(&mut s, KeyCode::Backspace);
    assert_eq!(s.draft.text, "a👩‍💻界");
    key(&mut s, KeyCode::Delete);
    assert_eq!(s.draft.text, "a👩‍💻");
    let mut k = Key::new(KeyCode::Enter);
    k.alt = true;
    s.key(k);
    s.paste("second\r\nthird\tline\u{1b}");
    assert_eq!(s.messages.len(), 0);
    key(&mut s, KeyCode::Enter);
    assert_eq!(s.messages, ["a👩‍💻\nsecond\nthird    line"]);
    let mut k = Key::new(KeyCode::Enter);
    k.release = true;
    s.key(k);
    assert_eq!(s.messages.len(), 1);
}
#[test]
fn animation_clock_and_input_blink_are_independent() {
    let mut s = DemoState::default();
    s.elapsed_seconds = 41;
    for _ in 0..3 {
        s.tick()
    }
    let phase = s.animation_frame;
    s.paste("draft");
    assert_eq!(s.animation_frame, phase);
    assert_eq!(s.cursor_blink_frame, 0);
    assert_eq!(s.elapsed_seconds, 41);
    for _ in 0..8 {
        s.tick()
    }
    assert_eq!(s.animation_frame, phase);
}
#[test]
fn staged_model_requires_enabled_provider_and_cancel_keeps_selection() {
    let mut s = DemoState::default();
    s.open_models();
    assert!(s.model_picker.is_none());
    s.plugins.enabled = true;
    s.open_models();
    s.paste("fable");
    key(&mut s, KeyCode::Enter);
    assert_eq!(
        s.model_picker.as_ref().unwrap().stage,
        models::Stage::Reasoning
    );
    assert_eq!(s.plugins.model, models::DEFAULT_MODEL);
    key(&mut s, KeyCode::Enter);
    assert_eq!(
        s.model_picker.as_ref().unwrap().stage,
        models::Stage::Output
    );
    key(&mut s, KeyCode::Esc);
    key(&mut s, KeyCode::Esc);
    key(&mut s, KeyCode::Esc);
    assert!(s.model_picker.is_none());
    assert_eq!(s.plugins.model, models::DEFAULT_MODEL);
}
#[test]
fn credentials_never_serialize_and_retirement_clears_only_private_edits() {
    let mut s = DemoState::default();
    s.paste("retained local conversation draft");
    s.open_plugin_settings();
    s.paste("synthetic-private-editor-value");
    let json = serde_json::to_string(&s).unwrap();
    assert!(!json.contains("synthetic-private-editor-value"));
    s.retire_secrets();
    assert!(s.plugins.key_draft.text.is_empty());
    assert_eq!(s.draft.text, "retained local conversation draft");
}

#[test]
fn export_preserves_original_calls_children_and_pending_evidence_without_keys() {
    let mut s = DemoState::default();
    s.plugins.key_draft.insert("synthetic-editor-secret");
    s.select_agent(Some(2));
    s.paste("child original one");
    key(&mut s, KeyCode::Enter);
    s.paste("child original two");
    key(&mut s, KeyCode::Enter);
    s.select_agent(None);
    s.paste("/export");
    key(&mut s, KeyCode::Enter);
    let download = s.take_download().unwrap();
    assert_eq!(download.filename, "coder-demo.atif.json");
    let text = String::from_utf8(download.bytes).unwrap();
    assert!(!text.contains("synthetic-editor-secret"));
    let value: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(value["extra"]["demo"], true);
    assert_eq!(value["extra"]["timestamps"], "synthetic-zero");
    assert_eq!(value["extra"]["exported_at"], "1970-01-01T00:00:00.000Z");
    let children = value["subagent_trajectories"].as_array().unwrap();
    assert_eq!(children.len(), 4);
    assert!(
        children
            .iter()
            .all(|child| child["extra"]["exported_at"] == "1970-01-01T00:00:00.000Z")
    );
    let child = children[2].to_string();
    assert!(child.contains("child original one"));
    assert!(child.contains("child original two"));
    let steps = value["steps"].as_array().unwrap();
    assert_eq!(
        steps.len(),
        1 + agents::MAIN_TOOLS.len() + agents::MAIN_PLUGINS.len() + 4
    );
    let pending = steps
        .iter()
        .filter(|step| {
            step.pointer("/tool_calls/0/extra/running")
                .and_then(serde_json::Value::as_bool)
                == Some(true)
        })
        .collect::<Vec<_>>();
    assert!(!pending.is_empty());
    assert!(pending.iter().all(|step| step.get("observation").is_none()));
}
