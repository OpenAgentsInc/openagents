use std::collections::BTreeMap;
use std::sync::Arc;

use axum::body::{Body, to_bytes};
use axum::http::{HeaderMap, Request, StatusCode, header};
use tower::ServiceExt;

use super::funnel::{Funnel, Stage};
use super::*;
use crate::{Config, router};

const LOCAL: &str = "127.0.0.1:4300";
const IPHONE: &str = "Mozilla/5.0 (iPhone; CPU iPhone OS 18_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.0 Mobile/15E148 Safari/604.1";
const BOT: &str = "Mozilla/5.0 (compatible; Googlebot/2.1; +http://www.google.com/bot.html)";
const NOW: u64 = 1_793_000_000_000;

fn batch(session: &str, events: serde_json::Value) -> Vec<u8> {
    serde_json::to_vec(&serde_json::json!({
        "v": 1, "s": session, "n": NOW,
        "u": {"source": "reddit", "campaign": "reddit-gamedev"},
        "e": events,
    }))
    .unwrap()
}

fn click(label: &str) -> serde_json::Value {
    serde_json::json!({"k": "click", "t": NOW - 500, "p": "/playtest?x=1", "l": label, "x": {"tag": "button"}})
}

#[test]
fn a_batch_is_checked_bounded_and_scrubbed() {
    let body = batch(
        "ps_abcdef123456",
        serde_json::json!([
            click("Write to me at kiki.fan@example.com, card 4111111111111111"),
            {"k": "page_view", "t": NOW - 3_600_000, "p": "/"},
            {"k": "keylogger", "p": "/"},
            {"k": "step", "l": "survey_completed", "p": "/"},
            {"k": "click", "p": "not-a-path"},
            {"k": "milestone", "l": "tutorial_moved", "p": "/playtest", "x": {"ok": "yes", "Bad Key": "x", "nested": {"a": 1}, "n": 3}},
        ]),
    );
    let parsed = ingest::parse(&body, NOW).unwrap();
    assert_eq!(parsed.session.as_deref(), Some("ps_abcdef123456"));
    assert_eq!(parsed.source, "reddit");
    assert_eq!(parsed.campaign, "reddit-gamedev");
    assert_eq!(parsed.events.len(), 3);
    assert_eq!(
        parsed.refused, 3,
        "unknown kind, a step from a page, a bad path"
    );
    let first = &parsed.events[0];
    assert_eq!(first.path, "/playtest", "no query string");
    assert_eq!(first.label, "Write to me at [email] card #");
    assert_eq!(first.at, NOW - 500);
    assert_eq!(
        parsed.events[1].at,
        NOW - 600_000,
        "a client clock can't place an event over ten minutes back"
    );
    let props = &parsed.events[2].props;
    assert_eq!(props.get("ok").map(String::as_str), Some("yes"));
    assert_eq!(props.get("n").map(String::as_str), Some("3"));
    assert_eq!(props.len(), 2);
}

#[test]
fn malformed_and_oversized_batches_are_refused_whole() {
    assert_eq!(
        ingest::parse(&vec![b' '; ingest::MOST_BYTES + 1], NOW),
        Err(ingest::Refusal::TooLarge)
    );
    assert_eq!(ingest::parse(b"nope", NOW), Err(ingest::Refusal::Malformed));
    assert_eq!(
        ingest::parse(&batch("short", serde_json::json!([])), NOW),
        Err(ingest::Refusal::Malformed),
        "a session id is 8 to 64 safe characters"
    );
    assert_eq!(
        ingest::parse(&batch("ps_a b c d e f", serde_json::json!([])), NOW),
        Err(ingest::Refusal::Malformed)
    );
    let many: Vec<_> = (0..=ingest::MOST_EVENTS).map(|_| click("x")).collect();
    assert_eq!(
        ingest::parse(&batch("ps_abcdef123456", many.into()), NOW),
        Err(ingest::Refusal::Malformed)
    );
    let wrong_version = br#"{"v":2,"s":"ps_abcdef123456","e":[]}"#;
    assert_eq!(
        ingest::parse(wrong_version, NOW),
        Err(ingest::Refusal::Malformed)
    );
}

#[test]
fn aggregate_batches_keep_no_label_or_detail() {
    let body = br#"{"v":1,"a":1,"s":"ps_abcdef123456","e":[{"k":"click","p":"/","l":"secret","x":{"a":"b"}}]}"#;
    let parsed = ingest::parse(body, NOW).unwrap();
    assert_eq!(parsed.session, None);
    assert_eq!(parsed.events[0].label, "");
    assert!(parsed.events[0].props.is_empty());
}

fn research() -> Research {
    Research::new(None, RETENTION_DAYS)
}

#[test]
fn opting_out_keeps_only_counts_and_drops_held_events() {
    let r = research();
    let parsed = ingest::parse(&batch("ps_abcdef123456", vec![click("a")].into()), NOW).unwrap();
    assert_eq!(
        r.admit(
            parsed,
            false,
            "mobile/ios/safari",
            Some("acct_1".into()),
            NOW
        ),
        Admission::Kept { refused: 0 }
    );
    let (events, _) = r.snapshot();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].account.as_deref(), Some("acct_1"));
    assert_eq!(events[0].device, "mobile/ios/safari");
    assert_eq!(events[0].source, "reddit");

    let out = br#"{"v":1,"s":"ps_abcdef123456","o":1,"e":[]}"#;
    r.admit(ingest::parse(out, NOW).unwrap(), false, "", None, NOW);
    assert!(r.snapshot().0.is_empty(), "the held events are dropped");
    let later = ingest::parse(&batch("ps_abcdef123456", vec![click("b")].into()), NOW).unwrap();
    r.admit(later, false, "mobile/ios/safari", None, NOW);
    r.step(
        "ps_abcdef123456",
        "survey_completed",
        &[("score", "5")],
        NOW,
    );
    let (events, counts) = r.snapshot();
    assert!(events.is_empty());
    let total: u64 = counts.iter().map(|c| c.count).sum();
    assert_eq!(total, 2);
    assert!(
        counts
            .iter()
            .any(|c| c.kind == "click" && c.path == "/playtest")
    );
}

#[test]
fn do_not_track_keeps_only_counts() {
    let r = research();
    let parsed = ingest::parse(&batch("ps_abcdef123456", vec![click("a")].into()), NOW).unwrap();
    r.admit(parsed, true, "desktop/mac/chrome", Some("acct".into()), NOW);
    let (events, counts) = r.snapshot();
    assert!(events.is_empty());
    assert_eq!(counts.len(), 1);
}

#[test]
fn a_session_and_the_process_are_rate_limited() {
    let r = research();
    let full: Vec<_> = (0..ingest::MOST_EVENTS).map(|_| click("x")).collect();
    let mut admitted = 0;
    for _ in 0..20 {
        let parsed = ingest::parse(&batch("ps_ratelimited", full.clone().into()), NOW).unwrap();
        match r.admit(parsed, false, "", None, NOW) {
            Admission::Kept { .. } => admitted += ingest::MOST_EVENTS,
            Admission::Limited => break,
        }
    }
    assert_eq!(admitted, MOST_PER_SESSION);
    // A window later the session may send again.
    let parsed = ingest::parse(&batch("ps_ratelimited", full.clone().into()), NOW).unwrap();
    let later = NOW + WINDOW.as_millis() as u64;
    assert_eq!(
        r.admit(parsed, false, "", None, later),
        Admission::Kept { refused: 0 }
    );

    let r = research();
    let mut kept = 0;
    for i in 0..(MOST_PER_WINDOW / ingest::MOST_EVENTS + 1) {
        let parsed = ingest::parse(
            &batch(&format!("ps_session{i:06}"), full.clone().into()),
            NOW,
        )
        .unwrap();
        if r.admit(parsed, false, "", None, NOW) != Admission::Limited {
            kept += ingest::MOST_EVENTS;
        }
    }
    assert_eq!(kept, MOST_PER_WINDOW);
}

#[test]
fn server_steps_join_the_session_and_its_account() {
    let r = research();
    let parsed = ingest::parse(&batch("ps_abcdef123456", vec![click("a")].into()), NOW).unwrap();
    r.admit(parsed, false, "", None, NOW);
    r.remember_account("ps_abcdef123456", Some("acct_9".into()));
    r.step(
        "ps_abcdef123456",
        "survey_completed",
        &[("rating", "5"), ("Bad", "x")],
        NOW,
    );
    r.step("ps_abcdef123456", "Not A Step", &[], NOW);
    r.step("bad id", "survey_started", &[], NOW);
    let (events, counts) = r.snapshot();
    let step = events.iter().find(|e| e.kind == "step").unwrap();
    assert_eq!(step.label, "survey_completed");
    assert_eq!(step.account.as_deref(), Some("acct_9"));
    assert_eq!(step.props.len(), 1);
    assert_eq!(events.len(), 2);
    assert_eq!(counts.len(), 1, "a bad id is only counted");
}

fn event(session: &str, at: u64, kind: &str, path: &str, label: &str) -> Event {
    Event {
        at,
        session: session.into(),
        account: None,
        kind: kind.into(),
        path: path.into(),
        label: label.into(),
        props: BTreeMap::new(),
        device: "mobile/ios/safari".into(),
        source: String::new(),
        campaign: String::new(),
    }
}

#[test]
fn the_funnel_counts_drop_off_and_median_time() {
    let mut events = Vec::new();
    // a: the whole way; b: stops after starting; c: homepage only;
    // d: a server-only session that started and finished the survey.
    for (s, start) in [("a", 0), ("b", 1_000), ("c", 2_000)] {
        events.push(event(s, start, "page_view", "/", ""));
    }
    for (s, at) in [("a", 10_000), ("b", 21_000)] {
        events.push(event(s, at, "step", "", "playtest_started"));
    }
    events.push(event(
        "a",
        40_000,
        "milestone",
        "/playtest",
        "tutorial_moved",
    ));
    events.push(event("a", 50_000, "step", "", "tutorial_jumped"));
    events.push(event("a", 60_000, "step", "", "survey_started"));
    events.push(event("a", 90_000, "step", "", "survey_completed"));
    events.push(event("d", 5_000, "step", "", "survey_started"));
    events.push(event("d", 9_000, "step", "", "survey_completed"));
    let mut sources = events.clone();
    for e in sources.iter_mut().filter(|e| e.session == "a") {
        e.source = "reddit".into();
    }
    let sessions = funnel::sessions(&sources);
    assert_eq!(sessions.len(), 4);
    let f = Funnel::of(&sessions);
    assert_eq!(f.reached, [4, 3, 2, 1, 2, 2]);
    assert_eq!(f.drop_off(1).map(|d| d.round()), Some(33.0));
    assert_eq!(f.drop_off(2).map(|d| d.round()), Some(50.0));
    // Homepage to start: a took 10s, b took 20s; the lower middle is 10s.
    assert_eq!(f.median_ms[1], Some(10_000));
    assert_eq!(f.median_ms[4], Some(4_000), "median of 30s and 4s");
    let by_source = funnel::by(&sessions, |s| s.source.clone());
    assert_eq!(by_source[0].0, "direct");
    assert_eq!(by_source[0].1.reached[0], 3);
    assert_eq!(by_source[1].0, "reddit");
    assert_eq!(by_source[1].1.reached[5], 1);
    let milestones = funnel::milestones(&sessions);
    assert_eq!(
        milestones
            .iter()
            .map(|(n, c, _)| (n.as_str(), *c))
            .collect::<Vec<_>>(),
        [("tutorial_moved", 1), ("tutorial_jumped", 1)]
    );
    assert_eq!(sessions[0].furthest(), Stage::SurveyCompleted);
    assert_eq!(sessions[0].device, "mobile");
    assert_eq!(funnel::median(&mut []), None);
    assert_eq!(funnel::duration(185_000), "3m 05s");
}

#[test]
fn csv_fields_are_quoted_and_never_formulas() {
    assert_eq!(dashboard::field("plain"), "plain");
    assert_eq!(dashboard::field("a,b"), "\"a,b\"");
    assert_eq!(dashboard::field("say \"hi\""), "\"say \"\"hi\"\"\"");
    assert_eq!(dashboard::field("=HYPERLINK(1)"), "'=HYPERLINK(1)");
    assert_eq!(dashboard::iso(0), "1970-01-01T00:00:00Z");
    let csv = dashboard::events_csv(&[event("a", 0, "click", "/", "Go, now")]);
    assert_eq!(csv.lines().count(), 2);
    assert!(csv.contains("\"Go, now\""));
}

#[test]
fn the_tag_goes_in_once_and_the_policy_allows_this_site() {
    let page = b"<html><head><title>x</title></head><body></body></html>";
    let tagged = String::from_utf8(add_tag(page)).unwrap();
    assert!(tagged.contains("<script src=\"/static/r.js\" defer></script></head>"));
    assert_eq!(add_tag(tagged.as_bytes()), tagged.as_bytes());
    assert_eq!(
        allow_self(crate::SITE_POLICY),
        "default-src 'none'; style-src 'self'; font-src 'self'; img-src 'self'; base-uri 'none'; form-action 'self'; frame-ancestors 'none'; script-src 'self'; connect-src 'self'"
    );
    assert_eq!(
        allow_self(
            "default-src 'none'; script-src 'self' 'wasm-unsafe-eval'; connect-src https://a.example"
        ),
        "default-src 'none'; script-src 'self' 'wasm-unsafe-eval'; connect-src https://a.example 'self'"
    );
    assert!(funnel_page("/") && funnel_page("/playtest") && funnel_page("/playtest/survey"));
    assert!(funnel_page("/new-lander/verse") && funnel_page("/new-lander-house"));
    assert!(!funnel_page("/verse/hands/left.glb") && !funnel_page("/new-lander/app.js"));
    assert!(
        !funnel_page("/chat") && !funnel_page("/playtesting") && !funnel_page("/admin/research")
    );
}

#[tokio::test]
async fn flushes_append_new_objects_and_old_days_are_deleted() {
    let root = tempfile::tempdir().unwrap();
    let r = Research::new(Some(Store::disk(root.path().into())), 180);
    let now = now_ms();
    for label in ["one", "two"] {
        let body = serde_json::to_vec(&serde_json::json!({
            "v": 1, "s": "ps_abcdef123456", "n": now,
            "e": [{"k": "click", "t": now, "p": "/", "l": label}],
        }))
        .unwrap();
        r.admit(ingest::parse(&body, now).unwrap(), false, "", None, now);
        r.flush().await.unwrap();
    }
    let store = Store::disk(root.path().into());
    let names = store.list("research/events/").await.unwrap();
    assert_eq!(names.len(), 2, "each flush writes a new object");
    let (events, _) = r.load(1).await.unwrap();
    assert_eq!(events.len(), 2);
    let old = format!(
        "research/events/{}/old-00000001.json",
        crate::analytics::date(now / 86_400_000 - 200)
    );
    store.put(&old, b"{}".to_vec()).await.unwrap();
    assert_eq!(r.prune(now / 86_400_000).await.unwrap(), 1);
    assert_eq!(store.list("research/events/").await.unwrap().len(), 2);
}

async fn send(site: &axum::Router, request: Request<Body>) -> (StatusCode, HeaderMap, String) {
    let response = site.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let body = to_bytes(response.into_body(), 16 * 1024 * 1024)
        .await
        .unwrap();
    (status, headers, String::from_utf8_lossy(&body).into_owned())
}

fn post(body: Vec<u8>, agent: &str) -> axum::http::request::Builder {
    let _ = body;
    Request::builder()
        .method("POST")
        .uri(BEACON)
        .header(header::HOST, LOCAL)
        .header(header::USER_AGENT, agent)
        .header(header::CONTENT_TYPE, "text/plain")
}

fn site() -> (tempfile::TempDir, Arc<Research>, axum::Router) {
    let root = tempfile::tempdir().unwrap();
    let mut config = Config::development(root.path().join("tasks"));
    let research = Arc::new(research());
    config.research = research.clone();
    config.analytics = Arc::new(crate::analytics::Analytics::new(None, Some("research-key")));
    (root, research, router(config))
}

#[tokio::test]
async fn the_beacon_keeps_a_coarse_device_and_drops_bots_and_quiet_details() {
    let (_root, research, site) = site();
    let body = batch("ps_abcdef123456", vec![click("Start")].into());
    let (status, headers, _) = send(
        &site,
        post(body.clone(), IPHONE)
            .body(Body::from(body.clone()))
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_eq!(headers[header::CACHE_CONTROL], "no-store");
    let (events, _) = research.snapshot();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].device, "mobile/ios/safari");
    let stored = serde_json::to_string(&events).unwrap();
    assert!(!stored.contains("iPhone OS") && !stored.contains("127.0.0.1"));

    let (status, _, _) = send(
        &site,
        post(body.clone(), BOT)
            .body(Body::from(body.clone()))
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, _, _) = send(
        &site,
        post(body.clone(), IPHONE)
            .header("sec-gpc", "1")
            .body(Body::from(body.clone()))
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (events, counts) = research.snapshot();
    assert_eq!(
        events.len(),
        1,
        "neither the bot nor the quiet batch is kept"
    );
    assert_eq!(counts.iter().map(|c| c.count).sum::<u64>(), 1);

    let (status, _, _) = send(&site, post(vec![], IPHONE).body(Body::from("{}")).unwrap()).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let big = vec![b'x'; ingest::MOST_BYTES + 1];
    let (status, _, _) = send(&site, post(vec![], IPHONE).body(Body::from(big)).unwrap()).await;
    assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);
}

fn admin(path: &str, key: Option<&str>) -> Request<Body> {
    let mut request = Request::builder()
        .uri(path)
        .header(header::HOST, LOCAL)
        .header(header::USER_AGENT, IPHONE)
        .header(header::ACCEPT, "text/html");
    if let Some(key) = key {
        request = request.header(header::AUTHORIZATION, format!("Bearer {key}"));
    }
    request.body(Body::empty()).unwrap()
}

#[tokio::test]
async fn the_dashboard_and_export_are_a_plain_404_without_the_admin_gate() {
    let (_root, research, site) = site();
    research.step("ps_abcdef123456", "playtest_started", &[], now_ms());
    research.step("ps_abcdef123456", "survey_completed", &[], now_ms());
    for path in [DASHBOARD, EXPORT] {
        let (status, _, body) = send(&site, admin(path, None)).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{path}");
        assert!(!body.contains("Playtest research"));
        let (status, _, _) = send(&site, admin(path, Some("wrong"))).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{path}");
    }
    let (status, headers, body) = send(&site, admin(DASHBOARD, Some("research-key"))).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers[header::CACHE_CONTROL], "no-store");
    crate::copy_guard::assert_plain(DASHBOARD, &body);
    assert!(body.contains("Playtest research"));
    assert!(body.contains("Survey completed"));
    assert!(body.contains("?session=ps_abcdef123456"));
    let (status, _, timeline) = send(
        &site,
        admin(
            &format!("{DASHBOARD}?session=ps_abcdef123456"),
            Some("research-key"),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(timeline.contains("survey_completed"));
    let (status, headers, csv) = send(
        &site,
        admin(&format!("{EXPORT}?what=sessions"), Some("research-key")),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        headers[header::CONTENT_TYPE]
            .to_str()
            .unwrap()
            .starts_with("text/csv")
    );
    assert!(csv.starts_with("session,account,"));
    assert!(csv.contains("ps_abcdef123456"));
}

#[tokio::test]
async fn funnel_pages_get_the_script_and_a_policy_that_loads_it() {
    let (_root, _research, site) = site();
    let (status, headers, body) = send(&site, admin("/", None)).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("<script src=\"/static/r.js\" defer></script>"));
    // A browser that accepts brotli gets the tagged page compressed.
    let mut asked = admin("/", None);
    asked
        .headers_mut()
        .insert(header::ACCEPT_ENCODING, "br, gzip".parse().unwrap());
    let response = site.clone().oneshot(asked).await.unwrap();
    assert_eq!(response.headers()[header::CONTENT_ENCODING], "br");
    let compressed = to_bytes(response.into_body(), 16 * 1024 * 1024)
        .await
        .unwrap();
    let mut plain = Vec::new();
    brotli::BrotliDecompress(&mut &compressed[..], &mut plain).unwrap();
    assert!(String::from_utf8_lossy(&plain).contains("/static/r.js"));
    let policy = headers[header::CONTENT_SECURITY_POLICY].to_str().unwrap();
    assert!(policy.contains("script-src") && policy.contains("connect-src"));
    assert!(
        !headers.contains_key(header::CONTENT_LENGTH)
            || headers[header::CONTENT_LENGTH] == body.len().to_string().as_str()
    );
    let (status, headers, script) = send(&site, admin(SCRIPT, None)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        headers[header::CONTENT_TYPE],
        "text/javascript; charset=utf-8"
    );
    assert!(script.contains("\"/a/r\""));
    assert!(
        !script.contains("http://") && !script.contains("https://"),
        "first-party only"
    );
    let (_, _, privacy) = send(&site, admin("/privacy", None)).await;
    assert!(
        !privacy.contains("/static/r.js"),
        "other pages get no script"
    );
    assert!(privacy.contains("Playtest research"));
}

#[test]
fn the_capture_script_tests_pass() {
    let crate_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    match std::process::Command::new("node")
        .arg("--test")
        .arg(crate_dir.join("static/r.test.js"))
        .output()
    {
        Ok(out) => assert!(
            out.status.success(),
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        ),
        Err(_) => eprintln!("node is not installed; static/r.test.js did not run"),
    }
}
