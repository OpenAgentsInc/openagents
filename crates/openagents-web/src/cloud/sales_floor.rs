//! Paul, the sales floor, the outbox, and the private Agora board, supervised
//! through the same owner adapter delegation (WEB-15).
//!
//! Every read comes from the sales owner's canonical books through a binding
//! with an explicit supervision grant, and the owner role still applies. Lists
//! carry no contact, message body, or reply payload. The one exact outbox
//! subject an owner reviews is shown verbatim and is never editable here; a
//! decision binds that subject digest and the original outbox revision, and the
//! owner rechecks both, the controller, and current authority before recording
//! anything. Approval is not dispatch: handoff stays on the owner host.
//!
//! **Stop dispatch** pauses the outbox controller, which fences every pending
//! handoff and keeps unknown deliveries unknown; restart needs the owner's
//! correction on the sales host. Replies are untrusted and authorize nothing;
//! meeting suggestions book nothing. The private board expires three seconds
//! after its observation, clears on any failed refresh, and never carries a
//! bell event, record, or person-linked amount.

use super::*;
use crate::cloud::ui;
use crate::layout::escape;
use coder::task::sales::remote::{BOARD_TTL_SECONDS, Board, Floor, Proposal};
use coder::task::sales::{agents, expenses, floor as report, meetings, outbox, replies, town};
use maud::{Markup, html};

const OUTBOX_JOURNAL_SCHEMA: &str = "openagents.cloud.sales-web-outbox-requests.v1";

pub(super) fn routes() -> Router<App> {
    Router::new()
        .route("/cloud/app/sales/{id}/floor", get(floor_page))
        .route("/cloud/app/sales/{id}/floor/board", get(board_fragment))
        .route("/cloud/app/sales/{id}/floor/stop", post(stop))
        .route(
            "/cloud/app/sales/{id}/outbox/{proposal}",
            get(proposal_page),
        )
        .route(
            "/cloud/app/sales/{id}/outbox/{proposal}/decide",
            post(decide),
        )
}

/// What one outbox request asks the owner to do.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum Intent {
    Decide {
        proposal: String,
        subject_sha256: String,
        approve: bool,
    },
    Stop,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    intent: Intent,
    revision: u64,
    command: String,
    at: u64,
    receipt: Option<Receipt>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct OutboxJournal {
    schema: String,
    delegation: String,
    records: BTreeMap<String, Entry>,
}

impl OutboxJournal {
    fn name(delegation: &Delegation) -> String {
        format!("outbox-{}.json", &delegation.identity["sha256:".len()..])
    }

    fn load(root: &FsPath, delegation: &Delegation) -> Result<Self, Failure> {
        custody::checked_root(root)?;
        let Some(bytes) =
            custody::read_private(&root.join(Self::name(delegation)), 4 * 1024 * 1024)?
        else {
            return Ok(Self {
                schema: OUTBOX_JOURNAL_SCHEMA.into(),
                delegation: delegation.identity.clone(),
                records: BTreeMap::new(),
            });
        };
        let journal: Self = serde_json::from_slice(&bytes)
            .map_err(|_| Failure::Session(SessionError::Unavailable))?;
        if journal.schema != OUTBOX_JOURNAL_SCHEMA || journal.delegation != delegation.identity {
            return Err(Failure::Session(SessionError::Unavailable));
        }
        Ok(journal)
    }

    fn save(&self, root: &FsPath, delegation: &Delegation) -> Result<(), Failure> {
        custody::checked_root(root)?;
        let bytes =
            serde_json::to_vec(self).map_err(|_| Failure::Session(SessionError::Unavailable))?;
        custody::write_private(root, &Self::name(delegation), &bytes)?;
        Ok(())
    }
}

/// The exact outbox command an intent sends, and its digest. The same
/// request, revision, and intent always form the same bytes.
fn outbox_command(
    request: &str,
    revision: u64,
    intent: &Intent,
) -> Result<(String, String), Failure> {
    let operation = match intent {
        Intent::Decide {
            proposal,
            subject_sha256,
            approve,
        } => outbox::Operation::Decide {
            proposal: proposal.clone(),
            subject_sha256: subject_sha256.clone(),
            approve: *approve,
        },
        Intent::Stop => outbox::Operation::Pause {
            incident: outbox::IncidentKind::OwnerStop,
            reference_sha256: hex(&Sha256::digest(
                format!("openagents.cloud.sales-stop:{request}").as_bytes(),
            )),
        },
    };
    let command = serde_json::to_string(&outbox::Command {
        schema: outbox::COMMAND_SCHEMA.into(),
        id: request.into(),
        expected_revision: revision,
        operation,
    })
    .map_err(|_| Failure::Session(SessionError::Unavailable))?;
    let exact = hex(&Sha256::digest(command.as_bytes()));
    Ok((command, exact))
}

impl Delegations {
    async fn supervised(
        &self,
        viewer: &Viewer,
        id: &str,
    ) -> Result<(Arc<Delegation>, Standing), Failure> {
        let delegation = self.get(viewer, id)?.clone();
        let standing: Standing = self.call(viewer, &delegation, Op::Standing).await?;
        if !standing.supervise {
            return Err(Failure::Owner(Code::AccessDenied));
        }
        Ok((delegation, standing))
    }

    async fn floor(&self, viewer: &Viewer, id: &str) -> Result<Floor, Failure> {
        let delegation = self.get(viewer, id)?.clone();
        self.call(viewer, &delegation, Op::Floor).await
    }

    async fn board(&self, viewer: &Viewer, id: &str) -> Result<Board, Failure> {
        let delegation = self.get(viewer, id)?.clone();
        self.call(viewer, &delegation, Op::Board).await
    }

    async fn proposal(
        &self,
        viewer: &Viewer,
        id: &str,
        proposal: &str,
    ) -> Result<Proposal, Failure> {
        if !record_id(proposal) {
            return Err(Failure::Session(SessionError::InvalidRequest));
        }
        let delegation = self.get(viewer, id)?.clone();
        self.call(
            viewer,
            &delegation,
            Op::Proposal {
                proposal: proposal.to_owned(),
            },
        )
        .await
    }

    /// Journaled outbox requests, newest first; one proposal's, or every one.
    fn outbox_requests(
        &self,
        viewer: &Viewer,
        id: &str,
        proposal: Option<&str>,
    ) -> Result<Vec<(String, Entry)>, Failure> {
        let delegation = self.get(viewer, id)?;
        let journal = OutboxJournal::load(&self.journals, delegation)?;
        let mut records: Vec<(String, Entry)> = journal
            .records
            .into_iter()
            .filter(|(_, r)| match (&r.intent, proposal) {
                (_, None) => true,
                (Intent::Decide { proposal: p, .. }, Some(want)) => p == want,
                (Intent::Stop, Some(_)) => false,
            })
            .collect();
        records.sort_by(|a, b| b.1.at.cmp(&a.1.at).then(a.0.cmp(&b.0)));
        records.truncate(16);
        Ok(records)
    }

    /// One exact outbox decision or stop. The request identity, revision,
    /// intent, and command digest are journaled before dispatch; a retry
    /// with the same identity reconciles and never sends different bytes.
    async fn outbox_effect(
        &self,
        viewer: &Viewer,
        id: &str,
        request: &str,
        revision: u64,
        intent: Intent,
    ) -> Result<Receipt, Failure> {
        if !request_id(request) {
            return Err(Failure::Session(SessionError::InvalidRequest));
        }
        if let Intent::Decide {
            proposal,
            subject_sha256,
            ..
        } = &intent
            && (!record_id(proposal) || !sha256_hex(subject_sha256))
        {
            return Err(Failure::Session(SessionError::InvalidRequest));
        }
        let delegation = self.get(viewer, id)?.clone();
        let _turn = delegation.lock.clone().lock_owned().await;
        let mut journal = OutboxJournal::load(&self.journals, &delegation)?;
        if let Some(entry) = journal.records.get(request).cloned() {
            if entry.intent != intent || entry.revision != revision {
                return Err(Failure::Reused);
            }
            if let Some(receipt) = entry.receipt {
                return Ok(receipt);
            }
            return self
                .outbox_reconcile(viewer, &delegation, &mut journal, request, entry)
                .await;
        }
        self.outbox_current(viewer, &delegation, revision, &intent)
            .await?;
        let (command, exact) = outbox_command(request, revision, &intent)?;
        if journal.records.len() >= RECORDS_MAX {
            let oldest = journal
                .records
                .iter()
                .filter(|(_, r)| r.receipt.is_some())
                .min_by_key(|(_, r)| r.at)
                .map(|(k, _)| k.clone())
                .ok_or(Failure::Owner(Code::Busy))?;
            journal.records.remove(&oldest);
        }
        journal.records.insert(
            request.to_owned(),
            Entry {
                intent,
                revision,
                command: exact,
                at: now(),
                receipt: None,
            },
        );
        journal.save(&self.journals, &delegation)?;
        let result = self
            .call(
                viewer,
                &delegation,
                Op::Outbox {
                    request: request.to_owned(),
                    command,
                },
            )
            .await;
        self.outbox_settle(&delegation, &mut journal, request, result)
    }

    /// A decision must still name the exact proposed subject at the reviewed
    /// outbox revision; anything else changed and is refused unsent.
    async fn outbox_current(
        &self,
        viewer: &Viewer,
        delegation: &Delegation,
        revision: u64,
        intent: &Intent,
    ) -> Result<(), Failure> {
        let Intent::Decide {
            proposal,
            subject_sha256,
            ..
        } = intent
        else {
            return Ok(());
        };
        let current: Proposal = self
            .call(
                viewer,
                delegation,
                Op::Proposal {
                    proposal: proposal.clone(),
                },
            )
            .await?;
        if current.subject_sha256 != *subject_sha256
            || current.outbox_revision != revision
            || current.phase != outbox::Phase::Proposed
        {
            return Err(Failure::Changed);
        }
        Ok(())
    }

    async fn outbox_reconcile(
        &self,
        viewer: &Viewer,
        delegation: &Delegation,
        journal: &mut OutboxJournal,
        request: &str,
        entry: Entry,
    ) -> Result<Receipt, Failure> {
        let settled: Result<Settled, Failure> = self
            .call(
                viewer,
                delegation,
                Op::Reconcile {
                    request: request.to_owned(),
                    digest: entry.command.clone(),
                },
            )
            .await;
        let result = match settled {
            Ok(Settled::Recorded { receipt }) => Ok(receipt),
            Ok(Settled::Absent) => {
                // The owner never journaled it, so nothing was applied. Send
                // the same exact command only while it is still current.
                let (command, exact) = outbox_command(request, entry.revision, &entry.intent)?;
                let current = self
                    .outbox_current(viewer, delegation, entry.revision, &entry.intent)
                    .await;
                if exact != entry.command || matches!(current, Err(Failure::Changed)) {
                    journal.records.remove(request);
                    journal.save(&self.journals, delegation)?;
                    return Err(Failure::Changed);
                }
                current?;
                self.call(
                    viewer,
                    delegation,
                    Op::Outbox {
                        request: request.to_owned(),
                        command,
                    },
                )
                .await
            }
            Err(error) => Err(error),
        };
        self.outbox_settle(delegation, journal, request, result)
    }

    fn outbox_settle(
        &self,
        delegation: &Delegation,
        journal: &mut OutboxJournal,
        request: &str,
        result: Result<Receipt, Failure>,
    ) -> Result<Receipt, Failure> {
        match result {
            Ok(receipt) => {
                if let Some(record) = journal.records.get_mut(request) {
                    record.receipt = Some(receipt.clone());
                }
                journal.save(&self.journals, delegation)?;
                Ok(receipt)
            }
            Err(Failure::Owner(code)) if code.definitive() => {
                if journal
                    .records
                    .get(request)
                    .is_some_and(|r| r.receipt.is_none())
                {
                    journal.records.remove(request);
                    journal.save(&self.journals, delegation)?;
                }
                Err(Failure::Owner(code))
            }
            Err(error) => Err(error),
        }
    }
}

fn sha256_hex(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn outbox_target(delegation: &Delegation, subject: &str, request: &str) -> String {
    format!("{}:outbox:{subject}:{request}", delegation.identity)
}

fn usd(millionths: u64) -> String {
    format!(
        "USD {}.{:06}",
        millionths / 1_000_000,
        millionths % 1_000_000
    )
}

fn word<T: Serialize>(value: &T) -> String {
    serde_json::to_value(value)
        .ok()
        .and_then(|v| v.as_str().map(|s| s.replace('_', " ")))
        .unwrap_or_else(|| "unknown".into())
}

fn short(value: &str) -> String {
    value[..value.len().min(16)].to_owned()
}

fn phase_label(phase: outbox::Phase) -> &'static str {
    use outbox::Phase as P;
    match phase {
        P::Proposed => "Proposed · awaiting an exact owner decision",
        P::Approved => "Approved · not yet handed off",
        P::Rejected => "Rejected",
        P::DispatchIntent => "Handoff in progress",
        P::Accepted => "Provider accepted · delivery not proven",
        P::Delivered => "Delivered",
        P::Failed => "Failed",
        P::HardBounce => "Hard bounce",
        P::Unknown => "Delivery unknown · never resent automatically",
        P::Cancelled => "Cancelled",
        P::Invalidated => "Invalidated · needs a new proposal",
        P::OwnerReported => "Owner reported sent",
    }
}

fn cert_label(state: agents::CertState, measured: bool) -> &'static str {
    match state {
        agents::CertState::Qualified if measured => "Qualified (measured)",
        agents::CertState::Qualified => "Qualified mark without measurement",
        agents::CertState::OwnerMarked => "Owner-marked · not qualified",
        agents::CertState::InTraining => "In training",
        agents::CertState::Suspended => "Suspended",
    }
}

fn reservation_label(status: expenses::Status) -> &'static str {
    match status {
        expenses::Status::Reserved => "Reserved · hold retained",
        expenses::Status::Unknown => "Unknown · hold retained",
        expenses::Status::Known => "Known",
        expenses::Status::Breach => "Breach · new model work stopped",
    }
}

/// The private board fragment. It always answers 200 so a refresh replaces
/// it; anything but a current observation clears it.
fn board_html(board: Result<&Board, &'static str>) -> Markup {
    let board = match board {
        Ok(board) if now().saturating_sub(board.observed_at) < BOARD_TTL_SECONDS => board,
        Ok(_) => {
            return html! {
                p class="cloud-note" { "Private board cleared: the observation is older than three seconds." }
            };
        }
        Err(reason) => {
            return html! {
                p class="cloud-note" {
                    "Private board cleared: " (reason)
                    ". Nothing is shown until a current observation arrives."
                }
            };
        }
    };
    let shared = board.shared.map_or_else(
        || "No reviewed shared aggregate is current.".into(),
        |[sales, net]| {
            format!("Reviewed shared aggregate: {sales} earned sales, net at least USD {net}.")
        },
    );
    let details = ui::Details::new()
        .row(
            "Pipeline",
            format!(
                "New {} · Qualified {} · Pilot {} · Active {} · Closed {}",
                board.pipeline[0],
                board.pipeline[1],
                board.pipeline[2],
                board.pipeline[3],
                board.pipeline[4],
            ),
        )
        .row("Drafts awaiting review", board.pending_drafts)
        .row(
            "Certification",
            format!(
                "Qualified {} · In training or marked {} · Suspended {}",
                board.certifications[0], board.certifications[1], board.certifications[2],
            ),
        )
        .row("Practice runs", board.practice_runs)
        .row(
            "Meeting proposals awaiting the owner",
            board.meeting_proposals,
        )
        .row(
            "Outbox proposals",
            format!(
                "Live {} · Fixture {} · Delivery unknown {}",
                board.outbox_live_proposals, board.outbox_fixture_proposals, board.outbox_unknown,
            ),
        )
        .row(
            "Paul",
            format!(
                "{} · model {}",
                if board.idle { "Idle" } else { "Working" },
                if board.model_available {
                    "available"
                } else {
                    "unavailable"
                },
            ),
        );
    html! {
        div class="sales-board-live" data-observed-at=(board.observed_at) {
            (details)
            p { (shared) }
            p class="cloud-note" { "Observed at " (board.observed_at) "; expires three seconds later." }
        }
    }
}

async fn board_fragment(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Response {
    let fragment = |body: Markup| {
        protect(
            (
                StatusCode::OK,
                [(axum::http::header::CONTENT_TYPE, "text/html; charset=utf-8")],
                body.into_string(),
            )
                .into_response(),
        )
    };
    let context = match context(&app, &headers).await {
        Ok(value) => value,
        Err(_) => return fragment(board_html(Err("the session is unavailable"))),
    };
    if context
        .sales
        .supervised(&context.viewer, &id)
        .await
        .is_err()
    {
        return fragment(board_html(Err("floor supervision is unavailable")));
    }
    match context.sales.board(&context.viewer, &id).await {
        Ok(board) => fragment(board_html(Ok(&board))),
        Err(_) => fragment(board_html(Err("the refresh failed"))),
    }
}

async fn floor_page(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Response {
    let context = match context(&app, &headers).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    let (delegation, standing) = match context.sales.supervised(&context.viewer, &id).await {
        Ok(value) => value,
        Err(error) => return answer(error),
    };
    let floor = match context.sales.floor(&context.viewer, &id).await {
        Ok(value) => value,
        Err(error) => return answer(error),
    };
    let base = format!("{PAGE}/{id}");

    // Paul and the crew.
    let paul = html! {
        section class="cloud-card" id="sales-paul" {
            h3 { "Paul" }
            @match &floor.paul {
                None => {
                    p { "Unavailable: Paul is not configured for this sales owner. No queue is inferred." }
                }
                Some(paul) => {
                    p {
                        (if paul.idle { "Idle" } else { "Working" })
                        " · model " (if paul.model_available { "available" } else { "unavailable" })
                        " · qualification inferred: " (if paul.qualification_inferred { "yes" } else { "no" })
                        " · external effects: " (if paul.external_effects { "yes" } else { "none" })
                    }
                    @if paul.rows.is_empty() {
                        p { "No assigned records." }
                    } @else {
                        (paul.rows.iter().fold(
                            ui::table("Paul's records").header([
                                "Record",
                                "Stage",
                                "Drafts awaiting review",
                                "Meetings awaiting the owner",
                            ]),
                            |table, row| {
                                table.row([
                                    html! {
                                        a href=(format!("{base}/leads/{}", row.lead)) { (short(&row.lead)) }
                                        " · revision " (row.revision)
                                    },
                                    html! { (stage_label(row.stage)) },
                                    html! { (row.pending_drafts) },
                                    html! { (row.meetings_awaiting_owner) },
                                ])
                            },
                        ))
                    }
                }
            }
        }
    };
    let crew = floor.crew.iter().fold(
        ui::table("Crew").header([
            "Member",
            "Role",
            "Lifecycle",
            "Station",
            "Activity",
            "Queued",
        ]),
        |table, member| {
            table.row([
                html! { (member.name) },
                html! {
                    (match member.role {
                        town::Role::Leader => "Leader",
                        town::Role::Hire => "Hire",
                    })
                },
                html! { (word(&member.lifecycle)) },
                html! { (member.station.as_deref().unwrap_or("Unplaced")) },
                html! {
                    @match (&member.activity, &member.source_kind) {
                        (Some(activity), Some(kind)) => { (activity) " (" (kind) ")" }
                        (Some(activity), None) => { (activity) }
                        _ => { "Idle" }
                    }
                },
                html! { (member.queued) },
            ])
        },
    );

    // Expense holds.
    let reservations = floor.reservations.iter().fold(
        ui::table("Model reservations").header([
            "Reservation",
            "Agent",
            "Chicago day",
            "State",
            "Maximum",
            "Estimate",
            "Billed",
        ]),
        |table, r| {
            let unknown = || "Unknown".to_string();
            table.row([
                html! { (short(&r.id)) @if r.training { " · training" } },
                html! { (r.agent) },
                html! { (r.day) },
                html! {
                    (reservation_label(r.status))
                    @if r.execution_unknown { " · execution unknown" }
                },
                html! { (usd(r.maximum_usd_millionths)) },
                html! { (r.estimated_usd_millionths.map_or_else(unknown, usd)) },
                html! { (r.billed_usd_millionths.map_or_else(unknown, usd)) },
            ])
        },
    );

    // Floor report and escalations.
    let r = &floor.report;
    let measure = |m: &report::Measure| match m {
        report::Measure::Known { value } => usd(*value),
        report::Measure::Unknown { reason } => format!("Unknown ({reason})"),
    };
    let stages: Vec<String> = r.stages.iter().map(|(k, v)| format!("{k} {v}")).collect();
    let report_details = ui::Details::new()
        .row(
            "Records",
            format!(
                "{} · {}",
                r.leads,
                if stages.is_empty() {
                    "no stages".into()
                } else {
                    stages.join(" · ")
                }
            ),
        )
        .row(
            "Next actions",
            format!(
                "{} due · {} overdue",
                r.next_actions_due, r.next_actions_overdue
            ),
        )
        .row(
            "Messages",
            format!(
                "drafted {} · owner reviewed {} · rejected {} · proposed {} · approved {} · sent {} · replied {}",
                r.messages.drafted,
                r.messages.owner_reviewed,
                r.messages.rejected,
                r.messages.proposed,
                r.messages.approved,
                r.messages.sent,
                r.messages.replied,
            ),
        )
        .row(
            "Delivery",
            format!(
                "delivered {} · hard bounce {} · failed {} · unknown {} · opt-out {} · complaint {}{}",
                r.delivery.delivered,
                r.delivery.hard_bounce,
                r.delivery.failed,
                r.delivery.unknown,
                r.delivery.opt_out,
                r.delivery.complaint,
                if r.delivery.telemetry_absent {
                    " · provider delivery telemetry absent"
                } else {
                    ""
                },
            ),
        )
        .row(
            "Expense",
            format!(
                "reserved estimate {} · billed {} · unknown reservations {} · breaches {} · per qualified record {}",
                usd(r.costs.reserved_estimate_usd_millionths),
                measure(&r.costs.billed_usd_millionths),
                r.costs.reservations_unknown,
                r.costs.reservations_breached,
                measure(&r.costs.per_qualified_lead_usd_millionths),
            ),
        )
        .row(
            "Open",
            format!(
                "{} incidents · {} replies awaiting review · outbox {}",
                r.unresolved_incidents,
                r.unresolved_replies,
                if r.outbox_paused { "stopped" } else { "running" },
            ),
        );

    // Outbox.
    let o = &floor.outbox;
    let outbox_rows = o.rows.iter().fold(
        ui::table("Outbox proposals").header([
            "Proposal",
            "Record",
            "Kind",
            "Mode",
            "State",
            "Chicago day",
            "Subject",
        ]),
        |table, row| {
            table.row([
                html! {
                    @if row.reviewable {
                        a href=(format!("{base}/outbox/{}", row.id)) { (short(&row.id)) }
                    } @else {
                        (short(&row.id)) " · minimized"
                    }
                },
                html! { (short(&row.lead)) },
                html! { (word(&row.kind)) },
                html! { (word(&row.mode)) },
                html! { (phase_label(row.phase)) },
                html! { (row.business_day) },
                html! { code { (short(&row.subject_sha256)) } },
            ])
        },
    );
    let unresolved: Vec<_> = o.incidents.iter().filter(|i| !i.resolved).collect();
    let stop_form = if !o.paused && standing.effects.contains(&Effect::OutboxStop) {
        let request = fresh_request();
        let csrf = match context.service.csrf(
            &headers,
            &context.viewer,
            "sales-outbox",
            &outbox_target(&delegation, "stop", &request),
        ) {
            Ok(value) => value,
            Err(error) => return refused(error),
        };
        Some(
            ui::BoundForm::new(format!("{base}/floor/stop"))
                .csrf(&csrf)
                .bind("request", &request)
                .bind("revision", &o.revision.to_string())
                .submit(&format!("Stop dispatch at outbox revision {}", o.revision)),
        )
    } else {
        None
    };
    let requests = match context.sales.outbox_requests(&context.viewer, &id, None) {
        Ok(value) => value,
        Err(error) => return answer(error),
    };
    let request_list = match request_list(&context, &headers, &delegation, &base, &requests) {
        Ok(list) => list,
        Err(response) => return response,
    };

    // The private Agora board.
    let board = match context.sales.board(&context.viewer, &id).await {
        Ok(board) => board_html(Ok(&board)),
        Err(_) => board_html(Err("the observation is unavailable")),
    };

    let content = html! {
        (ui::links([(PAGE, "Private sales")]))
        h2 { "Sales floor" }
        p {
            "Supervised through delegation " (delegation.id()) " as " (standing.principal)
            ". Every figure is recomputed by the sales owner from its canonical books; nothing on this page authorizes an outbound effect. Budgets and caps count America/Chicago business day "
            (floor.business_day) " against the fixed floor-wide ceiling of "
            (usd(floor.ceiling_usd_millionths))
            " per day; unknown expense keeps its hold."
        }
        (paul)
        section class="cloud-card" id="sales-crew" {
            h3 { "Crew" }
            p { "Paul plus at most three active hires. A hire joins only through an exact confirmed proposal and trains before real drafting." }
            (crew)
            p { "Hire proposals awaiting the owner: " (floor.pending_hires) "." }
        }
        // Certification and practice.
        section class="cloud-card" id="sales-certification" {
            h3 { "Certification" }
            @if floor.certifications.is_empty() {
                p { "No certification is recorded." }
            } @else {
                ul {
                    @for cert in &floor.certifications {
                        li {
                            (cert.agent) " · version " (cert.version) " · "
                            (cert_label(cert.state, cert.measured_qualified))
                            " · expires " (cert.expires_at) " · outbound authority: "
                            (if cert.outbound_authority { "yes" } else { "none" })
                        }
                    }
                }
            }
        }
        section class="cloud-card" id="sales-expense" {
            h3 { "Model reservations" }
            @if floor.reservations.is_empty() {
                p { "No current reservation." }
            } @else {
                (reservations)
            }
        }
        section class="cloud-card" id="sales-report" {
            h3 { "Floor report" }
            (report_details)
            @if !r.gaps.is_empty() {
                p { "Gaps:" }
                ul { @for gap in &r.gaps { li { (gap) } } }
            }
            @if !floor.escalations.is_empty() {
                h4 { "Escalations" }
                ul {
                    @for e in &floor.escalations {
                        li {
                            (match e.severity {
                                report::Severity::Immediate => "Immediate",
                                report::Severity::Review => "Review",
                            })
                            " · " (e.kind.replace('_', " ")) " · at " (e.at)
                        }
                    }
                }
            }
        }
        section class="cloud-card" id="sales-outbox" {
            h3 { "Outbox" }
            p {
                "Revision " (o.revision) " · controller epoch " (o.controller_epoch) " · "
                (if o.paused { "dispatch stopped" } else { "dispatch running" })
                " · daily cap " (o.cap) " · today live " (o.live_messages_and_reservations)
                " and fixture " (o.fixture_messages_and_reservations)
                " messages and reservations. Level 0: each message needs the owner's approval of its exact proposal at its original revision before any handoff. Approval is not dispatch; handoff stays on the sales host."
            }
            @if o.rows.is_empty() {
                p { "No outbox proposal." }
            } @else {
                div class="sales-outbox" { (outbox_rows) }
            }
            @if !unresolved.is_empty() {
                p { "Unresolved incidents:" }
                ul {
                    @for incident in &unresolved {
                        li { (short(&incident.id)) " · " (word(&incident.kind)) " · at " (incident.at) }
                    }
                }
            }
            @if o.paused {
                p { "Dispatch is stopped. Pending handoffs are fenced and unknown deliveries stay unknown. Restart needs the owner's correction of every incident on the sales host." }
            } @else if let Some(form) = &stop_form {
                (form)
                p class="cloud-note" { "Stop pauses the outbox controller: approved messages are not handed off and unknown deliveries keep their state." }
            }
            (request_list)
        }
        // Replies and meetings.
        section class="cloud-card" id="sales-replies" {
            h3 { "Replies" }
            p { "Replies are untrusted. They cannot authorize work, payments, disclosure, or follow-ups; the owner reviews them on the sales host. Their text stays there." }
            @if floor.replies.is_empty() {
                p { "No reply is recorded." }
            } @else {
                ul {
                    @for reply in &floor.replies {
                        li {
                            (short(&reply.id)) " · record "
                            (reply.lead.as_deref().map_or_else(|| "unmatched".into(), short))
                            " · received " (reply.received_at) " · "
                            (match reply.safety {
                                replies::Safety::Ordinary => "ordinary".to_string(),
                                other => format!("held: {}", word(&other)),
                            })
                            " · "
                            (reply.owner_label.map_or_else(
                                || "awaiting owner review".into(),
                                |l| format!("owner label {}", word(&l)),
                            ))
                            @if reply.minimized { " · minimized" }
                        }
                    }
                }
            }
        }
        section class="cloud-card" id="sales-meetings" {
            h3 { "Meetings" }
            p { "Suggestions book nothing; a human confirms and closes." }
            @if floor.meetings.is_empty() {
                p { "No meeting proposal." }
            } @else {
                ul {
                    @for m in &floor.meetings {
                        li {
                            (short(&m.id)) " · record " (short(&m.lead)) " · "
                            (match m.phase {
                                meetings::Phase::Pending => "Pending",
                                meetings::Phase::OwnerConfirmed => "Owner confirmed",
                                meetings::Phase::Accepted => "Accepted",
                                meetings::Phase::Declined => "Declined",
                                meetings::Phase::Retired => "Retired",
                            })
                            " · " (m.start_at) " to " (m.end_at)
                            @if m.owner_confirmation_needed { " · awaiting owner confirmation" }
                        }
                    }
                }
            }
        }
        section class="cloud-card" id="sales-board" {
            h3 { "Private Agora board" }
            p { "Counts from the sales owner, current for three seconds. A failed refresh or an inactive view clears them; no bell event, record, or person-linked amount appears here." }
            div hx-get=(format!("{base}/floor/board")) hx-trigger="every 1s" hx-swap="innerHTML"
                hx-sync="this:drop" aria-live="off" {
                (board)
            }
        }
    };
    shell(&context, &headers, content)
}

fn request_list(
    context: &Context<'_>,
    headers: &HeaderMap,
    delegation: &Delegation,
    base: &str,
    requests: &[(String, Entry)],
) -> Result<Markup, Response> {
    if requests.is_empty() {
        return Ok(html! {});
    }
    let mut items = Vec::with_capacity(requests.len());
    for (request, entry) in requests {
        let (label, action, subject, extra): (String, String, String, Vec<(&str, String)>) =
            match &entry.intent {
                Intent::Decide {
                    proposal,
                    subject_sha256,
                    approve,
                } => (
                    format!(
                        "{} {} at outbox revision {}",
                        if *approve { "Approve" } else { "Reject" },
                        short(proposal),
                        entry.revision
                    ),
                    format!("{base}/outbox/{proposal}/decide"),
                    subject_sha256.clone(),
                    vec![
                        ("subject", subject_sha256.clone()),
                        ("approve", approve.to_string()),
                    ],
                ),
                Intent::Stop => (
                    format!("Stop dispatch at outbox revision {}", entry.revision),
                    format!("{base}/floor/stop"),
                    "stop".to_string(),
                    Vec::new(),
                ),
            };
        let item = match &entry.receipt {
            Some(receipt) => html! {
                li {
                    (label) " · request " (&request[..8]) " · Recorded at outbox revision "
                    (receipt.revision) " (" (receipt.outcome.replace('_', " ")) ")"
                }
            },
            None => {
                let csrf = context
                    .service
                    .csrf(
                        headers,
                        &context.viewer,
                        "sales-outbox",
                        &outbox_target(delegation, &subject, request),
                    )
                    .map_err(refused)?;
                let retry = extra.iter().fold(
                    ui::retry(action, &csrf)
                        .bind("request", request)
                        .bind("revision", &entry.revision.to_string()),
                    |form, (name, value)| form.bind(name, value),
                );
                html! {
                    li {
                        (label) " · request " (&request[..8])
                        (ui::outcome_unknown(
                            html! {
                                "This request may or may not have been recorded. Retry the same request to recover it; it never creates a new one."
                            },
                            Some(retry),
                        ))
                    }
                }
            }
        };
        items.push(item);
    }
    Ok(html! {
        h4 { "Requests" }
        ul class="sales-requests" { @for item in &items { (item) } }
    })
}

async fn proposal_page(
    State(app): State<App>,
    headers: HeaderMap,
    Path((id, proposal)): Path<(String, String)>,
) -> Response {
    let context = match context(&app, &headers).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    let (delegation, standing) = match context.sales.supervised(&context.viewer, &id).await {
        Ok(value) => value,
        Err(error) => return answer(error),
    };
    let found = match context
        .sales
        .proposal(&context.viewer, &id, &proposal)
        .await
    {
        Ok(value) => value,
        Err(error) => return answer(error),
    };
    let base = format!("{PAGE}/{id}");
    let details = ui::Details::new()
        .row("Kind", word(&found.kind))
        .row("Mode", word(&found.mode))
        .row("State", phase_label(found.phase))
        .row(
            "Record",
            html! { a href=(format!("{base}/leads/{}", found.lead)) { (short(&found.lead)) } },
        )
        .row("Chicago business day", found.business_day)
        .row("Expires", found.expires_at)
        .row("Sender", &found.sender)
        .row("Recipient", &found.recipient)
        .row("Subject line", &found.subject)
        .row("Body", html! { pre { (found.body) } })
        .row(
            "Attachments",
            html! {
                @if found.attachments.is_empty() {
                    "None"
                } @else {
                    ul {
                        @for a in &found.attachments {
                            li { (a.filename) " · " (a.bytes) " bytes · " code { (a.sha256) } }
                        }
                    }
                }
            },
        )
        .row(
            "Certification",
            found.certification_reference.as_deref().unwrap_or("None"),
        )
        .row(
            "Graded draft",
            found.draft_reference.as_deref().unwrap_or("None"),
        )
        .row(
            "Maximum cost",
            format!("{} micro-USD", found.maximum_cost_microusd),
        )
        .row("Subject digest", html! { code { (found.subject_sha256) } })
        .row("Outbox revision", found.outbox_revision);
    let decidable = found.phase == outbox::Phase::Proposed
        && !found.paused
        && standing.effects.contains(&Effect::OutboxDecide);
    let mut forms = Vec::new();
    if decidable {
        for (approve, label) in [(true, "Approve this exact message"), (false, "Reject")] {
            let request = fresh_request();
            let csrf = match context.service.csrf(
                &headers,
                &context.viewer,
                "sales-outbox",
                &outbox_target(&delegation, &found.subject_sha256, &request),
            ) {
                Ok(value) => value,
                Err(error) => return refused(error),
            };
            forms.push(
                ui::BoundForm::new(format!("{base}/outbox/{}/decide", found.id))
                    .csrf(&csrf)
                    .bind("request", &request)
                    .bind("revision", &found.outbox_revision.to_string())
                    .bind("subject", &found.subject_sha256)
                    .bind("approve", &approve.to_string())
                    .submit_with(ui::submit(
                        &format!("{label} at outbox revision {}", found.outbox_revision),
                        approve,
                    )),
            );
        }
    }
    let requests = match context
        .sales
        .outbox_requests(&context.viewer, &id, Some(&found.id))
    {
        Ok(value) => value,
        Err(error) => return answer(error),
    };
    let request_list = match request_list(&context, &headers, &delegation, &base, &requests) {
        Ok(list) => list,
        Err(response) => return response,
    };
    let content = html! {
        (ui::links([(format!("{base}/floor").as_str(), "Sales floor")]))
        h2 { "Outbox proposal " (found.id) }
        p { "This is the exact subject the sales owner would hand off. It cannot be edited here: a changed draft needs a new proposal and its own grading. A decision binds this subject digest and outbox revision; the owner rechecks both, the controller, the reserved Chicago day, suppression, and current authority before recording it." }
        div class="sales-proposal" { (details) }
        @if decidable {
            @for form in &forms { (form) }
        } @else if found.paused {
            p { "Dispatch is stopped; no decision is offered until the owner restarts it on the sales host." }
        } @else if found.phase == outbox::Phase::Proposed {
            p { "This delegation has no outbox decision grant." }
        } @else {
            p { "This proposal is no longer awaiting a decision." }
        }
        (request_list)
    };
    shell(&context, &headers, content)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DecideForm {
    csrf: String,
    request: String,
    revision: u64,
    subject: String,
    approve: bool,
}

async fn decide(
    State(app): State<App>,
    headers: HeaderMap,
    Path((id, proposal)): Path<(String, String)>,
    form: Result<Form<DecideForm>, FormRejection>,
) -> Response {
    let Ok(Form(form)) = form else {
        return refused(SessionError::InvalidRequest);
    };
    let context = match context(&app, &headers).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    let delegation = match context.sales.get(&context.viewer, &id) {
        Ok(value) => value.clone(),
        Err(error) => return refused(error),
    };
    if let Err(error) = context.service.verify_csrf(
        &headers,
        Some(&context.viewer),
        "sales-outbox",
        &outbox_target(&delegation, &form.subject, &form.request),
        &form.csrf,
    ) {
        return refused(error);
    }
    let intent = Intent::Decide {
        proposal: proposal.clone(),
        subject_sha256: form.subject,
        approve: form.approve,
    };
    match context
        .sales
        .outbox_effect(&context.viewer, &id, &form.request, form.revision, intent)
        .await
    {
        Ok(_) => protect(
            Redirect::to(&format!(
                "{PAGE}/{}/outbox/{}",
                escape(&id),
                escape(&proposal)
            ))
            .into_response(),
        ),
        Err(error) => answer(error),
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StopForm {
    csrf: String,
    request: String,
    revision: u64,
}

async fn stop(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    form: Result<Form<StopForm>, FormRejection>,
) -> Response {
    let Ok(Form(form)) = form else {
        return refused(SessionError::InvalidRequest);
    };
    let context = match context(&app, &headers).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    let delegation = match context.sales.get(&context.viewer, &id) {
        Ok(value) => value.clone(),
        Err(error) => return refused(error),
    };
    if let Err(error) = context.service.verify_csrf(
        &headers,
        Some(&context.viewer),
        "sales-outbox",
        &outbox_target(&delegation, "stop", &form.request),
        &form.csrf,
    ) {
        return refused(error);
    }
    match context
        .sales
        .outbox_effect(
            &context.viewer,
            &id,
            &form.request,
            form.revision,
            Intent::Stop,
        )
        .await
    {
        Ok(_) => protect(Redirect::to(&format!("{PAGE}/{}/floor", escape(&id))).into_response()),
        Err(error) => answer(error),
    }
}

/// The index page's link for a supervised delegation.
pub(super) fn floor_link(id: &str) -> Markup {
    html! {
        p {
            a href=(format!("{PAGE}/{id}/floor")) { "Supervise the sales floor" }
            ": Paul, crew, certification, expense holds, outbox decisions, replies, meetings, and the private Agora board."
        }
    }
}
