//! Partners, referrals, earnings, and payout views (WEB-16).
//!
//! Every line here is an original retained record read under the viewer's own
//! current session and selected workspace, from the owner that keeps it:
//!
//! - referral source, attribution decisions, and any accepted commission
//!   agreement from the account service, pinned to the viewer's account;
//! - payee earnings and payouts from the joined statement's separate payee
//!   section (WEB-11's owner), under the account's own payee read approval;
//! - partner assignments, and for the sales owner Arthur's partner brief,
//!   Vanna's attribution view, and the earned-sale ledger, from the resident
//!   sales adapter (WEB-13) under an operator-provisioned delegation.
//!
//! Nothing is recomputed: native totals are shown as the owner reported them,
//! roles and units are never summed together, an unresolved payout stays
//! unresolved, and a record this page cannot type exactly is refused rather
//! than partly shown. An invitation is not an obligation, attribution is not
//! a commission, and no identity, agent, or self-referral is a payment right.
//! These pages offer no control that moves money; payout management stays
//! with its native owner and its own admission.

use super::sales::Failure;
use super::session::{SessionError, Viewer};
use super::ui;
use super::{failure, protect, refused, service, workspace_shell};
use crate::App;
use axum::Router;
use axum::body::Body;
use axum::extract::{Query, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::get;
use coder::task::sales::Role;
use coder::task::sales::earned::{Ledger, Settlement};
use coder::task::sales::partners::{Assignment, Status, Terms, View};
use coder::task::sales::remote::{Op, PartnerCursor, Standing, record_id};
use coder::task::sales::roles::{AttributionView as DeskAttribution, Brief, Desk};
use maud::{Markup, html};
use openagents_ui::forms::{Field, Input};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

const PAGE: &str = "/cloud/app/partners";
const EARNINGS_PAGE: usize = 50;

pub(super) fn routes() -> Router<App> {
    Router::new()
        .route(PAGE, get(index))
        .route("/cloud/app/partners/export", get(export))
}

/// Reachable whenever a workspace is selected; each section says whether
/// its own owner admits it.
pub(crate) fn available(viewer: &Viewer) -> bool {
    viewer.workspace.is_some()
}

// ---- Query ---------------------------------------------------------------------

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct Selection {
    #[serde(default)]
    after_earning: Option<i64>,
    #[serde(default)]
    after_payout: Option<i64>,
    /// A referred customer whose commission agreement to read; the account
    /// service decides whether this account is a party to it.
    #[serde(default)]
    customer: Option<String>,
    #[serde(default)]
    delegation: Option<String>,
    #[serde(default)]
    after_lead: Option<String>,
    #[serde(default)]
    after_assignment: Option<String>,
}

fn identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
}

impl Selection {
    fn check(&self) -> Result<(), SessionError> {
        let cursor_pair = self.after_lead.is_some() == self.after_assignment.is_some()
            && (self.after_lead.is_none() || self.delegation.is_some());
        if self.after_earning.is_some_and(|n| n < 0)
            || self.after_payout.is_some_and(|n| n < 0)
            || self.customer.as_deref().is_some_and(|c| !identifier(c))
            || self.delegation.as_deref().is_some_and(|d| !identifier(d))
            || self.after_lead.as_deref().is_some_and(|l| !record_id(l))
            || self
                .after_assignment
                .as_deref()
                .is_some_and(|a| !record_id(a))
            || !cursor_pair
        {
            return Err(SessionError::InvalidRequest);
        }
        Ok(())
    }

    fn href(&self, path: &str) -> String {
        let mut pairs = url::form_urlencoded::Serializer::new(String::new());
        for (name, value) in [
            ("after_earning", self.after_earning.map(|n| n.to_string())),
            ("after_payout", self.after_payout.map(|n| n.to_string())),
            ("customer", self.customer.clone()),
            ("delegation", self.delegation.clone()),
            ("after_lead", self.after_lead.clone()),
            ("after_assignment", self.after_assignment.clone()),
        ] {
            if let Some(value) = value {
                pairs.append_pair(name, &value);
            }
        }
        let query = pairs.finish();
        if query.is_empty() {
            path.into()
        } else {
            format!("{path}?{query}")
        }
    }

    fn cursor_for(&self, delegation: &str) -> Option<PartnerCursor> {
        if self.delegation.as_deref() != Some(delegation) {
            return None;
        }
        Some(PartnerCursor {
            lead: self.after_lead.clone()?,
            assignment: self.after_assignment.clone()?,
        })
    }
}

// ---- Payee statement (the joined statement's separate payee section) ----------

/// The payee section's exact shape. A field this page does not know refuses
/// the whole section rather than showing part of it.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Payee {
    party: String,
    unit: Value,
    statement: PayeeStatement,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct PayeeStatement {
    figures: Figures,
    earnings: Vec<Earning>,
    payouts: Vec<Payment>,
    next_earning: Option<i64>,
    next_payout: Option<i64>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Figures {
    earned_msat: i64,
    accrued_msat: i64,
    reserved_msat: i64,
    consumed_msat: i64,
    sent_msat: i64,
    rounding_msat: i64,
    unverified_sent_msat: i64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Earning {
    sequence: i64,
    resource: String,
    plugin_id: Option<String>,
    release_id: Option<String>,
    settled_at: i64,
    rule_version: i64,
    obligations: Vec<Obligation>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Obligation {
    role: String,
    amount_msat: i64,
    state: String,
    payout: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Payment {
    cursor: i64,
    id: String,
    amount_msat: i64,
    destination: String,
    rail: String,
    state: String,
    wallet_reference: Option<String>,
    attempts: i64,
    created_at: i64,
    updated_at: i64,
    sent_msat: Option<i64>,
    fee_msat: Option<i64>,
    reason: Option<String>,
    reason_digest: Option<String>,
    lookup_required: bool,
}

fn payee(value: &Value) -> Result<Payee, ()> {
    let payee: Payee = serde_json::from_value(value.clone()).map_err(|_| ())?;
    if payee.unit != json!({"kind":"millisatoshis"}) || payee.party.is_empty() {
        return Err(());
    }
    Ok(payee)
}

// ---- Gathered reads --------------------------------------------------------------

enum Lane<T> {
    Read(T),
    /// The owner has no record for this account, or none is admitted.
    Absent,
    Unavailable(&'static str),
}

struct Referrals {
    source: Lane<Option<jev::ReferralSource>>,
    attribution: Lane<Option<jev::AttributionView>>,
    workspace: Lane<Option<jev::WorkspaceAttribution>>,
    agreement: Lane<Option<jev::CommissionView>>,
    agreement_customer: Option<String>,
}

struct Delegated {
    id: String,
    standing: Result<Standing, &'static str>,
    partners: Result<Vec<View>, &'static str>,
    arthur: Option<Result<Brief, &'static str>>,
    vanna: Option<Result<DeskAttribution, &'static str>>,
    earned: Option<Result<Ledger, &'static str>>,
}

struct Gathered {
    referrals: Referrals,
    payee: Lane<Payee>,
    delegated: Vec<Delegated>,
}

/// A revoked native session refuses the whole page; any other native
/// refusal leaves only its own section unavailable.
fn lane<T>(read: jev::Result<T>) -> Result<Lane<T>, SessionError> {
    match read {
        Ok(value) => Ok(Lane::Read(value)),
        Err(jev::Error::Api(api)) if api.status == 401 => Err(SessionError::Unauthenticated),
        Err(jev::Error::Api(api)) if matches!(api.status, 403 | 404 | 409) => Ok(Lane::Absent),
        Err(_) => Ok(Lane::Unavailable(
            "The owner did not answer with a record this page can show exactly.",
        )),
    }
}

fn delegated_failure(error: &Failure) -> &'static str {
    match error {
        Failure::Owner(coder::task::sales::remote::Code::AccessDenied) => {
            "The sales owner refused this binding's current credential or record access."
        }
        Failure::Owner(coder::task::sales::remote::Code::Refused) => {
            "The sales owner has no current approved projection for this request."
        }
        Failure::Session(_) => "This delegation is not current for your session and workspace.",
        _ => "The sales owner is unavailable. Nothing is estimated in its place.",
    }
}

struct Context<'a> {
    app: &'a App,
    service: &'a super::session::CloudSession,
    viewer: Viewer,
    workspace: String,
    epoch: u64,
}

async fn context<'a>(app: &'a App, headers: &HeaderMap) -> Result<Context<'a>, Response> {
    let service = service(app)?;
    let viewer = match service.authenticate(headers).await {
        Ok(value) => value,
        Err(SessionError::Unauthenticated) => {
            return Err(protect(Redirect::to("/cloud/sign-in").into_response()));
        }
        Err(error) => return Err(refused(error)),
    };
    let Some(selected) = viewer.workspace.as_ref() else {
        return Err(failure(
            StatusCode::FORBIDDEN,
            "Select a workspace",
            "Partner, referral, and earnings reads bind one selected workspace and its current membership.",
        ));
    };
    let (workspace, epoch) = (selected.id.clone(), selected.members_epoch);
    Ok(Context {
        app,
        service,
        viewer,
        workspace,
        epoch,
    })
}

impl Context<'_> {
    /// A read finished under the membership it started with; otherwise its
    /// result is refused rather than shown as current.
    async fn still_current(&self, headers: &HeaderMap) -> Result<(), Response> {
        let now = self.service.authenticate(headers).await.map_err(refused)?;
        let same = now.account_id == self.viewer.account_id
            && now
                .workspace
                .as_ref()
                .is_some_and(|w| w.id == self.workspace && w.members_epoch == self.epoch);
        if !same {
            return Err(refused(SessionError::Conflict));
        }
        Ok(())
    }

    async fn gather(&self, selection: &Selection) -> Result<Gathered, Response> {
        let client = self.viewer.client();
        let account = || {
            client
                .account()
                .for_referrals_account(&self.viewer.account_id)
        };
        let source = lane(account().acquisition().await).map_err(refused)?;
        let attribution = lane(account().attribution().await).map_err(refused)?;
        let workspace =
            lane(account().workspace_attribution(&self.workspace).await).map_err(refused)?;
        // The viewer's own accepted binding names the customer by default;
        // a referrer names a customer and the service decides party access.
        let agreement_customer = selection.customer.clone().or_else(|| match &attribution {
            Lane::Read(Some(view)) if view.binding.is_some() => Some(view.customer.clone()),
            _ => None,
        });
        let agreement = match &agreement_customer {
            Some(customer) => {
                lane(account().commission_agreement(customer, None).await).map_err(refused)?
            }
            None => Lane::Absent,
        };
        let query = jev::JoinedStatementQuery {
            cursor: None,
            limit: Some(EARNINGS_PAGE),
            after_earning: selection.after_earning,
            after_payout: selection.after_payout,
        };
        let payee_lane = match lane(
            client
                .account()
                .joined_statement(&self.workspace, &query, false)
                .await,
        )
        .map_err(refused)?
        {
            Lane::Read(view) => match &view.payee {
                Some(value) => match payee(value) {
                    Ok(payee) => Lane::Read(payee),
                    Err(()) => {
                        return Err(failure(
                            StatusCode::CONFLICT,
                            "Earnings record unreadable",
                            "The native payee statement contained a record this page cannot show exactly. Nothing is shown in its place.",
                        ));
                    }
                },
                None => Lane::Absent,
            },
            Lane::Absent => Lane::Absent,
            Lane::Unavailable(why) => Lane::Unavailable(why),
        };
        let mut delegated = Vec::new();
        if let Some(delegations) = self.app.config.cloud_sales.as_deref() {
            for delegation in delegations.current(&self.viewer) {
                let id = delegation.id().to_owned();
                let standing = delegations
                    .read::<Standing>(&self.viewer, &id, Op::Standing)
                    .await
                    .map_err(|e| delegated_failure(&e));
                let partners = delegations
                    .read::<Vec<View>>(
                        &self.viewer,
                        &id,
                        Op::Partners {
                            after: selection.cursor_for(&id),
                            limit: coder::task::sales::partners::MAX_LISTED,
                        },
                    )
                    .await
                    .map_err(|e| delegated_failure(&e));
                let owner = matches!(&standing, Ok(s) if s.role == Role::Owner);
                let (mut arthur, mut vanna, mut earned) = (None, None, None);
                if owner {
                    arthur = Some(
                        delegations
                            .read::<Brief>(&self.viewer, &id, Op::Desk { desk: Desk::Arthur })
                            .await
                            .map_err(|e| delegated_failure(&e)),
                    );
                    vanna = Some(
                        delegations
                            .read::<DeskAttribution>(
                                &self.viewer,
                                &id,
                                Op::Desk { desk: Desk::Vanna },
                            )
                            .await
                            .map_err(|e| delegated_failure(&e)),
                    );
                    earned = Some(
                        delegations
                            .read::<Ledger>(&self.viewer, &id, Op::Earned)
                            .await
                            .map_err(|e| delegated_failure(&e)),
                    );
                }
                delegated.push(Delegated {
                    id,
                    standing,
                    partners,
                    arthur,
                    vanna,
                    earned,
                });
            }
        }
        Ok(Gathered {
            referrals: Referrals {
                source,
                attribution,
                workspace,
                agreement,
                agreement_customer,
            },
            payee: payee_lane,
            delegated,
        })
    }
}

// ---- Rendering -------------------------------------------------------------------

fn msat(value: i64) -> String {
    format!("{value} msat")
}

/// A section whose owner could not be read: the title and the owner's reason.
fn unavailable(title: &str, why: &str) -> Markup {
    ui::unavailable(&format!("{title} \u{b7} Unavailable"), why)
}

fn snake<T: Serialize>(value: &T) -> String {
    match serde_json::to_value(value) {
        Ok(Value::String(s)) => s.replace('_', " "),
        _ => "unknown".into(),
    }
}

fn render_referrals(r: &Referrals) -> Markup {
    let source = match &r.source {
        Lane::Read(Some(source)) => {
            let referrer = source.referrer.as_ref().map_or("none".into(), |i| {
                format!(
                    "{} version {} \u{b7} {}{}",
                    i.id,
                    i.version,
                    snake(&i.kind),
                    if i.source_only {
                        " \u{b7} source only, never paid"
                    } else {
                        ""
                    }
                )
            });
            html! {
                section class="cloud-card" id="referral-source" {
                    h4 { "Original source" }
                    (ui::Details::new()
                        .row("Outcome", &source.outcome)
                        .row("Referrer", referrer)
                        .row("Consent version", source.consent_version.as_deref().unwrap_or("none"))
                        .row("Captured at", source.captured_at))
                    p { "A captured source is attribution evidence only; it creates no commission." }
                }
            }
        }
        Lane::Read(None) | Lane::Absent => html! {
            p id="referral-source" { "No original referral source is recorded for this account." }
        },
        Lane::Unavailable(why) => unavailable("Original source", why),
    };
    let attribution = match &r.attribution {
        Lane::Read(Some(view)) => {
            let binding = view.binding.as_ref().map_or_else(
                || "none accepted".to_owned(),
                |b| {
                    format!(
                        "{} \u{b7} referrer {} version {} \u{b7} policy {} \u{b7} accepted decision {}",
                        b.id,
                        b.referrer.id,
                        b.referrer.version,
                        b.policy_digest,
                        b.accepted_decision
                    )
                },
            );
            html! {
                section class="cloud-card" id="attribution" {
                    h4 { "Attribution" }
                    (ui::Details::new()
                        .row("Customer", &view.customer)
                        .row("Status", snake(&view.status))
                        .row("Permanent binding", binding)
                        .row(
                            "Commission eligibility",
                            if view.commission_eligibility {
                                "eligible for an accepted agreement; attribution alone accrues nothing"
                            } else {
                                "not eligible"
                            },
                        ))
                    ol class="attribution-decisions" {
                        @for d in &view.decisions {
                            li {
                                "Decision " (d.digest) " \u{b7} sequence " (d.sequence)
                                " \u{b7} " (snake(&d.introduction)) " \u{b7} " (snake(&d.status))
                                @if let Some(review) = &d.review {
                                    " \u{b7} under review: " (snake(review))
                                }
                                " \u{b7} policy " (d.policy_digest)
                            }
                        }
                    }
                    p { "A self-referral, a source-only identity, or an agent's own link is reviewed and never paid." }
                }
            }
        }
        Lane::Read(None) | Lane::Absent => html! {
            p id="attribution" { "No attribution decision is recorded for this account." }
        },
        Lane::Unavailable(why) => unavailable("Attribution", why),
    };
    let workspace = match &r.workspace {
        Lane::Read(Some(w)) => html! {
            p id="workspace-attribution" {
                "Workspace " (w.workspace) " adopts binding " (w.binding.id) " (" (snake(&w.status)) ")."
            }
        },
        Lane::Read(None) | Lane::Absent => html! {
            p id="workspace-attribution" { "This workspace has adopted no attribution." }
        },
        Lane::Unavailable(why) => unavailable("Workspace attribution", why),
    };
    let agreement = match &r.agreement {
        Lane::Read(Some(view)) => {
            let a = &view.agreement;
            let mut details = ui::Details::new()
                .row("Agreement", &a.id)
                .row("Customer", &a.customer)
                .row(
                    "Accepted terms",
                    format!(
                        "version {} \u{b7} {}",
                        view.terms.terms["version"].as_str().unwrap_or("unknown"),
                        a.terms_digest
                    ),
                )
                .row("State", &view.state);
            for (role, acceptance) in &a.acceptances {
                details = details.row(
                    &format!("Accepted by {role}"),
                    format!("{} at {}", acceptance.actor, acceptance.accepted_at),
                );
            }
            details = details
                .row(
                    "Accrual",
                    if view.accrual_enabled {
                        "enabled"
                    } else {
                        "not enabled"
                    },
                )
                .row(
                    "Payout",
                    if view.payout_enabled && view.payout_qualified {
                        "enabled by its own native admission"
                    } else {
                        "not enabled"
                    },
                );
            html! {
                section class="cloud-card" id="commission-agreement" {
                    h4 { "Commission agreement" }
                    (details)
                    p { "An accepted agreement names terms; only original settled revenue under it earns, and only its native owner pays." }
                }
            }
        }
        Lane::Read(None) | Lane::Absent => html! {
            @if let Some(customer) = &r.agreement_customer {
                p id="commission-agreement" {
                    "No commission agreement you are party to is recorded for customer " (customer) "."
                }
            }
        },
        Lane::Unavailable(why) => unavailable("Commission agreement", why),
    };
    let customer = Field::new("partners-customer", "Referred customer").required(true);
    html! {
        h3 { "Referrals and attribution" }
        (source)
        (attribution)
        (workspace)
        (agreement)
        form class="cloud-form" method="get" action=(PAGE) {
            (customer.clone().control(
                Input::new("customer").maxlength(128).required(true).aria(customer.aria()),
            ))
            div class="cloud-form-actions" { (ui::submit("Read agreement", false)) }
        }
    }
}

fn render_payee(lane: &Lane<Payee>, selection: &Selection) -> Markup {
    let payee = match lane {
        Lane::Read(payee) => payee,
        Lane::Absent => {
            return html! {
                h3 { "Earnings and payouts" }
                p id="payee" { "No payee read is approved for this account and workspace. Earnings appear only under the account's own original payee authority." }
            };
        }
        Lane::Unavailable(why) => {
            return html! {
                h3 { "Earnings and payouts" }
                (unavailable("Payee statement", why))
            };
        }
    };
    let f = &payee.statement.figures;
    let mut figures = ui::Details::new();
    for (label, value) in [
        ("Earned", f.earned_msat),
        ("Accrued, not reserved", f.accrued_msat),
        ("Reserved for a payout in progress", f.reserved_msat),
        ("Consumed by sent payouts", f.consumed_msat),
        ("Sent on the rail", f.sent_msat),
        ("Rounding", f.rounding_msat),
        (
            "Sent without a verified rail amount",
            f.unverified_sent_msat,
        ),
    ] {
        figures = figures.row(label, msat(value));
    }
    let payouts: Vec<String> = payee
        .statement
        .payouts
        .iter()
        .map(|p| {
            let mut line = format!(
                "Payout {} \u{b7} {} to {} on {} \u{b7} {}",
                p.id,
                msat(p.amount_msat),
                p.destination,
                p.rail,
                p.state
            );
            if let Some(sent) = p.sent_msat {
                line.push_str(&format!(" \u{b7} rail amount {}", msat(sent)));
            }
            if let Some(fee) = p.fee_msat {
                line.push_str(&format!(" \u{b7} fee {}", msat(fee)));
            }
            if let Some(reference) = &p.wallet_reference {
                line.push_str(&format!(" \u{b7} wallet reference {reference}"));
            }
            line.push_str(&format!(" \u{b7} {} attempts", p.attempts));
            if p.lookup_required {
                line.push_str(
                    " \u{b7} Outcome unresolved: only a wallet lookup by its owner can settle it",
                );
            }
            if let Some(reason) = &p.reason {
                line.push_str(&format!(" \u{b7} {}", reason.replace('_', " ")));
            }
            line
        })
        .collect();
    let mut links = Vec::new();
    if let Some(next) = payee.statement.next_earning {
        links.push((
            Selection {
                after_earning: Some(next),
                after_payout: selection.after_payout,
                customer: selection.customer.clone(),
                ..Default::default()
            }
            .href(PAGE),
            "Older earnings",
        ));
    }
    if let Some(next) = payee.statement.next_payout {
        links.push((
            Selection {
                after_earning: selection.after_earning,
                after_payout: Some(next),
                customer: selection.customer.clone(),
                ..Default::default()
            }
            .href(PAGE),
            "Older payouts",
        ));
    }
    html! {
        h3 { "Earnings and payouts" }
        section class="cloud-card" id="payee" {
            p {
                "Party " (payee.party) " \u{b7} unit msat. These are the payee owner's own totals over original settlements; this page adds nothing to them, and earnings are never netted against spending."
            }
            (figures)
        }
        section class="cloud-card" {
            h4 { "Earnings by original settlement" }
            @if payee.statement.earnings.is_empty() {
                p { "No earnings on this page." }
            } @else {
                ol class="payee-earnings" {
                    @for e in &payee.statement.earnings {
                        li {
                            "Settlement " (e.sequence) " \u{b7} " (e.resource)
                            @if let (Some(p), Some(r)) = (&e.plugin_id, &e.release_id) {
                                " \u{b7} plugin " (p) " release " (r)
                            }
                            " \u{b7} settled at " (e.settled_at) " \u{b7} rule version " (e.rule_version)
                            ul {
                                @for o in &e.obligations {
                                    li {
                                        (o.role) " share " (msat(o.amount_msat)) " \u{b7} " (o.state)
                                        @if let Some(p) = &o.payout { " \u{b7} payout " (p) }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        section class="cloud-card" {
            h4 { "Payouts" }
            @if payouts.is_empty() {
                p { "No payouts recorded." }
            } @else {
                ol class="payee-payouts" {
                    @for line in &payouts { li { (line) } }
                }
            }
            p { "Payout destinations and payout runs are managed only by their native owner under its own admission; this page cannot start, retry, or redirect a payout." }
            @if !links.is_empty() {
                (ui::links(links.iter().map(|(href, label)| (href.as_str(), *label))))
            }
        }
    }
}

fn status_label(status: Status) -> &'static str {
    match status {
        Status::Proposed => "Pending invitation",
        Status::Accepted => "Accepted",
        Status::Delivered => "Delivered",
        Status::Completed => "Completed with accepted support handoff",
        Status::Refused => "Refused",
        Status::TimedOut => "Expired",
        Status::Cancelled => "Cancelled",
    }
}

fn render_assignment(
    lead: &str,
    a: &Assignment,
    fulfillment: Option<&receipts::service_sale::Fulfillment>,
) -> Markup {
    let p = &a.proposal;
    let mut details = ui::Details::new()
        .row("Pipeline record", lead)
        .row("Responsible human", &p.recipient_human)
        .row("Owner", &a.owner_human)
        .row("Expires at", p.expires_at);
    match &p.terms {
        Terms::Discovery { permitted_use, .. } => {
            details = details.row("Scope", format!("discovery \u{b7} {permitted_use}"));
        }
        Terms::Fulfillment {
            offer_version,
            service_sale,
            obligation,
            ..
        } => {
            details = details
                .row(
                    "Scope",
                    format!(
                        "fulfillment \u{b7} offer {offer_version} \u{b7} service sale {service_sale}"
                    ),
                )
                .row(
                    "Agreed compensation",
                    format!(
                        "{} minor units of {} (scale {}) \u{b7} agreement {} \u{b7} acceptance {}",
                        obligation.amount_minor,
                        obligation.currency,
                        obligation.currency_scale,
                        obligation.agreement.sha256,
                        obligation.acceptance.sha256
                    ),
                );
        }
    }
    details = match &a.proposal.commission {
        Some(c) => details.row(
            "Referral agreement",
            format!(
                "{} \u{b7} attribution {} \u{b7} accepted reference only, not an earned commission",
                c.agreement.sha256, c.attribution_id
            ),
        ),
        None => details.row("Referral agreement", "none"),
    };
    if let Some(next) = &a.next {
        details = details.row(
            "Next action",
            format!("{} by {}", next.description, next.due_at),
        );
    }
    details = details
        .row(
            "Support",
            match &a.handoff {
                Some(h) => format!(
                    "handoff to {} proposed, pending until {}",
                    h.target, h.expires_at
                ),
                None if a.events.iter().any(|e| e.outcome == "handoff_accepted") => {
                    "accepted by its support human".into()
                }
                None => "no handoff".into(),
            },
        )
        .row(
            "Delivery",
            match (&a.delivery_sale, fulfillment) {
                (Some(sale), Some(f)) => format!(
                    "service sale {sale} \u{b7} bill {} \u{b7} payment {}",
                    f.bill.as_ref().map_or("not billed", |_| "recorded"),
                    f.payment.as_ref().map_or("not paid", |_| "recorded")
                ),
                (Some(sale), None) => {
                    format!("service sale {sale} \u{b7} fulfillment not visible to you")
                }
                (None, _) => "not delivered".into(),
            },
        );
    html! {
        li class="partner-assignment" id=(format!("assignment-{}", p.id)) {
            h5 { (p.id) " \u{b7} " (status_label(a.status)) }
            (details)
            ol class="partner-events" {
                @for e in &a.events {
                    li { (e.outcome.replace('_', " ")) " \u{b7} " (e.actor) " at " (e.at) }
                }
            }
        }
    }
}

fn render_delegated(d: &Delegated, selection: &Selection) -> Markup {
    let more = match &d.partners {
        Ok(views) if views.len() == coder::task::sales::partners::MAX_LISTED => {
            let (lead, assignment) = match views.last() {
                Some(View::Accepted {
                    lead, assignment, ..
                }) => (lead.clone(), assignment.proposal.id.clone()),
                Some(View::Invitation { lead, id, .. }) => (lead.clone(), id.clone()),
                None => unreachable!(),
            };
            Some(
                Selection {
                    customer: selection.customer.clone(),
                    delegation: Some(d.id.clone()),
                    after_lead: Some(lead),
                    after_assignment: Some(assignment),
                    ..Default::default()
                }
                .href(PAGE),
            )
        }
        _ => None,
    };
    html! {
        section class="cloud-card" id=(format!("partners-{}", d.id)) {
            h4 { "Sales delegation " (d.id) }
            @match &d.standing {
                Ok(s) => {
                    p {
                        "Bound principal " (s.principal) " \u{b7} "
                        (match s.role {
                            Role::Owner => "sales owner",
                            Role::Writer => "writer",
                            Role::Reader => "reader",
                        })
                        "."
                    }
                }
                Err(why) => { p { (why) } }
            }
            @match &d.partners {
                Ok(views) if views.is_empty() => {
                    p { "No partner assignment or invitation names you." }
                }
                Ok(views) => {
                    ol class="partner-assignments" {
                        @for view in views {
                            @match view {
                                View::Accepted { lead, assignment, canonical_fulfillment, .. } => {
                                    (render_assignment(lead, assignment, canonical_fulfillment.as_ref()))
                                }
                                View::Invitation { id, kind, status, expires_at, .. } => {
                                    li class="partner-invitation" id=(format!("assignment-{id}")) {
                                        h5 { (id) " \u{b7} " (status_label(*status)) }
                                        p {
                                            (kind) " assignment, open until " (expires_at)
                                            ". An invitation creates no obligation and shows no terms until you accept it with its owner."
                                        }
                                    }
                                }
                            }
                        }
                    }
                    @if let Some(href) = &more {
                        p { a href=(href) { "More assignments" } }
                    }
                }
                Err(why) => { p { "Partner assignments: Unavailable. " (why) } }
            }
            @if let Some(arthur) = &d.arthur {
                h5 { "Arthur \u{b7} partner desk" }
                @match arthur {
                    Ok(brief) => {
                        p {
                            "Projected at " (brief.generated_at) " from current assignments; growth owner "
                            (brief.growth_owner) ". " (brief.disclosure)
                        }
                        ul class="desk-offerings" {
                            @for o in &brief.offerings {
                                li {
                                    (o.lead_reference) " \u{b7} " (o.kind) " \u{b7} " (status_label(o.status))
                                    @if o.handoff_pending { " \u{b7} handoff awaiting the owner" }
                                }
                            }
                        }
                    }
                    Err(why) => { p { "Unavailable. " (why) } }
                }
            }
            @if let Some(vanna) = &d.vanna {
                h5 { "Vanna \u{b7} affiliate desk" }
                @match vanna {
                    Ok(view) => {
                        p {
                            "Projected at " (view.generated_at) " from canonical acquisition sources; "
                            (view.under_review) " under review; payout authority: none. " (view.disclosure)
                        }
                        ul class="desk-attribution" {
                            @for row in &view.rows {
                                li {
                                    (row.lead_reference) " \u{b7} " (snake(&row.outcome))
                                    " \u{b7} earns: " (snake(&row.earns)) " \u{b7} "
                                    (row.settled_sales) " settled, " (row.reversed_sales) " reversed"
                                    @if !row.findings.is_empty() {
                                        " \u{b7} review: "
                                        (row.findings.iter().map(snake).collect::<Vec<_>>().join(", "))
                                    }
                                }
                            }
                        }
                    }
                    Err(why) => { p { "Unavailable. " (why) } }
                }
            }
            @if let Some(earned) = &d.earned {
                h5 { "Earned sales" }
                @match earned {
                    Ok(ledger) => { (render_earned(ledger)) }
                    Err(why) => { p { "Unavailable. " (why) } }
                }
            }
        }
    }
}

fn render_earned(ledger: &Ledger) -> Markup {
    let t = &ledger.totals;
    html! {
        p {
            "The sales owner's ledger at " (ledger.generated_at)
            ": a sale earns only on verified paid settlement and reconciled delivery. Reading it rings no bell; a refund or dispute adjusts its own row and never rings again."
        }
        ol class="earned-rows" {
            @for row in &ledger.rows {
                li {
                    "Sale " (&row.key[..row.key.len().min(23)])
                    " \u{b7} settlement "
                    (match row.settlement {
                        Settlement::Paid => "paid",
                        Settlement::Reversed => "reversed",
                        Settlement::Disputed => "disputed",
                        Settlement::Unknown => "unknown",
                        Settlement::Pending => "pending",
                        Settlement::Invalid => "invalid",
                    })
                    " \u{b7} gross " (row.gross_usd_millionths)
                    " \u{b7} refunded " (row.refunded_usd_millionths)
                    " \u{b7} net " (row.net_usd_millionths) " USD millionths \u{b7} delivery "
                    (if row.delivery_reconciled { "reconciled" } else { "not reconciled" })
                    " \u{b7} "
                    @if row.eligible {
                        "eligible"
                    } @else {
                        "not eligible: " (row.ineligible_because.join(", "))
                    }
                    @if let Some(at) = row.rung_at { " \u{b7} bell rang once at " (at) }
                }
            }
        }
        p {
            "Owner totals (USD millionths): gross " (t.gross_usd_millionths)
            " \u{b7} refunded " (t.refunded_usd_millionths)
            " \u{b7} net " (t.net_usd_millionths)
            " \u{b7} " (t.earned_sales) " earned sales \u{b7} "
            (t.unknown_settlement) " unknown settlements \u{b7} "
            (t.reversals) " reversals \u{b7} " (t.rung) " bells."
        }
    }
}

// ---- Handlers --------------------------------------------------------------------

async fn index(
    State(app): State<App>,
    headers: HeaderMap,
    Query(selection): Query<Selection>,
) -> Response {
    let context = match context(&app, &headers).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    if let Err(error) = selection.check() {
        return refused(error);
    }
    let gathered = match context.gather(&selection).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    if let Err(response) = context.still_current(&headers).await {
        return response;
    }
    let content = html! {
        h2 { "Partners, referrals, and earnings" }
        p {
            "Workspace " code { (context.workspace) }
            ". Each section is an original record read from its own owner under your current session. An invitation is not an obligation, attribution is not a commission, and nothing here moves money. "
            a href=(selection.href(&format!("{PAGE}/export"))) { "Export these records (NDJSON)" }
        }
        (render_referrals(&gathered.referrals))
        (render_payee(&gathered.payee, &selection))
        h3 { "Partner assignments" }
        @if gathered.delegated.is_empty() {
            p { "Partner assignments \u{b7} Unavailable: no sales delegation is provisioned for this account, workspace, and membership." }
        }
        @for d in &gathered.delegated {
            (render_delegated(d, &selection))
        }
    };
    workspace_shell(
        context.app,
        &headers,
        context.service,
        &context.viewer,
        "partners",
        Some(content),
        None,
    )
}

fn line<T: Serialize>(out: &mut String, record: &str, value: &T) -> Result<(), ()> {
    let mut value = serde_json::to_value(value).map_err(|_| ())?;
    value = json!({"record":record,"value":value});
    out.push_str(&serde_json::to_string(&value).map_err(|_| ())?);
    out.push('\n');
    Ok(())
}

/// The same scoped reads as the page, one original record per line. Nothing
/// outside the viewer's own account, payee, and named assignments is read.
fn ndjson(context: &Context<'_>, g: &Gathered) -> Result<String, ()> {
    let mut out = String::new();
    line(
        &mut out,
        "scope",
        &json!({"account":context.viewer.account_id,"workspace":context.workspace,"members_epoch":context.epoch}),
    )?;
    if let Lane::Read(Some(v)) = &g.referrals.source {
        line(&mut out, "referral_source", v)?;
    }
    if let Lane::Read(Some(v)) = &g.referrals.attribution {
        line(&mut out, "attribution", v)?;
    }
    if let Lane::Read(Some(v)) = &g.referrals.workspace {
        line(&mut out, "workspace_attribution", v)?;
    }
    if let Lane::Read(Some(v)) = &g.referrals.agreement {
        line(&mut out, "commission_agreement", v)?;
    }
    if let Lane::Read(v) = &g.payee {
        line(&mut out, "payee", v)?;
    }
    for d in &g.delegated {
        if let Ok(views) = &d.partners {
            for view in views {
                line(
                    &mut out,
                    "partner_assignment",
                    &json!({"delegation":d.id,"view":view}),
                )?;
            }
        }
        if let Some(Ok(v)) = &d.arthur {
            line(
                &mut out,
                "arthur_brief",
                &json!({"delegation":d.id,"brief":v}),
            )?;
        }
        if let Some(Ok(v)) = &d.vanna {
            line(
                &mut out,
                "vanna_attribution",
                &json!({"delegation":d.id,"view":v}),
            )?;
        }
        if let Some(Ok(v)) = &d.earned {
            line(
                &mut out,
                "earned_ledger",
                &json!({"delegation":d.id,"ledger":v}),
            )?;
        }
    }
    Ok(out)
}

async fn export(
    State(app): State<App>,
    headers: HeaderMap,
    Query(selection): Query<Selection>,
) -> Response {
    let context = match context(&app, &headers).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    if let Err(error) = selection.check() {
        return refused(error);
    }
    let gathered = match context.gather(&selection).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    if let Err(response) = context.still_current(&headers).await {
        return response;
    }
    let Ok(body) = ndjson(&context, &gathered) else {
        return refused(SessionError::Unavailable);
    };
    let mut response = Body::from(body).into_response();
    let headers = response.headers_mut();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/x-ndjson"),
    );
    headers.insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_static("attachment; filename=\"partners-and-earnings.ndjson\""),
    );
    protect(response)
}
