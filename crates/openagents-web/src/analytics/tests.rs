use std::sync::Arc;

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode, header};
use tower::ServiceExt;

use super::*;
use crate::{Config, router};

const LOCAL: &str = "127.0.0.1:4300";
const CHROME: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/129.0 Safari/537.36";

fn web(analytics: Arc<Analytics>) -> (tempfile::TempDir, axum::Router) {
    let root = tempfile::tempdir().unwrap();
    let mut config = Config::development(root.path().join("tasks"));
    config.analytics = analytics;
    let router = router(config);
    (root, router)
}

async fn send(router: &axum::Router, request: Request<Body>) -> (StatusCode, HeaderMap, String) {
    let response = router.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let body = to_bytes(response.into_body(), 8 * 1024 * 1024)
        .await
        .unwrap();
    (status, headers, String::from_utf8_lossy(&body).into_owned())
}

fn get(path: &str) -> axum::http::request::Builder {
    page(path, CHROME, "text/html")
}

fn page(path: &str, agent: &str, accept: &str) -> axum::http::request::Builder {
    Request::builder()
        .uri(path)
        .header(header::HOST, LOCAL)
        .header(header::USER_AGENT, agent)
        .header(header::ACCEPT, accept)
}

fn beacon_request(body: &str) -> axum::http::request::Builder {
    let _ = body;
    Request::builder()
        .method("POST")
        .uri(BEACON)
        .header(header::HOST, LOCAL)
        .header(header::USER_AGENT, CHROME)
        .header(header::CONTENT_TYPE, "text/plain;charset=UTF-8")
}

fn of(rows: &[Row], series: Series) -> Vec<&Row> {
    rows.iter().filter(|r| r.series == series).collect()
}

#[tokio::test]
async fn a_page_view_counts_its_template_referrer_and_device_and_sets_no_cookie() {
    let analytics = Arc::new(Analytics::default());
    let (_root, site) = web(analytics.clone());
    let (status, headers, _) = send(
        &site,
        get("/download")
            .header(header::REFERER, "https://news.ycombinator.com/item?id=1")
            .header("x-forwarded-for", "198.51.100.23")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(headers.get(header::SET_COOKIE).is_none());
    let policy = headers[header::CONTENT_SECURITY_POLICY].to_str().unwrap();
    assert!(policy.contains("script-src 'self'") && policy.contains("connect-src 'self'"));
    let rows = analytics.snapshot();
    let views = of(&rows, Series::View);
    assert_eq!(views.len(), 1, "{rows:?}");
    assert_eq!(
        (views[0].name.as_str(), views[0].detail.as_str()),
        ("/download", "human")
    );
    assert_eq!(of(&rows, Series::Referrer)[0].name, "ycombinator.com");
    assert_eq!(of(&rows, Series::Device)[0].name, "desktop");
    assert_eq!(of(&rows, Series::Status)[0].detail, "200");
    // The script, the beacon and the dashboard set nothing either.
    for request in [
        get(SCRIPT).body(Body::empty()).unwrap(),
        beacon_request("")
            .body(Body::from("e=install_copied&d=shell"))
            .unwrap(),
        get(DASHBOARD).body(Body::empty()).unwrap(),
    ] {
        let (_, headers, _) = send(&site, request).await;
        assert!(headers.get(header::SET_COOKIE).is_none());
    }
}

#[tokio::test]
async fn ids_in_addresses_become_templates_and_unknown_paths_one_name() {
    let analytics = Arc::new(Analytics::default());
    let (_root, site) = web(analytics.clone());
    send(
        &site,
        get("/chat/c-1234567890abcdef").body(Body::empty()).unwrap(),
    )
    .await;
    send(
        &site,
        get("/no/such/page/4242?email=a@b.c")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    send(&site, get("/docs/coder").body(Body::empty()).unwrap()).await;
    let names: Vec<String> = analytics
        .snapshot()
        .into_iter()
        .filter(|r| r.series == Series::Status)
        .map(|r| r.name)
        .collect();
    assert!(names.contains(&"/chat/{id}".to_owned()), "{names:?}");
    assert!(names.contains(&"(not found)".to_owned()), "{names:?}");
    assert!(names.contains(&"/docs/coder".to_owned()), "{names:?}");
    assert!(
        names
            .iter()
            .all(|n| !n.contains("1234") && !n.contains('@'))
    );
}

/// A tiny deterministic generator for the property test.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn pick<'a>(&mut self, items: &[&'a str]) -> &'a str {
        items[(self.next() % items.len() as u64) as usize]
    }
}

#[tokio::test]
async fn no_row_ever_holds_an_address_an_id_or_text_from_the_request() {
    let analytics = Arc::new(Analytics::default());
    let (_root, site) = web(analytics.clone());
    let mut rng = Rng(0x9e37_79b9_7f4a_7c15);
    let mut secrets = Vec::new();
    for _ in 0..300 {
        let ip = format!(
            "{}.{}.{}.{}",
            rng.next() % 223 + 1,
            rng.next() % 256,
            rng.next() % 256,
            rng.next() % 256
        );
        let ip6 = format!(
            "2001:db8:{:x}::{:x}",
            rng.next() % 65_535,
            rng.next() % 65_535
        );
        let id = format!("z{:016x}", rng.next());
        let path = match rng.next() % 6 {
            0 => format!("/chat/{id}"),
            1 => format!("/docs/{id}"),
            2 => format!("/{id}?q={id}"),
            3 => format!("/chat/{id}/transcript"),
            4 => "/".to_owned(),
            _ => "/download".to_owned(),
        };
        let referer = match rng.next() % 4 {
            0 => format!("https://{id}.example.com/{id}?ip={ip}"),
            1 => format!("http://{ip}/{id}"),
            2 => format!("http://[{ip6}]/x"),
            _ => String::new(),
        };
        let agent = rng.pick(&[
            CHROME,
            "curl/8.7.1",
            "Mozilla/5.0 (compatible; Googlebot/2.1)",
            "Claude-User/1.0",
            "Mozilla/5.0 (iPhone; CPU iPhone OS 18_0) Mobile",
        ]);
        let agent = format!("{agent} {id} {ip}");
        let mut request = page(&path, &agent, "text/html")
            .header("x-forwarded-for", format!("{ip}, {ip6}"))
            .header("x-real-ip", ip.as_str())
            .header("forwarded", format!("for={ip}"))
            .header(
                header::COOKIE,
                format!("oa_ask={id}; oa_cloud_session={id}"),
            )
            .header("x-openagents-account", id.as_str());
        if !referer.is_empty() {
            request = request.header(header::REFERER, referer);
        }
        send(&site, request.body(Body::empty()).unwrap()).await;
        let body = format!("e=install_copied&d={id}{ip}");
        send(&site, beacon_request("").body(Body::from(body)).unwrap()).await;
        secrets.push(ip);
        secrets.push(ip6);
        secrets.push(id);
    }
    let rows = analytics.snapshot();
    assert!(rows.len() > 5);
    for row in &rows {
        assert!(row.valid(), "{row:?}");
        for secret in &secrets {
            assert!(
                !row.name.contains(secret.as_str()) && !row.detail.contains(secret.as_str()),
                "{row:?} holds {secret}"
            );
        }
        // No field could hold an address: no digits-and-dots runs of four.
        for text in [&row.name, &row.detail] {
            let dotted = text
                .split(|c: char| !(c.is_ascii_digit() || c == '.'))
                .any(|part| part.split('.').filter(|p| !p.is_empty()).count() >= 4);
            assert!(!dotted, "{row:?}");
        }
    }
    // The row type itself has no place for anything else.
    let json = serde_json::to_value(&rows[0]).unwrap();
    let mut fields: Vec<_> = json.as_object().unwrap().keys().cloned().collect();
    fields.sort();
    assert_eq!(fields, ["count", "detail", "hour", "name", "series"]);
}

#[tokio::test]
async fn the_beacon_takes_only_its_fixed_events_and_values() {
    let analytics = Arc::new(Analytics::default());
    let (_root, site) = web(analytics.clone());
    let card = openagents_chat::home_cards::HOME_CARDS[1].href;
    let card_id = openagents_chat::home_cards::HOME_CARDS[1].id.to_owned();
    let label = openagents_chat::suggestions::SUGGESTIONS[0].label;
    let download = format!(
        "{}/1.0.0-rc.6/coder-1.0.0-rc.6-linux-x86_64-musl.tar.gz",
        crate::pages::download::CODER_BASE
    );
    let cases = [
        ("e=chat_sent&d=", StatusCode::BAD_REQUEST),
        ("e=signin_completed", StatusCode::BAD_REQUEST),
        (
            "e=card_clicked&d=https://evil.example/",
            StatusCode::BAD_REQUEST,
        ),
        ("e=install_copied&d=bash", StatusCode::BAD_REQUEST),
        (
            "e=starter_clicked&d=Tell+me+my+secrets",
            StatusCode::BAD_REQUEST,
        ),
        ("d=shell", StatusCode::BAD_REQUEST),
        ("e=install_copied&d=shell", StatusCode::NO_CONTENT),
    ];
    for (body, want) in cases {
        let (status, _, _) = send(&site, beacon_request("").body(Body::from(body)).unwrap()).await;
        assert_eq!(status, want, "{body}");
    }
    for (event, value) in [
        ("card_clicked", card.to_owned()),
        ("starter_clicked", label.to_owned()),
        ("download_clicked", download),
    ] {
        let body = url::form_urlencoded::Serializer::new(String::new())
            .append_pair("e", event)
            .append_pair("d", &value)
            .finish();
        let (status, _, _) = send(&site, beacon_request("").body(Body::from(body)).unwrap()).await;
        assert_eq!(status, StatusCode::NO_CONTENT, "{event}");
    }
    let (status, _, _) = send(
        &site,
        beacon_request("")
            .body(Body::from("x".repeat(2000)))
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);
    let mut events: Vec<(String, String)> = analytics
        .snapshot()
        .into_iter()
        .filter(|r| r.series == Series::Event)
        .map(|r| (r.name, r.detail))
        .collect();
    events.sort();
    let starter = openagents_chat::suggestions::SUGGESTIONS[0].id.to_owned();
    assert_eq!(
        events,
        [
            ("card_clicked".to_owned(), card_id),
            (
                "download_clicked".to_owned(),
                "coder-linux-x86_64-musl".to_owned()
            ),
            ("install_copied".to_owned(), "shell".to_owned()),
            ("starter_clicked".to_owned(), starter),
        ]
    );
}

#[tokio::test]
async fn do_not_track_and_global_privacy_control_leave_only_the_page_view() {
    for signal in ["dnt", "sec-gpc"] {
        let analytics = Arc::new(Analytics::default());
        let (_root, site) = web(analytics.clone());
        send(
            &site,
            get("/download")
                .header(signal, "1")
                .header(header::REFERER, "https://example.org/")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
        send(
            &site,
            get("/cli/install.sh")
                .header(signal, "1")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
        let (status, _, _) = send(
            &site,
            beacon_request("")
                .header(signal, "1")
                .body(Body::from("e=install_copied&d=shell"))
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::NO_CONTENT);
        let rows = analytics.snapshot();
        assert_eq!(of(&rows, Series::View).len(), 1, "{signal}: {rows:?}");
        assert!(of(&rows, Series::Referrer).is_empty(), "{signal}");
        assert!(of(&rows, Series::Device).is_empty(), "{signal}");
        assert!(of(&rows, Series::Event).is_empty(), "{signal}");
    }
}

#[tokio::test]
async fn agents_and_bots_are_their_own_series_and_bots_send_no_events() {
    let analytics = Arc::new(Analytics::default());
    let (_root, site) = web(analytics.clone());
    send(
        &site,
        page("/cli/install.sh", "curl/8.7.1", "*/*")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    send(
        &site,
        page(
            "/download",
            "Mozilla/5.0 (compatible; bingbot/2.0)",
            "text/html",
        )
        .body(Body::empty())
        .unwrap(),
    )
    .await;
    send(
        &site,
        page("/download", CHROME, "text/markdown")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    let rows = analytics.snapshot();
    let views: Vec<&str> = of(&rows, Series::View)
        .iter()
        .map(|r| r.detail.as_str())
        .collect();
    assert!(
        views.contains(&"bot") && views.contains(&"agent"),
        "{rows:?}"
    );
    let events: Vec<_> = of(&rows, Series::Event);
    assert_eq!(events.len(), 1);
    assert_eq!(
        (events[0].name.as_str(), events[0].detail.as_str()),
        ("installer_fetched", "shell")
    );
}

fn row(hour: u64, series: Series, name: &str, detail: &str, count: u64) -> Row {
    Row {
        hour,
        series,
        name: name.into(),
        detail: detail.into(),
        count,
    }
}

#[tokio::test]
async fn rollups_sum_every_instance_and_flushes_never_double_count() {
    let root = tempfile::tempdir().unwrap();
    let store = Store::disk(root.path().to_path_buf());
    let day = 20_000;
    let hour = day * 24 + 5;
    store
        .put(
            &format!("raw/{}/05/aaaa.json", date(day)),
            encode(vec![
                row(hour, Series::View, "/", "human", 3),
                row(hour, Series::Event, "chat_sent", "new", 1),
            ]),
        )
        .await
        .unwrap();
    store
        .put(
            &format!("raw/{}/06/bbbb.json", date(day)),
            encode(vec![
                row(hour + 1, Series::View, "/", "human", 4),
                row(hour, Series::View, "/", "human", 2),
                // Malformed rows from a bad writer are dropped.
                row(hour, Series::Event, "made_up", "", 9),
                row(hour, Series::View, "/Chat/198.51.100.7", "human", 9),
            ]),
        )
        .await
        .unwrap();
    let rolled = rollup(&store, day).await.unwrap();
    assert_eq!(
        rolled,
        [
            row(hour, Series::View, "/", "human", 5),
            row(hour, Series::Event, "chat_sent", "new", 1),
            row(hour + 1, Series::View, "/", "human", 4),
        ]
    );
    let daily = store
        .get(&format!("daily/{}.json", date(day)))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(decode(&daily), rolled);
    assert_eq!(date(day), "2024-10-04");

    // An instance rewrites its own hour whole, so flushing again (with or
    // without new counts) never adds what was already written.
    let analytics = Analytics::new(Some(Store::disk(root.path().to_path_buf())), None);
    analytics.event("chat_sent", "");
    analytics.flush().await.unwrap();
    analytics.flush().await.unwrap();
    analytics.event("chat_sent", "");
    analytics.flush().await.unwrap();
    let today = hour_now() / 24;
    let counts = analytics.load(1).await.unwrap();
    let sent: u64 = counts
        .iter()
        .filter(|(k, _)| k.series == Series::Event && k.name == "chat_sent" && k.hour / 24 == today)
        .map(|(_, n)| *n)
        .sum();
    assert_eq!(sent, 2);
}

#[tokio::test]
async fn the_dashboard_needs_its_key() {
    // No key configured: no page.
    let (_root, site) = web(Arc::new(Analytics::default()));
    let (status, _, _) = send(&site, get(DASHBOARD).body(Body::empty()).unwrap()).await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let analytics = Arc::new(Analytics::new(None, Some("correct horse")));
    let (_root, site) = web(analytics.clone());
    send(&site, get("/").body(Body::empty()).unwrap()).await;
    let (status, headers, body) = send(&site, get(DASHBOARD).body(Body::empty()).unwrap()).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert!(body.contains("type=\"password\"") && !body.contains("Top pages"));
    assert_eq!(headers[header::CACHE_CONTROL], "no-store");
    let (status, _, body) = send(
        &site,
        get(DASHBOARD)
            .header(header::AUTHORIZATION, "Bearer wrong")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert!(!body.contains("Top pages"));
    let (status, _, body) = send(
        &site,
        get(DASHBOARD)
            .header(header::AUTHORIZATION, "Bearer correct horse")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        body.contains("Top pages for people") && body.contains("<code>/</code>"),
        "{body}"
    );
    let (status, headers, body) = send(
        &site,
        Request::builder()
            .method("POST")
            .uri(DASHBOARD)
            .header(header::HOST, LOCAL)
            .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
            .body(Body::from(
                "username=openagents-analytics&key=correct+horse",
            ))
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("Top pages for people"));
    assert!(headers.get(header::SET_COOKIE).is_none());
    // The owner's own visits to the dashboard aren't counted.
    assert!(analytics.snapshot().iter().all(|r| r.name != DASHBOARD));
}

#[test]
fn the_dashboard_reads_in_plain_words() {
    let hour = hour_now();
    let mut counts = Counts::new();
    add(
        &mut counts,
        [
            row(hour, Series::View, "/", "human", 10),
            row(hour, Series::View, "/", "agent", 4),
            row(hour, Series::Event, "chat_sent", "new", 3),
            row(hour, Series::Event, "answer_shown", "", 2),
            row(hour, Series::Status, "/chat/{id}", "500", 1),
            row(hour, Series::Latency, "/", ">3s", 1),
        ],
    );
    let html = dashboard::report(&counts, hour, true).into_string();
    assert!(html.contains("oa-chart-bar") && html.contains("500 /chat/{id}"));
    crate::copy_guard::assert_plain("/admin/analytics", &html);
}
