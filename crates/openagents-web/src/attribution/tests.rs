//! `/go/` and `/r/` links, their cookie, consent and the scoreboard
//! (#11698). Sign-up attribution end to end is in `crate::auth::tests`.

use std::sync::Arc;

use axum::body::{Body, to_bytes};
use axum::http::{HeaderMap, Request, StatusCode, header};
use tower::ServiceExt;

use super::cookie::{self, Basis, Ticket};
use super::*;
use crate::{Config, router};

const LOCAL: &str = "127.0.0.1:4300";
const CHROME: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/129.0 Safari/537.36";
const TOKEN: &str = "rfr_00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff";

fn web(attribution: Arc<Attribution>) -> (tempfile::TempDir, axum::Router) {
    let root = tempfile::tempdir().unwrap();
    let mut config = Config::development(root.path().join("tasks"));
    config.attribution = attribution;
    (root, router(config))
}

/// Visitors from outside the consent countries, told by `x-country`.
fn noticed() -> Arc<Attribution> {
    Arc::new(Attribution::new(&[7; 32], None, Some("X-Country".into())))
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

fn visit(path: &str) -> axum::http::request::Builder {
    Request::builder()
        .uri(path)
        .header(header::HOST, LOCAL)
        .header(header::USER_AGENT, CHROME)
        .header(header::ACCEPT, "text/html")
        .header("sec-fetch-mode", "navigate")
}

fn answer(path: &str, choice: &str) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(path)
        .header(header::HOST, LOCAL)
        .header(header::USER_AGENT, CHROME)
        .header("sec-fetch-site", "same-origin")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(format!("choice={choice}")))
        .unwrap()
}

fn set_cookie(headers: &HeaderMap) -> Option<String> {
    headers
        .get_all(header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .find(|v| v.starts_with("oa_ref="))
        .map(str::to_owned)
}

fn value(set: &str) -> String {
    set.split(';')
        .next()
        .unwrap()
        .strip_prefix("oa_ref=")
        .unwrap()
        .to_owned()
}

fn key() -> [u8; 32] {
    cookie::derive_key(&[7; 32])
}

fn ticket(code: &str, at: u64) -> Ticket {
    Ticket {
        code: Some(code.into()),
        token: None,
        series: Some("cost".into()),
        click: format!("clk_{}", "1".repeat(32)),
        at,
        basis: Basis::Notice,
    }
}

#[tokio::test]
async fn a_click_where_notice_is_enough_sets_the_signed_cookie_and_records_it() {
    let attribution = noticed();
    let (_root, site) = web(attribution.clone());
    let (status, headers, _) = send(
        &site,
        visit("/go/yt-cost")
            .header("x-country", "US")
            .header("x-forwarded-for", "198.51.100.23")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::SEE_OTHER);
    assert_eq!(headers[header::LOCATION], "/pricing");
    let set = set_cookie(&headers).expect("cookie");
    assert!(set.contains("HttpOnly") && set.contains("SameSite=Lax") && set.contains("Path=/"));
    assert!(set.contains(&format!("Max-Age={WINDOW}")));
    let ticket = cookie::decode(&value(&set), &key(), now()).unwrap();
    assert_eq!(ticket.code.as_deref(), Some("yt-cost"));
    assert_eq!(ticket.basis, Basis::Notice);
    let days = attribution.snapshot();
    let clicks: Vec<_> = days.values().flat_map(|d| d.clicks.clone()).collect();
    assert_eq!(clicks.len(), 1);
    assert_eq!(clicks[0].id, ticket.click);
    assert_eq!(clicks[0].key, "yt-cost");
    assert_eq!(clicks[0].traffic, "human");
    assert!(clicks[0].credited);
    // Nothing from the request but the link is kept.
    let stored = serde_json::to_string(&days).unwrap();
    assert!(!stored.contains("198.51.100.23") && !stored.contains("Chrome"));
}

#[tokio::test]
async fn the_last_click_wins() {
    let (_root, site) = web(noticed());
    let (_, first, _) = send(
        &site,
        visit("/go/x-cost")
            .header("x-country", "US")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    let first = value(&set_cookie(&first).unwrap());
    let (_, second, _) = send(
        &site,
        visit(&format!("/r/{TOKEN}/launch"))
            .header("x-country", "US")
            .header(header::COOKIE, format!("oa_ref={first}"))
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    let set = set_cookie(&second).unwrap();
    let ticket = cookie::decode(&value(&set), &key(), now()).unwrap();
    assert_eq!(ticket.token.as_deref(), Some(TOKEN));
    assert_eq!(ticket.series.as_deref(), Some("launch"));
    assert!(ticket.code.is_none());
}

#[test]
fn the_cookie_lasts_thirty_days_and_only_with_our_signature() {
    let at = 1_760_000_000;
    let value = cookie::encode(&ticket("yt-cost", at), &key());
    assert!(cookie::decode(&value, &key(), at + WINDOW).is_some());
    assert!(cookie::decode(&value, &key(), at + WINDOW + 1).is_none());
    // Another server's key, an edited payload, or a code not on our list.
    assert!(cookie::decode(&value, &cookie::derive_key(&[8; 32]), at).is_none());
    let (body, signature) = value.rsplit_once('.').unwrap();
    let edited = format!("{body}x.{signature}");
    assert!(cookie::decode(&edited, &key(), at).is_none());
    let foreign = cookie::encode(&ticket("yt-somethingelse", at), &key());
    assert!(cookie::decode(&foreign, &key(), at).is_none());
}

#[tokio::test]
async fn where_consent_is_needed_the_visitor_is_asked_first() {
    for country in [Some("DE"), Some("gb"), None] {
        let attribution = noticed();
        let (_root, site) = web(attribution.clone());
        let mut request = visit("/go/tt-capability");
        if let Some(country) = country {
            request = request.header("x-country", country);
        }
        let (status, headers, body) = send(&site, request.body(Body::empty()).unwrap()).await;
        assert_eq!(status, StatusCode::OK, "{country:?}");
        assert!(set_cookie(&headers).is_none());
        assert!(body.contains("Before you go on") && body.contains("our TikTok video"));
        assert!(body.contains("action=\"/go/tt-capability\""));
        crate::copy_guard::assert_plain("/go/tt-capability", &body);
        assert!(attribution.snapshot().values().all(|d| d.clicks.is_empty()));

        // Yes: the cookie, on consent.
        let (status, headers, _) = send(&site, answer("/go/tt-capability", "allow")).await;
        assert_eq!(status, StatusCode::SEE_OTHER);
        assert_eq!(headers[header::LOCATION], "/");
        let set = set_cookie(&headers).unwrap();
        let ticket = cookie::decode(&value(&set), &key(), now()).unwrap();
        assert_eq!(ticket.basis, Basis::Consent);
    }
}

#[tokio::test]
async fn without_a_country_header_everyone_is_asked() {
    let (_root, site) = web(Arc::new(Attribution::new(&[7; 32], None, None)));
    let (status, headers, body) = send(
        &site,
        visit(&format!("/r/{TOKEN}"))
            .header("x-client-geo-location", "US")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(set_cookie(&headers).is_none());
    assert!(body.contains("someone who recommends us"));
    crate::copy_guard::assert_plain("/r/{token}", &body);
}

#[tokio::test]
async fn declined_consent_and_do_not_track_set_nothing_and_only_count() {
    let attribution = Arc::new(Attribution::new(&[7; 32], None, None));
    let (_root, site) = web(attribution.clone());
    let (status, headers, _) = send(&site, answer("/go/yt-cost", "decline")).await;
    assert_eq!(status, StatusCode::SEE_OTHER);
    assert!(set_cookie(&headers).is_none());
    for signal in ["sec-gpc", "dnt"] {
        let (status, headers, _) = send(
            &site,
            visit("/go/yt-cost")
                .header(signal, "1")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::SEE_OTHER);
        assert_eq!(headers[header::LOCATION], "/pricing");
        assert!(set_cookie(&headers).is_none());
    }
    // A post from another site sets nothing either.
    let cross = Request::builder()
        .method("POST")
        .uri("/go/yt-cost")
        .header(header::HOST, LOCAL)
        .header(header::USER_AGENT, CHROME)
        .header("sec-fetch-site", "cross-site")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from("choice=allow"))
        .unwrap();
    let (_, headers, _) = send(&site, cross).await;
    assert!(set_cookie(&headers).is_none());
    let days = attribution.snapshot();
    assert!(days.values().all(|d| d.clicks.is_empty()));
    let totals: Vec<_> = days.values().flat_map(|d| d.totals.clone()).collect();
    let of = |c: Counted| {
        totals
            .iter()
            .filter(|t| t.counted == c)
            .map(|t| t.count)
            .sum::<u64>()
    };
    assert_eq!(of(Counted::Declined), 1);
    assert_eq!(of(Counted::Quiet), 2);
}

#[tokio::test]
async fn agents_bots_and_prefetches_are_never_credited() {
    let attribution = noticed();
    let (_root, site) = web(attribution.clone());
    for agent in ["Googlebot/2.1", "claude-code/1.0", "curl/8.4.0", ""] {
        let request = Request::builder()
            .uri("/go/li-download")
            .header(header::HOST, LOCAL)
            .header(header::USER_AGENT, agent)
            .header("x-country", "US")
            .body(Body::empty())
            .unwrap();
        let (status, headers, _) = send(&site, request).await;
        assert_eq!(status, StatusCode::SEE_OTHER, "{agent}");
        assert_eq!(headers[header::LOCATION], "/download");
        assert!(set_cookie(&headers).is_none(), "{agent}");
    }
    let (_, headers, _) = send(
        &site,
        visit("/go/li-download")
            .header("x-country", "US")
            .header("sec-purpose", "prefetch")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert!(set_cookie(&headers).is_none());
    let clicks: Vec<_> = attribution
        .snapshot()
        .values()
        .flat_map(|d| d.clicks.clone())
        .collect();
    assert_eq!(clicks.len(), 4);
    assert!(clicks.iter().all(|c| !c.credited && c.traffic != "human"));
}

#[tokio::test]
async fn unknown_codes_and_malformed_tokens_go_home_and_keep_nothing_typed() {
    let attribution = noticed();
    let (_root, site) = web(attribution.clone());
    for path in [
        "/go/yt-secret-campaign",
        "/r/not-a-token",
        "/r/rfr_short/cost",
    ] {
        let (status, headers, _) = send(
            &site,
            visit(path)
                .header("x-country", "US")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::SEE_OTHER, "{path}");
        assert_eq!(headers[header::LOCATION], "/");
        assert!(set_cookie(&headers).is_none());
    }
    // An affiliate's odd series is dropped, the click still counts.
    let (_, headers, _) = send(
        &site,
        visit(&format!("/r/{TOKEN}/Not%20A%20Series"))
            .header("x-country", "US")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    let ticket = cookie::decode(&value(&set_cookie(&headers).unwrap()), &key(), now()).unwrap();
    assert!(ticket.series.is_none());
    let stored = serde_json::to_string(&attribution.snapshot()).unwrap();
    for typed in ["secret", "not-a-token", "rfr_short", "Series", TOKEN] {
        assert!(!stored.contains(typed), "{typed} kept: {stored}");
    }
    assert!(stored.contains(&cookie::affiliate_key(TOKEN)));
}

#[test]
fn every_code_is_a_channel_and_series_with_a_page_of_ours() {
    use tenancy::accounts::referrals::campaign_word;
    let mut seen = std::collections::BTreeSet::new();
    for code in codes::CODES {
        assert!(campaign_word(code.code), "{}", code.code);
        assert!(codes::CHANNELS.contains(&code.channel()), "{}", code.code);
        assert!(!code.series().is_empty());
        assert!(
            crate::upstream::owned(code.destination),
            "{}",
            code.destination
        );
        assert!(seen.insert(code.code), "{} twice", code.code);
    }
    assert!(codes::find("yt-cost").is_some() && codes::find("x-cost").is_some());
    assert!(codes::find("tt-capability").is_some());
}

#[tokio::test]
async fn the_scoreboard_is_a_plain_404_without_admin_and_counts_per_link_and_week() {
    let attribution = noticed();
    let root = tempfile::tempdir().unwrap();
    let mut config = Config::development(root.path().join("tasks"));
    config.attribution = attribution.clone();
    config.analytics = Arc::new(crate::analytics::Analytics::new(None, Some("board-key")));
    let site = router(config);
    let (status, _, _) = send(&site, visit(SCOREBOARD).body(Body::empty()).unwrap()).await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    for _ in 0..3 {
        send(
            &site,
            visit("/go/yt-cost")
                .header("x-country", "US")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
    }
    send(
        &site,
        visit("/go/yt-cost")
            .header("dnt", "1")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    attribution.signed_up(&Ticket {
        click: format!("clk_{}", "2".repeat(32)),
        ..ticket("yt-cost", now())
    });
    let (status, headers, body) = send(
        &site,
        visit(SCOREBOARD)
            .header(header::AUTHORIZATION, "Bearer board-key")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers[header::CACHE_CONTROL], "no-store");
    crate::copy_guard::assert_plain(SCOREBOARD, &body);
    assert!(body.contains("<code>yt-cost</code>"));
    let scores = scoreboard::scores(&attribution.snapshot());
    let (_, score) = scores.iter().next().unwrap();
    assert_eq!((score.clicks, score.signups), (3, 1));
    assert!(body.contains("33.3%"));
    assert!(body.contains("do not track"));
}

#[test]
fn weeks_start_on_monday() {
    // 2026-10-15 is a Thursday; its week starts Monday 2026-10-12.
    let thursday = 20_741;
    assert_eq!(crate::analytics::date(thursday), "2026-10-15");
    assert_eq!(
        crate::analytics::date(scoreboard::week_of(thursday)),
        "2026-10-12"
    );
    assert_eq!(scoreboard::week_of(thursday - 3), thursday - 3);
}

#[tokio::test]
async fn a_link_click_leaves_only_its_template_in_the_analytics_counts() {
    let analytics = Arc::new(crate::analytics::Analytics::default());
    let root = tempfile::tempdir().unwrap();
    let mut config = Config::development(root.path().join("tasks"));
    config.analytics = analytics.clone();
    config.attribution = noticed();
    let site = router(config);
    for path in ["/go/yt-cost".to_owned(), format!("/r/{TOKEN}/cost")] {
        send(
            &site,
            visit(&path)
                .header("x-country", "US")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
    }
    let rows = analytics.snapshot();
    assert!(!rows.is_empty());
    for row in &rows {
        for text in [&row.name, &row.detail] {
            assert!(
                !text.contains("yt-cost") && !text.contains("rfr_"),
                "{row:?}"
            );
        }
    }
    assert!(rows.iter().any(|r| r.name == "/go/{code}"));
    assert!(rows.iter().any(|r| r.name == "/r/{token}/{series}"));
}
