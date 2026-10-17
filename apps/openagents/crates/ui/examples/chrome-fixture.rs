//! Offscreen captures of the composer's Runs on control and the sidebar
//! account row (#11959). Pass an empty output directory. The sign-in is a
//! fixture account served from loopback; credentials stay in memory.
use ::openagents_account::{
    Controller,
    openagents::{Client, CredentialRecord, CredentialStore, Deployment, StorageError},
};
use axum::{Json, Router, routing::get};
use gpui::{AppContext, AsyncApp, Bounds, WindowBounds, WindowOptions, px, size};
use openagents_ui::{openagents_account::AppAccount, *};
use serde_json::json;
use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Duration,
};

#[derive(Default)]
struct Store(Mutex<Option<CredentialRecord>>);
impl CredentialStore for Store {
    fn load(&self, _: &Deployment) -> Result<Option<CredentialRecord>, StorageError> {
        Ok(self.0.lock().unwrap().clone())
    }
    fn save(&self, r: &CredentialRecord) -> Result<(), StorageError> {
        *self.0.lock().unwrap() = Some(r.clone());
        Ok(())
    }
    fn clear(&self, _: &Deployment) -> Result<(), StorageError> {
        *self.0.lock().unwrap() = None;
        Ok(())
    }
}

async fn pause(cx: &mut AsyncApp) {
    cx.background_executor()
        .timer(Duration::from_millis(400))
        .await;
}

fn capture(window: gpui::AnyWindowHandle, cx: &mut AsyncApp, output: &Path, name: &str) {
    window
        .update(cx, |_, window, cx| {
            window.draw(cx).clear();
            window
                .render_to_image()
                .unwrap()
                .save(output.join(name))
                .unwrap();
        })
        .unwrap();
}

fn main() -> anyhow::Result<()> {
    let output = PathBuf::from(std::env::args_os().nth(1).expect("output directory"));
    std::fs::create_dir_all(&output)?;
    let runtime = tokio::runtime::Runtime::new()?;
    let origin = runtime.block_on(async {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let account =
            json!({"account":{"id":"acct_fixture","label":"Alex Rivera"},"workspaces":[]});
        let empty = json!({"data":[],"next":null});
        let router = Router::new()
            .route("/v1/account", get(move || async move { Json(account) }))
            .fallback(get(move || async move { Json(empty) }));
        tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
        origin
    });
    let deployment = Deployment::new(&format!("{origin}/v1"), &origin)?;
    let store = Arc::new(Store::default());
    store.save(&serde_json::from_value(json!({
        "deployment": deployment,
        "account": {"id": "acct_fixture", "label": "Alex Rivera"},
        "expires_at": null,
        "credential": format!("sess_{:064x}", 1),
    }))?)?;
    let controller = Controller::new(
        Client::new(deployment)?,
        store,
        output.join("profile/signed-out"),
    );
    runtime.block_on(controller.restore());
    anyhow::ensure!(controller.lease().is_some(), "fixture sign-in failed");
    let data = output.join("workspace");
    openagents_gpui_test::real_application()
        .with_assets(icons::Assets)
        .run(move |cx| {
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
            app_menus::init(cx);
            AppAccount::install_fixture(controller.clone(), cx);
            let transfer = openagents_proto::capabilities::CHAT_HOME_TRANSFER_V1;
            let state = cx.new(|_| {
                let mut s = state::AppState::new();
                s.connection = openagents_proto::view::ConnectionStatus::Ready;
                s.workspace_scope = Some(openagents_proto::WorkspaceScope::Local);
                s.local_device_id = Some("local".into());
                s.devices = vec![
                    serde_json::from_value(json!({"id":"local","name":"Alex's MacBook Pro","platform":"macos","lastSeenAt":null,"capabilities":[transfer]})).unwrap(),
                    serde_json::from_value(json!({"id":"build","name":"Build server","platform":"linux","lastSeenAt":chrono::Utc::now(),"capabilities":[transfer]})).unwrap(),
                ];
                s.spaces = vec![serde_json::from_value(json!({"id":"project","deviceId":"local","path":"/tmp/fieldnotes","gitDetected":true,"createdAt":"2026-10-17T00:00:00Z"})).unwrap()];
                s.chats = vec![serde_json::from_value(json!({"id":"chat","deviceId":"local","spaceId":"project","title":"Polish the composer footer","branch":"main","archived":false,"createdAt":"2026-10-17T00:00:00Z","config":{"harness":"claude-code","model":"claude-opus-5-5","reasoning":null,"sandbox":"workspace-write"}})).unwrap()];
                if std::env::var_os("CHROME_FIXTURE_IDLE").is_none() {
                    s.sessions = vec![serde_json::from_value(json!({"chatId":"chat","deviceId":"local","status":"working","startedAt":chrono::Utc::now(),"updatedAt":chrono::Utc::now()})).unwrap()];
                }
                s.selected_chat = Some("chat".into());
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
                default_harness: HarnessId::ClaudeCode,
            };
            let window = cx
                .open_window(
                    WindowOptions {
                        window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                            None,
                            size(px(1120.), px(720.)),
                            cx,
                        ))),
                        focus: false,
                        show: false,
                        ..Default::default()
                    },
                    |_, cx| cx.new(|cx| shell::Shell::new(state.clone(), boot, cx)),
                )
                .unwrap();
            let output = output.clone();
            cx.spawn(async move |cx| {
                pause(cx).await;
                pause(cx).await;
                capture(window.into(), cx, &output, "window.png");
                if std::env::var_os("CHROME_FIXTURE_MENUS").is_some() {
                    window
                        .update(cx, |shell, _, cx| {
                            shell
                                .fixture_continuation_composer()
                                .update(cx, |composer, cx| composer.fixture_open_runs_on(cx));
                        })
                        .unwrap();
                    pause(cx).await;
                    capture(window.into(), cx, &output, "runs-on-menu.png");
                    window
                        .update(cx, |shell, _, cx| {
                            shell.fixture_continuation_composer().update(cx, |_, cx| cx.notify());
                            AppAccount::global(cx)
                                .unwrap()
                                .update(cx, |account, cx| account.fixture_open_menu(cx));
                        })
                        .unwrap();
                    pause(cx).await;
                    capture(window.into(), cx, &output, "account-menu.png");
                }
                cx.update(|cx| cx.quit());
            })
            .detach();
        });
    Ok(())
}
