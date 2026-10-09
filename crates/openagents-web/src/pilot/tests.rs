use super::*;
use axum::body::{Body, to_bytes};
use axum::http::Request as HttpRequest;
use tower::ServiceExt;

const HOST: &str = "127.0.0.1:4300";
fn fixture() -> (tempfile::TempDir, crate::Config, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("host");
    let owner_file = dir.path().join("owner");
    let token_file = dir.path().join("intake");
    let mut store = Store::open(&root).unwrap();
    store.initialize("operator", &owner_file).unwrap();
    let owner = store
        .authenticate(&Store::read_credential(&owner_file).unwrap())
        .unwrap();
    store
        .issue_intake(
            &owner,
            intake::Policy {
                schema: intake::POLICY_SCHEMA.into(),
                id: "fixture-v1".into(),
                offer: intake::OFFER.into(),
                origin: format!("http://{HOST}"),
                public_owner: "Fixture operator".into(),
                support_email: "operator@example.invalid".into(),
                commercial_approval: "fixture:commercial-review".into(),
                responsibility_acceptance: "fixture:standing-human-review".into(),
                consent_version: "email-review-v1".into(),
                expires_at: coder::task::sales::unix_now() + 14 * 86400,
                retention_seconds: 30 * 86400,
                review_within_seconds: 86400,
                max_leads: 4,
            },
            &token_file,
        )
        .unwrap();
    drop(store);
    let mut config = crate::Config::development(root.clone());
    config.pilot = Some(Arc::new(Intake::new(root, &token_file).unwrap()));
    (dir, config, owner_file)
}
async fn get(config: crate::Config, uri: &str) -> (StatusCode, HeaderMap, String) {
    response(
        crate::router(config)
            .oneshot(
                HttpRequest::builder()
                    .uri(uri)
                    .header(header::HOST, HOST)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await
}
async fn response(response: Response) -> (StatusCode, HeaderMap, String) {
    let status = response.status();
    let headers = response.headers().clone();
    let body = String::from_utf8(
        to_bytes(response.into_body(), 1024 * 1024)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    (status, headers, body)
}
fn encoded(value: &str) -> String {
    value
        .bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.') {
                char::from(b).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect()
}
fn body(ticket: &str, email: &str, workflow: &str, consent: bool) -> String {
    let mut fields = vec![
        ("ticket", ticket),
        ("email", email),
        ("account", "Synthetic buyer"),
        ("jurisdiction", "US"),
        ("workflow", workflow),
        ("consent_version", "email-review-v1"),
        ("website", ""),
    ];
    if consent {
        fields.push(("consent", "yes"));
    }
    fields
        .into_iter()
        .map(|(k, v)| format!("{k}={}", encoded(v)))
        .collect::<Vec<_>>()
        .join("&")
}
async fn post(
    config: crate::Config,
    cookie: &str,
    content: String,
    origin: &str,
) -> (StatusCode, HeaderMap, String) {
    response(
        crate::router(config)
            .oneshot(
                HttpRequest::builder()
                    .method("POST")
                    .uri("/pilot")
                    .header(header::HOST, HOST)
                    .header(header::ORIGIN, origin)
                    .header(header::COOKIE, cookie)
                    .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                    .body(Body::from(content))
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await
}
fn form(config: crate::Config, referral: &str) -> (String, String) {
    let intake = config.pilot.as_ref().unwrap();
    let cookie = random();
    let ticket = intake.ticket(&cookie, Some(referral.to_owned()));
    (format!("{COOKIE}={cookie}"), ticket)
}
fn leads(config: &crate::Config, owner_file: &Path) -> Vec<coder::task::sales::Lead> {
    let mut store = Store::open(&config.store).unwrap();
    let owner = store
        .authenticate(&Store::read_credential(owner_file).unwrap())
        .unwrap();
    store.list(&owner, None, 32).unwrap()
}
#[tokio::test]
async fn public_pilot_pages_are_not_found_and_the_copy_stays_archived() {
    let dir = tempfile::tempdir().unwrap();
    let config = crate::Config::development(dir.path().join("absent"));
    let (status, _, html) = get(config.clone(), "/pilot").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(html.contains("<h1>Not found</h1>"));
    assert!(!html.contains("proposed service fee is USD 250"));
    let (install_status, _, _) = get(config.clone(), "/pilot/install").await;
    assert_eq!(install_status, StatusCode::NOT_FOUND);
    assert!(ARCHIVED_OFFER.contains("proposed service fee is USD 250"));
    assert!(ARCHIVED_OFFER.contains("zero promotional credits"));
    assert!(ARCHIVED_INSTALL.contains("scripts/install-coder.sh"));
    assert!(ARCHIVED_INSTALL.contains("CODER_CLOUD=off"));
    assert!(ARCHIVED_INSTALL.contains("OPENAGENTS_JEV_HOSTED=off"));
    assert!(ARCHIVED_INSTALL.contains("your own supported provider login"));
    assert!(ARCHIVED_INSTALL.contains("TYPESAFE_*"));
    assert!(ARCHIVED_INSTALL.contains("CODER_DECISION_*"));
    assert!(ARCHIVED_INSTALL.contains("~/.openagents/jev.json"));
    assert!(ARCHIVED_INSTALL.contains("without deleting your saved configuration"));
    assert!(ARCHIVED_INSTALL.contains("each exact decision recipient and payer before work"));
    assert!(ARCHIVED_INSTALL.contains("release download alone does not qualify"));
    assert!(!dir.path().join("absent").exists());
    assert_eq!(
        post(config, "", String::new(), &format!("http://{HOST}"))
            .await
            .0,
        StatusCode::SERVICE_UNAVAILABLE
    );
    assert!(crate::upstream::owned("/pilot"));
    assert!(crate::upstream::owned("/pilot/install"));
}

#[tokio::test]
async fn unconfigured_host_never_proxies_intake_contact_content() {
    let (_dir, mut config, owner_file) = fixture();
    let (upstream, hits) = crate::tests::echo_upstream().await;
    config.upstream = Some(Arc::new(crate::upstream::Upstream::new(&upstream).unwrap()));
    let reply = crate::router(config.clone())
        .oneshot(
            HttpRequest::builder()
                .method("POST")
                .uri("/pilot")
                .header(header::HOST, "unconfigured.invalid")
                .body(Body::from("email=synthetic%40example.invalid"))
                .unwrap(),
        )
        .await
        .unwrap();
    let (status, _, body) = response(reply).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert!(!body.contains("synthetic"));
    assert_eq!(hits.load(std::sync::atomic::Ordering::Relaxed), 0);
    assert!(leads(&config, &owner_file).is_empty());
}
#[tokio::test]
async fn isolated_form_reaches_one_private_lead_and_preserves_email_consent_source_and_referral() {
    let (_dir, config, owner_file) = fixture();
    let (cookie, ticket) = form(config.clone(), "opaque-referral");
    let (status, headers, html) = post(
        config.clone(),
        &cookie,
        body(
            &ticket,
            "synthetic@example.invalid",
            "A small public fix",
            true,
        ),
        &format!("http://{HOST}"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{html}");
    assert!(html.contains("Pilot request received"));
    crate::copy_guard::assert_plain("/pilot", &html);
    assert!(!html.contains("synthetic@example.invalid"));
    assert!(!html.contains("A small public fix"));
    assert!(!html.contains("lead_"));
    assert_eq!(headers[header::CACHE_CONTROL], "no-store");
    let lead = leads(&config, &owner_file).remove(0);
    assert_eq!(lead.contact, "email:synthetic@example.invalid");
    assert_eq!(lead.responsible_human, "operator");
    assert_eq!(
        lead.intake.unwrap().referral,
        Some("opaque-referral".into())
    );
    assert_eq!(lead.details.permission.channels, ["email"]);
    assert_eq!(lead.details.data.recipients, ["human:operator"]);
    assert!(lead.source.contains("http://127.0.0.1:4300/pilot"));
    let (_, _, public) = get(config.clone(), "/pilot").await;
    assert!(!public.contains("synthetic@example.invalid"));
    assert!(!public.contains("A small public fix"));
}
#[tokio::test]
async fn lost_response_restart_and_new_duplicate_form_create_no_competing_leads() {
    let (dir, config, owner_file) = fixture();
    let (cookie, ticket) = form(config.clone(), "first-referral");
    let content = body(
        &ticket,
        "synthetic@example.invalid",
        "First scoped request",
        true,
    );
    let first = post(
        config.clone(),
        &cookie,
        content.clone(),
        &format!("http://{HOST}"),
    )
    .await;
    assert_eq!(first.0, StatusCode::OK);
    let mut restarted = crate::Config::development(config.store.clone());
    restarted.pilot = Some(Arc::new(
        Intake::new(config.store.clone(), &dir.path().join("intake")).unwrap(),
    ));
    let retry = post(
        restarted.clone(),
        &cookie,
        content,
        &format!("http://{HOST}"),
    )
    .await;
    assert_eq!(retry.0, StatusCode::OK);
    assert_eq!(retry.2, first.2);
    assert_eq!(
        post(
            restarted.clone(),
            &cookie,
            body(&ticket, "synthetic@example.invalid", "Changed retry", true),
            &format!("http://{HOST}")
        )
        .await
        .0,
        StatusCode::SERVICE_UNAVAILABLE
    );
    let (other_cookie, other_ticket) = form(restarted.clone(), "second-referral");
    assert_eq!(
        post(
            restarted.clone(),
            &other_cookie,
            body(
                &other_ticket,
                "SYNTHETIC@EXAMPLE.INVALID",
                "Duplicate changed request",
                true
            ),
            &format!("http://{HOST}")
        )
        .await
        .0,
        StatusCode::OK
    );
    let records = leads(&restarted, &owner_file);
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].details.workflow, "First scoped request");
    assert_eq!(
        records[0].intake.as_ref().unwrap().referral,
        Some("first-referral".into())
    );
}
#[tokio::test]
async fn invalid_oversized_cross_origin_and_tampered_requests_are_redacted_and_inert() {
    let (_dir, config, owner_file) = fixture();
    for kind in 0..6 {
        let (cookie, ticket) = form(config.clone(), "referral");
        let (payload, origin) = match kind {
            0 => (
                body(
                    &ticket,
                    "synthetic@example.invalid",
                    "Private workflow",
                    false,
                ),
                format!("http://{HOST}"),
            ),
            1 => (
                body(&ticket, "not a valid address", "Private workflow", true),
                format!("http://{HOST}"),
            ),
            2 => (
                body(
                    &ticket,
                    "synthetic@example.invalid",
                    &"x".repeat(2049),
                    true,
                ),
                format!("http://{HOST}"),
            ),
            3 => (
                body(
                    &ticket,
                    "synthetic@example.invalid",
                    &"x".repeat(10000),
                    true,
                ),
                format!("http://{HOST}"),
            ),
            4 => (
                body(
                    &ticket,
                    "synthetic@example.invalid",
                    "Private workflow",
                    true,
                ),
                "https://attacker.invalid".into(),
            ),
            _ => (
                body(
                    &format!("{ticket}changed"),
                    "synthetic@example.invalid",
                    "Private workflow",
                    true,
                ),
                format!("http://{HOST}"),
            ),
        };
        let (status, _, html) = post(config.clone(), &cookie, payload, &origin).await;
        assert!(status.is_client_error(), "{status} {html}");
        assert!(!html.contains("synthetic@example.invalid"));
        assert!(!html.contains("Private workflow"));
    }
    assert!(leads(&config, &owner_file).is_empty());
    assert_eq!(
        get(config, "/pilot?reference=person%40example.invalid")
            .await
            .0,
        StatusCode::NOT_FOUND
    );
}
#[tokio::test]
async fn revoked_capability_reports_unavailable_without_contact_content_or_new_record() {
    let (_dir, config, owner_file) = fixture();
    let (cookie, ticket) = form(config.clone(), "referral");
    let mut store = Store::open(&config.store).unwrap();
    let owner = store
        .authenticate(&Store::read_credential(&owner_file).unwrap())
        .unwrap();
    store.revoke_intake(&owner, "fixture-v1").unwrap();
    drop(store);
    let (status, _, html) = post(
        config.clone(),
        &cookie,
        body(
            &ticket,
            "synthetic@example.invalid",
            "Private workflow",
            true,
        ),
        &format!("http://{HOST}"),
    )
    .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert!(html.contains("We couldn't confirm we got your request"));
    crate::copy_guard::assert_plain("/pilot", &html);
    assert!(!html.contains("synthetic@example.invalid"));
    assert!(leads(&config, &owner_file).is_empty());
    assert_eq!(get(config, "/pilot").await.0, StatusCode::NOT_FOUND);
}
#[tokio::test]
async fn ticket_expiry_cookie_binding_signature_validation_and_abuse_bounds_are_enforced() {
    let (_dir, config, owner_file) = fixture();
    let intake = config.pilot.as_ref().unwrap();
    let cookie = random();
    let ticket = intake.ticket(&cookie, None);
    assert!(intake.verify(&random(), &ticket).is_err());
    let malformed = format!("{}.{}", ticket.split('.').next().unwrap(), "é".repeat(32));
    assert!(intake.verify(&cookie, &malformed).is_err());
    let old = serde_json::to_vec(&Ticket {
        request: random(),
        issued_at: coder::task::sales::unix_now() - 1801,
        referral: None,
    })
    .unwrap();
    assert!(
        intake
            .verify(
                &cookie,
                &format!(
                    "{}.{}",
                    URL_SAFE_NO_PAD.encode(&old),
                    intake.sign(&cookie, &old)
                )
            )
            .is_err()
    );
    let cookie_header = format!("{COOKIE}={cookie}");
    for _ in 0..8 {
        assert_eq!(
            post(
                config.clone(),
                &cookie_header,
                body(
                    &ticket,
                    "synthetic@example.invalid",
                    "Private workflow",
                    false
                ),
                &format!("http://{HOST}")
            )
            .await
            .0,
            StatusCode::BAD_REQUEST
        );
    }
    assert_eq!(
        post(
            config.clone(),
            &cookie_header,
            body(
                &ticket,
                "synthetic@example.invalid",
                "Private workflow",
                true
            ),
            &format!("http://{HOST}")
        )
        .await
        .0,
        StatusCode::TOO_MANY_REQUESTS
    );
    assert!(leads(&config, &owner_file).is_empty());
}
#[test]
fn private_configuration_rejects_public_modes_symlinks_and_oversized_files() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.json");
    std::fs::write(&path, b"{}").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert!(private_json::<serde_json::Value>(&path).is_err());
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
        assert!(private_json::<serde_json::Value>(&path).is_ok());
        let link = dir.path().join("link");
        std::os::unix::fs::symlink(&path, &link).unwrap();
        assert!(private_json::<serde_json::Value>(&link).is_err());
    }
    std::fs::write(&path, vec![b'x'; 16 * 1024 + 1]).unwrap();
    assert!(private_json::<serde_json::Value>(&path).is_err());
}
