//! Browser action acceptance against isolated native accounts and a resident.

use super::*;
use coder_access::{Right, Rights};

const CONNECTION: &str = "/cloud/app/hosts/resident";
const CREATE: &str = "/cloud/app/hosts/resident/new-task";

async fn get_page(fixture: &Fixture, cookies: &Cookies, path: &str) -> Answer {
    request(&fixture.site, Method::GET, path, cookies, None, None).await
}

async fn post_form(fixture: &Fixture, cookies: &Cookies, path: &str, input: &str) -> Answer {
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

async fn controlled(rights: Rights, controls: bool) -> (Fixture, Resident, Cookies) {
    let mut fixture = fixture().await;
    let native = resident_with_controls(&mut fixture, rights, controls).await;
    let mut cookies = login(&fixture, "alice").await;
    choose_personal(&fixture, &mut cookies).await;
    (fixture, native, cookies)
}

fn operator() -> Rights {
    Rights::new([Right::Observe, Right::Operate]).unwrap()
}

fn control_form(html: &str) -> &str {
    html.split("<form ")
        .skip(1)
        .map(|form| form.split("</form>").next().unwrap())
        .find(|form| form.contains("name=\"request\""))
        .unwrap()
}

fn input_fields(html: &str, names: &[&str]) -> Vec<(String, String)> {
    let html = control_form(html);
    names
        .iter()
        .map(|name| ((*name).into(), field(html, name)))
        .collect()
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

fn task_dir(fixture: &Fixture) -> PathBuf {
    fixture
        .config
        .cloud_build
        .as_ref()
        .unwrap()
        .parent()
        .unwrap()
        .join("resident-tasks")
}

fn journal_dir(fixture: &Fixture) -> PathBuf {
    task_dir(fixture).with_file_name("browser-controls")
}

fn saved(fixture: &Fixture, id: &str) -> Value {
    serde_json::from_slice(&std::fs::read(journal_dir(fixture).join(format!("{id}.json"))).unwrap())
        .unwrap()
}

fn native_tasks(fixture: &Fixture) -> Vec<String> {
    let mut tasks = std::fs::read_dir(task_dir(fixture).join(coder::task::TASK_DIR))
        .unwrap()
        .filter_map(|entry| {
            entry
                .unwrap()
                .file_name()
                .to_str()
                .and_then(|name| name.strip_suffix(".json"))
                .map(str::to_owned)
        })
        .collect::<Vec<_>>();
    tasks.sort();
    tasks
}

async fn enroll(fixture: &Fixture, cookies: &Cookies) {
    let page = get_page(fixture, cookies, CONNECTION).await;
    expect(&page, StatusCode::OK);
    assert!(page.body.contains("server adapter holds this device key"));
    let csrf = action_token(&page.body, &format!("{CONNECTION}/enroll"), None);
    let input = form(&[("csrf", &csrf), ("custody", "yes")]);
    let answer = post_form(fixture, cookies, &format!("{CONNECTION}/enroll"), &input).await;
    expect(&answer, StatusCode::SEE_OTHER);
}

async fn create_preview(fixture: &Fixture, cookies: &Cookies) -> (String, Vec<(String, String)>) {
    let page = get_page(fixture, cookies, CREATE).await;
    expect(&page, StatusCode::OK);
    assert!(page.body.contains("name=\"task:prompt\""));
    let mut fields = input_fields(&page.body, &["csrf", "request"]);
    fields.extend([
        ("title".into(), "Browser intent <script>".into()),
        ("task:prompt".into(), "Exact original browser intent".into()),
        ("engine".into(), String::new()),
    ]);
    let id = fields
        .iter()
        .find(|(key, _)| key == "request")
        .unwrap()
        .1
        .clone();
    let staged = post_form(fixture, cookies, CREATE, &encode(&fields)).await;
    expect(&staged, StatusCode::SEE_OTHER);
    assert_eq!(
        staged.headers[header::LOCATION],
        format!("{CONNECTION}/requests/{id}")
    );
    (id, fields)
}

async fn confirm_input(fixture: &Fixture, cookies: &Cookies, id: &str) -> (String, String) {
    let path = format!("{CONNECTION}/requests/{id}");
    let page = get_page(fixture, cookies, &path).await;
    expect(&page, StatusCode::OK);
    let confirm = format!("{path}/confirm");
    let csrf = action_token(&page.body, &confirm, None);
    (confirm, form(&[("csrf", &csrf)]))
}

async fn switch_workspace(fixture: &Fixture, cookies: &mut Cookies, workspace: &str) {
    let page = get_page(fixture, cookies, "/cloud/app").await;
    expect(&page, StatusCode::OK);
    let csrf = action_token(&page.body, "/cloud/select-workspace", Some(workspace));
    let answer = post_form(
        fixture,
        cookies,
        "/cloud/select-workspace",
        &form(&[("workspace", workspace), ("csrf", &csrf)]),
    )
    .await;
    expect(&answer, StatusCode::SEE_OTHER);
    cookies.apply(&answer);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn control_custody_is_explicit_and_native_observe_never_grants_operation() {
    let (fixture, native, cookies) = controlled(operator(), false).await;
    let page = get_page(&fixture, &cookies, CONNECTION).await;
    expect(&page, StatusCode::OK);
    assert!(page.body.contains("Browser control custody is unavailable"));
    assert!(
        !page
            .body
            .contains("action=\"/cloud/app/hosts/resident/enroll\"")
    );
    expect(
        &get_page(&fixture, &cookies, CREATE).await,
        StatusCode::FORBIDDEN,
    );
    assert_eq!(native_tasks(&fixture), [native.task.clone()]);
    assert!(!fixture.local_store.exists());
    native.stop().await;

    let (fixture, native, cookies) = controlled(Rights::new([Right::Observe]).unwrap(), true).await;
    expect(
        &get_page(&fixture, &cookies, CREATE).await,
        StatusCode::FORBIDDEN,
    );
    enroll(&fixture, &cookies).await;
    expect(
        &get_page(&fixture, &cookies, CREATE).await,
        StatusCode::FORBIDDEN,
    );
    let page = get_page(&fixture, &cookies, CONNECTION).await;
    assert!(page.body.contains("native grant admits no task effects"));
    assert_eq!(native_tasks(&fixture), [native.task.clone()]);
    native.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn enrollment_requires_reviewed_custody_and_exact_session_origin_and_action() {
    let (fixture, native, cookies) = controlled(operator(), true).await;
    let page = get_page(&fixture, &cookies, CONNECTION).await;
    expect(&page, StatusCode::OK);
    let path = format!("{CONNECTION}/enroll");
    let csrf = action_token(&page.body, &path, None);
    expect(
        &post_form(&fixture, &cookies, &path, &form(&[("csrf", &csrf)])).await,
        StatusCode::BAD_REQUEST,
    );
    let input = form(&[("csrf", &csrf), ("custody", "yes")]);
    let evil = request(
        &fixture.site,
        Method::POST,
        &path,
        &cookies,
        Some(&input),
        Some("https://untrusted.example"),
    )
    .await;
    expect(&evil, StatusCode::FORBIDDEN);
    let bob = login(&fixture, "bob").await;
    expect(
        &post_form(&fixture, &bob, &path, &input).await,
        StatusCode::FORBIDDEN,
    );
    expect(
        &get_page(&fixture, &cookies, CREATE).await,
        StatusCode::FORBIDDEN,
    );
    expect(
        &post_form(&fixture, &cookies, &path, &input).await,
        StatusCode::SEE_OTHER,
    );
    let page = get_page(&fixture, &cookies, CREATE).await;
    expect(&page, StatusCode::OK);
    let mut fields = input_fields(&page.body, &["request"]);
    fields.extend([
        ("csrf".into(), csrf),
        ("title".into(), "Wrong action ticket".into()),
        ("task:prompt".into(), "Must not stage".into()),
        ("engine".into(), String::new()),
    ]);
    expect(
        &post_form(&fixture, &cookies, CREATE, &encode(&fields)).await,
        StatusCode::FORBIDDEN,
    );
    assert_eq!(native_tasks(&fixture), [native.task.clone()]);
    native.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn preview_persists_exact_packet_and_confirm_creates_one_inert_native_task_after_restart() {
    let (mut fixture, native, cookies) = controlled(operator(), true).await;
    enroll(&fixture, &cookies).await;
    let (id, fields) = create_preview(&fixture, &cookies).await;
    let original = saved(&fixture, &id)["pending"].clone();
    assert_eq!(native_tasks(&fixture), [native.task.clone()]);
    expect(
        &post_form(&fixture, &cookies, CREATE, &encode(&fields)).await,
        StatusCode::SEE_OTHER,
    );
    assert_eq!(saved(&fixture, &id)["pending"], original);
    let mut changed = fields.clone();
    changed
        .iter_mut()
        .find(|(key, _)| key == "task:prompt")
        .unwrap()
        .1 = "Changed intent".into();
    expect(
        &post_form(&fixture, &cookies, CREATE, &encode(&changed)).await,
        StatusCode::CONFLICT,
    );
    assert_eq!(saved(&fixture, &id)["pending"], original);

    fixture.config.cloud_hosts = Some(Arc::new(
        super::super::hosts::Hosts::load(&native.config).unwrap(),
    ));
    fixture.config.cloud = Some(Arc::new(
        CloudSession::load(&native.config.with_file_name("cloud.json")).unwrap(),
    ));
    fixture.site = crate::router(fixture.config.clone());
    let (confirm, input) = confirm_input(&fixture, &cookies, &id).await;
    assert_eq!(saved(&fixture, &id)["pending"], original);
    assert_eq!(native_tasks(&fixture), [native.task.clone()]);
    let result = post_form(&fixture, &cookies, &confirm, &input).await;
    expect(&result, StatusCode::OK);
    assert!(result.body.contains("Exact original browser intent"));
    let frozen = saved(&fixture, &id)["outcome"].clone();
    expect(
        &post_form(&fixture, &cookies, &confirm, &input).await,
        StatusCode::OK,
    );
    assert_eq!(saved(&fixture, &id)["outcome"], frozen);
    assert_eq!(native_tasks(&fixture).len(), 2);
    let task = coder::task::retained_task(&task_dir(&fixture), &id).unwrap();
    assert_eq!(task.intent.prompt, "Exact original browser intent");
    assert_eq!(task.status, coder::task::Status::Queued);
    assert_eq!(task.execution, coder::task::Execution::NotStarted);
    assert!(task.run.is_none());
    assert!(task.intent.configuration.model.is_none());
    let canonical = get_page(&fixture, &cookies, &format!("{CONNECTION}/tasks/{id}")).await;
    expect(&canonical, StatusCode::OK);
    assert!(canonical.body.contains("Browser intent &lt;script&gt;"));
    assert!(canonical.body.contains("Exact original browser intent"));
    assert!(!fixture.local_store.exists());
    native.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn complete_native_engine_options_preserve_explicit_codex_in_the_original_packet() {
    let (fixture, native, cookies) = controlled(operator(), true).await;
    enroll(&fixture, &cookies).await;
    let page = get_page(&fixture, &cookies, CREATE).await;
    expect(&page, StatusCode::OK);
    let html = control_form(&page.body);
    for engine in nostr::cj_conversation::Engine::ALL {
        assert!(html.contains(&format!(
            "<option value=\"{}\">{}</option>",
            engine.word(),
            engine.name()
        )));
    }
    let mut fields = input_fields(&page.body, &["csrf", "request"]);
    fields.extend([
        ("title".into(), "Explicit native executor request".into()),
        (
            "task:prompt".into(),
            "Keep the exact request.\r\nDo not start an engine.".into(),
        ),
    ]);
    let id = fields
        .iter()
        .find(|(key, _)| key == "request")
        .unwrap()
        .1
        .clone();
    expect(
        &post_form(&fixture, &cookies, CREATE, &encode(&fields)).await,
        StatusCode::BAD_REQUEST,
    );
    assert!(!journal_dir(&fixture).join(format!("{id}.json")).exists());
    fields.push(("engine".into(), "codex".into()));
    expect(
        &post_form(&fixture, &cookies, CREATE, &encode(&fields)).await,
        StatusCode::SEE_OTHER,
    );
    let record = saved(&fixture, &id);
    assert_eq!(record["action"]["task"]["engine"], "codex");
    assert_eq!(
        record["pending"]["request"]["op"]["task"]["engine"],
        "codex"
    );
    let original = record["pending"].clone();
    expect(
        &post_form(&fixture, &cookies, CREATE, &encode(&fields)).await,
        StatusCode::SEE_OTHER,
    );
    assert_eq!(saved(&fixture, &id)["pending"], original);
    assert_eq!(native_tasks(&fixture), [native.task.clone()]);
    let (confirm, input) = confirm_input(&fixture, &cookies, &id).await;
    expect(
        &post_form(&fixture, &cookies, &confirm, &input).await,
        StatusCode::OK,
    );
    let task = coder::task::retained_task(&task_dir(&fixture), &id).unwrap();
    assert_eq!(
        task.intent.prompt,
        "Keep the exact request.\r\nDo not start an engine."
    );
    assert_eq!(task.status, coder::task::Status::Queued);
    assert!(task.run.is_none() && task.intent.configuration.model.is_none());
    native.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn confirmation_rechecks_actor_workspace_csrf_and_origin_before_any_effect() {
    let (fixture, native, mut cookies) = controlled(operator(), true).await;
    enroll(&fixture, &cookies).await;
    let (id, _) = create_preview(&fixture, &cookies).await;
    let (confirm, input) = confirm_input(&fixture, &cookies, &id).await;
    expect(
        &post_form(&fixture, &cookies, &confirm, "csrf=").await,
        StatusCode::FORBIDDEN,
    );
    let evil = request(
        &fixture.site,
        Method::POST,
        &confirm,
        &cookies,
        Some(&input),
        Some("https://untrusted.example"),
    )
    .await;
    expect(&evil, StatusCode::FORBIDDEN);
    let bob = login(&fixture, "bob").await;
    expect(
        &post_form(&fixture, &bob, &confirm, &input).await,
        StatusCode::FORBIDDEN,
    );
    switch_workspace(&fixture, &mut cookies, "alice-team").await;
    expect(
        &post_form(&fixture, &cookies, &confirm, &input).await,
        StatusCode::FORBIDDEN,
    );
    assert_eq!(native_tasks(&fixture), [native.task.clone()]);
    native.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn enrollment_removal_invalidates_the_private_control_projection() {
    let (fixture, native, cookies) = controlled(operator(), true).await;
    enroll(&fixture, &cookies).await;
    let page = get_page(&fixture, &cookies, CREATE).await;
    expect(&page, StatusCode::OK);
    assert!(page.body.contains("id=\"cloud-private\" hidden"));
    let descriptor = resource_descriptor(&page.body);
    let endpoint = descriptor["endpoint"].as_str().unwrap();
    expect(
        &get_page(&fixture, &cookies, endpoint).await,
        StatusCode::OK,
    );
    for entry in std::fs::read_dir(journal_dir(&fixture)).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap().to_str().unwrap();
        if name.starts_with("enrollment-") && name.ends_with(".json") {
            std::fs::remove_file(path).unwrap();
        }
    }
    expect(
        &get_page(&fixture, &cookies, endpoint).await,
        StatusCode::CONFLICT,
    );
    expect(
        &get_page(&fixture, &cookies, CREATE).await,
        StatusCode::FORBIDDEN,
    );
    expect(
        &get_page(&fixture, &cookies, &format!("{CONNECTION}/tasks")).await,
        StatusCode::OK,
    );
    assert_eq!(native_tasks(&fixture), [native.task.clone()]);
    native.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn revoked_native_grant_cannot_confirm_a_saved_packet() {
    let (fixture, native, cookies) = controlled(operator(), true).await;
    enroll(&fixture, &cookies).await;
    let (id, _) = create_preview(&fixture, &cookies).await;
    let (confirm, input) = confirm_input(&fixture, &cookies, &id).await;
    native.authority.revoke(&native.device, now()).unwrap();
    let refused = post_form(&fixture, &cookies, &confirm, &input).await;
    assert!(matches!(
        refused.status,
        StatusCode::FORBIDDEN | StatusCode::SERVICE_UNAVAILABLE
    ));
    private(&refused);
    assert_eq!(native_tasks(&fixture), [native.task.clone()]);
    assert_eq!(saved(&fixture, &id)["state"], "prepared");
    native.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn changed_account_projection_requires_another_exact_enrollment_review() {
    let (fixture, native, cookies) = controlled(operator(), true).await;
    enroll(&fixture, &cookies).await;
    let (id, _) = create_preview(&fixture, &cookies).await;
    let (confirm, input) = confirm_input(&fixture, &cookies, &id).await;
    fixture.state.lock().unwrap().team_name = Some("Changed native account projection".into());
    expect(
        &post_form(&fixture, &cookies, &confirm, &input).await,
        StatusCode::FORBIDDEN,
    );
    expect(
        &get_page(&fixture, &cookies, CREATE).await,
        StatusCode::FORBIDDEN,
    );
    let page = get_page(&fixture, &cookies, CONNECTION).await;
    expect(&page, StatusCode::OK);
    assert!(
        page.body
            .contains("action=\"/cloud/app/hosts/resident/enroll\"")
    );
    enroll(&fixture, &cookies).await;
    expect(&get_page(&fixture, &cookies, CREATE).await, StatusCode::OK);
    // Reviewing the new projection does not relabel an older saved action.
    expect(
        &get_page(&fixture, &cookies, &format!("{CONNECTION}/requests/{id}")).await,
        StatusCode::FORBIDDEN,
    );
    assert_eq!(native_tasks(&fixture), [native.task.clone()]);
    native.stop().await;
}

fn correct_native_task(fixture: &Fixture, task: &str) {
    let command = coder::task::Command {
        schema: coder::task::COMMAND_SCHEMA.into(),
        command_id: "synthetic-native-correction".into(),
        task_id: task.into(),
        expected_revision: Some(1),
        action: coder::task::Action::Correct {
            prompt: "New native revision".into(),
            reason: "Synthetic acceptance fixture changed the owner state".into(),
        },
    };
    coder::task::Store::open(&task_dir(fixture))
        .unwrap()
        .apply(&serde_json::to_vec(&command).unwrap())
        .unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_revision_change_refuses_a_staged_command_atomically() {
    let (fixture, native, cookies) = controlled(operator(), true).await;
    enroll(&fixture, &cookies).await;
    let path = format!("{CONNECTION}/tasks/{}/actions", native.task);
    let page = get_page(&fixture, &cookies, &path).await;
    expect(&page, StatusCode::OK);
    let mut fields = input_fields(&page.body, &["csrf", "request", "issued_at", "revision"]);
    fields.extend([
        ("action".into(), "queue".into()),
        ("task:prompt".into(), "Stale queued command".into()),
    ]);
    let staged = post_form(&fixture, &cookies, &path, &encode(&fields)).await;
    expect(&staged, StatusCode::SEE_OTHER);
    let id = fields
        .iter()
        .find(|(key, _)| key == "request")
        .unwrap()
        .1
        .clone();
    let (confirm, input) = confirm_input(&fixture, &cookies, &id).await;
    correct_native_task(&fixture, &native.task);
    expect(
        &post_form(&fixture, &cookies, &path, &encode(&fields)).await,
        StatusCode::CONFLICT,
    );
    expect(
        &post_form(&fixture, &cookies, &confirm, &input).await,
        StatusCode::OK,
    );
    let receipt = get_page(&fixture, &cookies, &format!("{CONNECTION}/requests/{id}")).await;
    expect(&receipt, StatusCode::OK);
    assert!(receipt.body.contains("Original native refusal"));
    assert!(receipt.body.contains("Stale"));
    assert!(!receipt.body.contains(&format!("action=\"{confirm}\"")));
    let current = coder::task::retained_task(&task_dir(&fixture), &native.task).unwrap();
    assert_eq!(current.revision, 2);
    assert_eq!(current.effective_prompt(), "New native revision");
    assert!(current.run.is_none());
    native.stop().await;
}

async fn stage_lease(fixture: &Fixture, cookies: &Cookies, path: &str) -> (String, String) {
    let page = get_page(fixture, cookies, path).await;
    expect(&page, StatusCode::OK);
    let mut fields = input_fields(&page.body, &["csrf", "request", "revision", "queue_digest"]);
    let id = fields
        .iter()
        .find(|(key, _)| key == "request")
        .unwrap()
        .1
        .clone();
    let digest = fields
        .iter()
        .find(|(key, _)| key == "queue_digest")
        .unwrap()
        .1
        .clone();
    fields.extend([
        ("action".into(), "lease".into()),
        ("command".into(), String::new()),
        ("text".into(), String::new()),
        ("order".into(), String::new()),
    ]);
    expect(
        &post_form(fixture, cookies, path, &encode(&fields)).await,
        StatusCode::SEE_OTHER,
    );
    (id, digest)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_queue_digest_fences_an_edit_staged_before_another_lease() {
    let (fixture, native, cookies) = controlled(operator(), true).await;
    enroll(&fixture, &cookies).await;
    let path = format!("{CONNECTION}/tasks/{}/queue", native.task);
    let (stale, before) = stage_lease(&fixture, &cookies, &path).await;
    let (current, same) = stage_lease(&fixture, &cookies, &path).await;
    assert_eq!(before, same);
    let (confirm, input) = confirm_input(&fixture, &cookies, &current).await;
    expect(
        &post_form(&fixture, &cookies, &confirm, &input).await,
        StatusCode::OK,
    );
    let frozen = saved(&fixture, &current)["outcome"].clone();
    expect(
        &post_form(&fixture, &cookies, &confirm, &input).await,
        StatusCode::OK,
    );
    assert_eq!(saved(&fixture, &current)["outcome"], frozen);
    let page = get_page(&fixture, &cookies, &path).await;
    expect(&page, StatusCode::OK);
    assert_ne!(field(control_form(&page.body), "queue_digest"), before);
    let (confirm, input) = confirm_input(&fixture, &cookies, &stale).await;
    expect(
        &post_form(&fixture, &cookies, &confirm, &input).await,
        StatusCode::OK,
    );
    let receipt = get_page(
        &fixture,
        &cookies,
        &format!("{CONNECTION}/requests/{stale}"),
    )
    .await;
    expect(&receipt, StatusCode::OK);
    assert!(receipt.body.contains("Stale"));
    assert!(!receipt.body.contains(&format!("action=\"{confirm}\"")));
    assert_eq!(
        coder::task::retained_task(&task_dir(&fixture), &native.task)
            .unwrap()
            .revision,
        1
    );
    native.stop().await;
}
