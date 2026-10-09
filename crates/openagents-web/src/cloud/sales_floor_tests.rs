//! WEB-15 floor supervision against a scripted owner adapter that speaks the
//! real remote protocol types. The canonical owner behavior (exact subject
//! decisions, stop fencing, unknown delivery) is covered against the real
//! sales books in `coder::task::sales::agents::tests::remote_floor`; here the
//! site's journaling, exact review binding, expiry, and disclosure are checked
//! with synthetic people only.

use super::*;
use coder::task::sales::remote::{self, Code, Effect, Op, Settled, Standing};
use coder::task::sales::{Receipt, Role, Stage, agents, expenses, floor, meetings, outbox};
use coder::task::sales::{replies, town};
use std::sync::atomic::{AtomicBool, Ordering};

const PAGE: &str = "/cloud/app/sales/owner-floor";
const CONTACT: &str = "prospect@synthetic.invalid";
const BODY: &str = "Synthetic pilot scope for one repository maintenance task.";
const SITE_BEARER: &str = "synthetic-floor-site-bearer";
const LEAD: &str = "lead_0000000000000000000000000000000000000000000000000000000000000001";

#[derive(Default)]
struct Script {
    revision: u64,
    approved: Option<bool>,
    paused: bool,
    supervise: bool,
    board_age: u64,
    board_fails: bool,
    applied: Vec<String>,
    journal: BTreeMap<String, (String, Receipt)>,
}

struct Owner {
    script: Arc<Mutex<Script>>,
    lose: Arc<AtomicBool>,
    url: String,
    server: tokio::task::JoinHandle<()>,
}

impl Drop for Owner {
    fn drop(&mut self) {
        self.server.abort();
    }
}

fn subject() -> String {
    "ab".repeat(32)
}

fn report(paused: bool) -> floor::Report {
    floor::Report {
        schema: floor::REPORT_SCHEMA.into(),
        generated_at: now(),
        owner: "operator".into(),
        stages: [("qualified".to_string(), 1)].into(),
        leads: 1,
        accepted_assignments: 1,
        next_actions_due: 1,
        next_actions_overdue: 0,
        messages: floor::Messages::default(),
        delivery: floor::Delivery {
            unknown: 1,
            ..Default::default()
        },
        meetings: BTreeMap::new(),
        certification: BTreeMap::new(),
        unresolved_incidents: 0,
        unresolved_replies: 1,
        costs: floor::Costs {
            reserved_estimate_usd_millionths: 120_000,
            billed_usd_millionths: floor::Measure::Unknown {
                reason: "provider billing not imported".into(),
            },
            reservations_unknown: 1,
            reservations_breached: 0,
            per_qualified_lead_usd_millionths: floor::Measure::Unknown {
                reason: "billing unknown".into(),
            },
        },
        outbox_paused: paused,
        gaps: vec![],
    }
}

fn floor_view(script: &Script) -> remote::Floor {
    let at = now();
    remote::Floor {
        schema: remote::FLOOR_SCHEMA.into(),
        observed_at: at,
        business_day: 20_000,
        timezone: remote::TIMEZONE.into(),
        ceiling_usd_millionths: expenses::FLOOR_USD_MILLIONTHS,
        paul: Some(remote::Paul {
            idle: false,
            model_available: false,
            qualification_inferred: false,
            external_effects: false,
            rows: vec![remote::PaulRow {
                lead: LEAD.into(),
                revision: 2,
                stage: Stage::Qualified,
                pending_drafts: 1,
                meetings_awaiting_owner: 0,
            }],
        }),
        crew: vec![remote::Member {
            name: "paul".into(),
            role: town::Role::Leader,
            lifecycle: town::Lifecycle::Active,
            station: Some("agora:office".into()),
            activity: Some("drafting".into()),
            source_kind: Some("draft".into()),
            queued: 0,
            idle: false,
        }],
        pending_hires: 0,
        certifications: vec![remote::Certification {
            agent: "paul".into(),
            version: 1,
            state: agents::CertState::InTraining,
            measured_qualified: false,
            expires_at: at + 3600,
            outbound_authority: false,
        }],
        reservations: vec![remote::Reservation {
            id: "reservation-one".into(),
            agent: "paul".into(),
            day: 20_000,
            status: expenses::Status::Unknown,
            execution_unknown: true,
            training: false,
            maximum_usd_millionths: 120_000,
            estimated_usd_millionths: None,
            billed_usd_millionths: None,
        }],
        report: report(script.paused),
        escalations: vec![],
        outbox: remote::Outbox {
            revision: script.revision,
            controller_epoch: 1,
            paused: script.paused,
            cap: 5,
            live_messages_and_reservations: 0,
            fixture_messages_and_reservations: 1,
            rows: vec![remote::OutboxRow {
                id: "original".into(),
                lead: LEAD.into(),
                phase: phase(script),
                mode: outbox::Mode::Fixture,
                kind: outbox::MessageKind::FirstMessage,
                business_day: 20_000,
                expires_at: at + 600,
                subject_sha256: subject(),
                count_consumed: false,
                decided: script.approved,
                reviewable: true,
            }],
            incidents: vec![],
            outbound_authority: false,
        },
        replies: vec![remote::ReplyRow {
            id: "reply-one".into(),
            lead: Some(LEAD.into()),
            received_at: at - 60,
            safety: replies::Safety::Injection,
            owner_label: None,
            minimized: false,
        }],
        meetings: vec![remote::MeetingRow {
            id: "meeting-one".into(),
            lead: LEAD.into(),
            phase: meetings::Phase::Pending,
            start_at: at + 86_400,
            end_at: at + 88_200,
            owner_confirmation_needed: true,
        }],
    }
}

fn phase(script: &Script) -> outbox::Phase {
    match script.approved {
        None => outbox::Phase::Proposed,
        Some(true) => outbox::Phase::Approved,
        Some(false) => outbox::Phase::Rejected,
    }
}

fn result(value: impl serde::Serialize) -> Response {
    Json(json!({"schema":remote::RESPONSE_SCHEMA,"result":value})).into_response()
}

fn refuse(code: Code) -> Response {
    (
        StatusCode::from_u16(code.status()).unwrap(),
        Json(json!({"schema":remote::RESPONSE_SCHEMA,"error":code.as_str()})),
    )
        .into_response()
}

fn digest_hex(bytes: &[u8]) -> String {
    use sha2::Digest;
    sha2::Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

async fn scripted(
    State((script, lose)): State<(Arc<Mutex<Script>>, Arc<AtomicBool>)>,
    headers: HeaderMap,
    body: String,
) -> Response {
    if headers
        .get(header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
        != Some(&format!("Bearer {SITE_BEARER}"))
        || headers.contains_key(header::ORIGIN)
    {
        return refuse(Code::AccessDenied);
    }
    let request: remote::Request = serde_json::from_str(&body).unwrap();
    let mut s = script.lock().unwrap();
    let supervised = s.supervise;
    match request.op {
        Op::Standing => result(Standing {
            binding: "owner-floor".into(),
            principal: "operator".into(),
            role: Role::Owner,
            effects: vec![Effect::OutboxDecide, Effect::OutboxStop],
            supervise: supervised,
        }),
        Op::Floor if supervised => result(floor_view(&s)),
        Op::Board if supervised && !s.board_fails => {
            let at = now() - s.board_age;
            result(remote::Board {
                schema: remote::BOARD_SCHEMA.into(),
                observed_at: at,
                expires_at: at + remote::BOARD_TTL_SECONDS,
                pipeline: [0, 1, 0, 0, 0],
                pending_drafts: 1,
                certifications: [0, 1, 0],
                practice_runs: 2,
                meeting_proposals: 1,
                outbox_live_proposals: 0,
                outbox_fixture_proposals: 1,
                outbox_unknown: 0,
                idle: false,
                model_available: false,
                shared: None,
            })
        }
        Op::Board if supervised => refuse(Code::Unavailable),
        Op::Proposal { proposal } if supervised && proposal == "original" => {
            result(remote::Proposal {
                schema: remote::PROPOSAL_SCHEMA.into(),
                outbox_revision: s.revision,
                controller_epoch: 1,
                paused: s.paused,
                id: "original".into(),
                lead: LEAD.into(),
                phase: phase(&s),
                mode: outbox::Mode::Fixture,
                kind: outbox::MessageKind::FirstMessage,
                business_day: 20_000,
                expires_at: now() + 600,
                subject_sha256: subject(),
                sender: "human:operator".into(),
                recipient: CONTACT.into(),
                subject: outbox::QUALIFIED_AGENT_SUBJECT.into(),
                body: BODY.into(),
                attachments: vec![],
                certification_reference: None,
                draft_reference: Some("draft-one".into()),
                maximum_cost_microusd: 0,
            })
        }
        Op::Reconcile { request, digest } => match s.journal.get(&request) {
            Some((exact, _)) if *exact != digest => refuse(Code::Conflict),
            Some((_, receipt)) => result(Settled::Recorded {
                receipt: receipt.clone(),
            }),
            None => result(Settled::Absent),
        },
        Op::Outbox { request, command } => {
            let exact = digest_hex(command.as_bytes());
            if let Some((digest, receipt)) = s.journal.get(&request) {
                return if *digest == exact {
                    result(receipt.clone())
                } else {
                    refuse(Code::Conflict)
                };
            }
            let parsed: outbox::Command = serde_json::from_str(&command).unwrap();
            assert_eq!(parsed.id, request);
            if parsed.expected_revision != s.revision {
                return refuse(Code::Stale);
            }
            let outcome = match parsed.operation {
                outbox::Operation::Decide {
                    proposal,
                    subject_sha256,
                    approve,
                } => {
                    if proposal != "original"
                        || subject_sha256 != subject()
                        || s.approved.is_some()
                        || s.paused
                    {
                        return refuse(Code::Refused);
                    }
                    s.approved = Some(approve);
                    "outbox_decided"
                }
                outbox::Operation::Pause {
                    incident: outbox::IncidentKind::OwnerStop,
                    ..
                } => {
                    s.paused = true;
                    "outbox_stopped"
                }
                _ => return refuse(Code::AccessDenied),
            };
            s.revision += 1;
            let receipt = Receipt {
                schema: remote::OUTBOX_RECEIPT_SCHEMA.into(),
                command_digest: exact.clone(),
                lead: "original".into(),
                revision: s.revision,
                sequence: 0,
                at: now(),
                outcome: outcome.into(),
            };
            s.applied.push(command);
            s.journal.insert(request, (exact, receipt.clone()));
            if lose.swap(false, Ordering::SeqCst) {
                return StatusCode::BAD_GATEWAY.into_response();
            }
            result(receipt)
        }
        _ => refuse(Code::AccessDenied),
    }
}

async fn owner(supervise: bool) -> Owner {
    let script = Arc::new(Mutex::new(Script {
        revision: 4,
        supervise,
        ..Default::default()
    }));
    let lose = Arc::new(AtomicBool::new(false));
    let router = Router::new()
        .route(crate::sales_remote::PATH, axum::routing::post(scripted))
        .with_state((script.clone(), lose.clone()));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!(
        "http://{}{}",
        listener.local_addr().unwrap(),
        crate::sales_remote::PATH
    );
    let server = tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    Owner {
        script,
        lose,
        url,
        server,
    }
}

fn private_dir(path: &std::path::Path) {
    std::fs::create_dir_all(path).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700)).unwrap();
}

fn private_file(path: &std::path::Path, bytes: &[u8]) {
    std::fs::write(path, bytes).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)).unwrap();
}

fn attach(fixture: &mut Fixture, owner: &Owner, site: &std::path::Path) {
    private_dir(site);
    let bearer = site.join("bearer");
    private_file(&bearer, SITE_BEARER.as_bytes());
    let path = site.join("sales.json");
    private_file(
        &path,
        &serde_json::to_vec(
            &json!({"schema":super::super::sales::SCHEMA,"directory":site,
            "delegations":[{"id":"owner-floor","account":"alice","workspace":"alice-personal",
            "members_epoch":3,"endpoint":owner.url,"binding":"owner-floor","bearer_file":bearer,
            "development_loopback":true}]}),
        )
        .unwrap(),
    );
    fixture.config.cloud_sales = Some(Arc::new(
        super::super::sales::Delegations::load(&path).unwrap(),
    ));
    fixture.site = crate::router(fixture.config.clone());
}

async fn get(fixture: &Fixture, cookies: &Cookies, path: &str) -> Answer {
    request(&fixture.site, Method::GET, path, cookies, None, None).await
}

async fn post(
    fixture: &Fixture,
    cookies: &Cookies,
    path: &str,
    fields: &[(String, String)],
) -> Answer {
    let fields: Vec<(&str, &str)> = fields
        .iter()
        .map(|(n, v)| (n.as_str(), v.as_str()))
        .collect();
    request(
        &fixture.site,
        Method::POST,
        path,
        cookies,
        Some(&form(&fields)),
        Some(ORIGIN),
    )
    .await
}

/// The hidden fields of the first form posting to `action` in `html`.
fn hidden(html: &str, action: &str) -> Vec<(String, String)> {
    let form = html
        .split("<form ")
        .skip(1)
        .find(|form| form.contains(&format!("action=\"{action}\"")))
        .unwrap_or_else(|| panic!("no form for {action}"))
        .split("</form>")
        .next()
        .unwrap();
    form.split("<input type=\"hidden\" name=\"")
        .skip(1)
        .map(|part| {
            let (name, rest) = part.split_once("\" value=\"").unwrap();
            (
                name.into(),
                rest.split('"').next().unwrap().replace("&amp;", "&"),
            )
        })
        .collect()
}

fn with(mut fields: Vec<(String, String)>, name: &str, value: &str) -> Vec<(String, String)> {
    fields.retain(|(n, _)| n != name);
    fields.push((name.into(), value.into()));
    fields
}

fn undisclosed(text: &str) {
    assert!(!text.contains(SITE_BEARER), "disclosed the binding bearer");
    assert!(!text.contains(CONTACT), "disclosed a contact");
    assert!(!text.contains(BODY), "disclosed a message body");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn floor_supervision_needs_its_own_grant_and_lists_carry_no_contact() {
    let mut fixture = fixture().await;
    let mut cookies = login(&fixture, "alice").await;
    choose_personal(&fixture, &mut cookies).await;
    let temp = tempfile::tempdir().unwrap();
    let site = temp.path().canonicalize().unwrap().join("site");

    // A delegation without the supervision grant offers no floor.
    let unsupervised = owner(false).await;
    attach(&mut fixture, &unsupervised, &site);
    let index = get(&fixture, &cookies, "/cloud/app/sales").await;
    assert!(!index.body.contains("Supervise the sales floor"));
    let refused = get(&fixture, &cookies, &format!("{PAGE}/floor")).await;
    assert_eq!(refused.status, StatusCode::FORBIDDEN);
    let board = get(&fixture, &cookies, &format!("{PAGE}/floor/board")).await;
    assert_eq!(board.status, StatusCode::OK);
    assert!(board.body.contains("Private board cleared"));
    assert!(!board.body.contains("sales-board-live"));

    let owner = owner(true).await;
    attach(&mut fixture, &owner, &site);
    let index = get(&fixture, &cookies, "/cloud/app/sales").await;
    assert!(index.body.contains("Supervise the sales floor"));
    let floor = get(&fixture, &cookies, &format!("{PAGE}/floor")).await;
    assert_eq!(floor.status, StatusCode::OK, "{}", floor.body);
    private(&floor);
    undisclosed(&floor.body);
    for text in [
        "America/Chicago business day 20000",
        "USD 5.000000 per day",
        "Unknown · hold retained",
        "Proposed · awaiting an exact owner decision",
        "Replies are untrusted",
        "held: injection",
        "Suggestions book nothing",
        "awaiting owner confirmation",
        "Paul plus at most three active hires",
        "drafting (draft)",
        "sales-board-live",
        "Stop dispatch at outbox revision 4",
    ] {
        assert!(floor.body.contains(text), "missing {text}");
    }

    // Bob holds no delegation and sees nothing.
    let bob = login(&fixture, "bob").await;
    let other = get(&fixture, &bob, &format!("{PAGE}/floor")).await;
    assert_ne!(other.status, StatusCode::OK);
    undisclosed(&other.body);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn outbox_decisions_bind_the_exact_reviewed_subject_and_revision() {
    let mut fixture = fixture().await;
    let mut cookies = login(&fixture, "alice").await;
    choose_personal(&fixture, &mut cookies).await;
    let temp = tempfile::tempdir().unwrap();
    let site = temp.path().canonicalize().unwrap().join("site");
    let owner = owner(true).await;
    attach(&mut fixture, &owner, &site);

    let path = format!("{PAGE}/outbox/original");
    let review = get(&fixture, &cookies, &path).await;
    assert_eq!(review.status, StatusCode::OK, "{}", review.body);
    private(&review);
    assert!(review.body.contains(CONTACT));
    assert!(review.body.contains(BODY));
    assert!(review.body.contains(&subject()));
    assert!(!review.body.contains("<textarea"));
    let action = format!("{path}/decide");
    let approve = hidden(&review.body, &action);
    assert!(approve.contains(&("approve".into(), "true".into())));

    // An edited subject digest fails the form's binding before any call.
    let edited = post(
        &fixture,
        &cookies,
        &action,
        &with(approve.clone(), "subject", &"cd".repeat(32)),
    )
    .await;
    assert_eq!(edited.status, StatusCode::FORBIDDEN);
    assert!(owner.script.lock().unwrap().applied.is_empty());

    // A form rendered before the outbox moved refuses unsent.
    owner.script.lock().unwrap().revision = 5;
    let stale = post(&fixture, &cookies, &action, &approve).await;
    assert_eq!(stale.status, StatusCode::CONFLICT, "{}", stale.body);
    assert!(stale.body.contains("Record changed"));
    assert!(owner.script.lock().unwrap().applied.is_empty());

    // The current exact approval applies once; a retry recovers it and the
    // same identity with a different decision conflicts.
    let review = get(&fixture, &cookies, &path).await;
    let approve = hidden(&review.body, &action);
    let approved = post(&fixture, &cookies, &action, &approve).await;
    assert_eq!(approved.status, StatusCode::SEE_OTHER, "{}", approved.body);
    let again = post(&fixture, &cookies, &action, &approve).await;
    assert_eq!(again.status, StatusCode::SEE_OTHER);
    assert_eq!(owner.script.lock().unwrap().applied.len(), 1);
    let flipped = post(
        &fixture,
        &cookies,
        &action,
        &with(approve, "approve", "false"),
    )
    .await;
    assert_eq!(flipped.status, StatusCode::CONFLICT);
    assert_eq!(owner.script.lock().unwrap().applied.len(), 1);
    let decided: outbox::Command =
        serde_json::from_str(&owner.script.lock().unwrap().applied[0]).unwrap();
    assert_eq!(decided.expected_revision, 5);
    assert!(matches!(
        decided.operation,
        outbox::Operation::Decide { approve: true, ref subject_sha256, .. } if *subject_sha256 == subject()
    ));
    let review = get(&fixture, &cookies, &path).await;
    assert!(review.body.contains("Recorded at outbox revision 6"));
    assert!(review.body.contains("no longer awaiting a decision"));

    // The site's journal keeps digests and receipts, never the contact or body.
    for entry in std::fs::read_dir(site.join("sales-requests")).unwrap() {
        let text = std::fs::read_to_string(entry.unwrap().path()).unwrap();
        undisclosed(&text);
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn stop_reconciles_a_lost_reply_and_fences_later_decisions() {
    let mut fixture = fixture().await;
    let mut cookies = login(&fixture, "alice").await;
    choose_personal(&fixture, &mut cookies).await;
    let temp = tempfile::tempdir().unwrap();
    let site = temp.path().canonicalize().unwrap().join("site");
    let owner = owner(true).await;
    attach(&mut fixture, &owner, &site);

    let floor = get(&fixture, &cookies, &format!("{PAGE}/floor")).await;
    let action = format!("{PAGE}/floor/stop");
    let fields = hidden(&floor.body, &action);
    owner.lose.store(true, Ordering::SeqCst);
    let lost = post(&fixture, &cookies, &action, &fields).await;
    assert_eq!(lost.status, StatusCode::SERVICE_UNAVAILABLE);
    assert!(lost.body.contains("Outcome unknown"));
    assert_eq!(owner.script.lock().unwrap().applied.len(), 1);

    // A restarted site shows the unknown request and reconciles it.
    attach(&mut fixture, &owner, &site);
    let floor = get(&fixture, &cookies, &format!("{PAGE}/floor")).await;
    assert!(floor.body.contains("Outcome unknown"));
    assert!(floor.body.contains("Dispatch is stopped"));
    let retry = hidden(floor.body.split("Outcome unknown").nth(1).unwrap(), &action);
    let settled = post(&fixture, &cookies, &action, &retry).await;
    assert_eq!(settled.status, StatusCode::SEE_OTHER, "{}", settled.body);
    assert_eq!(owner.script.lock().unwrap().applied.len(), 1);
    let floor = get(&fixture, &cookies, &format!("{PAGE}/floor")).await;
    assert!(
        floor
            .body
            .contains("Recorded at outbox revision 5 (outbox stopped)")
    );
    assert!(!floor.body.contains("Outcome unknown"));
    assert!(!floor.body.contains(&format!("action=\"{action}\"")));

    // Once stopped, the exact subject is still reviewable but undecidable.
    let review = get(&fixture, &cookies, &format!("{PAGE}/outbox/original")).await;
    assert!(review.body.contains("Dispatch is stopped"));
    assert!(!review.body.contains("/decide\""));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_private_board_expires_after_three_seconds_and_clears_on_failure() {
    let mut fixture = fixture().await;
    let mut cookies = login(&fixture, "alice").await;
    choose_personal(&fixture, &mut cookies).await;
    let temp = tempfile::tempdir().unwrap();
    let site = temp.path().canonicalize().unwrap().join("site");
    let owner = owner(true).await;
    attach(&mut fixture, &owner, &site);
    let path = format!("{PAGE}/floor/board");

    let fresh = get(&fixture, &cookies, &path).await;
    assert_eq!(fresh.status, StatusCode::OK);
    private(&fresh);
    assert!(fresh.body.contains("sales-board-live"));
    assert!(fresh.body.contains("Fixture 1"));
    assert!(fresh.body.contains("No reviewed shared aggregate"));
    undisclosed(&fresh.body);

    owner.script.lock().unwrap().board_age = 3;
    let stale = get(&fixture, &cookies, &path).await;
    assert_eq!(stale.status, StatusCode::OK);
    assert!(!stale.body.contains("sales-board-live"));
    assert!(stale.body.contains("older than three seconds"));

    {
        let mut script = owner.script.lock().unwrap();
        script.board_age = 0;
        script.board_fails = true;
    }
    let failed = get(&fixture, &cookies, &path).await;
    assert_eq!(failed.status, StatusCode::OK);
    assert!(!failed.body.contains("sales-board-live"));
    assert!(failed.body.contains("the refresh failed"));

    // Signed out, the fragment clears rather than redirecting.
    let anonymous = get(&fixture, &Cookies::default(), &path).await;
    assert_eq!(anonymous.status, StatusCode::OK);
    assert!(!anonymous.body.contains("sales-board-live"));
}
