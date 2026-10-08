//! Native workbench navigation acceptance with isolated account and host fixtures.

use super::*;
use ::workbench as contract;

const PAGE: &str = "/cloud/app/hosts/resident/workbench";
const BROWSER_ROUTE: &str = "wss://terminal.example.invalid/";

fn decoded_pre(html: &str, id: &str) -> Value {
    let value = html
        .split_once(&format!("<pre id=\"{id}\" hidden>"))
        .unwrap()
        .1
        .split("</pre>")
        .next()
        .unwrap()
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&");
    serde_json::from_str(&value).unwrap()
}

fn assets(fixture: &Fixture) {
    let build = fixture.config.cloud_build.as_ref().unwrap();
    // These route tests inspect navigation only. The browser check uses real Wasm.
    std::fs::write(build.join("coder_browser_web.js"), "synthetic route asset").unwrap();
    std::fs::write(
        build.join("coder_browser_web_bg.wasm"),
        b"synthetic route asset",
    )
    .unwrap();
}

fn browser(fixture: &mut Fixture, native: &Resident, route: Option<&str>) {
    let mut document: Value =
        serde_json::from_slice(&std::fs::read(&native.config).unwrap()).unwrap();
    document["bindings"][0]["browser"] = json!({
        "route":route,
        "capabilities":coder_pty::ext::Features::ALL.capabilities()
    });
    private_file(&native.config, &serde_json::to_vec(&document).unwrap());
    fixture.config.cloud_hosts = Some(Arc::new(
        super::super::hosts::Hosts::load(&native.config).unwrap(),
    ));
    fixture.site = crate::router(fixture.config.clone());
}

async fn connected() -> (Fixture, Resident, Cookies) {
    let mut fixture = fixture().await;
    let native = resident(&mut fixture).await;
    assets(&fixture);
    browser(&mut fixture, &native, Some(BROWSER_ROUTE));
    let mut cookies = login(&fixture, "alice").await;
    choose_personal(&fixture, &mut cookies).await;
    (fixture, native, cookies)
}

async fn get(fixture: &Fixture, cookies: &Cookies, route: &str) -> Answer {
    request(&fixture.site, Method::GET, route, cookies, None, None).await
}

fn private_navigation(answer: &Answer) {
    private(answer);
    for value in [
        CANARY,
        "Original private request",
        "synthetic-credential",
        "sess_",
        "resident-device.key",
    ] {
        assert!(!answer.body.contains(value), "Navigation disclosed {value}");
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn browser_navigation_requires_explicit_config_assets_and_current_native_observation() {
    let mut fixture = fixture().await;
    let native = resident(&mut fixture).await;
    let mut cookies = login(&fixture, "alice").await;
    choose_personal(&fixture, &mut cookies).await;
    let index = get(&fixture, &cookies, "/cloud/app/workbench").await;
    assert_eq!(index.status, StatusCode::OK);
    assert!(
        index
            .body
            .contains("No qualified browser terminal connection")
    );
    assert!(!index.body.contains("cloud-workbench-config"));
    let disabled = get(&fixture, &cookies, PAGE).await;
    assert_eq!(disabled.status, StatusCode::SERVICE_UNAVAILABLE);
    assets(&fixture);
    assert_eq!(
        get(&fixture, &cookies, PAGE).await.status,
        StatusCode::SERVICE_UNAVAILABLE
    );
    browser(&mut fixture, &native, Some(BROWSER_ROUTE));
    let page = get(&fixture, &cookies, PAGE).await;
    assert_eq!(page.status, StatusCode::OK, "{}", page.body);
    private_navigation(&page);
    assert_eq!(native.running.as_ref().unwrap().terminals(), 0);
    let config = decoded_pre(&page.body, "cloud-workbench-config");
    assert_eq!(config["generation"], 7);
    assert_eq!(config["workspace"], "checkout");
    assert_eq!(config["route"], BROWSER_ROUTE);
    assert!(
        config["capabilities"]
            .as_array()
            .unwrap()
            .iter()
            .any(|value| value == "term-typist")
    );
    assert!(config.get("access").is_none());
    assert!(config.get("device_key").is_none());
    assert_eq!(
        config
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([
            "host",
            "generation",
            "workspace",
            "relay",
            "route",
            "capabilities",
            "loopback"
        ])
    );
    let secret_hex = std::fs::read(&native.secret)
        .unwrap()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    assert!(!page.body.contains(&secret_hex));
    assert!(page.body.contains("Enroll") || page.body.contains("Enrollment"));
    assert!(
        page.body
            .contains("Retail Cloud tasks offer no customer shell")
    );
    assert!(!fixture.local_store.exists());
    let index = get(&fixture, &cookies, "/cloud/app/workbench").await;
    assert!(index.body.contains(PAGE));
    let ordinary = get(&fixture, &cookies, "/cloud/app").await;
    let ordinary_csp = ordinary.headers[header::CONTENT_SECURITY_POLICY]
        .to_str()
        .unwrap();
    assert!(!ordinary_csp.contains("terminal.example.invalid"));
    assert!(ordinary_csp.contains("connect-src 'self';"));
    let csp = page.headers[header::CONTENT_SECURITY_POLICY]
        .to_str()
        .unwrap();
    let connect = csp
        .split("connect-src ")
        .nth(1)
        .unwrap()
        .split(';')
        .next()
        .unwrap();
    let expected = BTreeSet::from([
        "'self'".to_owned(),
        url::Url::parse(config["relay"].as_str().unwrap())
            .unwrap()
            .origin()
            .ascii_serialization(),
        url::Url::parse(BROWSER_ROUTE)
            .unwrap()
            .origin()
            .ascii_serialization(),
    ]);
    assert_eq!(
        connect
            .split_whitespace()
            .map(str::to_owned)
            .collect::<BTreeSet<_>>(),
        expected
    );
    assert!(!connect.contains('*'));
    native.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn workbench_navigation_clears_on_account_workspace_epoch_and_native_grant_changes() {
    let (fixture, native, cookies) = connected().await;
    let page = get(&fixture, &cookies, PAGE).await;
    assert_eq!(page.status, StatusCode::OK, "{}", page.body);
    let pin = resource_descriptor(&page.body);
    let endpoint = pin["endpoint"].as_str().unwrap();
    assert_eq!(
        get(&fixture, &cookies, endpoint).await.status,
        StatusCode::OK
    );
    let bob = login(&fixture, "bob").await;
    for path in [PAGE, endpoint] {
        let refused = get(&fixture, &bob, path).await;
        assert_eq!(refused.status, StatusCode::FORBIDDEN);
        assert!(!refused.body.contains("cloud-workbench-config"));
        private_navigation(&refused);
    }
    let mut other_workspace = Cookies(cookies.0.clone());
    other_workspace
        .0
        .insert("oa_cloud_workspace".into(), "alice-team".into());
    assert_eq!(
        get(&fixture, &other_workspace, PAGE).await.status,
        StatusCode::FORBIDDEN
    );
    fixture.state.lock().unwrap().epoch += 1;
    for path in [PAGE, endpoint] {
        let retired = get(&fixture, &cookies, path).await;
        assert_eq!(retired.status, StatusCode::FORBIDDEN);
        assert!(!retired.body.contains("cloud-workbench-config"));
    }
    fixture.state.lock().unwrap().epoch -= 1;
    fixture.state.lock().unwrap().offline = true;
    assert_eq!(
        get(&fixture, &cookies, PAGE).await.status,
        StatusCode::SERVICE_UNAVAILABLE
    );
    fixture.state.lock().unwrap().offline = false;
    native.authority.revoke(&native.device, now()).unwrap();
    for path in [PAGE, endpoint] {
        let revoked = get(&fixture, &cookies, path).await;
        assert!(matches!(
            revoked.status,
            StatusCode::FORBIDDEN | StatusCode::SERVICE_UNAVAILABLE
        ));
        assert!(!revoked.body.contains("cloud-workbench-config"));
        private_navigation(&revoked);
    }
    native.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn original_terminal_reference_and_session_are_preserved_and_substitution_is_refused() {
    let (fixture, native, cookies) = connected().await;
    let initial = get(&fixture, &cookies, PAGE).await;
    assert_eq!(initial.status, StatusCode::OK, "{}", initial.body);
    let config = decoded_pre(&initial.body, "cloud-workbench-config");
    let host = config["host"].as_str().unwrap();
    let reference = contract::ResourceRef::terminal(
        contract::Host::Paired { key: host.into() },
        coder_host::mailbox::terminal_generation(host, 7),
        "e".repeat(64),
    )
    .in_workspace(coder_host::mailbox::workspace_id("checkout"));
    let encoded = |reference: &contract::ResourceRef| {
        URL_SAFE_NO_PAD.encode(serde_json::to_vec(reference).unwrap())
    };
    let session = "f".repeat(64);
    let page = get(
        &fixture,
        &cookies,
        &format!("{PAGE}?resource={}&session={session}", encoded(&reference)),
    )
    .await;
    assert_eq!(page.status, StatusCode::OK, "{}", page.body);
    let selected = decoded_pre(&page.body, "cloud-workbench-config");
    assert_eq!(
        selected["terminal"]["generation"],
        reference.generation.clone().unwrap()
    );
    assert_eq!(selected["terminal"]["terminal"], reference.id);
    assert_eq!(selected["session"], session);
    assert!(page.body.contains("Original work reference"));
    for change in ["host", "generation", "workspace"] {
        let mut changed = reference.clone();
        match change {
            "host" => {
                changed.host = contract::Host::Paired {
                    key: "9".repeat(64),
                }
            }
            "generation" => changed.generation = Some("8".repeat(64)),
            "workspace" => changed.workspace = Some(coder_host::mailbox::workspace_id("other")),
            _ => unreachable!(),
        }
        let refused = get(
            &fixture,
            &cookies,
            &format!("{PAGE}?resource={}", encoded(&changed)),
        )
        .await;
        assert_eq!(
            refused.status,
            StatusCode::CONFLICT,
            "{change}: {}",
            refused.body
        );
        assert!(!refused.body.contains("cloud-workbench-config"));
    }
    for query in [
        "resource=bad-base64!",
        "session=not-a-native-id",
        "other=yes",
        "session=a&session=b",
        "resource=a&resource=b",
    ] {
        let refused = get(&fixture, &cookies, &format!("{PAGE}?{query}")).await;
        assert_eq!(
            refused.status,
            StatusCode::BAD_REQUEST,
            "{query}: {}",
            refused.body
        );
        assert!(!refused.body.contains("cloud-workbench-config"));
    }
    let huge = format!("{PAGE}?resource={}", "a".repeat(8193));
    assert_eq!(
        get(&fixture, &cookies, &huge).await.status,
        StatusCode::BAD_REQUEST
    );
    let run = contract::ResourceRef::new(
        contract::Kind::Run,
        contract::Host::Paired {
            key: "7".repeat(64),
        },
        "native-task-other-owner",
    )
    .with_revision(contract::Revision::Counter(18));
    let fallback = get(
        &fixture,
        &cookies,
        &format!("{PAGE}?resource={}", encoded(&run)),
    )
    .await;
    assert_eq!(fallback.status, StatusCode::OK, "{}", fallback.body);
    assert!(fallback.body.contains("native-task-other-owner"));
    assert!(fallback.body.contains("its own admitted owner viewer"));
    assert!(
        decoded_pre(&fallback.body, "cloud-workbench-config")
            .get("terminal")
            .is_none()
    );
    assert!(!fallback.body.contains("task:prompt"));
    assert!(!fixture.local_store.exists());
    assert_eq!(native.running.as_ref().unwrap().terminals(), 0);
    native.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn workbench_standing_retires_after_config_replacement_assets_loss_or_native_disconnect() {
    for cause in ["config", "assets", "native"] {
        let (mut fixture, mut native, cookies) = connected().await;
        let page = get(&fixture, &cookies, PAGE).await;
        assert_eq!(page.status, StatusCode::OK, "{}", page.body);
        let pin = resource_descriptor(&page.body);
        let endpoint = pin["endpoint"].as_str().unwrap();
        match cause {
            "config" => {
                let mut value: Value =
                    serde_json::from_slice(&std::fs::read(&native.config).unwrap()).unwrap();
                value["bindings"][0]["browser"]["route"] =
                    json!("wss://replacement.example.invalid/");
                private_file(&native.config, &serde_json::to_vec(&value).unwrap());
            }
            "assets" => std::fs::remove_file(
                fixture
                    .config
                    .cloud_build
                    .as_ref()
                    .unwrap()
                    .join("coder_browser_web_bg.wasm"),
            )
            .unwrap(),
            "native" => native.running.take().unwrap().shutdown().await,
            _ => unreachable!(),
        }
        for path in [PAGE, endpoint] {
            let retired = get(&fixture, &cookies, path).await;
            assert!(
                matches!(
                    retired.status,
                    StatusCode::FORBIDDEN | StatusCode::CONFLICT | StatusCode::SERVICE_UNAVAILABLE
                ),
                "{cause}: {} {}",
                retired.status,
                retired.body
            );
            assert!(!retired.body.contains("cloud-workbench-config"));
            private_navigation(&retired);
        }
        if cause == "config" {
            fixture.config.cloud_hosts = Some(Arc::new(
                super::super::hosts::Hosts::load(&native.config).unwrap(),
            ));
            fixture.site = crate::router(fixture.config.clone());
            let fresh = get(&fixture, &cookies, PAGE).await;
            assert_eq!(fresh.status, StatusCode::OK, "{}", fresh.body);
            assert_ne!(
                resource_descriptor(&fresh.body)["identity"],
                pin["identity"]
            );
            assert_ne!(
                get(&fixture, &cookies, endpoint).await.status,
                StatusCode::OK
            );
        }
        native.stop().await;
    }
}
