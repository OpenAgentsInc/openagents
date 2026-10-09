//! Operator Cloud HTTP acceptance through an isolated native owner and backend.

use super::*;

const BASE: &str = "/cloud/app/hosts/resident";
const CLOUD: &str = "/cloud/app/hosts/resident/cloud/synthetic-cloud";
const NEW: &str = "/cloud/app/hosts/resident/cloud/synthetic-cloud/new";
async fn connected(cloud: bool, controls: bool) -> (Fixture, Resident, Cookies) {
    let mut fixture = fixture().await;
    let native = resident_with_services(
        &mut fixture,
        coder_access::Rights::new([coder_access::Right::Observe, coder_access::Right::Operate])
            .unwrap(),
        controls,
        false,
        cloud,
    )
    .await;
    let mut cookies = login(&fixture, "alice").await;
    choose_personal(&fixture, &mut cookies).await;
    (fixture, native, cookies)
}
fn directory(fixture: &Fixture) -> PathBuf {
    fixture
        .config
        .cloud_build
        .as_ref()
        .unwrap()
        .parent()
        .unwrap()
        .into()
}
fn jobs(fixture: &Fixture) -> Vec<coder_cloud::Record> {
    coder_cloud::Store::under(directory(fixture).join("operator-cloud/jobs"))
        .list()
        .unwrap()
}
fn saved(fixture: &Fixture, id: &str) -> Value {
    serde_json::from_slice(
        &std::fs::read(
            directory(fixture)
                .join("browser-controls")
                .join(format!("{id}.json")),
        )
        .unwrap(),
    )
    .unwrap()
}
fn request_form(html: &str) -> &str {
    html.split("<form ")
        .skip(1)
        .map(|form| form.split("</form>").next().unwrap())
        .find(|form| form.contains("name=\"request\""))
        .unwrap()
}
fn encode(fields: &[(String, String)]) -> String {
    url::form_urlencoded::Serializer::new(String::new())
        .extend_pairs(
            fields
                .iter()
                .map(|(key, value)| (key.as_str(), value.as_str())),
        )
        .finish()
}
async fn get(fixture: &Fixture, cookies: &Cookies, path: &str) -> Answer {
    request(&fixture.site, Method::GET, path, cookies, None, None).await
}
async fn post(fixture: &Fixture, cookies: &Cookies, path: &str, input: &str) -> Answer {
    request(
        &fixture.site,
        Method::POST,
        path,
        cookies,
        Some(input),
        Some(ORIGIN),
    )
    .await
}
fn expect(answer: &Answer, status: StatusCode) {
    assert_eq!(answer.status, status, "{}", answer.body);
    private(answer);
}
async fn enroll(fixture: &Fixture, cookies: &Cookies) {
    let page = get(fixture, cookies, BASE).await;
    expect(&page, StatusCode::OK);
    let csrf = action_token(&page.body, &format!("{BASE}/enroll"), None);
    expect(
        &post(
            fixture,
            cookies,
            &format!("{BASE}/enroll"),
            &form(&[("csrf", &csrf), ("custody", "yes")]),
        )
        .await,
        StatusCode::SEE_OTHER,
    );
}
async fn preview(fixture: &Fixture, cookies: &Cookies) -> (String, Vec<(String, String)>) {
    let page = get(fixture, cookies, NEW).await;
    expect(&page, StatusCode::OK);
    let html = request_form(&page.body);
    let mut fields = ["csrf", "request", "profile", "basis"]
        .iter()
        .map(|name| ((*name).into(), field(html, name)))
        .collect::<Vec<_>>();
    fields.extend([
        (
            "task:prompt".into(),
            "Exact synthetic operator intent".into(),
        ),
        ("timeout".into(), "60".into()),
    ]);
    let id = fields
        .iter()
        .find(|(name, _)| name == "request")
        .unwrap()
        .1
        .clone();
    expect(
        &post(fixture, cookies, NEW, &encode(&fields)).await,
        StatusCode::SEE_OTHER,
    );
    (id, fields)
}
async fn confirm_form(fixture: &Fixture, cookies: &Cookies, id: &str) -> (String, String) {
    let path = format!("{BASE}/requests/{id}");
    let page = get(fixture, cookies, &path).await;
    expect(&page, StatusCode::OK);
    let confirm = format!("{path}/confirm");
    let csrf = action_token(&page.body, &confirm, None);
    (confirm, form(&[("csrf", &csrf)]))
}
async fn finished(fixture: &Fixture, id: &str) -> coder_cloud::Record {
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            if let Some(record) = jobs(fixture).into_iter().find(|record| record.id == id)
                && record.state.terminal()
                && record.cleanup_complete
            {
                return record;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("synthetic worker completes")
}

async fn composer_page(response: Response) -> Answer {
    let status = response.status();
    let headers = response.headers().clone();
    let body = String::from_utf8(
        to_bytes(response.into_body(), 1024 * 1024)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    Answer {
        status,
        headers,
        body,
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn chat_composer_stages_review_and_continuation_over_the_original_native_job() {
    use crate::chat_store::Selection;
    use crate::cloud::composer;

    let (fixture, native, cookies) = connected(true, true).await;
    enroll(&fixture, &cookies).await;
    let app = crate::App(Arc::new(crate::Inner {
        config: fixture.config.clone(),
    }));
    let mut headers = HeaderMap::new();
    headers.insert(header::HOST, HOST.parse().unwrap());
    headers.insert(header::COOKIE, cookies.header().parse().unwrap());
    headers.insert(header::ORIGIN, ORIGIN.parse().unwrap());
    let choices = composer::choices(&app, &headers).await.unwrap();
    assert_eq!(choices.len(), 1);
    assert!(choices[0].available);
    assert_eq!(choices[0].runtime.workspace, "checkout");
    assert_eq!(choices[0].runtime.account, "alice");
    assert_eq!(choices[0].runtime.members_epoch, 3);
    assert_eq!(choices[0].size, "small");
    let selected = Selection {
        revision: 1,
        repository: None,
        runtime: Some(choices[0].runtime.clone()),
    };
    let runtime = selected.runtime.as_ref().unwrap();
    composer::validate(&app, &headers, runtime).await.unwrap();
    composer::authorize(&app, &headers, runtime).await.unwrap();
    let prompt = "Exact chat-selected synthetic intent";
    let retained = composer::stage(&app, &headers, "owner", "message", &selected, prompt, None)
        .await
        .unwrap();
    assert!(jobs(&fixture).is_empty());
    let packet = saved(&fixture, &retained.request)["pending"].clone();
    assert_eq!(
        composer::stage(&app, &headers, "owner", "message", &selected, prompt, None)
            .await
            .unwrap(),
        retained
    );
    assert_eq!(saved(&fixture, &retained.request)["pending"], packet);
    assert_eq!(
        composer::stage(
            &app, &headers, "owner", "message", &selected, "Changed", None
        )
        .await
        .unwrap_err()
        .status(),
        StatusCode::CONFLICT
    );
    let review = composer_page(composer::view(&app, &headers, &retained).await).await;
    expect(&review, StatusCode::OK);
    assert!(review.body.contains("Exact reviewed operation"));
    assert!(review.body.contains(prompt));
    assert!(jobs(&fixture).is_empty());
    let (confirm, input) = confirm_form(&fixture, &cookies, &retained.request).await;
    expect(
        &post(&fixture, &cookies, &confirm, &input).await,
        StatusCode::OK,
    );
    let record = finished(&fixture, &retained.request).await;
    assert_eq!(record.spec.task, prompt);
    let job = composer_page(composer::view(&app, &headers, &retained).await).await;
    expect(&job, StatusCode::OK);
    assert!(job.body.contains("Original job projection"));
    assert!(job.body.contains(&format!(
        "method=\"post\" action=\"{CLOUD}/jobs/{}\"",
        retained.request
    )));
    let continuation = composer::stage(
        &app,
        &headers,
        "owner",
        "follow-up",
        &selected,
        "Continue the same synthetic job",
        Some(&retained),
    )
    .await
    .unwrap();
    assert_eq!(jobs(&fixture).len(), 1);
    assert_eq!(
        saved(&fixture, &continuation.request)["action"]["kind"],
        "cloud.continue"
    );
    let continuation_review =
        composer_page(composer::view(&app, &headers, &continuation).await).await;
    expect(&continuation_review, StatusCode::OK);
    assert!(
        continuation_review
            .body
            .contains("Exact reviewed operation")
    );
    assert!(continuation_review.body.contains(&retained.request));
    std::fs::write(
        directory(&fixture).join("resident-checkout/operator.fixture"),
        "Source drift\n",
    )
    .unwrap();
    assert!(composer::validate(&app, &headers, runtime).await.is_err());
    composer::authorize(&app, &headers, runtime).await.unwrap();
    native.authority.revoke(&native.device, now()).unwrap();
    assert!(matches!(
        composer::authorize(&app, &headers, runtime)
            .await
            .unwrap_err()
            .status(),
        StatusCode::FORBIDDEN | StatusCode::SERVICE_UNAVAILABLE
    ));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn stage_is_inert_and_signed_confirm_reuses_original_job_and_packet_after_restart() {
    let (mut fixture, native, cookies) = connected(true, true).await;
    enroll(&fixture, &cookies).await;
    let list = get(&fixture, &cookies, CLOUD).await;
    expect(&list, StatusCode::OK);
    assert!(jobs(&fixture).is_empty());
    let (id, fields) = preview(&fixture, &cookies).await;
    assert!(jobs(&fixture).is_empty());
    let packet = saved(&fixture, &id)["pending"].clone();
    expect(
        &post(&fixture, &cookies, NEW, &encode(&fields)).await,
        StatusCode::SEE_OTHER,
    );
    assert_eq!(saved(&fixture, &id)["pending"], packet);
    let mut changed = fields.clone();
    changed
        .iter_mut()
        .find(|(name, _)| name == "task:prompt")
        .unwrap()
        .1 = "Changed operator intent".into();
    expect(
        &post(&fixture, &cookies, NEW, &encode(&changed)).await,
        StatusCode::CONFLICT,
    );
    assert!(jobs(&fixture).is_empty());
    fixture.config.cloud_hosts = Some(Arc::new(
        super::super::hosts::Hosts::load(&native.config).unwrap(),
    ));
    fixture.site = crate::router(fixture.config.clone());
    let (confirm, input) = confirm_form(&fixture, &cookies, &id).await;
    let first = post(&fixture, &cookies, &confirm, &input).await;
    expect(&first, StatusCode::OK);
    expect(
        &post(&fixture, &cookies, &confirm, &input).await,
        StatusCode::OK,
    );
    let record = finished(&fixture, &id).await;
    assert_eq!(jobs(&fixture).len(), 1);
    assert_eq!(record.spec.task, "Exact synthetic operator intent");
    assert_eq!(record.state, coder_cloud::State::Completed);
    assert!(record.cleanup_complete);
    assert_eq!(saved(&fixture, &id)["pending"], packet);
    let path = format!("{CLOUD}/jobs/{id}");
    let page = get(&fixture, &cookies, &path).await;
    expect(&page, StatusCode::OK);
    for text in [
        "synthetic-served-model",
        "synthetic-requested-model",
        "completed",
        "Original job projection",
    ] {
        assert!(page.body.contains(text), "missing {text}");
    }
    let original = page
        .body
        .split("href=\"")
        .skip(1)
        .filter_map(|value| value.split('"').next())
        .find(|url| url.contains("/original?q="))
        .unwrap()
        .replace("&amp;", "&");
    let raw = get(&fixture, &cookies, &format!("{original}&download=yes")).await;
    assert_eq!(raw.status, StatusCode::OK);
    assert_eq!(raw.headers[header::X_CONTENT_TYPE_OPTIONS], "nosniff");
    assert!(!raw.body.is_empty());
    let before = serde_json::to_value(jobs(&fixture)).unwrap();
    expect(&get(&fixture, &cookies, &path).await, StatusCode::OK);
    assert_eq!(serde_json::to_value(jobs(&fixture)).unwrap(), before);
    assert!(!fixture.local_store.exists());
    native.stop().await;
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn account_signin_and_host_operation_do_not_replace_operator_policy_or_custody() {
    let (fixture, native, cookies) = connected(false, true).await;
    enroll(&fixture, &cookies).await;
    let denied = get(&fixture, &cookies, NEW).await;
    assert_ne!(denied.status, StatusCode::OK);
    assert!(jobs(&fixture).is_empty());
    native.stop().await;
    let (fixture, native, cookies) = connected(true, false).await;
    let page = get(&fixture, &cookies, NEW).await;
    expect(&page, StatusCode::OK);
    let html = request_form(&page.body);
    let mut fields = ["csrf", "request", "profile", "basis"]
        .iter()
        .map(|name| ((*name).into(), field(html, name)))
        .collect::<Vec<_>>();
    fields.extend([
        ("task:prompt".into(), "Unadmitted custody".into()),
        ("timeout".into(), "60".into()),
    ]);
    expect(
        &post(&fixture, &cookies, NEW, &encode(&fields)).await,
        StatusCode::FORBIDDEN,
    );
    assert!(jobs(&fixture).is_empty());
    let bob = login(&fixture, "bob").await;
    for path in [CLOUD, NEW] {
        expect(&get(&fixture, &bob, path).await, StatusCode::FORBIDDEN);
    }
    native.stop().await;
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn prepared_cloud_receipt_and_confirm_retire_when_native_operator_policy_disappears() {
    let (fixture, native, cookies) = connected(true, true).await;
    enroll(&fixture, &cookies).await;
    let (id, _) = preview(&fixture, &cookies).await;
    let packet = saved(&fixture, &id)["pending"].clone();
    let (confirm, input) = confirm_form(&fixture, &cookies, &id).await;
    std::fs::remove_file(directory(&fixture).join("operator-policy.json")).unwrap();
    for response in [
        get(&fixture, &cookies, &format!("{BASE}/requests/{id}")).await,
        post(&fixture, &cookies, &confirm, &input).await,
    ] {
        assert!(
            matches!(
                response.status,
                StatusCode::FORBIDDEN | StatusCode::CONFLICT | StatusCode::SERVICE_UNAVAILABLE
            ),
            "{} {}",
            response.status,
            response.body
        );
        assert!(!response.body.contains("Exact synthetic operator intent"));
    }
    assert_eq!(saved(&fixture, &id)["pending"], packet);
    assert!(jobs(&fixture).is_empty());
    native.stop().await;
}

#[path = "byo_release_tests.rs"]
mod byo_release;
