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
use coder::task::sales::remote::{BOARD_TTL_SECONDS, Board, Floor, Proposal};
use coder::task::sales::{agents, expenses, floor as report, meetings, outbox, replies, town};

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
    escape(&value[..value.len().min(16)])
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
fn board_html(board: Result<&Board, &'static str>) -> String {
    let board = match board {
        Ok(board) if now().saturating_sub(board.observed_at) < BOARD_TTL_SECONDS => board,
        Ok(_) => return "<p class=\"dim\">Private board cleared: the observation is older than three seconds.</p>".into(),
        Err(reason) => {
            return format!(
                "<p class=\"dim\">Private board cleared: {reason}. Nothing is shown until a current observation arrives.</p>"
            );
        }
    };
    let shared = board.shared.map_or_else(
        || "No reviewed shared aggregate is current.".into(),
        |[sales, net]| {
            format!("Reviewed shared aggregate: {sales} earned sales, net at least USD {net}.")
        },
    );
    format!(
        "<div class=\"sales-board-live\" data-observed-at=\"{}\"><dl class=\"sales-board\"><dt>Pipeline</dt><dd>New {} · Qualified {} · Pilot {} · Active {} · Closed {}</dd><dt>Drafts awaiting review</dt><dd>{}</dd><dt>Certification</dt><dd>Qualified {} · In training or marked {} · Suspended {}</dd><dt>Practice runs</dt><dd>{}</dd><dt>Meeting proposals awaiting the owner</dt><dd>{}</dd><dt>Outbox proposals</dt><dd>Live {} · Fixture {} · Delivery unknown {}</dd><dt>Paul</dt><dd>{} · model {}</dd></dl><p>{}</p><p class=\"dim\">Observed at {}; expires three seconds later.</p></div>",
        board.observed_at,
        board.pipeline[0],
        board.pipeline[1],
        board.pipeline[2],
        board.pipeline[3],
        board.pipeline[4],
        board.pending_drafts,
        board.certifications[0],
        board.certifications[1],
        board.certifications[2],
        board.practice_runs,
        board.meeting_proposals,
        board.outbox_live_proposals,
        board.outbox_fixture_proposals,
        board.outbox_unknown,
        if board.idle { "Idle" } else { "Working" },
        if board.model_available {
            "available"
        } else {
            "unavailable"
        },
        shared,
        board.observed_at,
    )
}

async fn board_fragment(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Response {
    let fragment = |body: String| {
        protect(
            (
                StatusCode::OK,
                [(axum::http::header::CONTENT_TYPE, "text/html; charset=utf-8")],
                body,
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
    let base = format!("{PAGE}/{}", escape(&id));
    let mut content = format!(
        "<p><a href=\"{PAGE}\">Private sales</a></p><h2>Sales floor</h2><p>Supervised through delegation {} as {}. Every figure is recomputed by the sales owner from its canonical books; nothing on this page authorizes an outbound effect. Budgets and caps count America/Chicago business day {} against the fixed floor-wide ceiling of {} per day; unknown expense keeps its hold.</p>",
        escape(delegation.id()),
        escape(&standing.principal),
        floor.business_day,
        usd(floor.ceiling_usd_millionths),
    );

    // Paul and the crew.
    content.push_str("<section class=\"cloud-card\" id=\"sales-paul\"><h3>Paul</h3>");
    match &floor.paul {
        None => content.push_str(
            "<p>Unavailable: Paul is not configured for this sales owner. No queue is inferred.</p>",
        ),
        Some(paul) => {
            content.push_str(&format!(
                "<p>{} · model {} · qualification inferred: {} · external effects: {}</p>",
                if paul.idle { "Idle" } else { "Working" },
                if paul.model_available {
                    "available"
                } else {
                    "unavailable"
                },
                if paul.qualification_inferred {
                    "yes"
                } else {
                    "no"
                },
                if paul.external_effects { "yes" } else { "none" },
            ));
            if paul.rows.is_empty() {
                content.push_str("<p>No assigned records.</p>");
            } else {
                content.push_str("<table><thead><tr><th>Record</th><th>Stage</th><th>Drafts awaiting review</th><th>Meetings awaiting the owner</th></tr></thead><tbody>");
                for row in &paul.rows {
                    content.push_str(&format!(
                        "<tr><td><a href=\"{base}/leads/{}\">{}</a> · revision {}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
                        escape(&row.lead),
                        short(&row.lead),
                        row.revision,
                        stage_label(row.stage),
                        row.pending_drafts,
                        row.meetings_awaiting_owner,
                    ));
                }
                content.push_str("</tbody></table>");
            }
        }
    }
    content.push_str("</section><section class=\"cloud-card\" id=\"sales-crew\"><h3>Crew</h3><p>Paul plus at most three active hires. A hire joins only through an exact confirmed proposal and trains before real drafting.</p><table><thead><tr><th>Member</th><th>Role</th><th>Lifecycle</th><th>Station</th><th>Activity</th><th>Queued</th></tr></thead><tbody>");
    for member in &floor.crew {
        content.push_str(&format!(
            "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
            escape(&member.name),
            match member.role {
                town::Role::Leader => "Leader",
                town::Role::Hire => "Hire",
            },
            word(&member.lifecycle),
            member
                .station
                .as_deref()
                .map_or_else(|| "Unplaced".into(), escape),
            match (&member.activity, &member.source_kind) {
                (Some(activity), Some(kind)) => format!("{} ({})", escape(activity), escape(kind)),
                (Some(activity), None) => escape(activity),
                _ => "Idle".into(),
            },
            member.queued,
        ));
    }
    content.push_str(&format!(
        "</tbody></table><p>Hire proposals awaiting the owner: {}.</p></section>",
        floor.pending_hires
    ));

    // Certification and practice.
    content.push_str(
        "<section class=\"cloud-card\" id=\"sales-certification\"><h3>Certification</h3>",
    );
    if floor.certifications.is_empty() {
        content.push_str("<p>No certification is recorded.</p>");
    } else {
        content.push_str("<ul>");
        for cert in &floor.certifications {
            content.push_str(&format!(
                "<li>{} · version {} · {} · expires {} · outbound authority: {}</li>",
                escape(&cert.agent),
                cert.version,
                cert_label(cert.state, cert.measured_qualified),
                cert.expires_at,
                if cert.outbound_authority {
                    "yes"
                } else {
                    "none"
                },
            ));
        }
        content.push_str("</ul>");
    }
    content.push_str("</section>");

    // Expense holds.
    content
        .push_str("<section class=\"cloud-card\" id=\"sales-expense\"><h3>Model reservations</h3>");
    if floor.reservations.is_empty() {
        content.push_str("<p>No current reservation.</p>");
    } else {
        content.push_str("<table><thead><tr><th>Reservation</th><th>Agent</th><th>Chicago day</th><th>State</th><th>Maximum</th><th>Estimate</th><th>Billed</th></tr></thead><tbody>");
        for r in &floor.reservations {
            let unknown = || "Unknown".to_string();
            content.push_str(&format!(
                "<tr><td>{}{}</td><td>{}</td><td>{}</td><td>{}{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
                short(&r.id),
                if r.training { " · training" } else { "" },
                escape(&r.agent),
                r.day,
                reservation_label(r.status),
                if r.execution_unknown {
                    " · execution unknown"
                } else {
                    ""
                },
                usd(r.maximum_usd_millionths),
                r.estimated_usd_millionths.map_or_else(unknown, usd),
                r.billed_usd_millionths.map_or_else(unknown, usd),
            ));
        }
        content.push_str("</tbody></table>");
    }
    content.push_str("</section>");

    // Floor report and escalations.
    let r = &floor.report;
    let measure = |m: &report::Measure| match m {
        report::Measure::Known { value } => usd(*value),
        report::Measure::Unknown { reason } => format!("Unknown ({})", escape(reason)),
    };
    let stages: Vec<String> = r
        .stages
        .iter()
        .map(|(k, v)| format!("{} {v}", escape(k)))
        .collect();
    content.push_str(&format!(
        "<section class=\"cloud-card\" id=\"sales-report\"><h3>Floor report</h3><dl><dt>Records</dt><dd>{} · {}</dd><dt>Next actions</dt><dd>{} due · {} overdue</dd><dt>Messages</dt><dd>drafted {} · owner reviewed {} · rejected {} · proposed {} · approved {} · sent {} · replied {}</dd><dt>Delivery</dt><dd>delivered {} · hard bounce {} · failed {} · unknown {} · opt-out {} · complaint {}{}</dd><dt>Expense</dt><dd>reserved estimate {} · billed {} · unknown reservations {} · breaches {} · per qualified record {}</dd><dt>Open</dt><dd>{} incidents · {} replies awaiting review · outbox {}</dd></dl>",
        r.leads,
        if stages.is_empty() { "no stages".into() } else { stages.join(" · ") },
        r.next_actions_due,
        r.next_actions_overdue,
        r.messages.drafted,
        r.messages.owner_reviewed,
        r.messages.rejected,
        r.messages.proposed,
        r.messages.approved,
        r.messages.sent,
        r.messages.replied,
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
        usd(r.costs.reserved_estimate_usd_millionths),
        measure(&r.costs.billed_usd_millionths),
        r.costs.reservations_unknown,
        r.costs.reservations_breached,
        measure(&r.costs.per_qualified_lead_usd_millionths),
        r.unresolved_incidents,
        r.unresolved_replies,
        if r.outbox_paused { "stopped" } else { "running" },
    ));
    if !r.gaps.is_empty() {
        content.push_str("<p>Gaps:</p><ul>");
        for gap in &r.gaps {
            content.push_str(&format!("<li>{}</li>", escape(gap)));
        }
        content.push_str("</ul>");
    }
    if !floor.escalations.is_empty() {
        content.push_str("<h4>Escalations</h4><ul>");
        for e in &floor.escalations {
            content.push_str(&format!(
                "<li>{} · {} · at {}</li>",
                match e.severity {
                    report::Severity::Immediate => "Immediate",
                    report::Severity::Review => "Review",
                },
                escape(&e.kind.replace('_', " ")),
                e.at,
            ));
        }
        content.push_str("</ul>");
    }
    content.push_str("</section>");

    // Outbox.
    let o = &floor.outbox;
    content.push_str(&format!(
        "<section class=\"cloud-card\" id=\"sales-outbox\"><h3>Outbox</h3><p>Revision {} · controller epoch {} · {} · daily cap {} · today live {} and fixture {} messages and reservations. Level 0: each message needs the owner's approval of its exact proposal at its original revision before any handoff. Approval is not dispatch; handoff stays on the sales host.</p>",
        o.revision,
        o.controller_epoch,
        if o.paused {
            "dispatch stopped"
        } else {
            "dispatch running"
        },
        o.cap,
        o.live_messages_and_reservations,
        o.fixture_messages_and_reservations,
    ));
    if o.rows.is_empty() {
        content.push_str("<p>No outbox proposal.</p>");
    } else {
        content.push_str("<table class=\"sales-outbox\"><thead><tr><th>Proposal</th><th>Record</th><th>Kind</th><th>Mode</th><th>State</th><th>Chicago day</th><th>Subject</th></tr></thead><tbody>");
        for row in &o.rows {
            let name = if row.reviewable {
                format!(
                    "<a href=\"{base}/outbox/{}\">{}</a>",
                    escape(&row.id),
                    short(&row.id)
                )
            } else {
                format!("{} · minimized", short(&row.id))
            };
            content.push_str(&format!(
                "<tr><td>{name}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td><code>{}</code></td></tr>",
                short(&row.lead),
                word(&row.kind),
                word(&row.mode),
                phase_label(row.phase),
                row.business_day,
                short(&row.subject_sha256),
            ));
        }
        content.push_str("</tbody></table>");
    }
    let unresolved: Vec<_> = o.incidents.iter().filter(|i| !i.resolved).collect();
    if !unresolved.is_empty() {
        content.push_str("<p>Unresolved incidents:</p><ul>");
        for incident in unresolved {
            content.push_str(&format!(
                "<li>{} · {} · at {}</li>",
                short(&incident.id),
                word(&incident.kind),
                incident.at
            ));
        }
        content.push_str("</ul>");
    }
    if o.paused {
        content.push_str("<p>Dispatch is stopped. Pending handoffs are fenced and unknown deliveries stay unknown. Restart needs the owner's correction of every incident on the sales host.</p>");
    } else if standing.effects.contains(&Effect::OutboxStop) {
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
        content.push_str(&format!(
            "<form method=\"post\" action=\"{base}/floor/stop\">{}<input type=\"hidden\" name=\"request\" value=\"{request}\"><input type=\"hidden\" name=\"revision\" value=\"{}\"><button type=\"submit\">Stop dispatch at outbox revision {}</button></form><p class=\"dim\">Stop pauses the outbox controller: approved messages are not handed off and unknown deliveries keep their state.</p>",
            ticket(&csrf),
            o.revision,
            o.revision,
        ));
    }
    let requests = match context.sales.outbox_requests(&context.viewer, &id, None) {
        Ok(value) => value,
        Err(error) => return answer(error),
    };
    match request_list(&context, &headers, &delegation, &base, &requests) {
        Ok(list) => content.push_str(&list),
        Err(response) => return response,
    }
    content.push_str("</section>");

    // Replies and meetings.
    content.push_str("<section class=\"cloud-card\" id=\"sales-replies\"><h3>Replies</h3><p>Replies are untrusted. They cannot authorize work, payments, disclosure, or follow-ups; the owner reviews them on the sales host. Their text stays there.</p>");
    if floor.replies.is_empty() {
        content.push_str("<p>No reply is recorded.</p>");
    } else {
        content.push_str("<ul>");
        for reply in &floor.replies {
            content.push_str(&format!(
                "<li>{} · record {} · received {} · {} · {}{}</li>",
                short(&reply.id),
                reply
                    .lead
                    .as_deref()
                    .map_or_else(|| "unmatched".into(), short),
                reply.received_at,
                match reply.safety {
                    replies::Safety::Ordinary => "ordinary".to_string(),
                    other => format!("held: {}", word(&other)),
                },
                reply.owner_label.map_or_else(
                    || "awaiting owner review".into(),
                    |l| format!("owner label {}", word(&l))
                ),
                if reply.minimized { " · minimized" } else { "" },
            ));
        }
        content.push_str("</ul>");
    }
    content.push_str("</section><section class=\"cloud-card\" id=\"sales-meetings\"><h3>Meetings</h3><p>Suggestions book nothing; a human confirms and closes.</p>");
    if floor.meetings.is_empty() {
        content.push_str("<p>No meeting proposal.</p>");
    } else {
        content.push_str("<ul>");
        for m in &floor.meetings {
            content.push_str(&format!(
                "<li>{} · record {} · {} · {} to {}{}</li>",
                short(&m.id),
                short(&m.lead),
                match m.phase {
                    meetings::Phase::Pending => "Pending",
                    meetings::Phase::OwnerConfirmed => "Owner confirmed",
                    meetings::Phase::Accepted => "Accepted",
                    meetings::Phase::Declined => "Declined",
                    meetings::Phase::Retired => "Retired",
                },
                m.start_at,
                m.end_at,
                if m.owner_confirmation_needed {
                    " · awaiting owner confirmation"
                } else {
                    ""
                },
            ));
        }
        content.push_str("</ul>");
    }
    content.push_str("</section>");

    // The private Agora board.
    let board = match context.sales.board(&context.viewer, &id).await {
        Ok(board) => board_html(Ok(&board)),
        Err(_) => board_html(Err("the observation is unavailable")),
    };
    content.push_str(&format!(
        "<section class=\"cloud-card\" id=\"sales-board\"><h3>Private Agora board</h3><p>Counts from the sales owner, current for three seconds. A failed refresh or an inactive view clears them; no bell event, record, or person-linked amount appears here.</p><div hx-get=\"{base}/floor/board\" hx-trigger=\"every 1s\" hx-swap=\"innerHTML\" hx-sync=\"this:drop\" aria-live=\"off\">{board}</div></section>"
    ));
    shell(&context, &headers, &content)
}

fn request_list(
    context: &Context<'_>,
    headers: &HeaderMap,
    delegation: &Delegation,
    base: &str,
    requests: &[(String, Entry)],
) -> Result<String, Response> {
    if requests.is_empty() {
        return Ok(String::new());
    }
    let mut out = String::from("<h4>Requests</h4><ul class=\"sales-requests\">");
    for (request, entry) in requests {
        let (label, action, subject, extra) = match &entry.intent {
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
                format!("{base}/outbox/{}/decide", escape(proposal)),
                subject_sha256.clone(),
                format!(
                    "<input type=\"hidden\" name=\"subject\" value=\"{}\"><input type=\"hidden\" name=\"approve\" value=\"{approve}\">",
                    escape(subject_sha256)
                ),
            ),
            Intent::Stop => (
                format!("Stop dispatch at outbox revision {}", entry.revision),
                format!("{base}/floor/stop"),
                "stop".to_string(),
                String::new(),
            ),
        };
        match &entry.receipt {
            Some(receipt) => out.push_str(&format!(
                "<li>{label} · request {} · Recorded at outbox revision {} ({})</li>",
                escape(&request[..8]),
                receipt.revision,
                escape(&receipt.outcome.replace('_', " ")),
            )),
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
                out.push_str(&format!(
                    "<li>{label} · request {} · Outcome unknown<form method=\"post\" action=\"{action}\">{}<input type=\"hidden\" name=\"request\" value=\"{}\"><input type=\"hidden\" name=\"revision\" value=\"{}\">{extra}<button type=\"submit\">Retry the same request</button></form></li>",
                    escape(&request[..8]),
                    ticket(&csrf),
                    escape(request),
                    entry.revision,
                ));
            }
        }
    }
    out.push_str("</ul>");
    Ok(out)
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
    let base = format!("{PAGE}/{}", escape(&id));
    let mut attachments = String::new();
    for a in &found.attachments {
        attachments.push_str(&format!(
            "<li>{} · {} bytes · <code>{}</code></li>",
            escape(&a.filename),
            a.bytes,
            escape(&a.sha256)
        ));
    }
    let mut content = format!(
        "<p><a href=\"{base}/floor\">Sales floor</a></p><h2>Outbox proposal {}</h2><p>This is the exact subject the sales owner would hand off. It cannot be edited here: a changed draft needs a new proposal and its own grading. A decision binds this subject digest and outbox revision; the owner rechecks both, the controller, the reserved Chicago day, suppression, and current authority before recording it.</p><dl class=\"sales-proposal\"><dt>Kind</dt><dd>{}</dd><dt>Mode</dt><dd>{}</dd><dt>State</dt><dd>{}</dd><dt>Record</dt><dd><a href=\"{base}/leads/{}\">{}</a></dd><dt>Chicago business day</dt><dd>{}</dd><dt>Expires</dt><dd>{}</dd><dt>Sender</dt><dd>{}</dd><dt>Recipient</dt><dd>{}</dd><dt>Subject line</dt><dd>{}</dd><dt>Body</dt><dd><pre>{}</pre></dd><dt>Attachments</dt><dd>{}</dd><dt>Certification</dt><dd>{}</dd><dt>Graded draft</dt><dd>{}</dd><dt>Maximum cost</dt><dd>{} micro-USD</dd><dt>Subject digest</dt><dd><code>{}</code></dd><dt>Outbox revision</dt><dd>{}</dd></dl>",
        escape(&found.id),
        word(&found.kind),
        word(&found.mode),
        phase_label(found.phase),
        escape(&found.lead),
        short(&found.lead),
        found.business_day,
        found.expires_at,
        escape(&found.sender),
        escape(&found.recipient),
        escape(&found.subject),
        escape(&found.body),
        if attachments.is_empty() {
            "None".into()
        } else {
            format!("<ul>{attachments}</ul>")
        },
        found
            .certification_reference
            .as_deref()
            .map_or_else(|| "None".into(), escape),
        found
            .draft_reference
            .as_deref()
            .map_or_else(|| "None".into(), escape),
        found.maximum_cost_microusd,
        escape(&found.subject_sha256),
        found.outbox_revision,
    );
    let decidable = found.phase == outbox::Phase::Proposed
        && !found.paused
        && standing.effects.contains(&Effect::OutboxDecide);
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
            content.push_str(&format!(
                "<form method=\"post\" action=\"{base}/outbox/{}/decide\">{}<input type=\"hidden\" name=\"request\" value=\"{request}\"><input type=\"hidden\" name=\"revision\" value=\"{}\"><input type=\"hidden\" name=\"subject\" value=\"{}\"><input type=\"hidden\" name=\"approve\" value=\"{approve}\"><button type=\"submit\">{label} at outbox revision {}</button></form>",
                escape(&found.id),
                ticket(&csrf),
                found.outbox_revision,
                escape(&found.subject_sha256),
                found.outbox_revision,
            ));
        }
    } else if found.paused {
        content.push_str("<p>Dispatch is stopped; no decision is offered until the owner restarts it on the sales host.</p>");
    } else if found.phase == outbox::Phase::Proposed {
        content.push_str("<p>This delegation has no outbox decision grant.</p>");
    } else {
        content.push_str("<p>This proposal is no longer awaiting a decision.</p>");
    }
    let requests = match context
        .sales
        .outbox_requests(&context.viewer, &id, Some(&found.id))
    {
        Ok(value) => value,
        Err(error) => return answer(error),
    };
    match request_list(&context, &headers, &delegation, &base, &requests) {
        Ok(list) => content.push_str(&list),
        Err(response) => return response,
    }
    shell(&context, &headers, &content)
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
pub(super) fn floor_link(id: &str) -> String {
    format!(
        "<p><a href=\"{PAGE}/{}/floor\">Supervise the sales floor</a>: Paul, crew, certification, expense holds, outbox decisions, replies, meetings, and the private Agora board.</p>",
        escape(id)
    )
}
