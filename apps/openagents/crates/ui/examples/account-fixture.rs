//! Exercise isolated account controls or an explicitly selected test-account chat.
//! Pass an empty scratch directory. Account credentials stay in memory.
//! The continuation mode starts its own engine and uses the selected test chat.
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    response::{Html, IntoResponse, Response},
    routing::{get, post},
};
use ::openagents_account::{
    Controller, Phase,
    openagents::{Client, CredentialRecord, CredentialStore, Deployment, StorageError},
};
use gpui::{AppContext, Bounds, WindowBounds, WindowOptions, px, size};
use serde_json::json;
use std::{
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};
use openagents_ui::{openagents_account::AppAccount, *};
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
    history_status: AtomicUsize,
    revision: AtomicUsize,
    downloads: AtomicUsize,
}
fn chat_id(n: usize) -> String {
    format!("{n:08x}-1234-4234-8234-123456789abc")
}
async fn history_list(
    State(s): State<Arc<Server>>,
    Query(q): Query<std::collections::HashMap<String, String>>,
) -> Response {
    if !s.active.load(Ordering::SeqCst) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let status = s.history_status.load(Ordering::SeqCst);
    if status != 0 {
        return (
            StatusCode::from_u16(status as u16).unwrap(),
            Json(json!({"error":{"code":"unavailable"}})),
        )
            .into_response();
    }
    let limit = q["limit"].parse::<usize>().unwrap();
    assert!(limit <= 200);
    let start = q
        .get("after")
        .map(|s| usize::from_str_radix(&s[..8], 16).unwrap() + 1)
        .unwrap_or(0);
    let end = (start + limit).min(205);
    let rows:Vec<_>=(start..end).map(|n|json!({"id":chat_id(n),"title":if n==204{"Pinned saved chat"}else if n==5{"Archived saved chat"}else{"Previous chat"},"revision":1,"updated_unix":1792100000+n,"pinned_unix":if n==204{Some(1792100000)}else{None},"archived_unix":if n==5{Some(1792100000)}else{None},"project":"prj_0000000000000001","source":match n%3{0=>"web",1=>"terminal",_=>"phone"},"messages":405})).collect();
    Json(json!({"data":rows,"next":(end<205).then(||chat_id(end-1))})).into_response()
}
const FILE: &str = "0123456789abcdef0123456789abcdef";
const IMAGE: &str = "11111111111111111111111111111111";
fn fixture_image() -> Vec<u8> {
    let image = image::RgbaImage::from_pixel(128, 64, image::Rgba([40, 160, 190, 255]));
    let mut bytes = std::io::Cursor::new(Vec::new());
    image.write_to(&mut bytes, image::ImageFormat::Png).unwrap();
    bytes.into_inner()
}
async fn saved_messages(
    State(s): State<Arc<Server>>,
    Path(chat): Path<String>,
    Query(q): Query<std::collections::HashMap<String, String>>,
) -> Response {
    if !s.active.load(Ordering::SeqCst) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let revision = s.revision.load(Ordering::SeqCst);
    if q.get("revision")
        .is_some_and(|r| r.parse::<usize>().unwrap() != revision)
    {
        return (
            StatusCode::CONFLICT,
            Json(json!({"error":{"code":"changed"}})),
        )
            .into_response();
    }
    let start = q
        .get("after")
        .map(|s| s.strip_prefix("msg_").unwrap().parse::<usize>().unwrap() + 1)
        .unwrap_or(0);
    let limit = q["limit"].parse::<usize>().unwrap();
    assert!(limit <= 200);
    let end = (start + limit).min(405);
    let data:Vec<_>=(start..end).map(|n|json!({"id":format!("msg_{n}"),"role":match n{0=>"user",1=>"thinking",2=>"tool",3=>"future-role",_=>"assistant"},"text":match n{0=>"# A saved question\n\n**Markdown**, `inline code`, and a code block:\n\n```rust\nlet answer = 42;\n```".to_owned(),1=>"Considering the earlier question before answering.".into(),2=>"".into(),4=>"# Saved answer\n\n**Markdown**, `inline code`, and a code block:\n\n```rust\nlet answer = 42;\n```".into(),_=>format!("Saved message {n}")},"files":if n==0{vec![json!({"id":FILE,"name":"fixture.txt","kind":"text","size":8})]}else if n==4{vec![json!({"id":IMAGE,"name":"fixture.png","kind":"png","size":fixture_image().len()})]}else{vec![]},"tool":if n==2{Some(json!({"name":"read","summary":"Read a saved file","status":"done","input":"fixture.txt","output":"Full saved output\nNothing was executed by this reader."}))}else{None},"generated_files":if n==3{json!("unsupported saved block")}else{json!([])},"artifacts":[]})).collect();
    let n = usize::from_str_radix(&chat[..8], 16).unwrap();
    Json(json!({"thread":{"id":chat,"title":if n==204{"Pinned saved chat"}else if n==5{"Archived saved chat"}else{"Previous chat"},"revision":revision,"updated_unix":1792100000+n,"pinned_unix":if n==204{Some(1792100000)}else{None},"archived_unix":if n==5{Some(1792100000)}else{None},"project":"prj_0000000000000001","source":match n%3{0=>"web",1=>"terminal",_=>"phone"},"messages":405},"revision":revision,"data":data,"next":(end<405).then(||format!("msg_{}",end-1)),"captures":[{"id":chat_id(555),"name":"Saved capture","state":"saved","size":3,"media":"text/plain"},{"id":chat_id(556),"name":"Second capture","state":"not_saved","size":null,"media":null}]})).into_response()
}
async fn saved_file(
    State(s): State<Arc<Server>>,
    Path((_, file)): Path<(String, String)>,
) -> Response {
    if !s.active.load(Ordering::SeqCst) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    s.downloads.fetch_add(1, Ordering::SeqCst);
    if file == IMAGE {
        fixture_image()
    } else {
        b"fixture!".to_vec()
    }
    .into_response()
}
async fn saved_capture(State(s): State<Arc<Server>>) -> Response {
    if !s.active.load(Ordering::SeqCst) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    s.downloads.fetch_add(1, Ordering::SeqCst);
    b"cap".to_vec().into_response()
}
fn capture_window(
    window: gpui::AnyWindowHandle,
    cx: &mut gpui::AsyncApp,
    output: &std::path::Path,
    name: &str,
) {
    window
        .update(cx, |_, w, cx| {
            w.draw(cx).clear();
            w.render_to_image()
                .unwrap()
                .save(output.join(format!("{name}.png")))
                .unwrap();
        })
        .unwrap();
}
fn main() -> anyhow::Result<()> {
    let continuation_mode = std::env::args().any(|arg| arg == "--continue-chat");
    let chat_mode = std::env::args().any(|arg| arg == "--public-chat");
    let film_mode = std::env::args().any(|arg| arg == "--film");
    let public_mode = continuation_mode
        || chat_mode
        || film_mode
        || std::env::args().any(|arg| arg == "--public-history");
    let public_ids: Vec<String> = if public_mode {
        serde_json::from_str(&std::env::var("SDK_FIXTURE_CHAT_IDS")?)?
    } else {
        Vec::new()
    };
    let reader_mode = std::env::args().any(|arg| arg == "--reader");
    let history_mode = reader_mode || std::env::args().any(|arg| arg == "--history");
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
        let server=Arc::new(Server::default());server.revision.store(1,Ordering::SeqCst);
        let code_origin=origin.clone();
        let router=Router::new()
            .route("/device",get(|| async {Html("<h1>Sign in to OpenAgents</h1><p>Fixture Mac · BCDF-GHJK</p><form action='/approve' method='post'><button>Approve</button></form>")}))
            .route("/approve",post(|State(s): State<Arc<Server>>| async move {s.approved.store(true,Ordering::SeqCst); Html("<h1>Approved</h1><p>Return to OpenAgents.</p>")}))
            .route("/v1/device/code",post(move || {let origin=code_origin.clone();async move {Json(json!({"device_code":"dvc_fixture","user_code":"BCDF-GHJK","verification_uri":format!("{origin}/device"),"verification_uri_complete":format!("{origin}/device?code=BCDF-GHJK"),"interval":1,"expires_in":120}))}}))
            .route("/v1/device/token",post(|State(s):State<Arc<Server>>|async move {if !s.approved.load(Ordering::SeqCst) {return (StatusCode::BAD_REQUEST,Json(json!({"error":"authorization_pending"})));} s.active.store(true,Ordering::SeqCst);(StatusCode::OK,Json(json!({"access_token":format!("sess_{:064x}",1),"token_type":"Bearer","expires_in":120,"account":{"id":"acct_fixture","label":"Fixture account"}})))}))
            .route("/v1/account",get(|State(s):State<Arc<Server>>|async move {if s.active.load(Ordering::SeqCst) {(StatusCode::OK,Json(json!({"account":{"id":"acct_fixture","label":"Fixture account"},"workspaces":[]})))}else{(StatusCode::UNAUTHORIZED,Json(json!({"error":"unauthorized"})))}}))
            .route("/v1/history",get(history_list))
            .route("/v1/history/{id}/messages",get(saved_messages))
            .route("/v1/history/{id}/files/{file}",get(saved_file))
            .route("/v1/history/{id}/captures/{capture}",get(saved_capture))
            .route("/v1/history/projects",get(||async{Json(json!({"data":[{"id":"prj_0000000000000001","name":"Shared project","shared":true}],"next":null}))}))
            .route("/v1/device/sign-out",post(|State(s):State<Arc<Server>>|async move {s.active.store(false,Ordering::SeqCst);Json(json!({"signed_out":true}))}))
            .with_state(server.clone());
        tokio::spawn(async move {axum::serve(listener,router).await.unwrap()});
        (origin,server)
    });
    std::fs::write(
        output.join("browser-url"),
        format!("{origin}/device?code=BCDF-GHJK"),
    )?;
    let client = Client::new(if public_mode {
        Deployment::new(
            &std::env::var("SDK_FIXTURE_API")?,
            &std::env::var("SDK_FIXTURE_WEBSITE")?,
        )?
    } else {
        Deployment::new(&format!("{origin}/v1"), &origin)?
    })?;
    let store = Arc::new(Store::default());
    if public_mode {
        let token = openagents_brand::env_var("OPENAGENTS_TOKEN")?;
        // Only this explicit test mode accepts the dedicated account's sign-in key.
        anyhow::ensure!(token.starts_with("oa_sk_"), "use a dedicated sign-in key");
        let session = runtime.block_on(client.authenticate(&token))?;
        store.save(&session.record())?;
        anyhow::ensure!(
            public_ids.len()
                == if film_mode || chat_mode || continuation_mode {
                    1
                } else {
                    3
                },
            "pass the three disposable fixture IDs"
        );
    }
    let fixture_space = format!("continuation-fixture-{}", uuid::Uuid::new_v4());
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
                s.connection = openagents_proto::view::ConnectionStatus::Ready;
                s.workspace_scope = Some(openagents_proto::WorkspaceScope::Local);
                s.local_device_id = Some("fixture".into());
                s.auto_selected = true;
                s.chats_synced = true;
                s.spaces_synced = true;
                s
            });
            let boot = EngineBootConfig {
                data_dir: data,
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
                            size(px(1120.), px(760.)),
                            cx,
                        ))),
                        focus: false,
                        show: false,
                        ..Default::default()
                    },
                    |_, cx| cx.new(|cx| shell::Shell::new(state.clone(), boot, cx)),
                )
                .unwrap();
            let shell_window = window;
            let window: gpui::AnyWindowHandle = window.into();
            if public_mode {
                runtime.spawn(controller.restore());
            } else {
                account.update(cx, |a, cx| a.sign_in(cx));
            }
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
                            if !public_mode {
                                assert_eq!(account.label, "Fixture account");
                            }
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
                            if public_mode {
                                let core = if continuation_mode {
                                    let account_id = controller.lease().unwrap().session().account().id.clone();
                                    let edge = std::env::var("SDK_FIXTURE_EDGE").unwrap();
                                    let config = openagents_engine::AuthConfig::new(&edge, output.join("engine"));
                                    let auth = openagents_engine::Auth::with_account(config, controller.clone());
                                    let registry = Arc::new(openagents_engine::default_registry());
                                    registry.register(Arc::new(openagents_harness::ClaudeHarness::new().with_executable(std::env::var("SDK_FIXTURE_CLAUDE").unwrap())));
                                    let core = runtime.block_on(async {
                                        let core = openagents_engine::EngineCore::assemble_with_identity(
                                            &output.join("engine"), registry, HarnessId::ClaudeCode,
                                            Some(openagents_engine::doc_host::EdgeConfig::new(&edge, Arc::new(auth.clone()))),
                                            &account_id, &account_id).unwrap();
                                        core.set_auth(auth);
                                        Arc::new(core)
                                    });
                                    #[cfg(unix)] {
                                    let shutdown = core.clone();
                                    runtime.spawn(async move {
                                        let mut term = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()).unwrap();
                                        term.recv().await;
                                        shutdown.shutdown().await;
                                        std::process::exit(143);
                                    });
                                    }
                                    std::fs::write(output.join("fixture-space"), &fixture_space).unwrap();
                                    std::fs::write(output.join("fixture-device"), &core.device_id).unwrap();
                                    let client = { let _entered = runtime.enter(); openagents_rpc::memory_client(core.rpc_service()) };
                                    let info = openagents_proto::EngineInfo {
                                        device_id: core.device_id.clone(), workspace_scope: core.workspace_scope(),
                                        cursor_sdk_version: None, capabilities: openagents_proto::capabilities::current(),
                                    };
                                    state.update(cx, |s, cx| s.fixture_account_engine(state::EngineHandle::fixture_memory(client, info), cx));
                                    let folder = output.join("chosen-project");
                                    std::fs::create_dir_all(&folder).unwrap();
                                    core.workspace.create_space(&fixture_space, &core.device_id, folder.to_str().unwrap(), Some("Continuation test".into()), false).unwrap();
                                    Some(core)
                                } else { None };
                                let panel = shell_window
                                    .update(cx, |shell, _, _| shell.fixture_saved_history().0)
                                    .unwrap()
                                    .unwrap();
                                let model = panel.update(cx, |panel, _| panel.model.clone());
                                let deadline = Instant::now() + Duration::from_secs(30);
                                while !public_ids.iter().all(|id| model.existing(id).is_some()) {
                                    assert!(
                                        Instant::now() < deadline,
                                        "deployed sidebar fixtures did not load"
                                    );
                                    cx.background_executor()
                                        .timer(Duration::from_millis(100))
                                        .await;
                                }
                                capture_window(window, cx, &output, "public-sidebar");
                                for (n, id) in public_ids.iter().enumerate() {
                                    let target = model.existing(id).unwrap();
                                    shell_window
                                        .update(cx, |shell, _, cx| {
                                            shell.open_saved_chat(target, cx)
                                        })
                                        .unwrap();
                                    let viewer = shell_window
                                        .update(cx, |shell, _, _| shell.fixture_saved_reader())
                                        .unwrap()
                                        .unwrap();
                                    let reader =
                                        viewer.update(cx, |viewer, _| viewer.model.clone());
                                    let deadline = Instant::now() + Duration::from_secs(30);
                                    while reader.snapshot().state
                                        == ::openagents_account::history::LoadState::Loading
                                    {
                                        assert!(Instant::now() < deadline);
                                        cx.background_executor()
                                            .timer(Duration::from_millis(100))
                                            .await;
                                    }
                                    if continuation_mode {
                                        let core = core.as_ref().unwrap();
                                        viewer.update(cx, |v, cx| v.fixture_continue(cx));
                                        cx.background_executor().timer(Duration::from_millis(400)).await;
                                        let deadline = Instant::now()+Duration::from_secs(30);
                                        while !state.read_with(cx, |s,_| s.spaces.iter().any(|p|p.id==fixture_space)) {
                                            assert!(Instant::now()<deadline, "local project did not load");
                                            cx.background_executor().timer(Duration::from_millis(100)).await;
                                        }
                                        state.update(cx, |s,cx| s.select_space(Some(fixture_space.clone()),cx));
                                        let composer = shell_window.update(cx,|s,_,_|s.fixture_continuation_composer()).unwrap();
                                        composer.update(cx,|c,cx|c.fixture_continuation_send(std::env::var("SDK_CHAT_PROMPT").unwrap(),cx));
                                        let expected=std::env::var("SDK_CHAT_EXPECTED").unwrap();
                                        let deadline=Instant::now()+Duration::from_secs(150);
                                        let chat=loop {
                                            if let Some(chat)=state.read_with(cx,|s,_|s.selected_chat.clone()) {
                                                std::fs::write(output.join("pending-chat"), &chat).unwrap();
                                                let doc={ let _entered=runtime.enter(); core.doc_host.open(&chat).unwrap() };
                                                let entries=doc.doc().read_entries().unwrap();
                                                if entries.iter().any(|e|e.role==openagents_doc::MessageRole::Assistant && e.parts.iter().any(|p|matches!(p,openagents_doc::MessagePart::Text{text,..} if text.to_lowercase().contains(&expected.to_lowercase())))) { break chat; }
                                            }
                                            assert!(Instant::now()<deadline, "Claude continuation did not answer with earlier context");
                                            cx.background_executor().timer(Duration::from_millis(200)).await;
                                        };
                                        let doc={ let _entered=runtime.enter(); core.doc_host.open(&chat).unwrap() };
                                        assert!(openagents_doc::continuation::get(doc.doc()).is_some());
                                        capture_window(window,cx,&output,"continued-on-computer");
                                        std::fs::write(output.join("continued-chat"), &chat).unwrap();
                                        // An optional phone probe can observe and steer this disposable session.
                                        if std::env::var_os("SDK_FIXTURE_WAIT_PHONE").is_some() {
                                            let relay = {
                                                let _entered = runtime.enter();
                                                core.start_host_relay(&std::env::var("SDK_FIXTURE_EDGE").unwrap())
                                            };
                                            let deadline=Instant::now()+Duration::from_secs(240);
                                            while !output.join("phone-done").exists() {
                                                assert!(Instant::now()<deadline,"phone probe timed out");
                                                cx.background_executor().timer(Duration::from_millis(200)).await;
                                            }
                                            let doc = { let _entered = runtime.enter(); core.doc_host.open(&chat).unwrap() };
                                            let commands = doc.doc().read_commands().unwrap();
                                            assert!(commands.iter().any(|c| matches!(c.payload, openagents_doc::SessionCommandPayload::Steer { .. }) && c.status == openagents_doc::SessionCommandStatus::Applied), "Phone steer was not applied by the host");
                                            let text = serde_json::to_string(&doc.doc().read_entries().unwrap()).unwrap();
                                            assert!(text.contains("Stop listing plants and repeat only the fictional flower name"));
                                            std::fs::write(output.join("phone-steer-confirmed"), b"applied").unwrap();
                                            drop(relay);
                                        }
                                        core.workspace.delete_space(&fixture_space).unwrap();
                                        cx.background_executor().timer(Duration::from_secs(2)).await;
                                        runtime.spawn({let core=core.clone();async move {core.shutdown().await}}).await.unwrap();
                                        continue;
                                    }
                                    if chat_mode {
                                        let prompt=std::env::var("SDK_CHAT_PROMPT").unwrap();
                                        let expected=std::env::var("SDK_CHAT_EXPECTED").unwrap();
                                        assert!(!expected.is_empty());
                                        viewer.update(cx,|viewer,cx|viewer.fixture_reply(prompt.clone(),cx));
                                        let deadline=Instant::now()+Duration::from_secs(100);
                                        loop {
                                            let turn=viewer.update(cx,|v,_|v.fixture_turn());
                                            if !turn.busy && (turn.accepted>0 || turn.error.is_some()) {
                                                assert!(turn.error.is_none(),"cloud send failed: {:?}",turn.error);
                                                break;
                                            }
                                            assert!(Instant::now()<deadline,"cloud reply timed out");
                                            cx.background_executor().timer(Duration::from_millis(100)).await;
                                        }
                                        let snapshot=reader.snapshot();
                                        assert!(snapshot.messages.iter().any(|m|m.role==::openagents_account::openagents::MessageRole::User && m.text==prompt));
                                        assert!(snapshot.messages.iter().rev().find(|m|m.role==::openagents_account::openagents::MessageRole::Assistant).unwrap().text.contains(&expected),"cloud reply lost earlier context");
                                        cx.background_executor().timer(Duration::from_millis(300)).await;
                                        capture_window(window,cx,&output,"public-cloud-reply");
                                        let before=reader.snapshot().messages.len();
                                        std::fs::write(output.join("reply-done"),b"ready").unwrap();
                                        let web_prompt=std::env::var("SDK_WEB_PROMPT").unwrap();
                                        let deadline=Instant::now()+Duration::from_secs(100);
                                        loop {
                                            let snapshot=reader.snapshot();
                                            if snapshot.messages.len()>before && snapshot.messages.iter().any(|m|m.role==::openagents_account::openagents::MessageRole::User && m.text==web_prompt) && snapshot.messages.iter().rev().find(|m|m.role==::openagents_account::openagents::MessageRole::Assistant).is_some_and(|m|m.text.contains(&expected)){break;}
                                            assert!(Instant::now()<deadline,"web reply did not appear on desktop");
                                            cx.background_executor().timer(Duration::from_millis(100)).await;
                                        }
                                        cx.background_executor().timer(Duration::from_millis(300)).await;
                                        capture_window(window,cx,&output,"public-cloud-from-web");
                                        std::fs::write(output.join("web-seen"),b"ready").unwrap();
                                        continue;
                                    }
                                    if film_mode {
                                        let frames = output.join("frames");
                                        std::fs::create_dir_all(&frames).unwrap();
                                        std::fs::write(output.join("ready"), id).unwrap();
                                        let mut stamps = Vec::new();
                                        let deadline = Instant::now() + Duration::from_secs(180);
                                        while !output.join("stop").exists() {
                                            assert!(Instant::now() < deadline, "film capture exceeded three minutes");
                                            cx.background_executor().timer(Duration::from_millis(100)).await;
                                            let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs_f64();
                                            let snapshot = reader.snapshot();
                                            let n = stamps.len();
                                            capture_window(window, cx, &frames, &format!("{n:06}"));
                                            stamps.push(json!({"frame":format!("{n:06}.png"),"epoch":t,
                                                "chat":id,"revision":snapshot.summary.as_ref().map(|s|s.revision),
                                                "messages":snapshot.messages.iter().map(|m|json!({"id":m.id,"role":m.role.as_str(),"text":m.text})).collect::<Vec<_>>()}));
                                            std::fs::write(output.join("frames.json"), serde_json::to_vec(&stamps).unwrap()).unwrap();
                                        }
                                        reader.clear();
                                        break;
                                    }
                                    assert_eq!(reader.snapshot().messages.len(), 200);
                                    runtime.spawn(reader.more()).await.unwrap();
                                    let snapshot = reader.snapshot();
                                    assert_eq!(snapshot.messages.len(), 205);
                                    assert_eq!(snapshot.messages.last().unwrap().id, "msg_204");
                                    assert!(!snapshot.has_more);
                                    assert_eq!(
                                        snapshot.messages[1].tool.as_ref().unwrap().output,
                                        "Saved fixture output"
                                    );
                                    assert_eq!(snapshot.messages[2].role.as_str(), "thinking");
                                    viewer.update(cx, |viewer, cx| viewer.reveal_message(0, cx));
                                    cx.background_executor().timer(Duration::from_millis(300)).await;
                                    capture_window(
                                        window,
                                        cx,
                                        &output,
                                        &format!("public-reader-{n}"),
                                    );
                                }
                                account.update(cx, |a, cx| a.sign_out(cx));
                                continue;
                            }
                            if history_mode {
                                let panel = shell_window
                                    .update(cx, |shell, _, _| shell.fixture_saved_history().0)
                                    .unwrap()
                                    .unwrap();
                                let model = panel.update(cx, |panel, _| panel.model.clone());
                                let deadline = Instant::now() + Duration::from_secs(10);
                                while model.snapshot().rows.len() != 200
                                    || model.snapshot().projects.is_empty()
                                {
                                    assert!(
                                        Instant::now() < deadline,
                                        "first sidebar page did not load"
                                    );
                                    cx.background_executor()
                                        .timer(Duration::from_millis(50))
                                        .await;
                                }
                                capture_window(window, cx, &output, "sidebar-200");
                                if reader_mode {
                                    assert!(model.existing(&chat_id(204)).is_none());
                                    let lease = controller.lease().unwrap();
                                    let chat = lease
                                        .client()
                                        .chat(lease.session(), &chat_id(204))
                                        .unwrap();
                                    let target = ::openagents_account::history::ExistingChat {
                                        chat: chat.clone(),
                                        lease,
                                        summary: ::openagents_account::openagents::ChatSummary {
                                            home: Default::default(),
                                            links: Default::default(),
                                            chat,
                                            title: "Saved chat".into(),
                                            revision: 1,
                                            updated_unix: 0,
                                            pinned_unix: None,
                                            archived_unix: None,
                                            project: None,
                                            source: ::openagents_account::openagents::ChatSource::Other(
                                                "unknown".into(),
                                            ),
                                            messages: 405,
                                        },
                                    };
                                    shell_window
                                        .update(cx, |shell, _, cx| {
                                            shell.open_saved_chat(target, cx)
                                        })
                                        .unwrap();
                                    let viewer = shell_window
                                        .update(cx, |shell, _, _| shell.fixture_saved_reader())
                                        .unwrap()
                                        .unwrap();
                                    let reader =
                                        viewer.update(cx, |viewer, _| viewer.model.clone());
                                    let deadline = Instant::now() + Duration::from_secs(10);
                                    while reader.snapshot().state
                                        == ::openagents_account::history::LoadState::Loading
                                    {
                                        assert!(Instant::now() < deadline);
                                        cx.background_executor()
                                            .timer(Duration::from_millis(50))
                                            .await;
                                    }
                                    assert_eq!(reader.snapshot().messages.len(), 200);
                                    assert_eq!(server.downloads.load(Ordering::SeqCst), 0);
                                    cx.background_executor()
                                        .timer(Duration::from_millis(100))
                                        .await;
                                    capture_window(window, cx, &output, "reader-200");
                                    runtime.spawn(reader.more()).await.unwrap();
                                    runtime.spawn(reader.more()).await.unwrap();
                                    assert_eq!(reader.snapshot().messages.len(), 405);
                                    assert_eq!(
                                        reader.snapshot().messages.last().unwrap().id,
                                        "msg_404"
                                    );
                                    assert!(!reader.snapshot().has_more);
                                    cx.background_executor()
                                        .timer(Duration::from_millis(100))
                                        .await;
                                    viewer.update(cx, |viewer, cx| viewer.reveal_message(404, cx));
                                    cx.background_executor()
                                        .timer(Duration::from_millis(100))
                                        .await;
                                    capture_window(window, cx, &output, "reader-405");
                                    let reference = reader.snapshot().messages[0].files[0]
                                        .download_ref()
                                        .unwrap()
                                        .clone();
                                    viewer.update(cx, |viewer, cx| {
                                        viewer.download(reference, "fixture.txt".into(), cx)
                                    });
                                    while matches!(
                                        reader.snapshot().download.as_ref().map(|d| &d.state),
                                        Some(::openagents_account::history::LoadState::Loading)
                                    ) {
                                        cx.background_executor()
                                            .timer(Duration::from_millis(50))
                                            .await;
                                    }
                                    assert_eq!(
                                        &**reader.snapshot().download.unwrap().bytes.unwrap(),
                                        b"fixture!"
                                    );
                                    assert_eq!(server.downloads.load(Ordering::SeqCst), 1);
                                    cx.background_executor()
                                        .timer(Duration::from_millis(100))
                                        .await;
                                    capture_window(window, cx, &output, "reader-file");
                                    assert!(reader.snapshot().captures[1].download_ref().is_none());
                                    let reference = reader.snapshot().captures[0]
                                        .download_ref()
                                        .unwrap()
                                        .clone();
                                    runtime
                                        .spawn(reader.download(reference, "Saved capture".into()))
                                        .await
                                        .unwrap();
                                    assert_eq!(
                                        &**reader.snapshot().download.unwrap().bytes.unwrap(),
                                        b"cap"
                                    );
                                    assert_eq!(server.downloads.load(Ordering::SeqCst), 2);
                                    reader.close_download();
                                    viewer.update(cx, |viewer, cx| viewer.reveal_message(0, cx));
                                    cx.background_executor().timer(Duration::from_millis(800)).await;
                                    capture_window(window, cx, &output, "normal-transcript");
                                    viewer.update(cx, |viewer, cx| viewer.reveal_message(5, cx));
                                    cx.background_executor().timer(Duration::from_millis(250)).await;
                                    capture_window(window, cx, &output, "normal-transcript-answer");
                                    reader.refresh().unwrap();
                                    runtime.spawn(reader.more()).await.unwrap();
                                    server.revision.store(2, Ordering::SeqCst);
                                    runtime.spawn(reader.more()).await.unwrap();
                                    assert!(reader.snapshot().reload_required);
                                    assert!(reader.snapshot().messages.is_empty());
                                    cx.background_executor()
                                        .timer(Duration::from_millis(100))
                                        .await;
                                    capture_window(window, cx, &output, "reader-changed");
                                    viewer.update(cx, |viewer, cx| viewer.refresh(cx));
                                    while reader.snapshot().state
                                        == ::openagents_account::history::LoadState::Loading
                                    {
                                        cx.background_executor()
                                            .timer(Duration::from_millis(50))
                                            .await;
                                    }
                                    assert_eq!(reader.snapshot().messages.len(), 200);
                                    assert_eq!(reader.snapshot().summary.unwrap().revision, 2);
                                    cx.background_executor()
                                        .timer(Duration::from_millis(100))
                                        .await;
                                    capture_window(window, cx, &output, "reader-reloaded");
                                    state.update(cx, |state, _| {
                                        assert!(state.selected_chat.is_none());
                                        assert!(state.chats.is_empty());
                                        assert!(state.engine().is_none());
                                    });
                                }

                                runtime.spawn(model.more()).await.unwrap();
                                cx.background_executor()
                                    .timer(Duration::from_millis(100))
                                    .await;
                                assert_eq!(model.snapshot().rows.len(), 205);
                                assert!(!model.snapshot().has_more);
                                assert_eq!(model.snapshot().rows[0].chat.id(), chat_id(204));
                                capture_window(window, cx, &output, "sidebar-205-pinned");
                                panel.update(cx, |panel, cx| panel.open(&chat_id(204), cx));
                                cx.background_executor()
                                    .timer(Duration::from_millis(100))
                                    .await;
                                let route = shell_window
                                    .update(cx, |shell, _, _| shell.fixture_saved_history().1)
                                    .unwrap();
                                assert_eq!(route, shell::Route::SavedHistory);
                                state.update(cx, |state, _| {
                                    assert!(state.selected_chat.is_none());
                                    assert!(state.chats.is_empty());
                                    assert!(state.engine().is_none());
                                });
                                capture_window(window, cx, &output, "selected-existing");
                                panel.update(cx, |panel, cx| panel.toggle_archived(cx));
                                panel.update(cx, |panel, cx| panel.open(&chat_id(5), cx));
                                assert!(
                                    model
                                        .existing(&chat_id(5))
                                        .unwrap()
                                        .summary
                                        .archived_unix
                                        .is_some()
                                );
                                cx.background_executor()
                                    .timer(Duration::from_millis(100))
                                    .await;
                                capture_window(window, cx, &output, "archived");
                                panel.update(cx, |panel, cx| panel.toggle_archived(cx));
                                let project = model.snapshot().projects[0].clone();
                                panel.update(cx, |panel, cx| panel.project(Some(&project), cx));
                                while model.snapshot().state
                                    == ::openagents_account::history::LoadState::Loading
                                {
                                    cx.background_executor()
                                        .timer(Duration::from_millis(50))
                                        .await;
                                }
                                assert_eq!(model.snapshot().rows.len(), 200);
                                cx.background_executor()
                                    .timer(Duration::from_millis(100))
                                    .await;
                                capture_window(window, cx, &output, "shared-project");
                                server.history_status.store(503, Ordering::SeqCst);
                                model.refresh().unwrap();
                                runtime.spawn(model.more()).await.unwrap();
                                cx.background_executor()
                                    .timer(Duration::from_millis(100))
                                    .await;
                                assert!(matches!(
                                    model.snapshot().state,
                                    ::openagents_account::history::LoadState::Failed { stale: true, .. }
                                ));
                                capture_window(window, cx, &output, "offline-retry");
                                server.history_status.store(0, Ordering::SeqCst);
                                runtime.spawn(model.more()).await.unwrap();
                                assert_eq!(
                                    model.snapshot().state,
                                    ::openagents_account::history::LoadState::Ready
                                );
                            }
                            account.update(cx, |a, cx| a.sign_out(cx));
                        } else if name == "signed-out" {
                            assert!(store.0.lock().unwrap().is_none());
                            assert!(!server.active.load(Ordering::SeqCst));
                            if reader_mode || public_mode {
                                let viewer = shell_window
                                    .update(cx, |shell, _, _| shell.fixture_saved_reader())
                                    .unwrap()
                                    .unwrap();
                                viewer.update(cx, |viewer, _| {
                                    assert!(viewer.model.snapshot().messages.is_empty());
                                    assert!(viewer.model.snapshot().download.is_none());
                                });
                            }
                            if history_mode || public_mode {
                                let (panel, route) = shell_window
                                    .update(cx, |shell, _, _| shell.fixture_saved_history())
                                    .unwrap();
                                assert_ne!(route, shell::Route::SavedHistory);
                                panel.unwrap().update(cx, |panel, _| {
                                    assert!(panel.model.snapshot().rows.is_empty());
                                    assert!(panel.selected.is_none());
                                });
                            }
                            passed.store(true, Ordering::SeqCst);
                            cx.update(|cx| cx.quit());
                            break;
                        }
                    }
                    if start.elapsed() > Duration::from_secs(if continuation_mode { 450 } else { 120 }) {
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
    println!("Native account journey and sign-out passed (public history: {public_mode}).");
    Ok(())
}
