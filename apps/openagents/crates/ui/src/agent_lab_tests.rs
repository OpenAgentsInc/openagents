use super::*;
use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
};
use gpui::{WindowHandle, size};
use openagents_account::openagents::{Client, Deployment};
use serde_json::{Value, json};
use std::sync::{
    Mutex,
    atomic::{AtomicBool, Ordering},
};

#[derive(Default)]
struct Server {
    uncertain: AtomicBool,
    saves: Mutex<Vec<Value>>,
    document: Mutex<Option<Value>>,
}
async fn save(State(server): State<Arc<Server>>, Json(body): Json<Value>) -> Response {
    server.saves.lock().unwrap().push(body.clone());
    if server.uncertain.load(Ordering::SeqCst) {
        // The request arrived, but the client cannot decode its response.
        return StatusCode::OK.into_response();
    }
    let document = json!({"id":body["request_id"],"deleted":false,"versions":[{
        "document":body["request_id"],"number":1,"parent":null,"author":"acct_lab_fixture",
        "summary":body["summary"],"created_unix_ms":1,"schema":"agent-experiment.v1",
        "content":body["content"],"content_sha256":lab_kit::digest(&body["content"]),
        "report":{"warnings":[],"facts":{"valid":true,"executed":false}},"assets":[]
    }]});
    *server.document.lock().unwrap() = Some(document.clone());
    Json(document).into_response()
}
struct Fixture {
    controller: Arc<Controller>,
    server: Arc<Server>,
    root: tempfile::TempDir,
    _runtime: tokio::runtime::Runtime,
}
fn fixture(cx: &mut gpui::TestAppContext) -> Fixture {
    cx.update(|cx| {
        gpui_base::init(cx);
        gpui_tokio::init(cx);
        cx.set_global(Theme::dark());
        crate::composer::init(cx, Default::default());
    });
    let root = tempfile::tempdir().unwrap();
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let server = Arc::new(Server::default());
    let controller = runtime.block_on(async {
        let router = Router::new()
            .route("/v1/account", get(|| async { Json(json!({"account":{"id":"acct_lab_fixture","label":"Lab test"},"workspaces":[]})) }))
            .route("/v1/device/sign-out", axum::routing::post(|| async { Json(json!({"signed_out":true})) }))
            .route("/v1/labs/agent/documents", get(|State(s): State<Arc<Server>>| async move {
                Json(json!({"data":s.document.lock().unwrap().iter().cloned().collect::<Vec<_>>(),"next":null}))
            }).post(save))
            .route("/v1/labs/agent/jobs", get(|| async { Json(json!({"data":[],"next":null})) }))
            .route("/v1/agent-definitions", get(|| async { Json(json!({"data":[],"next":null})) }))
            .route("/v1/labs/agent/hosts", get(|| async { Json(json!([{
                "id":"test-host","name":"Test computer","projects":[{"id":"test-project","name":"Disposable project"}],
                "runtimes":["codex"],"runners":[],"tools":[],"seen_ms":1,"always_on":false
            }])) }))
            .with_state(server.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        tokio::spawn(async move { axum::serve(listener, router).await.unwrap(); });
        let client = Client::new(Deployment::new(&format!("{base}/v1"), &base).unwrap()).unwrap();
        let controller = Controller::new(client, Arc::new(openagents_account::desktop::MemoryStore::default()), root.path().join("account/signed-out"));
        controller.import_credential(format!("sess_{:064x}", 1)).await;
        assert!(controller.lease().is_some());
        controller
    });
    Fixture {
        controller,
        server,
        root,
        _runtime: runtime,
    }
}
fn settle(cx: &mut gpui::TestAppContext, done: impl Fn(&mut gpui::TestAppContext) -> bool) {
    cx.executor().allow_parking();
    for _ in 0..400 {
        cx.run_until_parked();
        if done(cx) {
            return;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    panic!("Agent Lab did not settle");
}
fn draw(handle: WindowHandle<AgentLab>, cx: &mut gpui::TestAppContext) {
    cx.update_window(handle.into(), |_, window, cx| window.draw(cx).clear())
        .unwrap();
}
fn ready(cx: &mut gpui::TestAppContext, f: &Fixture) -> WindowHandle<AgentLab> {
    let controller = f.controller.clone();
    let root = f.root.path().join("agent-lab");
    let handle = cx.add_window(|_, cx| AgentLab::new(controller, root, cx));
    settle(cx, |cx| {
        draw(handle, cx);
        handle
            .update(cx, |lab, _, _| {
                lab.lab
                    .as_ref()
                    .is_some_and(|l| !l.snapshot().hosts.is_empty())
            })
            .unwrap()
    });
    draw(handle, cx);
    handle
}

#[gpui::test]
fn fields_keep_edits_on_refresh_enforce_limits_and_use_editing_keys(cx: &mut gpui::TestAppContext) {
    let f = fixture(cx);
    let handle = ready(cx, &f);
    let input = handle
        .update(cx, |lab, window, cx| {
            let input = lab.inputs["instructions"].entity.clone();
            window.focus(&input.focus_handle(cx), cx);
            input.update(cx, |input, cx| input.set_text("Review changes", cx));
            input
        })
        .unwrap();
    cx.run_until_parked();
    cx.simulate_keystrokes(handle.into(), "enter");
    cx.simulate_keystrokes(handle.into(), "x");
    cx.simulate_keystrokes(handle.into(), "backspace");
    cx.run_until_parked();
    let model = handle
        .update(cx, |lab, _, cx| {
            assert_eq!(input.read(cx).text(), "Review changes\n");
            assert_eq!(
                lab.lab
                    .as_ref()
                    .unwrap()
                    .snapshot()
                    .draft
                    .draft
                    .spec
                    .instructions,
                "Review changes\n"
            );
            lab.edit("title", "Too long".repeat(20), cx);
            assert!(lab.error.is_some());
            assert_eq!(
                lab.lab.as_ref().unwrap().snapshot().draft.title,
                "New experiment"
            );
            lab.lab.as_ref().unwrap().clone()
        })
        .unwrap();
    f._runtime.block_on(model.refresh()).unwrap();
    cx.run_until_parked();
    handle
        .update(cx, |lab, _, cx| {
            assert_eq!(input, lab.inputs["instructions"].entity);
            assert_eq!(input.read(cx).text(), "Review changes\n");
        })
        .unwrap();
    let name = handle
        .update(cx, |lab, window, cx| {
            let input = lab.inputs["name"].entity.clone();
            window.focus(&input.focus_handle(cx), cx);
            input
        })
        .unwrap();
    cx.simulate_keystrokes(handle.into(), "enter tab");
    handle
        .update(cx, |lab, window, cx| {
            assert_eq!(
                name.read(cx).text(),
                lab.lab.as_ref().unwrap().snapshot().draft.draft.spec.name
            );
            assert!(!name.focus_handle(cx).is_focused(window));
        })
        .unwrap();
    for size in [size(px(760.), px(560.)), size(px(1440.), px(1000.))] {
        cx.simulate_window_resize(handle.into(), size);
        for tab in [Tab::Agent, Tab::Tests, Tab::Results] {
            handle.update(cx, |lab, _, _| lab.tab = tab).unwrap();
            draw(handle, cx);
        }
    }
    f._runtime.block_on(f.controller.sign_out());
    settle(cx, |cx| {
        handle.update(cx, |lab, _, _| lab.lab.is_none()).unwrap()
    });
    handle
        .update(cx, |lab, _, _| {
            assert!(lab.view.is_none());
            assert!(lab.inputs.is_empty());
        })
        .unwrap();
}

#[gpui::test]
fn uncertain_saves_retry_the_same_request_with_the_edited_spec(cx: &mut gpui::TestAppContext) {
    let f = fixture(cx);
    let handle = ready(cx, &f);
    f.server.uncertain.store(true, Ordering::SeqCst);
    handle
        .update(cx, |lab, _, cx| {
            lab.edit("name", "Concurrency reviewer".into(), cx);
        })
        .unwrap();
    draw(handle, cx);
    let mut visual = gpui::VisualTestContext::from_window(handle.into(), cx);
    let save = visual.debug_bounds("save").unwrap();
    visual.simulate_click(save.center(), gpui::Modifiers::default());
    settle(cx, |cx| handle.update(cx, |lab, _, _| !lab.busy).unwrap());
    handle
        .update(cx, |lab, _, _| {
            assert!(lab.lab.as_ref().unwrap().snapshot().pending);
            assert!(find(&lab.view.as_ref().unwrap().view().root, "retry").is_some());
        })
        .unwrap();
    draw(handle, cx);
    handle
        .update(cx, |lab, _, cx| {
            assert!(lab.inputs["name"].entity.read(cx).read_only)
        })
        .unwrap();
    f.server.uncertain.store(false, Ordering::SeqCst);
    let retry = visual.debug_bounds("retry").unwrap();
    visual.simulate_click(retry.center(), gpui::Modifiers::default());
    settle(cx, |cx| handle.update(cx, |lab, _, _| !lab.busy).unwrap());
    handle
        .update(cx, |lab, _, _| {
            let snapshot = lab.lab.as_ref().unwrap().snapshot();
            assert!(!snapshot.pending, "{:?}", snapshot.notice);
            assert!(!snapshot.dirty);
            assert!(find(&lab.view.as_ref().unwrap().view().root, "start").is_some());
            assert!(find(&lab.view.as_ref().unwrap().view().root, "promote").is_some());
        })
        .unwrap();
    let requests = f.server.saves.lock().unwrap();
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[0], requests[1]);
    assert_eq!(
        requests[1]["content"]["experiment"]["draft"]["spec"]["name"],
        "Concurrency reviewer"
    );
}
