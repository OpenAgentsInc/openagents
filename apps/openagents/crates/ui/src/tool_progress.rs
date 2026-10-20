//! What a chat is doing while it runs a tool (#11969). The labels live in
//! `openagents_chat_view::tool_progress`; this adds the offscreen capture.

pub use openagents_chat_view::tool_progress::*;

#[cfg(feature = "account-fixture")]
use chrono::Utc;
#[cfg(feature = "account-fixture")]
use openagents_doc::SessionMessageEntry;
#[cfg(feature = "account-fixture")]
use openagents_proto::Chat;

/// Draw a chat waiting on another chat offscreen and save a PNG (#11969).
/// No window is shown: the headless context renders with Metal into an image.
#[cfg(feature = "account-fixture")]
pub fn capture(
    cx: &mut gpui::HeadlessAppContext,
    data: &std::path::Path,
    output: &std::path::Path,
) -> anyhow::Result<()> {
    use gpui::{AppContext as _, IntoElement, ParentElement, Render, Styled, div, px};
    use serde_json::json;

    use crate::{state::AppState, theme::Theme, transcript::Transcript};

    struct Host(gpui::Entity<Transcript>);
    impl Render for Host {
        fn render(
            &mut self,
            _: &mut gpui::Window,
            cx: &mut gpui::Context<Self>,
        ) -> impl IntoElement {
            let theme = Theme::of(cx).clone();
            div()
                .size_full()
                .bg(theme.surface)
                .text_color(theme.text)
                .font_family(crate::typography::effective_family_name(cx))
                .child(self.0.clone())
        }
    }

    let now = Utc::now();
    let chats: Vec<Chat> = serde_json::from_value(json!([
        {"id": "chat", "deviceId": "local", "title": "Polish the composer footer",
         "archived": false, "createdAt": now},
        {"id": "other", "deviceId": "local", "title": "Fix composer focus after send",
         "archived": false, "createdAt": now}
    ]))?;
    let sessions = serde_json::from_value(json!([{"chatId": "chat", "deviceId": "local",
        "status": "working", "startedAt": now - chrono::Duration::seconds(95), "updatedAt": now}]))?;
    let entries: Vec<SessionMessageEntry> = serde_json::from_value(json!([
        {"id": "u1", "role": "user", "createdAt": 1, "deviceId": "local",
         "parts": [{"kind": "text", "id": "t", "text": "Land the composer fix, then tidy the footer."}]},
        {"id": "a1", "role": "assistant", "createdAt": 2, "deviceId": "local", "status": "streaming",
         "parts": [
            {"kind": "text", "id": "t", "text": "I handed the composer fix to its own chat. I'll pick up the footer when it's done."},
            {"kind": "tool", "id": "wait", "resolved": false, "call": {"kind": "mcp",
             "server": "openagents", "tool": "wait_for_turn", "input": {"chat": "other"}}}
         ]}
    ]))?;
    mark_started("wait", now - chrono::Duration::seconds(80));
    cx.update(|cx| {
        gpui_base::init(cx);
        cx.set_global(Theme::dark());
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
    let state = cx.update(|cx| cx.new(|_| AppState::new()));
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
    let window = cx.open_window(gpui::size(px(720.0), px(320.0)), |_, cx| {
        cx.new(|_| Host(transcript))
    })?;
    for _ in 0..3 {
        cx.update_window(window.into(), |_, window, cx| window.draw(cx).clear())?;
        cx.run_until_parked();
    }
    cx.capture_screenshot(window.into())?.save(output)?;
    Ok(())
}
