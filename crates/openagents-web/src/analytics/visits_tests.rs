//! The Grid homepage's per-visit events (#12439).

use std::collections::BTreeMap;

use super::*;
use crate::analytics::grid;
use crate::analytics::visits::{self, Refused, VisitRow};

const VISIT: &str = "0123456789abcdef";

fn visit_request() -> axum::http::request::Builder {
    visit_request_as(CHROME)
}

fn visit_request_as(agent: &str) -> axum::http::request::Builder {
    Request::builder()
        .method("POST")
        .uri(visits::BEACON)
        .header(header::HOST, LOCAL)
        .header(header::USER_AGENT, agent)
        .header(header::CONTENT_TYPE, "text/plain;charset=UTF-8")
}

fn batch(events: serde_json::Value) -> Body {
    Body::from(serde_json::json!({"v": VISIT, "r": "/", "e": events}).to_string())
}

#[test]
fn only_known_events_details_and_values_are_admitted() {
    let detail = |v: serde_json::Value| v.as_object().cloned().unwrap();
    assert_eq!(
        visits::admit("made_up", None).unwrap_err(),
        Refused::UnknownEvent
    );
    assert_eq!(
        visits::admit("joined", Some(&detail(serde_json::json!({"text": "hi"})))).unwrap_err(),
        Refused::UnknownDetail
    );
    assert_eq!(
        visits::admit("joined", Some(&detail(serde_json::json!({"by": "mouse"})))).unwrap_err(),
        Refused::BadValue
    );
    assert_eq!(
        visits::admit(
            "portal_entered",
            Some(&detail(serde_json::json!({"zone": "198.51.100.7"})))
        )
        .unwrap_err(),
        Refused::BadValue
    );
    assert_eq!(
        visits::admit(
            "verse_ready",
            Some(&detail(serde_json::json!({"time": -4})))
        )
        .unwrap_err(),
        Refused::BadValue
    );
    // A duration is kept only as its bucket.
    let (name, kept) = visits::admit(
        "verse_ready",
        Some(&detail(serde_json::json!({"gpu": "webgpu", "time": 2_345}))),
    )
    .unwrap();
    assert_eq!(name, "verse_ready");
    assert_eq!(
        kept,
        BTreeMap::from([
            ("gpu".to_owned(), "webgpu".to_owned()),
            ("time".to_owned(), "1-3s".to_owned())
        ])
    );
    assert_eq!(visits::time_bucket(0), "<1s");
    assert_eq!(visits::time_bucket(45_000), "30s-1m");
    assert_eq!(visits::time_bucket(u64::MAX), ">30m");
    // Every word in the vocabulary is one a stored row may hold.
    for spec in visits::EVENTS {
        for (key, values) in spec.details {
            assert!(bounded(key));
            if let visits::Values::One(words) = values {
                assert!(words.iter().all(|w| bounded(w)), "{}", spec.name);
            }
        }
    }
    // Ids are 16 lowercase hex digits; routes are the fixed list.
    for body in [
        serde_json::json!({"v": "0123456789ABCDEF", "r": "/", "e": [["page_view", 1]]}),
        serde_json::json!({"v": "chris@openagents", "r": "/", "e": [["page_view", 1]]}),
        serde_json::json!({"v": VISIT, "r": "/chat/123", "e": [["page_view", 1]]}),
        serde_json::json!({"v": VISIT, "r": "/", "e": []}),
    ] {
        assert!(
            visits::parse(body.to_string().as_bytes()).is_none(),
            "{body}"
        );
    }
}

#[tokio::test]
async fn the_visit_beacon_keeps_checked_events_and_refuses_the_rest() {
    let analytics = Arc::new(Analytics::default());
    let (_root, site) = web(analytics.clone());
    let (status, headers, _) = send(
        &site,
        visit_request()
            .header("x-forwarded-for", "198.51.100.7")
            .body(batch(serde_json::json!([
                ["page_view", 1],
                ["joined", 2, {"by": "click"}],
                ["portal_entered", 3, {"zone": "lagrange2"}],
            ])))
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert!(headers.get(header::SET_COOKIE).is_none());
    let rows = analytics.visit_snapshot();
    assert_eq!(rows.len(), 3);
    assert!(
        rows.iter()
            .all(|r| r.visit == VISIT && r.route == "/" && r.valid())
    );
    assert_eq!(rows[2].get("zone"), "lagrange2");
    // Each is also counted in the hourly totals, by name only.
    let counted = analytics.snapshot();
    assert!(
        counted
            .iter()
            .any(|r| r.series == Series::Event && r.name == "grid.joined" && r.valid())
    );

    // An unknown event or detail is refused; the good ones still count.
    let (status, _, _) = send(
        &site,
        visit_request()
            .body(batch(serde_json::json!([
                ["message_sent", 4, {"model": "flint", "nth": "first"}],
                ["message_sent", 5, {"text": "my secret question"}],
                ["chat_text", 6, {"model": "flint"}],
            ])))
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let rows = analytics.visit_snapshot();
    assert_eq!(rows.len(), 4);
    let json = serde_json::to_string(&rows).unwrap();
    assert!(!json.contains("secret") && !json.contains("chat_text") && !json.contains("198.51"));

    // Not a batch at all.
    let (status, _, _) = send(&site, visit_request().body(Body::from("e=joined")).unwrap()).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn do_not_track_global_privacy_control_and_bots_keep_no_visit() {
    for (name, value, agent) in [
        ("dnt", "1", CHROME),
        ("sec-gpc", "1", CHROME),
        ("x-none", "1", "Mozilla/5.0 (compatible; Googlebot/2.1)"),
    ] {
        let analytics = Arc::new(Analytics::default());
        let (_root, site) = web(analytics.clone());
        let (status, _, _) = send(
            &site,
            visit_request_as(agent)
                .header(name, value)
                .body(batch(serde_json::json!([["page_view", 1]])))
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::NO_CONTENT);
        assert!(analytics.visit_snapshot().is_empty(), "{name}");
        assert!(
            of(&analytics.snapshot(), Series::Event).is_empty(),
            "{name}"
        );
    }
    // The page itself starts no visit under either signal: its script
    // checks both before it makes an id.
    let loader = crate::pages::grid_home::loader();
    let check = loader.find("nav.globalPrivacyControl===true").unwrap();
    assert!(loader.find("nav.doNotTrack===\"1\"").unwrap() < check);
    assert!(check < loader.find("crypto.getRandomValues").unwrap());
    assert!(!loader.contains("document.cookie"));
    // Portals come from Verse's own events, the refused ones as `opened: no`.
    assert!(loader.contains("addEventListener(\"verse-zone-load\""));
    assert!(loader.contains("addEventListener(\"verse-zone-unavailable\""));
    assert!(loader.contains("opened:\"no\""));
}

fn visit_row(visit: &str, minute: u64, n: u32, event: &str, detail: &[(&str, &str)]) -> VisitRow {
    VisitRow {
        visit: visit.to_owned(),
        minute,
        n,
        route: "/".to_owned(),
        event: event.to_owned(),
        detail: detail
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect(),
    }
}

#[tokio::test]
async fn visit_rows_are_deleted_after_thirty_days() {
    let root = tempfile::tempdir().unwrap();
    let store = Store::disk(root.path().to_path_buf());
    let today = 20_000;
    let id = "aaaaaaaaaaaaaaaa";
    for age in [0, 29, 30, 45] {
        let minute = (today - age) * 24 * 60 + 61;
        let rows = vec![visit_row(id, minute, 1, "page_view", &[])];
        visits::write(&store, &rows, "inst").await.unwrap();
    }
    assert_eq!(visits::prune(&store, today).await.unwrap(), 2);
    for (age, kept) in [(0, 1), (29, 1), (30, 0), (45, 0)] {
        assert_eq!(
            visits::read_day(&store, today - age).await.unwrap().len(),
            kept,
            "{age} days old"
        );
    }
    assert_eq!(visits::prune(&store, today).await.unwrap(), 0);
    // A reader never shows a row past the window either.
    let old = visit_row(id, (today - 30) * 24 * 60, 1, "page_view", &[]);
    assert!(!visits::kept(&old, today));
    assert!(visits::kept(
        &visit_row(id, (today - 29) * 24 * 60, 1, "page_view", &[]),
        today
    ));

    // Flushing writes the rows apart from the hourly totals, and a second
    // flush rewrites the same hour rather than adding to it.
    let analytics = Analytics::new(Some(Store::disk(root.path().to_path_buf())), None);
    let admitted = visits::parse(
        serde_json::json!({"v": id, "r": "/", "e": [["page_view", 1]]})
            .to_string()
            .as_bytes(),
    )
    .unwrap();
    analytics.visit(&admitted);
    analytics.flush().await.unwrap();
    analytics.flush().await.unwrap();
    assert_eq!(analytics.load_visits(1).await.unwrap().len(), 1);
    let listed = store.list("visits/").await.unwrap();
    assert!(listed.iter().all(|name| name.starts_with("visits/")));
}

#[test]
fn the_funnel_paths_and_time_on_the_page_come_from_each_visits_order() {
    let a = "aaaaaaaaaaaaaaaa";
    let b = "bbbbbbbbbbbbbbbb";
    let c = "cccccccccccccccc";
    let rows = vec![
        // a: all the way to an answer, through Lagrange 2.
        visit_row(a, 10, 1, "page_view", &[]),
        visit_row(
            a,
            10,
            2,
            "verse_ready",
            &[("gpu", "webgpu"), ("time", "1-3s")],
        ),
        visit_row(a, 10, 3, "joined", &[("by", "keys")]),
        visit_row(a, 11, 4, "portal_entered", &[("zone", "lagrange2")]),
        visit_row(
            a,
            11,
            5,
            "zone_ready",
            &[("zone", "lagrange2"), ("time", "3-10s")],
        ),
        visit_row(a, 11, 6, "plaza_returned", &[("from", "lagrange2")]),
        visit_row(a, 12, 7, "terminal_reached", &[]),
        visit_row(a, 12, 8, "message_sent", &[("model", "deepseek")]),
        visit_row(a, 12, 9, "heartbeat", &[("time", "1-3m")]),
        visit_row(a, 13, 10, "answered", &[("source", "group")]),
        visit_row(a, 14, 11, "tab_left", &[("time", "3-10m")]),
        // b: joined, but sent a message before reaching the terminal is
        // impossible; out of order it stops at Joined.
        visit_row(b, 20, 3, "message_sent", &[]),
        visit_row(b, 20, 1, "page_view", &[]),
        visit_row(b, 20, 2, "verse_ready", &[]),
        visit_row(b, 21, 4, "joined", &[("by", "click")]),
        visit_row(b, 21, 5, "tab_hidden", &[("time", "10-30s")]),
        // c: the Grid never loaded; Game Lab from the page anyway.
        visit_row(c, 30, 1, "page_view", &[]),
        visit_row(
            c,
            30,
            2,
            "verse_load_failed",
            &[("gpu", "webgl"), ("reason", "gpu")],
        ),
        visit_row(c, 31, 3, "game_lab_opened", &[("result", "open")]),
        visit_row(c, 31, 4, "game_lab_check", &[]),
        visit_row(c, 32, 5, "game_lab_version_saved", &[]),
        visit_row(
            c,
            33,
            6,
            "portal_entered",
            &[("zone", "everglade"), ("opened", "no")],
        ),
    ];
    let all = grid::visits(&rows);
    assert_eq!(all.iter().map(|v| v.id).collect::<Vec<_>>(), [c, b, a]);
    assert_eq!(grid::funnel(&all), [3, 2, 2, 1, 1, 1]);
    assert_eq!(grid::game_funnel(&all), [1, 1, 1, 0]);
    let paths = grid::top_paths(&all, 10);
    assert!(paths.contains(&(
        "page_view > verse_ready > joined > portal_entered:lagrange2 > zone_ready:lagrange2 > plaza_returned > terminal_reached > message_sent > ...".to_owned(),
        1
    )), "{paths:?}");
    assert!(paths.contains(&(
        "page_view > verse_ready > message_sent > joined".to_owned(),
        1
    )));
    let times = grid::time_on_page(&all);
    assert_eq!(
        times[visits::TIME.iter().position(|t| *t == "3-10m").unwrap()],
        1
    );
    assert_eq!(
        times[visits::TIME.iter().position(|t| *t == "10-30s").unwrap()],
        1
    );
    assert_eq!(times.iter().sum::<u64>(), 2);
    let zones = grid::zones(&rows);
    assert_eq!(zones.len(), 2);
    assert_eq!(
        (
            zones[0].0,
            zones[0].1.entered,
            zones[0].1.ready,
            zones[0].1.returned
        ),
        ("lagrange2", 1, 1, 1)
    );
    // A portal the web can't open is counted apart, and named in the path.
    assert_eq!(
        (
            zones[1].0,
            zones[1].1.entered,
            zones[1].1.desktop_only,
            zones[1].1.ready
        ),
        ("everglade", 1, 1, 0)
    );
    assert!(
        paths
            .iter()
            .any(|(path, _)| path.ends_with("desktop_only:everglade")),
        "{paths:?}"
    );

    // The dashboard reads in plain words and links each visit's timeline.
    let hour = hour_now();
    let html =
        dashboard::report_with(&Counts::new(), hour, true, grid::report(&rows, 7)).into_string();
    assert!(html.contains("Grid funnel") && html.contains("Game Lab funnel"));
    assert!(html.contains(&format!("{DASHBOARD}?visit={a}")));
    crate::copy_guard::assert_plain("/admin/analytics", &html);
    let timeline = grid::timeline(&rows, a).into_string();
    assert!(timeline.contains("portal_entered") && timeline.contains("lagrange2"));
    crate::copy_guard::assert_plain("/admin/analytics", &timeline);
    assert!(
        grid::timeline(&rows, "ffffffffffffffff")
            .into_string()
            .contains("no visit")
    );
    assert_eq!(
        dashboard::Asked::from_query(Some(&format!("visit={a}&days=30"))),
        dashboard::Asked {
            visit: Some(a.to_owned()),
            days: 30
        }
    );
    assert_eq!(
        dashboard::Asked::from_query(Some("visit=../x&days=9")).days,
        7
    );
}

#[test]
fn models_are_kept_only_as_a_family_from_the_fixed_list() {
    for (model, family) in [
        ("gemini-2.5-flash", "gemini"),
        ("google/gemini-3-pro-preview", "gemini"),
        ("deepseek-v4.1-flash", "deepseek"),
        ("flint@sha256:abc", "flint"),
        ("claude-sonnet-4-5", "claude"),
        ("gpt-5.1-codex", "gpt"),
        ("openai/gpt-oss-120b", "gpt"),
        ("o3-mini", "gpt"),
        ("Qwen3-30B-A3B", "qwen"),
        ("gemma-3-4b-it", "gemma"),
        ("Llama-3.3-70B", "llama"),
        ("olmo-2", "other"),
        ("my-secret-finetune", "other"),
        ("", "other"),
    ] {
        assert_eq!(visits::model_family(model), family, "{model}");
        assert!(visits::FAMILIES.contains(&family));
    }
    // The visit takes a family word, never a model's name.
    let detail = |v: serde_json::Value| v.as_object().cloned().unwrap();
    assert!(
        visits::admit(
            "answered",
            Some(&detail(
                serde_json::json!({"source": "hosted", "family": "gemini"})
            ))
        )
        .is_ok()
    );
    assert_eq!(
        visits::admit(
            "answered",
            Some(&detail(serde_json::json!({"family": "gemini-2.5-flash"})))
        )
        .unwrap_err(),
        Refused::BadValue
    );
    // A portal the web can't open: by zone, `opened: no`.
    assert!(
        visits::admit(
            "portal_entered",
            Some(&detail(
                serde_json::json!({"zone": "physics-lab", "opened": "no"})
            ))
        )
        .is_ok()
    );
    assert_eq!(
        visits::admit(
            "portal_entered",
            Some(&detail(serde_json::json!({"zone": "physics-lab-v1"})))
        )
        .unwrap_err(),
        Refused::BadValue
    );
}
