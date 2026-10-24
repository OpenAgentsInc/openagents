//! Route tests for the launch playtest, the waitlist, the profile, and the
//! staff export. Each test sends from its own client address, so the
//! shared hourly limits never cross tests.

use std::sync::Arc;

use axum::body::{Body, to_bytes};
use axum::http::{HeaderMap, Request, StatusCode, header};
use tower::ServiceExt;

use super::events::{Step, kept};
use super::store::{self, Profile, Survey, WaitlistEntry};
use crate::analytics::Analytics;
use crate::{Config, router};

const HOST: &str = "127.0.0.1:4300";
const IPHONE: &str = "Mozilla/5.0 (iPhone; CPU iPhone OS 18_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.0 Mobile/15E148 Safari/604.1";
const STAFF_KEY: &str = "launch staff key";

struct Site {
    _root: tempfile::TempDir,
    router: axum::Router,
    store: Arc<crate::chat_store::Store>,
}

fn site() -> Site {
    kept::start();
    let root = tempfile::tempdir().unwrap();
    let mut config = Config::development(root.path().join("tasks"));
    config.analytics = Arc::new(Analytics::new(None, Some(STAFF_KEY)));
    let store = config.chat_store.clone();
    Site {
        _root: root,
        router: router(config),
        store,
    }
}

struct Reply {
    status: StatusCode,
    headers: HeaderMap,
    body: String,
}

impl Reply {
    /// The playtest cookie this reply set, as `name=value`.
    fn cookie(&self) -> Option<String> {
        self.headers
            .get_all(header::SET_COOKIE)
            .iter()
            .filter_map(|v| v.to_str().ok())
            .find(|v| v.starts_with("oa_playtest="))
            .map(|v| v.split(';').next().unwrap().to_owned())
    }
}

impl Site {
    async fn send(&self, request: Request<Body>) -> Reply {
        let response = self.router.clone().oneshot(request).await.unwrap();
        let status = response.status();
        let headers = response.headers().clone();
        let body = to_bytes(response.into_body(), 8 * 1024 * 1024)
            .await
            .unwrap();
        Reply {
            status,
            headers,
            body: String::from_utf8_lossy(&body).into_owned(),
        }
    }

    async fn get(&self, uri: &str, client: &str, cookie: Option<&str>) -> Reply {
        let mut request = Request::builder()
            .uri(uri)
            .header(header::HOST, HOST)
            .header(header::USER_AGENT, IPHONE)
            .header("x-forwarded-for", client);
        if let Some(cookie) = cookie {
            request = request.header(header::COOKIE, cookie);
        }
        self.send(request.body(Body::empty()).unwrap()).await
    }

    async fn post(&self, uri: &str, client: &str, cookie: Option<&str>, form: &str) -> Reply {
        let mut request = Request::builder()
            .method("POST")
            .uri(uri)
            .header(header::HOST, HOST)
            .header(header::USER_AGENT, IPHONE)
            .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
            .header("sec-fetch-site", "same-origin")
            .header("x-forwarded-for", client);
        if let Some(cookie) = cookie {
            request = request.header(header::COOKIE, cookie);
        }
        self.send(request.body(Body::from(form.to_owned())).unwrap())
            .await
    }
}

fn session_id(cookie: &str) -> String {
    cookie
        .trim_start_matches("oa_playtest=")
        .split('.')
        .next()
        .unwrap()
        .to_owned()
}

#[tokio::test]
async fn the_waitlist_keeps_one_entry_per_email_and_answers_the_same_either_way() {
    let site = site();
    let client = "203.0.113.1";
    let first = site
        .post(
            "/waitlist",
            client,
            None,
            "email=Ada%40Example.com&role=student&source=HN",
        )
        .await;
    assert_eq!(first.status, StatusCode::OK);
    assert!(first.body.contains("You're on the list."));
    crate::copy_guard::assert_plain(super::WAITLIST, &first.body);
    let again = site
        .post(
            "/waitlist",
            client,
            None,
            "email=ada%40example.com&role=player",
        )
        .await;
    assert_eq!(again.status, StatusCode::OK);
    assert_eq!(again.body, first.body);
    let kept: Vec<WaitlistEntry> = store::all(&site.store, store::WAITLIST).await.unwrap();
    assert_eq!(kept.len(), 1);
    assert_eq!(
        (
            kept[0].email.as_str(),
            kept[0].role.as_str(),
            kept[0].source.as_str()
        ),
        ("ada@example.com", "student", "hn")
    );
}

#[tokio::test]
async fn the_waitlist_refuses_a_bad_email_a_bot_and_another_site() {
    let site = site();
    let client = "203.0.113.2";
    let bad = site
        .post("/waitlist", client, None, "email=nope&role=student")
        .await;
    assert_eq!(bad.status, StatusCode::BAD_REQUEST);
    assert!(bad.body.contains("Enter an email address"));
    // What they typed is shown again.
    assert!(bad.body.contains(r#"value="nope""#));
    let role = site
        .post("/waitlist", client, None, "email=a%40b.io&role=wizard")
        .await;
    assert_eq!(role.status, StatusCode::BAD_REQUEST);
    // The trap field: thanked, nothing kept.
    let bot = site
        .post("/waitlist", client, None, "email=bot%40b.io&website=spam")
        .await;
    assert_eq!(bot.status, StatusCode::OK);
    let elsewhere = site
        .send(
            Request::builder()
                .method("POST")
                .uri("/waitlist")
                .header(header::HOST, HOST)
                .header("sec-fetch-site", "cross-site")
                .header("x-forwarded-for", client)
                .body(Body::from("email=x%40b.io"))
                .unwrap(),
        )
        .await;
    assert_eq!(elsewhere.status, StatusCode::FORBIDDEN);
    assert!(
        store::all::<WaitlistEntry>(&site.store, store::WAITLIST)
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn the_waitlist_limits_one_address_to_a_few_an_hour() {
    let site = site();
    let client = "203.0.113.3";
    for n in 0..super::WAITLIST_PER_HOUR {
        let reply = site
            .post("/waitlist", client, None, &format!("email=p{n}%40b.io"))
            .await;
        assert_eq!(reply.status, StatusCode::OK, "{n}");
    }
    let limited = site
        .post("/waitlist", client, None, "email=late%40b.io")
        .await;
    assert_eq!(limited.status, StatusCode::TOO_MANY_REQUESTS);
    assert!(limited.body.contains("Try again in an hour"));
    // Another address is unaffected.
    let other = site
        .post("/waitlist", "203.0.113.33", None, "email=late%40b.io")
        .await;
    assert_eq!(other.status, StatusCode::OK);
}

#[tokio::test]
async fn a_playtest_runs_from_the_intro_through_the_house_to_the_survey() {
    let site = site();
    let client = "203.0.113.4";
    let intro = site
        .get("/playtest?from=home&utm_source=Discord", client, None)
        .await;
    assert_eq!(intro.status, StatusCode::OK);
    for text in [
        "What we record",
        "Play without recording",
        "ABOUT 5 MINUTES",
        r#"name="source" value="discord""#,
    ] {
        assert!(intro.body.contains(text), "{text}");
    }
    crate::copy_guard::assert_plain(super::INTRO, &intro.body);

    let started = site
        .post("/playtest/start", client, None, "record=1&source=discord")
        .await;
    assert_eq!(started.status, StatusCode::SEE_OTHER);
    assert_eq!(started.headers[header::LOCATION], super::HOUSE);
    let cookie = started.cookie().unwrap();
    let id = session_id(&cookie);
    assert!(super::valid_session(&id), "{id}");
    assert!(cookie.ends_with(".1.discord"));
    let set = started.headers[header::SET_COOKIE].to_str().unwrap();
    assert!(set.contains("HttpOnly") && set.contains("SameSite=Lax"));

    // The house shows the Finish playtest button only during a playtest.
    let house = site.get(super::HOUSE, client, Some(&cookie)).await;
    if crate::pages::new_lander_house::available() {
        assert!(house.body.contains(r#"id="playtest-finish""#));
        assert!(house.body.contains(r#"src="/playtest/playtest.js""#));
        let plain = site.get(super::HOUSE, client, None).await;
        assert!(!plain.body.contains("playtest-finish"));
    }

    // Steps only the page sees.
    let beacon = |step: &str| {
        Request::builder()
            .method("POST")
            .uri(super::EVENTS)
            .header(header::HOST, HOST)
            .header(header::COOKIE, cookie.as_str())
            .header("sec-fetch-site", "same-origin")
            .body(Body::from(format!(r#"{{"step":"{step}"}}"#)))
            .unwrap()
    };
    assert_eq!(
        site.send(beacon("finish_prompted")).await.status,
        StatusCode::NO_CONTENT
    );
    // The page can't claim a step the server owns.
    assert_eq!(
        site.send(beacon("survey_submitted")).await.status,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        site.send(beacon("nonsense")).await.status,
        StatusCode::BAD_REQUEST
    );

    let form = site
        .get("/playtest/survey?from=finish", client, Some(&cookie))
        .await;
    assert_eq!(form.status, StatusCode::OK);
    assert!(form.cookie().is_none());
    for name in [
        "ease",
        "finished",
        "confused",
        "make",
        "again",
        "email",
        "contact_ok",
    ] {
        assert!(form.body.contains(&format!(r#"name="{name}""#)), "{name}");
    }
    crate::copy_guard::assert_plain(super::SURVEY, &form.body);

    let answers = "ease=4&finished=partial&confused=The+save+button&make=A+maze&again=9&email=Ada%40example.com&contact_ok=on";
    let thanks = site
        .post(super::SURVEY, client, Some(&cookie), answers)
        .await;
    assert_eq!(thanks.status, StatusCode::OK);
    assert!(thanks.body.contains("Thank you!"));
    assert!(
        thanks
            .body
            .contains("utm_source=friend&amp;utm_medium=share")
    );
    assert!(
        !thanks.body.contains(&id),
        "the share link never carries the session"
    );
    crate::copy_guard::assert_plain(super::SURVEY, &thanks.body);

    let surveys: Vec<Survey> = store::all(&site.store, store::SURVEYS).await.unwrap();
    assert_eq!(surveys.len(), 1);
    let survey = &surveys[0];
    assert_eq!(survey.session, id);
    assert_eq!(
        (survey.ease, survey.again, survey.finished.as_str()),
        (4, 9, "partial")
    );
    assert_eq!(
        (survey.device.as_str(), survey.browser.as_str()),
        ("mobile", "Safari")
    );
    assert_eq!(survey.email, "ada@example.com");
    assert!(survey.contact_ok && survey.recorded);
    assert_eq!(survey.source, "discord");

    // A second survey for the same playtest keeps the first.
    let again = site
        .post(
            super::SURVEY,
            client,
            Some(&cookie),
            "ease=1&finished=no&again=0",
        )
        .await;
    assert_eq!(again.status, StatusCode::OK);
    let revisit = site.get(super::SURVEY, client, Some(&cookie)).await;
    assert!(revisit.body.contains("We already have your answers"));
    let surveys: Vec<Survey> = store::all(&site.store, store::SURVEYS).await.unwrap();
    assert_eq!((surveys.len(), surveys[0].ease), (1, 4));

    assert_eq!(
        kept::steps(&id),
        [
            Step::PlaytestStarted,
            Step::FinishPrompted,
            Step::FinishClicked,
            Step::SurveyViewed,
            Step::SurveySubmitted,
        ]
    );
}

#[tokio::test]
async fn a_playtest_without_recording_and_a_browser_that_declines_send_no_session() {
    let site = site();
    let client = "203.0.113.5";
    let started = site.post("/playtest/start", client, None, "record=0").await;
    let cookie = started.cookie().unwrap();
    assert!(cookie.contains(".0."));
    let id = session_id(&cookie);
    site.get("/playtest/survey?from=finish", client, Some(&cookie))
        .await;
    let reply = site
        .post(
            super::SURVEY,
            client,
            Some(&cookie),
            "ease=5&finished=yes&again=10",
        )
        .await;
    assert_eq!(reply.status, StatusCode::OK);
    assert!(kept::steps(&id).is_empty());
    let surveys: Vec<Survey> = store::all(&site.store, store::SURVEYS).await.unwrap();
    assert!(!surveys[0].recorded);

    // Agreed, but the browser sends Global Privacy Control.
    let agreed = site
        .post("/playtest/start", client, None, "record=1")
        .await
        .cookie()
        .unwrap();
    let request = Request::builder()
        .uri("/playtest/survey?from=finish")
        .header(header::HOST, HOST)
        .header(header::COOKIE, agreed.as_str())
        .header("sec-gpc", "1")
        .body(Body::empty())
        .unwrap();
    site.send(request).await;
    assert_eq!(kept::steps(&session_id(&agreed)), [Step::PlaytestStarted]);
}

#[tokio::test]
async fn the_survey_checks_its_answers_and_works_without_a_playtest_cookie() {
    let site = site();
    let client = "203.0.113.6";
    let bad = site
        .post(super::SURVEY, client, None, "ease=9&finished=yes&again=3")
        .await;
    assert_eq!(bad.status, StatusCode::BAD_REQUEST);
    assert!(bad.body.contains("from 1 to 5"));
    // The answers given are kept on the form.
    assert!(bad.body.contains(r#"value="yes" checked"#));
    let follow = site
        .post(
            super::SURVEY,
            client,
            None,
            "ease=3&finished=yes&again=3&contact_ok=on",
        )
        .await;
    assert_eq!(follow.status, StatusCode::BAD_REQUEST);
    assert!(follow.body.contains("Add your email"));
    // Reaching the survey directly starts a playtest that records nothing.
    let direct = site.get(super::SURVEY, client, None).await;
    let cookie = direct.cookie().unwrap();
    assert!(cookie.contains(".0."));
    let kept = site
        .post(
            super::SURVEY,
            client,
            Some(&cookie),
            "ease=3&finished=no&again=5",
        )
        .await;
    assert_eq!(kept.status, StatusCode::OK);
    let surveys: Vec<Survey> = store::all(&site.store, store::SURVEYS).await.unwrap();
    assert_eq!(surveys.len(), 1);
    assert_eq!(surveys[0].session, session_id(&cookie));
}

#[tokio::test]
async fn the_survey_limits_one_address_per_hour() {
    let site = site();
    let client = "203.0.113.7";
    for n in 0..super::SURVEY_PER_HOUR {
        let reply = site
            .post(super::SURVEY, client, None, "ease=3&finished=no&again=5")
            .await;
        assert_eq!(reply.status, StatusCode::OK, "{n}");
    }
    let limited = site
        .post(super::SURVEY, client, None, "ease=3&finished=no&again=5")
        .await;
    assert_eq!(limited.status, StatusCode::TOO_MANY_REQUESTS);
}

#[tokio::test]
async fn a_guest_profile_needs_an_email_and_prefills_where_they_heard() {
    let site = site();
    let client = "203.0.113.8";
    let form = site
        .get("/playtest/profile?ref=Newsletter", client, None)
        .await;
    assert_eq!(form.status, StatusCode::OK);
    assert!(form.body.contains(r#"name="heard" maxlength="40" placeholder="A friend, a newsletter, a video…" value="newsletter""#));
    for (value, _) in super::forms::ENGINES {
        assert!(
            form.body
                .contains(&format!(r#"name="engines" value="{value}""#))
        );
    }
    crate::copy_guard::assert_plain(super::PROFILE, &form.body);
    let missing = site.post(super::PROFILE, client, None, "role=indie").await;
    assert_eq!(missing.status, StatusCode::BAD_REQUEST);
    assert!(missing.body.contains("Enter your email"));
    // The answers stay checked.
    assert!(missing.body.contains(r#"value="indie" checked"#));
    let saved = site
        .post(
            super::PROFILE,
            client,
            None,
            "role=student&engines=godot&engines=web&experience=new&make=A+cozy+game&heard=newsletter&email=Sam%40example.com&contact_ok=on",
        )
        .await;
    assert_eq!(saved.status, StatusCode::OK);
    assert!(saved.body.contains("we saved your profile"));
    let profiles: Vec<Profile> = store::all(&site.store, store::PROFILES).await.unwrap();
    assert_eq!(profiles.len(), 1);
    let profile = &profiles[0];
    assert_eq!(profile.email, "sam@example.com");
    assert_eq!(profile.engines, ["godot", "web"]);
    assert!(profile.contact_ok && profile.account_id.is_none());
}

#[tokio::test]
async fn the_export_is_for_staff_only() {
    let site = site();
    site.post(
        "/waitlist",
        "203.0.113.9",
        None,
        "email=x%40b.io&source=a%2Cb",
    )
    .await;
    site.post(
        super::SURVEY,
        "203.0.113.9",
        None,
        "ease=2&finished=no&again=1&confused=%3DHYPERLINK%28%22x%22%29%2C+and+%22quotes%22",
    )
    .await;
    let paths = [
        "/admin/launch",
        "/admin/launch/export.json",
        "/admin/launch/waitlist.csv",
        "/admin/launch/profiles.csv",
        "/admin/launch/surveys.csv",
    ];
    for path in paths {
        assert!(crate::upstream::owned(path), "{path}");
        // Signed out: sent to sign in, nothing shown.
        let anyone = site.get(path, "203.0.113.9", None).await;
        assert!(anyone.status.is_redirection(), "{path}: {}", anyone.status);
        assert!(!anyone.body.contains("x@b.io"));
        // A wrong key is refused too.
        let wrong = site
            .send(
                Request::builder()
                    .uri(path)
                    .header(header::HOST, HOST)
                    .header(header::AUTHORIZATION, "Bearer wrong")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await;
        assert!(!wrong.status.is_success(), "{path}");
        assert!(!wrong.body.contains("x@b.io"));
    }
    let staff = |path: &str| {
        Request::builder()
            .uri(path)
            .header(header::HOST, HOST)
            .header(header::AUTHORIZATION, format!("Bearer {STAFF_KEY}"))
            .body(Body::empty())
            .unwrap()
    };
    let hub = site.send(staff("/admin/launch")).await;
    assert_eq!(hub.status, StatusCode::OK);
    assert!(hub.body.contains("Launch sign-ups"));
    crate::copy_guard::assert_plain(super::ADMIN, &hub.body);
    let json = site.send(staff("/admin/launch/export.json")).await;
    assert_eq!(json.status, StatusCode::OK);
    assert_eq!(json.headers[header::CACHE_CONTROL], "private, no-store");
    let value: serde_json::Value = serde_json::from_str(&json.body).unwrap();
    assert_eq!(value["waitlist"][0]["email"], "x@b.io");
    assert_eq!(value["surveys"].as_array().unwrap().len(), 1);
    let waitlist = site.send(staff("/admin/launch/waitlist.csv")).await;
    assert!(
        waitlist.headers[header::CONTENT_TYPE]
            .to_str()
            .unwrap()
            .starts_with("text/csv")
    );
    assert!(
        waitlist
            .body
            .starts_with("created_unix,email,role,source\n")
    );
    assert!(waitlist.body.contains(",x@b.io,,ab\n"));
    let surveys = site.send(staff("/admin/launch/surveys.csv")).await;
    // A formula is never handed to a spreadsheet, and quotes are escaped.
    assert!(
        surveys
            .body
            .contains(r#""'=HYPERLINK(""x""), and ""quotes""""#),
        "{}",
        surveys.body
    );
    let unknown = site.send(staff("/admin/launch/secrets.csv")).await;
    assert_eq!(unknown.status, StatusCode::NOT_FOUND);
}

#[test]
fn a_cookie_is_read_back_only_when_well_formed() {
    let session = super::Session::new(true, "Twitter");
    let set = session.cookie(true);
    let set = set.to_str().unwrap();
    assert!(set.contains("; Secure"));
    let pair = set.split(';').next().unwrap();
    let mut headers = HeaderMap::new();
    headers.insert(header::COOKIE, format!("other=1; {pair}").parse().unwrap());
    assert_eq!(super::session(&headers), Some(session));
    for bad in [
        "oa_playtest=pt_123.1.x",
        "oa_playtest=pt_0123456789abcdef01234567.2.x",
        "oa_playtest=xx_0123456789abcdef01234567.1.x",
    ] {
        let mut headers = HeaderMap::new();
        headers.insert(header::COOKIE, bad.parse().unwrap());
        assert_eq!(super::session(&headers), None, "{bad}");
    }
}

#[test]
fn every_launch_route_is_this_sites_own() {
    for path in [
        super::INTRO,
        super::START,
        super::SURVEY,
        super::PROFILE,
        super::EVENTS,
        super::SCRIPT,
        super::WAITLIST,
        super::ADMIN,
        "/admin/launch/export.json",
    ] {
        assert!(crate::upstream::owned(path), "{path}");
    }
    assert!(!super::owns("/playtests") && !super::owns("/waitlists"));
}

#[test]
fn csv_cells_escape_quotes_and_formulas() {
    assert_eq!(super::cell("plain"), "plain");
    assert_eq!(super::cell("a,b"), "\"a,b\"");
    assert_eq!(super::cell("=1+1"), "'=1+1");
    assert_eq!(super::cell("-2"), "'-2");
    assert_eq!(super::cell("say \"hi\"\n"), "\"say \"\"hi\"\"\n\"");
}
