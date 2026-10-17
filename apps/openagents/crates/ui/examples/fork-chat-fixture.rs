//! Offscreen captures for #11961–#11963: the session card without "New side
//! chat", the sidebar footer without the Pull requests button, and a forked
//! chat listed in Sessions with its "Forked from" link. Renders the
//! production Shell in a window placed off screen (never focused). It shows
//! a real window, so it runs only with `OPENAGENTS_GPUI_REAL_PLATFORM=1` and
//! never on the owner's desktop (see OPENAGENTS.md):
//!
//!   cargo run -p openagents-ui --example fork-chat-fixture \
//!     --features fork-chat-fixture -- <output-dir>
use gpui::{AppContext, AsyncApp, Bounds, WindowBounds, WindowOptions, px, size};
use openagents_doc::{MessagePart, MessageRole, MessageStatus, SessionMessageEntry};
use openagents_ui::*;
use std::{path::PathBuf, time::Duration};

fn entry(id: &str, role: MessageRole, parts: Vec<MessagePart>) -> SessionMessageEntry {
    SessionMessageEntry {
        duration_ms: None,
        id: id.into(),
        role,
        parts,
        created_at: chrono::Utc::now().timestamp_millis(),
        device_id: "local".into(),
        status: Some(MessageStatus::Complete),
        continuation_of: None,
    }
}

fn text(id: &str, role: MessageRole, text: &str) -> SessionMessageEntry {
    entry(
        id,
        role,
        vec![MessagePart::Text {
            id: format!("{id}-text"),
            text: text.into(),
        }],
    )
}

fn capture(
    window: gpui::AnyWindowHandle,
    cx: &mut AsyncApp,
    output: &std::path::Path,
    name: &str,
) -> anyhow::Result<()> {
    window.update(cx, |_, window, cx| {
        window.draw(cx).clear();
        window
            .render_to_image()?
            .save(output.join(format!("{name}.png")))?;
        Ok(())
    })?
}

fn main() -> anyhow::Result<()> {
    let output = PathBuf::from(
        std::env::args()
            .nth(1)
            .ok_or_else(|| anyhow::anyhow!("usage: fork-chat-fixture <output-dir>"))?,
    );
    std::fs::create_dir_all(&output)?;
    let runtime = tokio::runtime::Runtime::new()?;
    let _guard = runtime.enter();
    let temp = tempfile::tempdir()?;
    let data = temp.path().to_path_buf();
    openagents_gpui_test::real_application()
        .with_assets(icons::Assets)
        .run(move |cx| {
            gpui_tokio::init(cx);
            gpui_base::init(cx);
            let mut settings = settings::UiSettings::default();
            settings.sidebar_width = 300.0;
            settings.save(&data).unwrap();
            settings::init(settings.clone(), data.clone(), cx);
            let fonts = typography::register_fonts(cx);
            typography::init(
                settings.ui_font_family.clone(),
                settings.ui_font_size,
                settings.terminal_font_family.clone(),
                settings.terminal_font_size,
                settings.code_font_family.clone(),
                settings.code_font_size,
                fonts,
                cx,
            );
            theme_library::init(data.clone(), cx);
            appearance::init(
                appearance::AppearanceMode::Dark,
                settings.theme_selection,
                settings.accent,
                settings.surface,
                cx,
            );
            history::init(
                settings.git_history_columns,
                settings.git_history_column_widths,
                settings.git_history_column_order,
                settings.git_history_author_display,
                cx,
            );
            composer::init(cx, settings.composer_send_behavior);
            terminal::panel::init(cx);
            app_menus::init(cx);
            let project = data.join("fieldnotes");
            std::fs::create_dir_all(&project).unwrap();
            let state = cx.new(|_| {
                let mut s = state::AppState::new();
                s.connection = openagents_proto::view::ConnectionStatus::Ready;
                s.workspace_scope = Some(openagents_proto::WorkspaceScope::Local);
                s.local_device_id = Some("local".into());
                s.devices = vec![serde_json::from_value(serde_json::json!({"id":"local","name":"This Mac","platform":"macos","lastSeenAt":chrono::Utc::now()})).unwrap()];
                s.spaces = vec![serde_json::from_value(serde_json::json!({"id":"project","deviceId":"local","path":project,"createdAt":"2026-10-01T00:00:00Z"})).unwrap()];
                let chat = |id: &str, title: &str, minutes: i64| -> openagents_proto::Chat {
                    serde_json::from_value(serde_json::json!({
                        "id": id, "deviceId": "local", "spaceId": "project", "title": title,
                        "archived": false, "cwd": project,
                        "createdAt": chrono::Utc::now() - chrono::Duration::minutes(minutes),
                        "lastMessageAt": chrono::Utc::now() - chrono::Duration::minutes(minutes),
                        "lastSeenAt": chrono::Utc::now(),
                        "config": {"harness":"claude-code","model":"claude-opus-5-5","reasoning":null,"sandbox":"workspace-write"},
                    }))
                    .unwrap()
                };
                // An older build's side chat, listed like any other chat.
                let mut old_side = chat("old-side", "Check the release notes", 30);
                old_side.parent_chat_id = Some("source".into());
                s.chats = vec![
                    chat("fork", "Fork of Plan the onboarding flow", 0),
                    chat("source", "Plan the onboarding flow", 5),
                    old_side,
                    chat("other", "Fix authentication redirects", 60),
                ];
                s.selected_chat = Some("source".into());
                s.selected_space = Some("project".into());
                s.auto_selected = true;
                s.chats_synced = true;
                s.spaces_synced = true;
                s
            });
            let history = vec![
                text("u1", MessageRole::User, "Sketch the onboarding flow for new teams."),
                text(
                    "a1",
                    MessageRole::Assistant,
                    "Three steps: create the workspace, invite teammates, connect a repository.",
                ),
            ];
            state.update(cx, |s, _| s.apply_transcript(history.clone()));
            let boot = EngineBootConfig {
                data_dir: data.clone(),
                ipc_port: 0,
                edge_url: String::new(),
                edge_token: None,
                org_id: None,
                workos_client_id: None,
                default_harness: HarnessId::ClaudeCode,
            };
            let window = cx
                .open_window(
                    WindowOptions {
                        window_background: theme::Theme::of(cx).window_background_appearance(),
                        window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                            gpui::point(px(-12000.), px(-12000.)),
                            size(px(1600.), px(900.)),
                        ))),
                        focus: false,
                        show: true,
                        ..Default::default()
                    },
                    |_, cx| cx.new(|cx| shell::Shell::new(state.clone(), boot, cx)),
                )
                .unwrap();
            state.update(cx, |_, cx| cx.notify());
            cx.spawn(async move |cx| {
                let run: anyhow::Result<()> = async {
                    let any = window.into();
                    let pause = |cx: &mut AsyncApp, ms| {
                        cx.background_executor().timer(Duration::from_millis(ms))
                    };
                    pause(cx, 1200).await;
                    capture(any, cx, &output, "11961-11962-session-card-and-sidebar")?;
                    // The fork: copied history, then the seam linking back.
                    let mut forked = history.clone();
                    forked.push(entry(
                        "fork:fork",
                        MessageRole::System,
                        vec![MessagePart::Fork {
                            id: "fork:fork".into(),
                            source_chat_id: "source".into(),
                            source_title: "Plan the onboarding flow".into(),
                        }],
                    ));
                    forked.push(text("u2", MessageRole::User, "Now write the invite email."));
                    forked.push(text(
                        "a2",
                        MessageRole::Assistant,
                        "Subject: Join your team on OpenAgents.",
                    ));
                    state.update(cx, |s, cx| {
                        s.selected_chat = Some("fork".into());
                        s.apply_transcript(forked);
                        cx.notify();
                    });
                    pause(cx, 1200).await;
                    capture(any, cx, &output, "11963-fork-in-sessions")?;
                    Ok(())
                }
                .await;
                if let Err(error) = &run {
                    eprintln!("fixture failed: {error:#}");
                }
                cx.update(|cx| cx.quit());
            })
            .detach();
        });
    Ok(())
}
