//! Alice and Agent Studio acceptance against an isolated resident host.

use super::*;
use coder_access::{Right, Rights};
use std::sync::atomic::{AtomicUsize, Ordering};

const CONNECTION: &str = "/cloud/app/hosts/resident";
const AGENTS: &str = "/cloud/app/hosts/resident/agents";
const ALICE: &str = "/cloud/app/hosts/resident/agents/alice";

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

fn form_with<'a>(html: &'a str, action: &str) -> &'a str {
    html.split("<form ")
        .skip(1)
        .map(|form| form.split("</form>").next().unwrap())
        .find(|form| form.contains(&format!("action=\"{action}\"")))
        .unwrap_or_else(|| panic!("no form posts to {action}"))
}

/// A resident with Alice, whose engine counts the turns it starts.
async fn resident_with_alice(rights: Rights) -> (Fixture, Resident, Cookies, Arc<AtomicUsize>) {
    let mut fixture = fixture().await;
    let turns = Arc::new(AtomicUsize::new(0));
    let counted = turns.clone();
    let native = resident_with_inbox(
        &mut fixture,
        rights,
        true,
        false,
        false,
        move |inbox, private| {
            let root = private.join("agent-host");
            let workspace = private.join("alice-workspace");
            std::fs::create_dir_all(&workspace).unwrap();
            let store = coder::task::agent::Store::new(&root, "alice").unwrap();
            store.open(&workspace, now()).unwrap();
            let engine: coder::task::agent_host::EngineFactory = Arc::new(move |_record| {
                counted.fetch_add(1, Ordering::SeqCst);
                let turn = coder::task::coder_v1::Scripted {
                    ended: Some(coder::task::coder_v1::Ended::Finished {
                        reply: "Synthetic answer.".into(),
                        tokens: 0,
                    }),
                    ..coder::task::coder_v1::Scripted::default()
                };
                Ok((
                    Box::new(turn) as Box<dyn coder::task::coder_v1::Engine>,
                    "Synthetic engine".to_string(),
                ))
            });
            let agents = coder::task::agent_host::Agents::new(
                &root,
                private.join("resident-tasks"),
                BTreeMap::new(),
            )
            .with_engine(engine)
            // Relay the request as one step instead of planning with her
            // live model, whose unreachable endpoint costs seconds per turn.
            .with_mind(Arc::new(|_record| {
                Ok(coder::task::agent_steer::Mind::relay())
            }))
            .with_coder_state(private.join("agent-coder-state"));
            inbox.with_agents(agents)
        },
    )
    .await;
    let mut cookies = login(&fixture, "alice").await;
    choose_personal(&fixture, &mut cookies).await;
    (fixture, native, cookies, turns)
}

async fn enroll(fixture: &Fixture, cookies: &Cookies) {
    let page = get_page(fixture, cookies, CONNECTION).await;
    expect(&page, StatusCode::OK);
    let csrf = action_token(&page.body, &format!("{CONNECTION}/enroll"), None);
    let input = form(&[("csrf", &csrf), ("custody", "yes")]);
    let answer = post_form(fixture, cookies, &format!("{CONNECTION}/enroll"), &input).await;
    expect(&answer, StatusCode::SEE_OTHER);
}

async fn confirm(fixture: &Fixture, cookies: &Cookies, id: &str) -> Answer {
    let path = format!("{CONNECTION}/requests/{id}");
    let page = get_page(fixture, cookies, &path).await;
    expect(&page, StatusCode::OK);
    let action = format!("{path}/confirm");
    let csrf = action_token(&page.body, &action, None);
    post_form(fixture, cookies, &action, &form(&[("csrf", &csrf)])).await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn alice_and_studio_are_observed_with_honest_limits_and_no_controls_without_enrollment() {
    let (fixture, native, cookies, turns) =
        resident_with_alice(Rights::new([Right::Observe, Right::Operate]).unwrap()).await;
    let index = get_page(&fixture, &cookies, "/cloud/app/agents").await;
    expect(&index, StatusCode::OK);
    assert!(index.body.contains(&format!("href=\"{AGENTS}\"")));
    let page = get_page(&fixture, &cookies, AGENTS).await;
    expect(&page, StatusCode::OK);
    for text in [
        "id=\"agent-alice\"",
        "Unattested",
        "it grants no execution rights",
        "one running request and four waiting",
        "Coding-task runs bypass that meter",
        "Configuration: unavailable",
        "Agent Studio",
        "Enroll this browser at the Computer connection",
    ] {
        assert!(page.body.contains(text), "{text}: {}", page.body);
    }
    assert!(
        !page.body.contains("/studio/goals\""),
        "no control before enrollment"
    );
    let alice = get_page(&fixture, &cookies, ALICE).await;
    expect(&alice, StatusCode::OK);
    assert!(!alice.body.contains("name=\"mode\""));
    expect(
        &get_page(&fixture, &cookies, &format!("{AGENTS}/nobody")).await,
        StatusCode::NOT_FOUND,
    );
    assert_eq!(turns.load(Ordering::SeqCst), 0);
    native.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_explicit_mode_request_is_exact_and_a_retry_or_changed_bytes_never_repeat_work() {
    let (fixture, native, cookies, turns) =
        resident_with_alice(Rights::new([Right::Observe, Right::Operate]).unwrap()).await;
    enroll(&fixture, &cookies).await;
    let page = get_page(&fixture, &cookies, ALICE).await;
    expect(&page, StatusCode::OK);
    let ask = format!("{ALICE}/ask");
    let html = form_with(&page.body, &ask);
    assert!(html.contains("value=\"task\"") && html.contains("value=\"terminal\""));
    assert!(
        !html.contains("value=\"auto\""),
        "the host heuristic is never chosen"
    );
    let csrf = field(html, "csrf");
    let id = field(html, "request");
    // No mode, or the host's own choice, is refused before staging.
    for mode in ["", "auto"] {
        let input = form(&[
            ("csrf", &csrf),
            ("request", &id),
            ("mode", mode),
            ("text", "Summarize."),
        ]);
        expect(
            &post_form(&fixture, &cookies, &ask, &input).await,
            StatusCode::BAD_REQUEST,
        );
    }
    let input = form(&[
        ("csrf", &csrf),
        ("request", &id),
        ("mode", "terminal"),
        ("text", "Summarize the synthetic fixture."),
    ]);
    let staged = post_form(&fixture, &cookies, &ask, &input).await;
    expect(&staged, StatusCode::SEE_OTHER);
    assert_eq!(
        staged.headers[header::LOCATION],
        format!("{CONNECTION}/requests/{id}")
    );
    // Changed bytes under the same identity conflict.
    let changed = form(&[
        ("csrf", &csrf),
        ("request", &id),
        ("mode", "terminal"),
        ("text", "Delete the synthetic fixture."),
    ]);
    expect(
        &post_form(&fixture, &cookies, &ask, &changed).await,
        StatusCode::CONFLICT,
    );
    let answered = confirm(&fixture, &cookies, &id).await;
    expect(&answered, StatusCode::OK);
    assert!(
        answered.body.contains("studio.agent.ask"),
        "{}",
        answered.body
    );
    // The exact same form again reuses the packet; the receipt is terminal.
    let again = post_form(&fixture, &cookies, &ask, &input).await;
    expect(&again, StatusCode::SEE_OTHER);
    let receipt = get_page(&fixture, &cookies, &format!("{CONNECTION}/requests/{id}")).await;
    expect(&receipt, StatusCode::OK);
    assert!(
        !receipt.body.contains("/confirm\""),
        "an answered request offers no dispatch"
    );
    let start = std::time::Instant::now();
    while turns.load(Ordering::SeqCst) == 0 {
        assert!(start.elapsed() < std::time::Duration::from_secs(20));
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    assert_eq!(turns.load(Ordering::SeqCst), 1, "the request ran once");
    native.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn studio_controls_bind_the_displayed_stream_and_a_gap_refuses_them() {
    let (fixture, native, cookies, _) =
        resident_with_alice(Rights::new([Right::Observe, Right::Operate]).unwrap()).await;
    enroll(&fixture, &cookies).await;
    let page = get_page(&fixture, &cookies, AGENTS).await;
    expect(&page, StatusCode::OK);
    let action = format!("{CONNECTION}/studio/goals");
    let html = form_with(&page.body, &action);
    let fields = |stream: &str, sequence: &str| {
        form(&[
            ("csrf", &field(html, "csrf")),
            ("request", &field(html, "request")),
            ("stream", stream),
            ("sequence", sequence),
            ("workspace", "checkout"),
            ("text", "Synthetic goal."),
        ])
    };
    let stream = field(html, "stream");
    let sequence = field(html, "sequence");
    // Another stream or a point the host no longer holds needs a fresh snapshot.
    let gap = post_form(
        &fixture,
        &cookies,
        &action,
        &fields(&"f".repeat(stream.len()), &sequence),
    )
    .await;
    expect(&gap, StatusCode::CONFLICT);
    assert!(gap.body.contains("Fresh Studio snapshot required"));
    let ahead = post_form(&fixture, &cookies, &action, &fields(&stream, "999999")).await;
    expect(&ahead, StatusCode::CONFLICT);
    // The displayed point stages one exact goal submission for review.
    let staged = post_form(&fixture, &cookies, &action, &fields(&stream, &sequence)).await;
    expect(&staged, StatusCode::SEE_OTHER);
    // A review of an unknown task is unavailable, and offers no merge.
    let review = get_page(
        &fixture,
        &cookies,
        &format!(
            "{CONNECTION}/studio/tasks/unknown-task/review?stream={stream}&sequence={sequence}"
        ),
    )
    .await;
    assert_ne!(review.status, StatusCode::OK);
    assert!(!review.body.contains("name=\"verdict\""));
    native.stop().await;
}

#[test]
fn a_changed_or_removed_decision_or_task_is_touched_by_the_update() {
    use coder_access::studio::{Decision, DecisionKind, Kind, Removed, Update, View};
    let decision = Decision {
        decision: "d1".into(),
        goal: "g1".into(),
        task: None,
        seat: None,
        kind: DecisionKind::Question,
        text: "Which parser?".into(),
        based_on: 3,
        approval: None,
    };
    let mut update = Update {
        stream: "ab".into(),
        from: 1,
        sequence: 2,
        put: View::default(),
        removed: Vec::new(),
    };
    let touched = super::super::agents::touched;
    assert!(!touched(&update, Kind::Decision, "d1"));
    update.put.decisions.push(decision);
    assert!(touched(&update, Kind::Decision, "d1"));
    assert!(!touched(&update, Kind::Task, "d1"));
    update.removed.push(Removed {
        kind: Kind::Task,
        id: "t1".into(),
    });
    assert!(touched(&update, Kind::Task, "t1"));
}
