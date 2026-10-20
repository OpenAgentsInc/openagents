use super::*;
use openagents_rpc::BrowserGate as _;
use openagents_rpc::browsers::RequestState;
use secp256k1::SecretKey;
use serde_json::json;

/// The engine a browser reaches once allowed: two reads and a write.
struct Engine;
#[async_trait]
impl RpcService for Engine {
    async fn handle(&self, method: &str, _: serde_json::Value) -> Result<RpcReply, RpcError> {
        match method {
            methods::LIST_HARNESSES => RpcReply::value(&json!([{"id": "claude"}])),
            methods::LIST_WORKSPACE_DIRECTORY => RpcReply::value(&json!({"entries": ["src"]})),
            other => RpcReply::value(&json!({ "ran": other })),
        }
    }
}

fn value(reply: Result<RpcReply, RpcError>) -> serde_json::Value {
    match reply.unwrap() {
        RpcReply::Value(value) => value,
        RpcReply::Stream(_) => panic!("a value"),
    }
}

fn failed(reply: Result<RpcReply, RpcError>) -> String {
    match reply {
        Err(RpcError::Failed(message)) => message,
        other => panic!("expected a refusal, got {:?}", other.map(|_| ())),
    }
}

async fn host(temp: &std::path::Path) -> coder_host::Running {
    let mut config =
        coder_host::Config::new(temp.join("access"), vec!["ws://127.0.0.1:9/".into()], 1);
    config.policy = coder_access::RelayPolicy::LoopbackTest;
    config.iroh = Some(coder_host::config::Iroh::loopback());
    config.label = "Studio Mac".into();
    config.telemetry = false;
    config.advertise_listener = false;
    config.keys = Some(coder_host::serve::keys::Keys(Arc::new(
        coder_host::serve::keys::FileKeySource::new(temp.join("keys")),
    )));
    coder_host::start(config, Arc::new(coder_host::NoTasks))
        .await
        .unwrap()
}

fn public(key: &SecretKey) -> String {
    nostr::domain::RelaySigner::from_secret_hex(&key.display_secret().to_string())
        .unwrap()
        .pubkey()
        .to_owned()
}

fn hello(key: &SecretKey, conn: &str) -> serde_json::Value {
    let event = coder_access::browser::sign_hello(key, "mac-1", conn, now()).unwrap();
    json!({ "event": event })
}

fn ask(key: &SecretKey) -> serde_json::Value {
    json!({ "key": public(key), "label": "Chrome on macOS" })
}

/// #12264: a browser asks, the person at the computer allows it, and its
/// hello opens only the allowed reads until the grant is removed.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_browser_reaches_only_allowed_reads_after_the_computer_allows_it() {
    let temp = tempfile::tempdir().unwrap();
    let browsers = Browsers::new("mac-1", Arc::new(Engine));
    let gate = Gate(browsers.clone());
    let tab = gate.service("web-tab1");
    let key = SecretKey::new(&mut secp256k1::rand::rng());

    // Before the device connection runs, nothing.
    assert_eq!(
        failed(tab.handle(BROWSER_REQUEST, ask(&key)).await),
        UNAVAILABLE
    );
    let running = host(temp.path()).await;
    browsers.attach(&running);

    // Nothing reaches the engine before a hello.
    assert_eq!(
        failed(tab.handle(methods::LIST_HARNESSES, json!({})).await),
        ALLOW_FIRST
    );
    let asked: RequestState =
        serde_json::from_value(value(tab.handle(BROWSER_REQUEST, ask(&key)).await)).unwrap();
    assert_eq!(asked.state, State::Asked);
    // Asking again names the same request.
    let again: RequestState =
        serde_json::from_value(value(tab.handle(BROWSER_REQUEST, ask(&key)).await)).unwrap();
    assert_eq!(again.request, asked.request);
    // A hello without a grant is refused.
    assert_eq!(
        failed(tab.handle(BROWSER_HELLO, hello(&key, "web-tab1")).await),
        ALLOW_FIRST
    );

    // The person at the computer sees it and allows it.
    let waiting = browsers.waiting();
    assert_eq!(waiting.len(), 1);
    assert_eq!(waiting[0].label, "Chrome on macOS");
    assert_eq!(waiting[0].key, public(&key));
    browsers.answer(&asked.request, true).unwrap();
    assert!(browsers.waiting().is_empty());
    let status: RequestState = serde_json::from_value(value(
        tab.handle(BROWSER_REQUEST_STATUS, json!({"request": asked.request}))
            .await,
    ))
    .unwrap();
    assert_eq!(status.state, State::Allowed);

    // A hello signed for another connection opens nothing here.
    assert_eq!(
        failed(tab.handle(BROWSER_HELLO, hello(&key, "web-tab2")).await),
        ALLOW_FIRST
    );
    let opened: Hello = serde_json::from_value(value(
        tab.handle(BROWSER_HELLO, hello(&key, "web-tab1")).await,
    ))
    .unwrap();
    assert_eq!(opened.rights, ["observe", "operate"]);
    assert!(opened.expires_at <= now() + coder_access::protocol::BROWSER_GRANT_LIFETIME);

    // The allowed reads answer; anything else is refused.
    assert_eq!(
        value(tab.handle(methods::LIST_HARNESSES, json!({})).await),
        json!([{"id": "claude"}])
    );
    assert_eq!(
        value(
            tab.handle(methods::LIST_WORKSPACE_DIRECTORY, json!({"path": "/"}))
                .await
        ),
        json!({"entries": ["src"]})
    );
    // The run right covers the queue, answers, and file edits (#12268, #12272).
    for method in [
        methods::QUEUE_MESSAGE,
        methods::QUEUE_COMMAND,
        methods::WRITE_WORKSPACE_FILE,
    ] {
        assert_eq!(
            value(tab.handle(method, json!({})).await),
            json!({ "ran": method })
        );
    }
    for method in ["OpenTerminal", methods::MUTATE, "StopEngine"] {
        assert!(
            failed(tab.handle(method, json!({})).await).contains("can't do that"),
            "{method}"
        );
    }
    // Another tab of the same browser needs its own hello.
    let other = gate.service("web-tab2");
    assert_eq!(
        failed(other.handle(methods::LIST_HARNESSES, json!({})).await),
        ALLOW_FIRST
    );

    // The computer's app lists it, and removing it stops the next call.
    let allowed = tokio::task::spawn_blocking({
        let browsers = browsers.clone();
        move || browsers.allowed()
    })
    .await
    .unwrap();
    assert_eq!(allowed.len(), 1);
    assert_eq!(allowed[0].label, "Chrome on macOS");
    assert!(allowed[0].last_seen.is_some());
    tokio::task::spawn_blocking({
        let browsers = browsers.clone();
        let key = public(&key);
        move || browsers.revoke(&key)
    })
    .await
    .unwrap()
    .unwrap();
    assert_eq!(
        failed(tab.handle(methods::LIST_HARNESSES, json!({})).await),
        ENDED
    );
    assert_eq!(
        failed(tab.handle(methods::LIST_HARNESSES, json!({})).await),
        ALLOW_FIRST
    );
    assert_eq!(
        failed(tab.handle(BROWSER_HELLO, hello(&key, "web-tab1")).await),
        ALLOW_FIRST
    );
}

/// A denied request stays denied, and a browser can give its own access
/// back.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_denied_browser_gets_nothing_and_an_allowed_one_can_forget() {
    let temp = tempfile::tempdir().unwrap();
    let running = host(temp.path()).await;
    let browsers = Browsers::new("mac-1", Arc::new(Engine));
    browsers.attach(&running);
    let gate = Gate(browsers.clone());
    let tab = gate.service("web-a");

    let denied = SecretKey::new(&mut secp256k1::rand::rng());
    let asked: RequestState =
        serde_json::from_value(value(tab.handle(BROWSER_REQUEST, ask(&denied)).await)).unwrap();
    browsers.answer(&asked.request, false).unwrap();
    let status = browsers.status(&asked.request);
    assert_eq!(status.state, State::Denied);
    assert!(browsers.answer(&asked.request, true).is_err());
    assert_eq!(
        failed(tab.handle(BROWSER_HELLO, hello(&denied, "web-a")).await),
        ALLOW_FIRST
    );
    assert_eq!(browsers.status("unknown").state, State::Expired);

    let key = SecretKey::new(&mut secp256k1::rand::rng());
    let asked: RequestState =
        serde_json::from_value(value(tab.handle(BROWSER_REQUEST, ask(&key)).await)).unwrap();
    browsers.answer(&asked.request, true).unwrap();
    value(tab.handle(BROWSER_HELLO, hello(&key, "web-a")).await);
    value(tab.handle(BROWSER_FORGET, json!({})).await);
    let next = gate.service("web-b");
    assert_eq!(
        failed(next.handle(BROWSER_HELLO, hello(&key, "web-b")).await),
        ALLOW_FIRST
    );

    // Bad keys and too many waiting requests are refused.
    assert!(matches!(
        tab.handle(BROWSER_REQUEST, json!({"key": "nope"})).await,
        Err(RpcError::BadParams(_))
    ));
    for _ in 0..MAX_WAITING {
        let key = SecretKey::new(&mut secp256k1::rand::rng());
        value(tab.handle(BROWSER_REQUEST, ask(&key)).await);
    }
    let key = SecretKey::new(&mut secp256k1::rand::rng());
    assert!(failed(tab.handle(BROWSER_REQUEST, ask(&key)).await).contains("Too many"));
}

/// Every method a browser may call names one right, and the browser grant
/// holds each of them; nothing else is callable.
#[test]
fn each_browser_method_needs_a_right_the_grant_holds() {
    for method in ALLOWED {
        assert_eq!(required_right(method), Some(Right::Observe), "{method}");
        assert!(!OPERATE.contains(method), "{method} is in both lists");
    }
    for method in OPERATE {
        assert_eq!(required_right(method), Some(Right::Operate), "{method}");
    }
    for right in [Right::Observe, Right::Operate] {
        assert!(Rights::browser().contains(right));
    }
    for method in [
        methods::OPEN_TERMINAL,
        methods::MUTATE,
        methods::STOP_ENGINE,
    ] {
        assert_eq!(required_right(method), None, "{method}");
    }
}
