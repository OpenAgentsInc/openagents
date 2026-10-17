//! Offscreen capture of a saved agent's chat in the sidebar and session
//! header (#11966): the Foreman chat beside an ordinary Claude Code chat.
//! Nothing is shown on screen: a headless context renders into an image.
//!
//! cargo run -p openagents-ui --example agent-identity-fixture --features account-fixture -- OUT.png
use gpui::{AppContext, px, size};
use openagents_ui::*;
use serde_json::json;
use std::{path::PathBuf, sync::Arc};

fn main() -> anyhow::Result<()> {
    // A failed check exits instead of aborting, so no crash report opens.
    openagents_gpui_test::exit_on_panic();
    let output = PathBuf::from(
        std::env::args_os()
            .nth(1)
            .expect("pass the PNG path to write"),
    );
    let runtime = tokio::runtime::Runtime::new()?;
    let dir = tempfile::tempdir()?;
    let data = dir.path().to_path_buf();
    let mut cx = openagents_gpui_test::headless_app(Arc::new(icons::Assets));
    let (state, boot) = cx.update(|cx| {
        gpui_tokio::init_from_handle(cx, runtime.handle().clone());
        gpui_base::init(cx);
        let mut settings = settings::UiSettings::default();
        settings.sidebar_width = 300.0;
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
        let state = cx.new(|_| {
            let mut s = state::AppState::new();
            s.connection = openagents_proto::view::ConnectionStatus::Ready;
            s.workspace_scope = Some(openagents_proto::WorkspaceScope::Local);
            s.local_device_id = Some("local".into());
            s.devices = vec![serde_json::from_value(json!({"id":"local","name":"Alex's MacBook Pro","platform":"macos","lastSeenAt":null})).unwrap()];
            s.spaces = vec![serde_json::from_value(json!({"id":"project","deviceId":"local","path":"/tmp/fieldnotes","gitDetected":true,"createdAt":"2026-10-17T00:00:00Z"})).unwrap()];
            s.chats = vec![
                serde_json::from_value(json!({"id":"foreman","deviceId":"local","spaceId":"project","title":"Foreman","archived":false,"createdAt":"2026-10-17T01:00:00Z","lastMessageAt":"2026-10-17T02:00:00Z","config":{"harness":"claude-code","model":"claude-opus-5-5","reasoning":null,"sandbox":"danger-full-access","modelOptions":{"openagentsAssignment":"a","openagentsAgentName":"Foreman"}}})).unwrap(),
                serde_json::from_value(json!({"id":"chat","deviceId":"local","spaceId":"project","title":"Polish the composer footer","archived":false,"createdAt":"2026-10-17T00:00:00Z","lastMessageAt":"2026-10-17T01:30:00Z","config":{"harness":"claude-code","model":"claude-opus-5-5","reasoning":null,"sandbox":"workspace-write"}})).unwrap(),
            ];
            s.selected_chat = Some("foreman".into());
            s.selected_space = Some("project".into());
            s.auto_selected = true;
            s.chats_synced = true;
            s.spaces_synced = true;
            s
        });
        let boot = EngineBootConfig {
            data_dir: data.clone(),
            ipc_port: 0,
            edge_url: openagents_brand::DISABLED_EDGE_URL.into(),
            edge_token: None,
            org_id: None,
            workos_client_id: None,
            default_harness: openagents_proto::HarnessId::ClaudeCode,
        };
        (state, boot)
    });
    let window = cx.open_window(size(px(1120.), px(720.)), |_, cx| {
        cx.new(|cx| shell::Shell::new(state.clone(), boot, cx))
    })?;
    for _ in 0..2 {
        cx.update_window(window.into(), |_, window, cx| window.draw(cx).clear())?;
        cx.run_until_parked();
    }
    openagents_gpui_test::save_capture(&mut cx, window.into(), &output)?;
    println!("Wrote {}", output.display());
    Ok(())
}
