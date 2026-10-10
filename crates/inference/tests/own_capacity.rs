//! Own coding capacity (#11080): `openagents/code` under `pay: "mine"` on
//! the key owner's own linked computers and subscriptions, planned ahead
//! of the class table, failing over across the owner's accounts, and
//! metered at no charge.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use inference::error::ErrorType;
use inference::meter::{self, Api, Ledger, Meter, RateCard};
use inference::openagents::Payer;
use inference::request::CreateResponse;
use inference::router::{Bench, ClassTable, Context, Offering, PriceLimit, Scores, plan};
use inference::run::{CALLER_KEY, Caller, Gateway, OwnUpstreams, collect};
use inference::upstream::coder::{Agent, Linked, Report, Reports, Run, Runs, own_upstreams};
use inference::upstream::{AttemptError, BoxFuture, ErrorClass, Upstream};
use serde_json::json;

const OWNER: &str = "acct_owner";

fn linked(computer: &str, account: &str, agent: Agent, free: u32) -> Linked {
    Linked {
        owner: OWNER.into(),
        computer: computer.into(),
        computer_label: format!("{computer} desk"),
        account: account.into(),
        account_label: account.into(),
        agent,
        free_sessions: free,
    }
}

/// Runs that answer at once, except on spent accounts.
struct Desk {
    spent: Vec<String>,
    started: Mutex<Vec<String>>,
}

impl Runs for Desk {
    fn start(&self, run: Run) -> BoxFuture<'_, Result<Reports, AttemptError>> {
        Box::pin(async move {
            self.started.lock().unwrap().push(run.account.clone());
            if self.spent.contains(&run.account) {
                return Err(AttemptError::new(
                    ErrorClass::Payment,
                    "the subscription is spent",
                ));
            }
            let reports: Reports = Box::pin(futures_util::stream::iter(vec![
                Report::Step("Reading the repository.".into()),
                Report::Step("Running the tests.".into()),
                Report::Done {
                    text: "Opened the pull request.".into(),
                    usage: None,
                },
            ]));
            Ok(reports)
        })
    }
}

fn desk(spent: &[&str]) -> Arc<Desk> {
    Arc::new(Desk {
        spent: spent.iter().map(|&account| account.to_owned()).collect(),
        started: Mutex::new(Vec::new()),
    })
}

fn code(pay: &str, privacy: &str) -> CreateResponse {
    serde_json::from_value(json!({
        "model": "openagents/code",
        "input": "Fix the failing login test and open a pull request.",
        "stream": true,
        "openagents": {"pay": pay, "privacy": privacy}
    }))
    .unwrap()
}

/// The caller's own offerings, as the gateway marks them.
fn mine(upstreams: &[Arc<dyn Upstream>]) -> Vec<Offering> {
    upstreams
        .iter()
        .flat_map(|upstream| upstream.offerings())
        .map(|offering| Offering {
            payer: Payer::Mine,
            account: None,
            ..offering
        })
        .collect()
}

fn planned(
    offerings: &[Offering],
    scores: &Scores,
    request: &CreateResponse,
) -> Result<inference::router::Plan, inference::ApiError> {
    plan(
        request,
        &Context {
            offerings,
            classes: &ClassTable::default(),
            card: &RateCard::default(),
            ledger: &Ledger::default(),
            rates: &[],
            scores,
            bench: &Bench::default(),
            limits: PriceLimit::default(),
            judge: None,
            now_ms: 1_800_000_000_000,
        },
    )
}

#[test]
fn the_code_class_takes_the_owners_accounts_first_most_free_sessions_first() {
    let runs: Arc<dyn Runs> = desk(&[]);
    let upstreams = own_upstreams(
        OWNER,
        vec![
            linked("studio", "codex-a", Agent::Codex, 1),
            linked("laptop", "claude-a", Agent::ClaudeCode, 4),
        ],
        &runs,
    );
    let offerings = mine(&upstreams);
    let first = planned(&offerings, &Scores::default(), &code("mine", "standard")).unwrap();
    let route: Vec<(&str, &str)> = first
        .attempts
        .iter()
        .map(|candidate| (candidate.upstream.as_str(), candidate.model.as_str()))
        .collect();
    assert_eq!(
        route,
        [
            ("coder:laptop/claude-a", "coder/claude-code"),
            ("coder:studio/codex-a", "coder/codex"),
        ]
    );

    // A floor on the code class does not apply to the owner's own capacity.
    let mut scores = Scores::default();
    scores.by_class.insert(
        inference::router::TaskClass::Code,
        BTreeMap::from([("openai/gpt-5.6-sol".to_owned(), 0.9)]),
    );
    let mut floored = ClassTable::default();
    floored
        .classes
        .get_mut(&inference::router::TaskClass::Code)
        .unwrap()
        .floor = Some(0.8);
    let floored_plan = plan(
        &code("mine", "standard"),
        &Context {
            offerings: &offerings,
            classes: &floored,
            card: &RateCard::default(),
            ledger: &Ledger::default(),
            rates: &[],
            scores: &scores,
            bench: &Bench::default(),
            limits: PriceLimit::default(),
            judge: None,
            now_ms: 1_800_000_000_000,
        },
    )
    .unwrap();
    assert_eq!(floored_plan.attempts.len(), 2);
}

#[test]
fn own_capacity_is_never_offered_to_ours_or_to_strict_requests() {
    let runs: Arc<dyn Runs> = desk(&[]);
    let upstreams = own_upstreams(
        OWNER,
        vec![linked("studio", "codex-a", Agent::Codex, 2)],
        &runs,
    );
    let offerings = mine(&upstreams);
    // Paid by us: the caller's own capacity is not a candidate.
    let error = planned(&offerings, &Scores::default(), &code("ours", "standard")).unwrap_err();
    assert_eq!(error.kind, ErrorType::NoRoute);
    // Strict privacy: the subscription's provider terms are not verified.
    let error = planned(&offerings, &Scores::default(), &code("mine", "strict")).unwrap_err();
    assert_eq!(error.kind, ErrorType::NoRoute);
}

#[tokio::test]
async fn a_spent_account_falls_over_to_the_next_and_the_answer_is_free() {
    let desk = desk(&["codex-a"]);
    let runs: Arc<dyn Runs> = desk.clone();
    let own = own_upstreams(
        OWNER,
        vec![
            linked("studio", "codex-a", Agent::Codex, 3),
            linked("laptop", "codex-b", Agent::Codex, 1),
        ],
        &runs,
    );
    let meter = Arc::new(Meter::new(&meter::Config::default()));
    let gateway = Gateway::new(Vec::new(), meter.clone());
    let caller = Caller {
        request_id: "req_own".into(),
        api: Api::Responses,
        own: OwnUpstreams(own),
        ..Caller::default()
    };
    let routed = gateway
        .run(&code("mine", "standard"), &caller)
        .await
        .unwrap();
    assert_eq!(routed.upstream, "coder:laptop/codex-b");
    assert_eq!(routed.attempts.len(), 2);
    let response = collect(routed.events).await.unwrap();
    assert_eq!(response.output_text(), "Opened the pull request.");
    let info = response.openagents.clone().unwrap();
    assert_eq!(info.upstream, "coder:laptop/codex-b");
    // No price: the owner's subscription paid.
    assert!(info.cost.is_none());
    let progress = match &response.output[0] {
        inference::item::Item::Reasoning(reasoning) => reasoning.summary_text(),
        other => panic!("expected progress, got {other:?}"),
    };
    assert!(
        progress.starts_with("Working on laptop desk with your Codex account codex-b."),
        "{progress}"
    );
    assert_eq!(*desk.started.lock().unwrap(), ["codex-a", "codex-b"]);
    let attempts = meter.request("req_own");
    assert_eq!(attempts.len(), 2);
    assert!(
        attempts
            .iter()
            .all(|attempt| attempt.account.as_deref() == Some(CALLER_KEY))
    );
}
