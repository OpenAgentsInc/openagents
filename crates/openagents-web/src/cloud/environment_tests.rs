//! ENV-07 HTTP acceptance: the project environment panel over an isolated
//! resident operator, its environment records, and retained evidence.

use super::*;
use coder_environment::{
    self as env, BuildState, Command, Review,
    evidence::{CallIdentity, CallResult, Recorder, Redactor, StreamName},
    store::Store as EnvStore,
    transition::{BuildObservation as B, VerificationObservation as V},
};

const BASE: &str = "/cloud/app/hosts/resident";
const CLOUD: &str = "/cloud/app/hosts/resident/cloud/synthetic-cloud";
const PANEL: &str = "/cloud/app/hosts/resident/cloud/synthetic-cloud/environment";
const STDOUT: &[u8] = b"test result: ok. 3 passed <script>\n";

pub(super) fn operator_root(fixture: &Fixture) -> PathBuf {
    fixture
        .config
        .cloud_build
        .as_ref()
        .unwrap()
        .parent()
        .unwrap()
        .join("operator-cloud")
}
fn d(c: char) -> String {
    c.to_string().repeat(64)
}
fn recipe(install: char) -> env::Recipe {
    env::Recipe {
        schema: env::RECIPE_SCHEMA.into(),
        base: env::ImagePin {
            provider: env::Provider::Boat,
            image_id: "oa-coder-runtime-20261008".into(),
            digest: d('a'),
        },
        runtime: env::ArtifactPin {
            revision: "rt-1".into(),
            digest: d('b'),
        },
        platform: env::Platform {
            os: "linux".into(),
            architecture: "x86_64".into(),
        },
        install: env::Script {
            cwd: ".".into(),
            digest: d(install),
        },
        start: Default::default(),
        inputs: Default::default(),
        credential_names: Default::default(),
        qualification: env::Qualification {
            profile: "rust-library".into(),
            plan_digest: d('e'),
        },
        limits: env::Limits {
            deadline_seconds: 3600,
            concurrent_machines: 2,
            total_machine_allocations: 4,
            output_bytes: 1 << 28,
        },
        capture: Default::default(),
    }
}
fn apply(s: &EnvStore, c: Command) {
    s.apply("env-1", &c, 10).unwrap();
}
/// An environment whose revision 1 is built and verified with sealed
/// evidence, ready for a reviewed Save.
fn verified(root: &std::path::Path) -> EnvStore {
    let s = EnvStore::under(root.join("environments"));
    let e = env::Environment::new(
        "env-1",
        env::ProjectLink {
            workspace: "checkout".into(),
            project: "synthetic-cloud".into(),
        },
        env::SourcePin {
            repository: None,
            revision: "0".repeat(40),
            digest: d('f'),
        },
        recipe('1'),
        1,
    )
    .unwrap();
    s.create(&e).unwrap();
    apply(
        &s,
        Command::StartBuild {
            request_id: "rb1".into(),
            expected_draft_revision: 1,
        },
    );
    for o in [
        B::Linked {
            run: env::RunLink {
                cloud_job: "job-b1".into(),
                task: None,
            },
        },
        B::Progress {
            state: BuildState::Installing,
        },
        B::ImageReady {
            image: env::ImageIdentity {
                provider: env::Provider::Boat,
                image_id: "oaenv-build-1".into(),
                snapshot_id: Some("snap-1".into()),
                manifest_digest: d('c'),
            },
        },
    ] {
        apply(
            &s,
            Command::ObserveBuild {
                build_id: "build-1".into(),
                observation: o,
            },
        );
    }
    apply(
        &s,
        Command::StartVerification {
            request_id: "rv1".into(),
            build_id: "build-1".into(),
            plan_digest: d('e'),
        },
    );
    let run = env::RunLink {
        cloud_job: "job-v1".into(),
        task: Some("vjob-1".into()),
    };
    let mut r = Recorder::create(
        root.join(coder_cloud::operator::VERIFY_EVIDENCE)
            .join("vjob-1"),
        "verify",
        Some(run.clone()),
        Redactor::new(),
        1 << 20,
    )
    .unwrap();
    r.start_call(
        CallIdentity {
            id: "check-1".into(),
            parent: None,
            run: run.clone(),
            tool: "environment.verify.check".into(),
            request: None,
            operation: None,
        },
        &json!({"check": "cargo test"}),
        5,
    )
    .unwrap();
    r.output("check-1", StreamName::Stdout, STDOUT).unwrap();
    r.close_stream("check-1", StreamName::Stdout).unwrap();
    r.close_stream("check-1", StreamName::Stderr).unwrap();
    r.result(
        "check-1",
        CallResult::Exited {
            code: Some(0),
            success: true,
        },
        6,
    )
    .unwrap();
    let sealed = r.finish(7).unwrap();
    for o in [V::Linked { run }, sealed.passed()] {
        apply(
            &s,
            Command::ObserveVerification {
                verification_id: "verify-1".into(),
                observation: o,
            },
        );
    }
    s
}
pub(super) async fn get(fixture: &Fixture, cookies: &Cookies, path: &str) -> Answer {
    request(&fixture.site, Method::GET, path, cookies, None, None).await
}
pub(super) async fn post(fixture: &Fixture, cookies: &Cookies, path: &str, input: &str) -> Answer {
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
pub(super) fn link(html: &str, needle: &str) -> String {
    html.split("href=\"")
        .skip(1)
        .filter_map(|v| v.split('"').next())
        .find(|url| url.contains(needle))
        .unwrap_or_else(|| panic!("no link containing {needle}"))
        .replace("&amp;", "&")
}
pub(super) fn action_form(html: &str, action: &str) -> Vec<(String, String)> {
    let form = html
        .split("<form ")
        .skip(1)
        .map(|f| f.split("</form>").next().unwrap())
        .find(|f| f.contains(&format!("name=\"action\" value=\"{action}\"")))
        .unwrap_or_else(|| panic!("no {action} form"));
    [
        "csrf",
        "request",
        "action",
        "environment",
        "target",
        "fence",
        "basis",
    ]
    .iter()
    .map(|n| ((*n).to_string(), field(form, n)))
    .collect()
}
pub(super) fn encode(fields: &[(String, String)]) -> String {
    url::form_urlencoded::Serializer::new(String::new())
        .extend_pairs(fields.iter().map(|(k, v)| (k.as_str(), v.as_str())))
        .finish()
}
pub(super) async fn enroll(fixture: &Fixture, cookies: &Cookies) {
    let page = get(fixture, cookies, BASE).await;
    assert_eq!(page.status, StatusCode::OK);
    let csrf = action_token(&page.body, &format!("{BASE}/enroll"), None);
    let answer = post(
        fixture,
        cookies,
        &format!("{BASE}/enroll"),
        &form(&[("csrf", &csrf), ("custody", "yes")]),
    )
    .await;
    assert_eq!(answer.status, StatusCode::SEE_OTHER, "{}", answer.body);
}
pub(super) async fn confirm_form(
    fixture: &Fixture,
    cookies: &Cookies,
    id: &str,
) -> (String, String) {
    let path = format!("{BASE}/requests/{id}");
    let page = get(fixture, cookies, &path).await;
    assert_eq!(page.status, StatusCode::OK, "{}", page.body);
    let target = format!("{path}/confirm");
    let csrf = action_token(&page.body, &target, None);
    (target, form(&[("csrf", &csrf)]))
}

#[test]
fn wall_times_and_summary_states_are_explicit() {
    use super::super::environment::when;
    assert_eq!(when(0), "1970-01-01 00:00:00 UTC");
    assert_eq!(when(1_791_504_000_000), "2026-10-09 00:00:00 UTC");
    assert_eq!(when(951_782_400_000 + 3_723_000), "2000-02-29 01:02:03 UTC");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn environment_panel_reads_promotes_rolls_back_and_pages_evidence() {
    let mut fixture = fixture().await;
    let native = resident_with_services(
        &mut fixture,
        coder_access::Rights::new([coder_access::Right::Observe, coder_access::Right::Operate])
            .unwrap(),
        true,
        false,
        true,
    )
    .await;
    let mut cookies = login(&fixture, "alice").await;
    choose_personal(&fixture, &mut cookies).await;
    let root = operator_root(&fixture);

    // Empty state, and the read creates nothing in the operator state.
    let page = get(&fixture, &cookies, PANEL).await;
    assert_eq!(page.status, StatusCode::OK, "{}", page.body);
    private(&page);
    assert!(page.body.contains("No environment yet"));
    assert!(!root.join("environments").exists());
    // The jobs page links the panel.
    assert!(get(&fixture, &cookies, CLOUD).await.body.contains(PANEL));

    // Denied: a project the operator policy does not admit.
    let denied = get(
        &fixture,
        &cookies,
        "/cloud/app/hosts/resident/cloud/other-project/environment",
    )
    .await;
    assert_eq!(denied.status, StatusCode::FORBIDDEN);
    assert!(denied.body.contains("Denied") && denied.body.contains("role=\"alert\""));

    let s = verified(&root);
    let record = std::fs::read(root.join("environments/env-1.json")).unwrap();
    let page = get(&fixture, &cookies, PANEL).await;
    assert_eq!(page.status, StatusCode::OK, "{}", page.body);
    private(&page);
    for text in [
        "data-state=\"review\"",
        "Ready for review",
        "Candidate for review",
        "build-1",
        "verify-1",
        "oaenv-build-1",
        "Setup sessions",
        "No setup owner is composed",
        "Page the retained evidence",
        "Open the native workbench",
        "role=\"status\"",
        "aria-live=\"polite\"",
    ] {
        assert!(page.body.contains(text), "missing {text}");
    }
    // Not enrolled: the page is read-only and its controls are disabled.
    assert!(page.body.contains("this page stays read-only"));
    // The read changed nothing.
    assert_eq!(
        std::fs::read(root.join("environments/env-1.json")).unwrap(),
        record
    );

    // Evidence: summary, original bytes, exact download, and export.
    let evidence = link(&page.body, "/environment/evidence?q=");
    let summary = get(&fixture, &cookies, &evidence).await;
    assert_eq!(summary.status, StatusCode::OK, "{}", summary.body);
    assert!(summary.body.contains("check-1") && summary.body.contains("None disclosed"));
    let stdout = link(&summary.body, "evidence?q=");
    let stdout = summary
        .body
        .split("href=\"")
        .skip(1)
        .filter_map(|v| v.split('"').next())
        .filter(|u| u.contains("evidence?q="))
        .nth(1)
        .map(|u| u.replace("&amp;", "&"))
        .unwrap_or(stdout);
    let bytes = get(&fixture, &cookies, &stdout).await;
    assert_eq!(bytes.status, StatusCode::OK, "{}", bytes.body);
    assert!(
        bytes
            .body
            .contains("test result: ok. 3 passed &lt;script&gt;")
    );
    assert!(!bytes.body.contains("<script>"));
    let raw = get(&fixture, &cookies, &format!("{stdout}&download=yes")).await;
    assert_eq!(raw.status, StatusCode::OK);
    assert_eq!(raw.body.as_bytes(), STDOUT);
    assert_eq!(raw.headers[header::X_CONTENT_TYPE_OPTIONS], "nosniff");
    let export = get(&fixture, &cookies, &link(&summary.body, "download=export")).await;
    assert_eq!(export.status, StatusCode::OK);
    assert!(
        export.headers[header::CONTENT_DISPOSITION]
            .to_str()
            .unwrap()
            .starts_with("attachment")
    );
    let export: Value = serde_json::from_str(&export.body).unwrap();
    assert_eq!(export["summary"]["complete"], true);

    // Reviewed Save and select through the request book.
    enroll(&fixture, &cookies).await;
    let page = get(&fixture, &cookies, PANEL).await;
    let fields = action_form(&page.body, "promote");
    let id = fields
        .iter()
        .find(|(k, _)| k == "request")
        .unwrap()
        .1
        .clone();
    // A changed fence no longer matches the token issued for the page.
    let mut forged = fields.clone();
    forged.iter_mut().find(|(k, _)| k == "fence").unwrap().1 = format!("0:{}", d('0'));
    assert_eq!(
        post(&fixture, &cookies, PANEL, &encode(&forged))
            .await
            .status,
        StatusCode::CONFLICT
    );
    let staged = post(&fixture, &cookies, PANEL, &encode(&fields)).await;
    assert_eq!(staged.status, StatusCode::SEE_OTHER, "{}", staged.body);
    // Staging is inert.
    assert!(s.read("env-1").unwrap().versions.is_empty());
    let (target, input) = confirm_form(&fixture, &cookies, &id).await;
    let done = post(&fixture, &cookies, &target, &input).await;
    assert_eq!(done.status, StatusCode::OK, "{}", done.body);
    assert!(done.body.contains("Return to the project environment"));
    let e = s.read("env-1").unwrap();
    assert_eq!(e.versions.len(), 1);
    assert_eq!(e.selection.active.as_deref(), Some("v1"));
    // The same original request recovers; it never saves twice.
    assert_eq!(
        post(&fixture, &cookies, &target, &input).await.status,
        StatusCode::OK
    );
    let receipt = get(&fixture, &cookies, &format!("{BASE}/requests/{id}")).await;
    assert!(receipt.body.contains("environment_accepted"));
    assert_eq!(s.read("env-1").unwrap().versions.len(), 1);

    let page = get(&fixture, &cookies, PANEL).await;
    assert!(page.body.contains("Version 1"));
    assert!(page.body.contains("aria-current=\"true\""));
    assert!(page.body.contains("data-state=\"selected\""));

    // A new job pins the selected version, and its job view shows it.
    let new = format!("{CLOUD}/new");
    let compose = get(&fixture, &cookies, &new).await;
    let html = compose
        .body
        .split("<form ")
        .skip(1)
        .map(|f| f.split("</form>").next().unwrap())
        .find(|f| f.contains("name=\"request\""))
        .unwrap();
    let mut fields: Vec<(String, String)> = ["csrf", "request", "profile", "basis"]
        .iter()
        .map(|n| ((*n).to_string(), field(html, n)))
        .collect();
    fields.push(("task:prompt".into(), "Synthetic pinned job".into()));
    fields.push(("timeout".into(), "60".into()));
    let job = fields[1].1.clone();
    assert_eq!(
        post(&fixture, &cookies, &new, &encode(&fields))
            .await
            .status,
        StatusCode::SEE_OTHER
    );
    let (target, input) = confirm_form(&fixture, &cookies, &job).await;
    assert_eq!(
        post(&fixture, &cookies, &target, &input).await.status,
        StatusCode::OK
    );
    let pinned = coder_cloud::Store::under(root.join("jobs"))
        .read(&job)
        .unwrap()
        .environment
        .unwrap();
    assert_eq!(pinned.version_id, "v1");
    let view = get(&fixture, &cookies, &format!("{CLOUD}/jobs/{job}")).await;
    assert_eq!(view.status, StatusCode::OK, "{}", view.body);
    assert!(
        view.body.contains("Started from version 1"),
        "{}",
        view.body
    );
    native.stop().await;
}
