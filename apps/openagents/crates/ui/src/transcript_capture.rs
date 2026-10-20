//! Draw a fixture transcript offscreen and save PNGs, one per theme. Moving
//! transcript code between crates must leave these images unchanged.
//!
//! cargo run -p openagents-ui --example transcript-capture --features account-fixture -- DIR

use std::path::Path;

use chrono::Utc;
use gpui::{AppContext as _, IntoElement, ParentElement, Render, Styled, div, px};
use openagents_doc::SessionMessageEntry;
use openagents_proto::Chat;
use serde_json::json;

use crate::state::AppState;
use crate::theme::Theme;
use crate::transcript::Transcript;

struct Frame(gpui::Entity<Transcript>);

impl Render for Frame {
    fn render(&mut self, _: &mut gpui::Window, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let theme = Theme::of(cx).clone();
        div()
            .size_full()
            .bg(theme.surface)
            .text_color(theme.text)
            .font_family(crate::typography::effective_family_name(cx))
            .child(self.0.clone())
    }
}

fn entries() -> Vec<SessionMessageEntry> {
    serde_json::from_value(json!([
        {"id": "u1", "role": "user", "createdAt": 1, "deviceId": "local",
         "parts": [{"kind": "text", "id": "t", "text": "The composer drops focus after send. Find out why and fix it, then show me the diff."}]},
        {"id": "a1", "role": "assistant", "createdAt": 2, "deviceId": "local", "status": "complete",
         "durationMs": 48000,
         "parts": [
            {"kind": "reasoning", "id": "r", "text": "The send path clears the editor and **rebuilds** the input, so focus goes to the window.\n\n- check `on_send`\n- check the focus handle"},
            {"kind": "tool", "id": "t1", "resolved": true, "call": {"kind": "search", "pattern": "fn on_send", "path": "crates/ui/src"},
             "output": "crates/ui/src/composer.rs:9388: fn on_send(&mut self"},
            {"kind": "tool", "id": "t2", "resolved": true, "call": {"kind": "readFile", "path": "crates/ui/src/composer.rs"}},
            {"kind": "tool", "id": "t3", "resolved": true, "call": {"kind": "editFile", "path": "crates/ui/src/composer.rs"},
             "diff": {"path": "crates/ui/src/composer.rs", "oldText": "self.editor = Editor::new(cx);\n", "newText": "self.editor.clear(cx);\nwindow.focus(&self.focus);\n"}},
            {"kind": "tool", "id": "t4", "resolved": true, "call": {"kind": "exec", "command": "cargo test -p openagents-ui composer"},
             "output": "running 12 tests\ntest result: ok. 12 passed; 0 failed"},
            {"kind": "text", "id": "x", "text": "## Fixed\n\nThe send path **rebuilt** the editor, so its focus handle went away. It now clears the text and keeps focus:\n\n```rust\nself.editor.clear(cx);\nwindow.focus(&self.focus);\n```\n\n| Check | Result |\n| --- | --- |\n| Unit tests | 12 passed |\n| Manual send | keeps focus |\n\n1. Clear instead of rebuild.\n2. Refocus after send.\n\nSee `crates/ui/src/composer.rs` and [the issue](https://github.com/OpenAgentsInc/openagents/issues/1)."}
         ]},
        {"id": "u2", "role": "user", "createdAt": 3, "deviceId": "local",
         "parts": [{"kind": "text", "id": "t", "text": "Great. Now run the whole suite."}]},
        {"id": "a2", "role": "assistant", "createdAt": 4, "deviceId": "local", "status": "complete",
         "parts": [
            {"kind": "text", "id": "t", "text": "Running the full suite now."},
            {"kind": "tool", "id": "run", "resolved": true, "isError": true, "call": {"kind": "exec", "command": "cargo test -p openagents-ui"},
             "output": "test result: FAILED. 811 passed; 1 failed"}
         ]}
    ]))
    .expect("fixture entries")
}

/// Draw the fixture in dark and light and write `dark.png` and `light.png`
/// under `output`.
pub fn capture(
    cx: &mut gpui::HeadlessAppContext,
    data: &Path,
    output: &Path,
) -> anyhow::Result<()> {
    let now = Utc::now();
    let chats: Vec<Chat> = serde_json::from_value(json!([
        {"id": "chat", "deviceId": "local", "title": "Composer focus after send",
         "archived": false, "createdAt": now}
    ]))?;
    let sessions: Vec<openagents_proto::Session> =
        serde_json::from_value(json!([{"chatId": "chat", "deviceId": "local",
        "status": "idle", "updatedAt": now}]))?;
    cx.update(|cx| {
        gpui_base::init(cx);
        let settings = crate::settings::UiSettings::default();
        crate::settings::init(settings.clone(), data, cx);
        let fonts = crate::typography::register_fonts(cx);
        crate::typography::init(
            settings.ui_font_family.clone(),
            settings.ui_font_size,
            settings.terminal_font_family.clone(),
            settings.terminal_font_size,
            settings.code_font_family.clone(),
            settings.code_font_size,
            fonts,
            cx,
        );
    });
    for (name, theme) in [("dark", Theme::dark()), ("light", Theme::light())] {
        cx.update(|cx| cx.set_global(theme));
        let state = cx.update(|cx| cx.new(|_| AppState::new()));
        let entries = entries();
        let (chats, sessions) = (chats.clone(), sessions.clone());
        cx.update(|cx| {
            state.update(cx, |state, cx| {
                state.workspace_scope = Some(openagents_proto::WorkspaceScope::Local);
                state.local_device_id = Some("local".into());
                state.chats = chats;
                state.sessions = sessions;
                state.select_chat(Some("chat".into()), cx);
                state.receive_transcript_update(
                    openagents_doc::TranscriptUpdate {
                        frame: openagents_doc::TranscriptFrame::reset(&entries),
                        replay_baseline: None,
                        context_usage: None,
                    },
                    cx,
                )
            })
        })
        .map_err(|_| anyhow::anyhow!("fixture transcript did not apply"))?;
        let transcript = cx.update(|cx| {
            cx.new(|cx| Transcript::new(crate::transcript_host::AppHost::shared(&state), cx))
        });
        let window = cx.open_window(gpui::size(px(900.0), px(1600.0)), |_, cx| {
            cx.new(|_| Frame(transcript))
        })?;
        for _ in 0..4 {
            cx.update_window(window.into(), |_, window, cx| window.draw(cx).clear())?;
            cx.run_until_parked();
        }
        cx.capture_screenshot(window.into())?
            .save(output.join(format!("{name}.png")))?;
    }
    Ok(())
}
