//! Exercise the native account control with a local browser approval page.
//! Pass an empty scratch directory. No Keychain, engine, or provider is used.
use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    response::Html,
    routing::{get, post},
};
use coder_account::{
    Controller, Phase,
    openagents::{Client, CredentialRecord, CredentialStore, Deployment, StorageError},
};
use gpui::{AppContext, Bounds, WindowBounds, WindowOptions, px, size};
use serde_json::json;
use std::{
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
use zeron_ui::{openagents_account::AppAccount, *};
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
#[derive(Default)]
struct Server {
    approved: AtomicBool,
    active: AtomicBool,
}
fn main() -> anyhow::Result<()> {
    let output = PathBuf::from(
        std::env::args_os()
            .nth(1)
            .expect("pass a scratch directory"),
    );
    std::fs::create_dir_all(&output)?;
    let runtime = tokio::runtime::Runtime::new()?;
    let (origin, server)=runtime.block_on(async {
        let listener=tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin=format!("http://{}",listener.local_addr().unwrap());
        let server=Arc::new(Server::default());
        let code_origin=origin.clone();
        let router=Router::new()
            .route("/device",get(|| async {Html("<h1>Sign in to Coder Desktop</h1><p>Fixture Mac · BCDF-GHJK</p><form action='/approve' method='post'><button>Approve</button></form>")}))
            .route("/approve",post(|State(s): State<Arc<Server>>| async move {s.approved.store(true,Ordering::SeqCst); Html("<h1>Approved</h1><p>Return to Coder Desktop.</p>")}))
            .route("/v1/device/code",post(move || {let origin=code_origin.clone();async move {Json(json!({"device_code":"dvc_fixture","user_code":"BCDF-GHJK","verification_uri":format!("{origin}/device"),"verification_uri_complete":format!("{origin}/device?code=BCDF-GHJK"),"interval":1,"expires_in":120}))}}))
            .route("/v1/device/token",post(|State(s):State<Arc<Server>>|async move {if !s.approved.load(Ordering::SeqCst) {return (StatusCode::BAD_REQUEST,Json(json!({"error":"authorization_pending"})));} s.active.store(true,Ordering::SeqCst);(StatusCode::OK,Json(json!({"access_token":format!("sess_{:064x}",1),"token_type":"Bearer","expires_in":120,"account":{"id":"acct_fixture","label":"Fixture account"}})))}))
            .route("/v1/account",get(|State(s):State<Arc<Server>>|async move {if s.active.load(Ordering::SeqCst) {(StatusCode::OK,Json(json!({"account":{"id":"acct_fixture","label":"Fixture account"},"workspaces":[]})))}else{(StatusCode::UNAUTHORIZED,Json(json!({"error":"unauthorized"})))}}))
            .route("/v1/device/sign-out",post(|State(s):State<Arc<Server>>|async move {s.active.store(false,Ordering::SeqCst);Json(json!({"signed_out":true}))}))
            .with_state(server.clone());
        tokio::spawn(async move {axum::serve(listener,router).await.unwrap()});
        (origin,server)
    });
    std::fs::write(
        output.join("browser-url"),
        format!("{origin}/device?code=BCDF-GHJK"),
    )?;
    let client = Client::new(Deployment::new(&format!("{origin}/v1"), &origin)?)?;
    let store = Arc::new(Store::default());
    let controller = Controller::new(client, store.clone(), output.join("profile/signed-out"));
    let result = Arc::new(AtomicBool::new(false));
    let passed = result.clone();
    gpui_platform::application()
        .with_assets(icons::Assets)
        .run(move |cx| {
            gpui_tokio::init_from_handle(cx, runtime.handle().clone());
            gpui_base::init(cx);
            let data = output.join("workspace");
            let settings = settings::UiSettings::default();
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
            let account = AppAccount::install_fixture(controller.clone(), cx);
            let state = cx.new(|_| {
                let mut s = state::AppState::new();
                s.connection = zeron_proto::view::ConnectionStatus::Ready;
                s.workspace_scope = Some(zeron_proto::WorkspaceScope::Local);
                s.local_device_id = Some("fixture".into());
                s.auto_selected = true;
                s.chats_synced = true;
                s.spaces_synced = true;
                s
            });
            let boot = EngineBootConfig {
                data_dir: data,
                ipc_port: 0,
                edge_url: coder_brand::DISABLED_EDGE_URL.into(),
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
                            size(px(1120.), px(760.)),
                            cx,
                        ))),
                        focus: false,
                        show: false,
                        ..Default::default()
                    },
                    |_, cx| cx.new(|cx| shell::Shell::new(state, boot, cx)),
                )
                .unwrap();
            let window: gpui::AnyWindowHandle = window.into();
            account.update(cx, |a, cx| a.sign_in(cx));
            cx.spawn(async move |cx| {
                let start = Instant::now();
                let mut waiting = false;
                let mut signed_in = false;
                loop {
                    cx.background_executor()
                        .timer(Duration::from_millis(100))
                        .await;
                    let phase = controller.snapshot().phase;
                    let capture = match &phase {
                        Phase::Waiting { .. } if !waiting => {
                            waiting = true;
                            Some("waiting")
                        }
                        Phase::SignedIn { account } if !signed_in => {
                            assert_eq!(account.label, "Fixture account");
                            signed_in = true;
                            Some("signed-in")
                        }
                        Phase::SignedOut { .. }
                            if signed_in
                                && store.0.lock().unwrap().is_none()
                                && !server.active.load(Ordering::SeqCst) =>
                        {
                            Some("signed-out")
                        }
                        Phase::Failed { message, .. } => {
                            panic!("native account fixture: {message}")
                        }
                        _ => None,
                    };
                    if let Some(name) = capture {
                        window
                            .update(cx, |_, w, cx| {
                                w.draw(cx).clear();
                                w.render_to_image()
                                    .unwrap()
                                    .save(output.join(format!("{name}.png")))
                                    .unwrap();
                            })
                            .unwrap();
                        if name == "signed-in" {
                            account.update(cx, |a, cx| a.sign_out(cx));
                        } else if name == "signed-out" {
                            assert!(store.0.lock().unwrap().is_none());
                            assert!(!server.active.load(Ordering::SeqCst));
                            passed.store(true, Ordering::SeqCst);
                            cx.update(|cx| cx.quit());
                            break;
                        }
                    }
                    if start.elapsed() > Duration::from_secs(120) {
                        cx.update(|cx| cx.quit());
                        break;
                    }
                }
                drop(runtime);
            })
            .detach();
        });
    anyhow::ensure!(
        result.load(Ordering::SeqCst),
        "native account journey did not finish"
    );
    println!("Native account approval and sign-out passed.");
    Ok(())
}
