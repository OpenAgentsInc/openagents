//! Private sales modules over the separate sales-owner adapter: Evidence and
//! claims, Pilots and delivery, Invoices and fulfillment, Journeys and weekly
//! review, and a scoped record audit.
//!
//! Every module is a read of the owner's current records, fenced by the
//! owner's retention, suppression, and recipient checks on each request. No
//! module offers a form: nothing here signs an agreement, records a payment,
//! publishes, books, qualifies, accepts delivery, or performs cleanup.
//! Agreements, acceptances, support, invoices, payments, and reuse consent are
//! shown as the separate exact records the owner retains. Failed, unknown, and
//! repaired outcomes stay in every view.

use super::*;
use coder::task::sales::Audit;
use coder::task::sales::claims::{PriceTerms, Purpose, Verdict};
use coder::task::sales::offboarding::{self, View as Offboarding};
use coder::task::sales::remote::{Claims, Delivery, Handoff, RecordView, Weekly};
use receipts::sales_funnel::Journey;
use receipts::service_sale::{Disposition, FulfillmentTrigger, Reference, Sale};

/// The modules in navigation order: path suffix and title.
const MODULES: [(&str, &str); 5] = [
    ("", "Pipeline"),
    ("evidence", "Evidence and claims"),
    ("pilots", "Pilots and delivery"),
    ("invoices", "Invoices and fulfillment"),
    ("journeys", "Journeys and weekly review"),
];

/// The pinned requirement documents the views compare records against.
const KITS: [(&str, &str); 4] = [
    (
        "Pilot kit",
        include_str!("../../../../docs/sales/pilot-kit.json"),
    ),
    (
        "Delivery kit",
        include_str!("../../../../docs/sales/delivery-kit.json"),
    ),
    (
        "Install qualification",
        include_str!("../../../../docs/sales/install-qualification.json"),
    ),
    (
        "Team qualification",
        include_str!("../../../../docs/sales/team-qualification.json"),
    ),
];

pub(super) fn routes() -> Router<App> {
    Router::new()
        .route("/cloud/app/sales/{id}/evidence", get(evidence))
        .route("/cloud/app/sales/{id}/pilots", get(pilots))
        .route("/cloud/app/sales/{id}/invoices", get(invoices))
        .route("/cloud/app/sales/{id}/journeys", get(journeys))
        .route("/cloud/app/sales/{id}/leads/{lead}/audit", get(audit))
        .route(
            "/cloud/app/sales/{id}/leads/{lead}/services/{sale}",
            get(delivery),
        )
}

/// Module navigation for one delegation.
pub(super) fn nav(id: &str, current: &str) -> Markup {
    html! {
        nav class="sales-modules" aria-label="Sales modules" {
            ul {
                @for (suffix, title) in MODULES {
                    @let href = if suffix.is_empty() {
                        format!("{PAGE}#sales-{id}")
                    } else {
                        format!("{PAGE}/{id}/{suffix}")
                    };
                    li {
                        @if suffix == current {
                            a href=(href) aria-current="page" { (title) }
                        } @else {
                            a href=(href) { (title) }
                        }
                    }
                }
            }
        }
    }
}

impl Delegations {
    async fn records(&self, viewer: &Viewer, id: &str) -> Result<Vec<RecordView>, Failure> {
        let delegation = self.get(viewer, id)?.clone();
        self.call(
            viewer,
            &delegation,
            Op::Records {
                after: None,
                limit: 50,
            },
        )
        .await
    }

    async fn delivery(
        &self,
        viewer: &Viewer,
        id: &str,
        lead: &str,
        sale: &str,
    ) -> Result<Delivery, Failure> {
        if !record_id(lead) || !record_id(sale) {
            return Err(Failure::Session(SessionError::InvalidRequest));
        }
        let delegation = self.get(viewer, id)?.clone();
        self.call(
            viewer,
            &delegation,
            Op::Delivery {
                lead: lead.to_owned(),
                sale: sale.to_owned(),
            },
        )
        .await
    }

    async fn audit(&self, viewer: &Viewer, id: &str, lead: &str) -> Result<Vec<Audit>, Failure> {
        if !record_id(lead) {
            return Err(Failure::Session(SessionError::InvalidRequest));
        }
        let delegation = self.get(viewer, id)?.clone();
        self.call(
            viewer,
            &delegation,
            Op::Audit {
                lead: lead.to_owned(),
            },
        )
        .await
    }

    async fn claims(&self, viewer: &Viewer, id: &str) -> Result<Claims, Failure> {
        let delegation = self.get(viewer, id)?.clone();
        self.call(viewer, &delegation, Op::Claims).await
    }

    async fn weekly(&self, viewer: &Viewer, id: &str) -> Result<Option<Weekly>, Failure> {
        let delegation = self.get(viewer, id)?.clone();
        self.call(viewer, &delegation, Op::Weekly).await
    }
}

/// Authenticate, resolve the delegation, and read its current standing.
async fn open<'a>(
    app: &'a App,
    headers: &HeaderMap,
    id: &str,
) -> Result<(Context<'a>, Standing), Response> {
    let context = context(app, headers).await?;
    context.sales.get(&context.viewer, id).map_err(refused)?;
    let standing = context
        .sales
        .standing(&context.viewer, id)
        .await
        .map_err(answer)?;
    Ok((context, standing))
}

fn heading(id: &str, current: &str, title: &str, intro: &str) -> Markup {
    html! {
        p { a href=(PAGE) { "Private sales" } }
        (nav(id, current))
        h2 { (title) }
        p { (intro) }
    }
}

fn short(value: &str) -> String {
    value[..value.len().min(16)].to_owned()
}

fn reference(r: &Reference) -> String {
    format!("sha256 {}", short(&r.sha256))
}

fn label<T: Serialize>(value: &T) -> String {
    let value = serde_json::to_value(value).unwrap_or(Value::Null);
    let raw = match &value {
        Value::String(s) => s.clone(),
        Value::Object(map) => map
            .get("kind")
            .or_else(|| map.get("status"))
            .and_then(Value::as_str)
            .unwrap_or("unknown")
            .to_owned(),
        _ => "unknown".into(),
    };
    raw.replace('_', " ")
}

fn when(at: Option<u64>) -> String {
    at.map_or_else(|| "Not recorded".into(), |at| at.to_string())
}

/// An exact amount in its declared denomination, never converted.
fn amount(currency: &str, scale: u64, minor: u64) -> String {
    let digits = scale.checked_ilog10().unwrap_or(0) as usize;
    if scale > 1 && 10u64.checked_pow(digits as u32) == Some(scale) {
        format!(
            "{} {}.{:0width$}",
            currency,
            minor / scale,
            minor % scale,
            width = digits
        )
    } else {
        format!("{currency} {minor} (scale {scale})")
    }
}

fn record_heading(id: &str, record: &RecordView) -> Markup {
    let lead = &record.summary.id;
    html! {
        h3 {
            a href=(format!("{PAGE}/{id}/leads/{lead}")) { "Record " (short(lead)) }
            " \u{b7} " (stage_label(record.summary.stage))
            " \u{b7} responsible " (record.summary.responsible_human)
        }
    }
}

fn module_error(error: Failure) -> Result<Markup, Response> {
    match error {
        Failure::Session(error) => Err(refused(error)),
        Failure::Owner(Code::AccessDenied) => Ok(ui::denied(
            "Access refused",
            "Access refused for this binding's current credential or role. Nothing is shown.",
        )),
        Failure::Owner(Code::Stale) => Ok(ui::unavailable(
            "Sources changed",
            "The sources changed or exceed current custody. Nothing stale is shown; rebuild them with the sales owner.",
        )),
        _ => Ok(ui::unavailable(
            "Sales owner unavailable",
            "Sales owner: Unavailable. Nothing is shown.",
        )),
    }
}

/// "None recorded." or the given body.
fn none_or(empty: bool, body: Markup) -> Markup {
    html! {
        @if empty { p { "None recorded." } } @else { (body) }
    }
}

// ---- Pilots and delivery ---------------------------------------------------

async fn pilots(State(app): State<App>, headers: HeaderMap, Path(id): Path<String>) -> Response {
    let (context, _) = match open(&app, &headers, &id).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    let body = match context.sales.records(&context.viewer, &id).await {
        Ok(records) => {
            let shown: usize = records.iter().map(|r| r.services.len()).sum();
            let open: Vec<&RecordView> = records
                .iter()
                .filter(|r| r.summary.stage == Stage::Pilot && r.services.is_empty())
                .collect();
            html! {
                @for record in &records {
                    @for sale in &record.services { (pilot(&id, record, sale)) }
                }
                @if !open.is_empty() {
                    h3 { "Pilots without an accepted service record" }
                    p { "These records are at the Pilot stage. No accepted delivery or service record is retained for them yet; attempts, cost, and review stay with the owner until one is admitted." }
                    ul {
                        @for record in &open {
                            li {
                                a href=(format!("{PAGE}/{id}/leads/{}", record.summary.id)) {
                                    "Record " (short(&record.summary.id))
                                }
                                " \u{b7} responsible " (record.summary.responsible_human)
                                " \u{b7} workflow " (record.workflow)
                            }
                        }
                    }
                }
                @if shown == 0 {
                    (ui::empty("No service record", "No accepted service record is visible to this principal."))
                }
            }
        }
        Err(error) => match module_error(error) {
            Ok(markup) => markup,
            Err(response) => return response,
        },
    };
    let content = html! {
        (heading(
            &id,
            "pilots",
            "Pilots and delivery",
            "Each accepted pilot is shown from the sales owner's retained service record: workflow, exact source task and checks, the customer's decision maker, candidate, runbook, baseline comparison, and the separate agreement, review, handoff, customer acceptance, and support acceptance. This page records nothing; it signs, books, qualifies, accepts, and cleans up nothing.",
        ))
        (body)
        (kits())
    };
    shell(&context, &headers, content)
}

fn pilot(id: &str, record: &RecordView, sale: &Sale) -> Markup {
    let facts = &sale.facts;
    let sources = &sale.admission.sources;
    let list = |refs: &[Reference]| -> String {
        refs.iter().map(reference).collect::<Vec<_>>().join(", ")
    };
    let details = ui::Details::new()
        .row("Service record", sale.admission.id.as_str())
        .row("Account", sale.account.as_str())
        .row("Workflow", record.workflow.as_str())
        .row("Offer version", sale.admission.offer_version.as_str())
        .row("Task digest", short(&facts.task_digest))
        .row("Frozen checks", list(&facts.frozen_checks))
        .row("Accepted candidate", format!("sha256 {}", short(&facts.candidate_sha256)))
        .row("Runbook", reference(&facts.runbook))
        .row(
            "Baseline comparison",
            format!(
                "manifest {} \u{b7} report {} \u{b7} all failed, repair, and retry attempts included at review",
                reference(&facts.comparison_manifest),
                reference(&facts.comparison_report),
            ),
        )
        .row("Accepted checks", list(&facts.accepted_checks))
        .row("Deliverables", list(&facts.deliverables))
        .row("Customer decision maker", facts.customer_decision_maker.as_str())
        .row("Customer accepted at", facts.accepted_at)
        .row("Support owner", facts.support_human.as_str())
        .row(
            "Admitted by",
            format!("{} at {}", sale.admitted_by, sale.admitted_at),
        )
        .row(
            "Data use",
            format!(
                "{} \u{b7} recipients {} \u{b7} retained until {}",
                record.data.permitted_use,
                sale.admitted_recipients.join(", "),
                sale.retain_until,
            ),
        )
        .row(
            "Reuse and marketing",
            "No separate reuse, training, public example, or marketing permission is part of this record.",
        )
        .row("Offboarding", offboarding_line(record, &sale.admission.id));
    html! {
        section class="cloud-card sales-pilot" {
            (record_heading(id, record))
            (details)
            h4 { "Separate records" }
            ul class="sales-separate" {
                @for (name, r) in [
                    ("Agreement", &sources.agreement),
                    ("Agreement acceptance", &sources.agreement_acceptance),
                    ("Pilot review", &sources.pilot_review),
                    ("Delivery handoff", &sources.handoff),
                    ("Customer acceptance", &sources.customer_acceptance),
                    ("Support acceptance", &sources.support_acceptance),
                    ("Customer decision evidence", &facts.customer_decision_evidence),
                ] {
                    li { (name) ": " (reference(r)) }
                }
            }
            p {
                a href=(format!("{PAGE}/{id}/leads/{}/services/{}", record.summary.id, sale.admission.id)) {
                    "Delivery, support, and offboarding"
                }
            }
        }
    }
}

/// The pinned requirement documents, by exact digest. A kit is a document
/// requirement, never proof that a record satisfies it.
fn kits() -> Markup {
    let mut items = Vec::new();
    for (name, text) in KITS {
        let doc: Value = serde_json::from_str(text).unwrap_or(Value::Null);
        let digest = hex(&Sha256::digest(text.as_bytes()));
        let required = [
            "required_before_start",
            "required_before_customer_acceptance",
        ]
        .iter()
        .find_map(|f| doc[*f].as_array())
        .map_or_else(String::new, |r| {
            format!(" \u{b7} {} required fields", r.len())
        });
        let flags = match name {
            "Install qualification" => format!(
                " \u{b7} real customer qualified: {} \u{b7} commercial activation: {}",
                doc["scope"]["real_customer_qualified"], doc["scope"]["commercial_activation"]
            ),
            "Team qualification" => format!(
                " \u{b7} result {} \u{b7} production qualification: {}",
                doc["result"].as_str().unwrap_or("unknown"),
                doc["production_qualification"]
            ),
            _ => String::new(),
        };
        items.push(format!(
            "{name}: {} \u{b7} sha256 {}{required}{flags}",
            doc["schema"].as_str().unwrap_or("unknown"),
            &digest[..16],
        ));
    }
    html! {
        h3 { "Pinned kits" }
        p { "Records are compared against these exact documents. A kit is a requirement, not proof; filling one in records nothing." }
        ul class="sales-kits" {
            @for item in &items { li { (item) } }
        }
    }
}

// ---- Delivery, support, and offboarding ------------------------------------

async fn delivery(
    State(app): State<App>,
    headers: HeaderMap,
    Path((id, lead, sale)): Path<(String, String, String)>,
) -> Response {
    let (context, _) = match open(&app, &headers, &id).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    let found = match context
        .sales
        .delivery(&context.viewer, &id, &lead, &sale)
        .await
    {
        Ok(value) => value,
        Err(error) => return answer(error),
    };
    let top = html! {
        (heading(
            &id,
            "pilots",
            &format!("Delivery for service record {}", found.sale),
            "Read from the retained delivery handoff by its exact digest. The handoff plans support and offboarding; acceptance and verified cleanup are separate records. This page performs no cleanup and accepts nothing.",
        ))
        p {
            "Handoff sha256 " (short(&found.handoff_sha256)) " \u{b7} "
            a href=(format!("{PAGE}/{id}/leads/{lead}")) { "Record " (short(&lead)) }
        }
    };
    let Some(handoff) = found.handoff else {
        let reason = match found.unavailable.as_deref() {
            Some("not_configured") => {
                "The sales owner has no readable delivery evidence configured for this binding. No handoff detail is shown."
            }
            _ => {
                "The retained handoff is missing or no longer matches its recorded digest. No handoff detail is shown."
            }
        };
        let content = html! {
            (top)
            (ui::unavailable("No handoff detail", reason))
            (offboarding_section(found.offboarding.as_ref(), None))
        };
        return shell(&context, &headers, content);
    };
    let mut dependencies = ui::table("Dependencies").header([
        "Dependency",
        "Version or digest",
        "Scope",
        "Readiness",
        "Unavailable reason",
    ]);
    for d in &handoff.dependencies {
        dependencies = dependencies.row([
            d.id.as_str(),
            d.version_or_digest.as_str(),
            d.scope.as_str(),
            d.readiness.as_str(),
            d.unavailable_reason.as_str(),
        ]);
    }
    let support = &handoff.support;
    let content = html! {
        (top)
        (ui::Details::new()
            .row("Handoff", format!("{} {}", handoff.id, handoff.version))
            .row("Delivered at", when(handoff.delivered_at))
            .row("Reuse default", handoff.reuse_default.as_str()))
        h3 { "Dependencies" }
        (none_or(handoff.dependencies.is_empty(), dependencies.render()))
        h3 { "Known limits" }
        (none_or(handoff.known_limits.is_empty(), html! {
            ul { @for limit in &handoff.known_limits { li { (limit) } } }
        }))
        h3 { "Retained artifacts" }
        (none_or(handoff.retained_artifacts.is_empty(), html! {
            ul {
                @for a in &handoff.retained_artifacts {
                    li {
                        (a.id) " \u{b7} controller " (a.controller)
                        " \u{b7} retained until " (when(a.retain_until)) " \u{b7} " (a.purpose)
                    }
                }
            }
        }))
        h3 { "Support" }
        (ui::Details::new()
            .row("Responsible human", support.responsible_human.as_str())
            .row("Business hours", support.business_hours.as_str())
            .row("Response boundary", support.response_boundary.as_str())
            .row("Included work", support.included_work.as_str())
            .row("Out of scope", support.out_of_scope_route.as_str())
            .row("Ends at", when(support.ends_at)))
        p { "Support acceptance is the separate support-owner record listed with the pilot." }
        (offboarding_section(found.offboarding.as_ref(), Some(&handoff)))
    };
    shell(&context, &headers, content)
}

/// One cleanup item's due date, or "Not set".
fn due(at: Option<u64>) -> String {
    at.map_or_else(|| "Not set".into(), offboarding::date)
}

/// What happened to each planned cleanup item: removed, kept because it is
/// required, or not yet removed. Only the sales owner's checked record can
/// show an item as done; anything it does not cover reads as not yet removed.
fn offboarding_section(record: Option<&Offboarding>, handoff: Option<&Handoff>) -> Markup {
    let plan = handoff.map_or(&[][..], |h| h.cleanup_plan.as_slice());
    let responsible = |id: &str| {
        plan.iter()
            .find(|item| item.id == id)
            .map_or_else(String::new, |item| item.responsible_human.clone())
    };
    let mut cleanup =
        ui::table("Offboarding").header(["Item", "Kind", "Responsible", "Due", "Status"]);
    let summary = if let Some(record) = record {
        for row in &record.rows {
            cleanup = cleanup.row([
                row.id.clone(),
                row.class.replace('_', " "),
                responsible(&row.id),
                due(row.due_at),
                row.state.plain(),
            ]);
        }
        Some(record.summary())
    } else {
        for item in plan {
            cleanup = cleanup.row([
                item.id.clone(),
                item.class.replace('_', " "),
                item.responsible_human.clone(),
                due(item.due_at),
                "Not yet removed".to_owned(),
            ]);
        }
        None
    };
    html! {
        h3 { "Offboarding" }
        @match (summary, plan.is_empty()) {
            (Some(summary), _) => {
                p { (summary) "." }
                (cleanup.render())
            }
            (None, false) => {
                p { "No cleanup has been recorded yet." }
                (cleanup.render())
            }
            (None, true) => {
                p { "No cleanup plan was handed over for this delivery." }
            }
        }
    }
}

/// One sale's offboarding in a line, for the pilot list.
fn offboarding_line(record: &RecordView, sale: &str) -> String {
    record
        .offboarding
        .iter()
        .find(|o| o.sale == sale)
        .map_or_else(|| "No cleanup recorded yet".into(), Offboarding::summary)
}

// ---- Invoices and fulfillment ----------------------------------------------

async fn invoices(State(app): State<App>, headers: HeaderMap, Path(id): Path<String>) -> Response {
    let (context, _) = match open(&app, &headers, &id).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    let body = match context.sales.records(&context.viewer, &id).await {
        Ok(records) => {
            let shown: usize = records.iter().map(|r| r.services.len()).sum();
            html! {
                @for record in &records {
                    @for sale in &record.services { (invoice(&id, record, sale)) }
                }
                @if shown == 0 {
                    (ui::empty("No invoice", "No invoice is visible to this principal."))
                }
            }
        }
        Err(error) => match module_error(error) {
            Ok(markup) => markup,
            Err(response) => return response,
        },
    };
    let content = html! {
        (heading(
            &id,
            "invoices",
            "Invoices and fulfillment",
            "Invoices, verified payments, and separately priced fulfillment as the sales owner retains them. An invoice is not a payment, a payment funds no product usage, and only the owner's verification against receiver or processor evidence records one. Unknown, disputed, and reversed outcomes stay listed. This page records nothing.",
        ))
        (body)
    };
    shell(&context, &headers, content)
}

fn invoice(id: &str, record: &RecordView, sale: &Sale) -> Markup {
    let i = &sale.admission.invoice;
    let money = |minor: u64| amount(&i.currency, i.currency_scale, minor);
    let collection = match sale.summary() {
        Ok(summary) => html! {
            p {
                "Verified collection: paid " (money(summary.paid_minor))
                " \u{b7} refunded " (money(summary.refunded_minor))
                " \u{b7} refund reversals " (money(summary.refund_reversals_minor)) " \u{b7} "
                @if summary.unresolved {
                    "Unresolved: the latest outcome is pending, unknown, or disputed"
                } @else {
                    "Resolved"
                }
            }
        },
        Err(_) => html! {
            p { "Verified collection: the payment history does not reconcile. Treat it as unknown." }
        },
    };
    let mut payments = ui::table("Payment verifications").header([
        "Outcome",
        "Paid",
        "Reversed",
        "External reference",
        "Evidence",
        "Verified by",
        "At",
    ]);
    for v in &sale.payments {
        let outcome = match v.input.disposition {
            Disposition::Pending => "Pending",
            Disposition::Unknown => "Unknown",
            Disposition::Paid => "Paid",
            Disposition::Reversed => "Reversed",
            Disposition::Disputed => "Disputed",
        };
        payments = payments.row([
            outcome.to_owned(),
            v.input.paid_minor.map_or_else(|| "None".into(), money),
            v.input.reversed_minor.map_or_else(|| "None".into(), money),
            v.input
                .external_reference
                .clone()
                .unwrap_or_else(|| "None".into()),
            reference(&v.input.evidence),
            v.verified_by.clone(),
            v.verified_at.to_string(),
        ]);
    }
    let fulfillment = match sale.effective_fulfillment() {
        Ok(None) => html! { p { "No separately priced fulfillment obligation is admitted." } },
        Ok(Some(f)) => html! {
            (ui::Details::new()
                .row("Obligation", f.id.as_str())
                .row("Responsible human", f.responsible_human.as_str())
                .row("Amount", amount(&f.currency, f.currency_scale, f.amount_minor))
                .row("Trigger", match f.trigger {
                    FulfillmentTrigger::AcceptedDelivery => "Accepted delivery",
                    FulfillmentTrigger::VerifiedServicePayment => "Verified service payment",
                })
                .row("Agreement", reference(&f.agreement))
                .row("Acceptance", reference(&f.acceptance))
                .row("Bill", f.bill.as_ref().map_or_else(|| "Not billed".into(), reference))
                .row("Payment", f.payment.as_ref().map_or_else(|| "Not paid".into(), reference)))
            @for r in &sale.fulfillment_reconciliations {
                p {
                    "Reconciled by " (r.verified_by) " at " (r.verified_at)
                    ": bill " (reference(&r.input.bill))
                    @if let Some(p) = &r.input.payment { " \u{b7} payment " (reference(p)) }
                }
            }
        },
        Err(_) => {
            html! { p { "The fulfillment history does not reconcile. Treat it as unknown." } }
        }
    };
    html! {
        section class="cloud-card sales-invoice" {
            (record_heading(id, record))
            (ui::Details::new()
                .row("Invoice", format!("{} \u{b7} external {}", i.id, i.external_reference))
                .row("Amount", money(i.amount_minor))
                .row("Issued", i.issued_at)
                .row("Due", i.due_at)
                .row("Payment route", i.payment_route_reference.as_str())
                .row("Invoice evidence", reference(&i.evidence))
                .row("Invoice retention", sale.facts.invoice_retention_reference.as_str()))
            (collection)
            h4 { "Payment verifications" }
            @if sale.payments.is_empty() {
                p { "No payment is verified. The invoice remains unpaid until the owner records receiver or processor evidence." }
            } @else {
                (payments)
            }
            h4 { "Fulfillment" }
            (fulfillment)
        }
    }
}

// ---- Journeys and weekly review --------------------------------------------

async fn journeys(State(app): State<App>, headers: HeaderMap, Path(id): Path<String>) -> Response {
    let (context, standing) = match open(&app, &headers, &id).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    let body = match context.sales.records(&context.viewer, &id).await {
        Ok(records) => {
            let shown: usize = records.iter().map(|r| r.journeys.len()).sum();
            html! {
                @for record in &records {
                    @for j in &record.journeys { (journey(&id, record, j)) }
                }
                @if shown == 0 {
                    (ui::empty("No journey", "No consented journey is visible to this principal."))
                }
            }
        }
        Err(error) => match module_error(error) {
            Ok(markup) => markup,
            Err(response) => return response,
        },
    };
    let weekly = if standing.role != Role::Owner {
        html! { p { "The weekly review is owner-only and is not shown to this principal." } }
    } else {
        match context.sales.weekly(&context.viewer, &id).await {
            Ok(None) => {
                html! { p { "No weekly review sources are configured with the sales owner." } }
            }
            Ok(Some(weekly)) => review(&weekly),
            Err(error) => match module_error(error) {
                Ok(markup) => markup,
                Err(response) => return response,
            },
        }
    };
    let content = html! {
        (heading(
            &id,
            "journeys",
            "Journeys and weekly review",
            "Consented journey measurement, separate from the assisted pipeline. Every recorded event and failure stays listed, resolved or not. Measurement grants no outreach or tracking authority.",
        ))
        (body)
        h3 { "Weekly review" }
        (weekly)
    };
    shell(&context, &headers, content)
}

fn journey(id: &str, record: &RecordView, j: &Journey) -> Markup {
    let a = &j.admission;
    html! {
        section class="cloud-card sales-journey" {
            (record_heading(id, record))
            (ui::Details::new()
                .row("Journey", a.id.as_str())
                .row("Offer and cohort", format!("{} \u{b7} {}", a.offer_version, a.cohort))
                .row("Lane", format!("{} \u{b7} {}", label(&a.lane), label(&a.classification)))
                .row(
                    "Consent",
                    format!(
                        "at {} \u{b7} expires {} \u{b7} aggregate counts {} \u{b7} {}",
                        a.consent.at,
                        a.consent.expires_at,
                        if a.consent.aggregate_counts { "permitted" } else { "not permitted" },
                        reference(&a.consent.evidence),
                    ),
                )
                .row("Retained until", j.retain_until))
            @if j.events.is_empty() {
                p { "No event is recorded." }
            } @else {
                h4 { "Events" }
                ul {
                    @for e in &j.events {
                        li {
                            (e.input.id) " \u{b7} " (label(&e.input.kind)) " at " (e.input.at)
                            " \u{b7} recorded by " (e.recorded_by)
                        }
                    }
                }
            }
            @if !j.failures.is_empty() {
                h4 { "Failures and repairs" }
                ul {
                    @for f in &j.failures {
                        li {
                            (f.input.event) " \u{b7} " (label(&f.input.reason)) " \u{b7} "
                            (if f.input.resolved { "Resolved" } else { "Open" })
                            " \u{b7} responsible " (f.input.responsible_human)
                            " \u{b7} next " (f.input.next_action) " due " (f.input.due_at)
                            " \u{b7} " (reference(&f.input.evidence))
                        }
                    }
                }
            }
        }
    }
}

fn counts(value: &Value) -> String {
    value
        .as_object()
        .map(|map| {
            map.iter()
                .map(|(k, v)| format!("{} {}", k.replace('_', " "), v))
                .collect::<Vec<_>>()
                .join(" \u{b7} ")
        })
        .unwrap_or_default()
}

fn review(weekly: &Weekly) -> Markup {
    html! {
        (ui::Details::new()
            .row("Period", format!("{} to {}", weekly.period_start, weekly.period_end))
            .row("Generated", weekly.generated_at)
            .row("Manifest", short(&weekly.manifest_digest))
            .row("Finance", if weekly.finance_included { "Included" } else { "Not included" })
            .row(
                "Commercial activation attested",
                if weekly.commercial_activation_attested { "Yes" } else { "No" },
            )
            .row("Contribution scope", weekly.contribution_scope.as_str()))
        @for (title, items) in [("Gaps", &weekly.gaps), ("Limitations", &weekly.limitations)] {
            @if !items.is_empty() {
                h4 { (title) }
                ul { @for item in items.iter() { li { (item) } } }
            }
        }
        @if weekly.cohorts.is_empty() {
            p { "No cohort is in this period." }
        } @else {
            h4 { "Cohorts" }
            ul {
                @for c in &weekly.cohorts {
                    li {
                        (c["lane"].as_str().unwrap_or("unknown")) " \u{b7} "
                        (c["classification"].as_str().unwrap_or("unknown")) " \u{b7} "
                        (c["offer_version"].as_str().unwrap_or("")) " \u{b7} "
                        (c["cohort"].as_str().unwrap_or(""))
                        ": period " (counts(&c["period"])) " ; cumulative " (counts(&c["cumulative"]))
                    }
                }
            }
        }
        p { (weekly.journeys.len()) " journeys in this review, failed and unknown history included." }
    }
}

// ---- Evidence and claims ---------------------------------------------------

async fn evidence(State(app): State<App>, headers: HeaderMap, Path(id): Path<String>) -> Response {
    let (context, standing) = match open(&app, &headers, &id).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    let top = heading(
        &id,
        "evidence",
        "Evidence and claims",
        "Reviewed claims and price references with their review pins, expiry, withdrawal, and history. A verdict is the one recorded at review; a later read against current sources can still invalidate it. No claim of savings stands on an unverified demonstration. This page publishes and approves nothing.",
    );
    let body = if standing.role != Role::Owner {
        html! { p { "The claim register and its history are owner-only and are not shown to this principal." } }
    } else {
        match context.sales.claims(&context.viewer, &id).await {
            Ok(claims) => register(&claims),
            Err(error) => match module_error(error) {
                Ok(markup) => markup,
                Err(response) => return response,
            },
        }
    };
    let content = html! { (top) (body) };
    shell(&context, &headers, content)
}

fn register(claims: &Claims) -> Markup {
    let mut table = ui::table("Claim register").header([
        "Claim",
        "Purpose",
        "Source",
        "Verdict",
        "Price reference",
        "Reviewed",
        "Expires",
        "State",
    ]);
    for c in &claims.register {
        let (verdict, price) = match &c.verdict {
            Verdict::Allowed { claim } => {
                (format!("Allowed: {}", claim.wording), claim.price.as_ref())
            }
            Verdict::Unavailable {
                reason,
                proposed_price,
            } => (
                format!("Unavailable: {}", label(reason)),
                proposed_price.as_ref(),
            ),
            Verdict::Rejected { reason } => (format!("Rejected: {}", label(reason)), None),
        };
        let price = match price {
            None => "None".to_owned(),
            Some(PriceTerms::Service { terms }) => format!(
                "Service {} \u{b7} {}",
                terms.offer_version,
                amount(
                    &terms.currency,
                    terms.currency_scale,
                    terms.service_fee_minor_units
                )
            ),
            Some(PriceTerms::Retail { .. }) => "Retail price-book quote".to_owned(),
        };
        let mut state = Vec::new();
        if c.head {
            state.push("Current revision");
        } else {
            state.push("Superseded");
        }
        if c.withdrawn {
            state.push("Withdrawn");
        }
        if c.source_withdrawn {
            state.push("Source withdrawn");
        }
        if c.expired {
            state.push("Expired");
        }
        table = table.row([
            format!("{} r{}", c.pin.id, c.pin.revision),
            match c.purpose {
                Purpose::Capability => "Capability",
                Purpose::Price => "Price",
                Purpose::Comparison => "Comparison",
                Purpose::Launch => "Launch",
            }
            .to_owned(),
            format!("{} r{}", c.source.id, c.source.revision),
            verdict,
            price,
            format!(
                "{} at {} \u{b7} review sha256 {}",
                c.reviewer,
                c.reviewed_at,
                short(&c.review_sha256)
            ),
            c.expires_at.to_string(),
            state.join(" \u{b7} "),
        ]);
    }
    html! {
        @if claims.register.is_empty() {
            p { "No claim is reviewed." }
        } @else {
            div class="sales-claims" { (table) }
        }
        h3 { "Review history" }
        @if claims.history.is_empty() {
            p { "No review decision is recorded." }
        } @else {
            ol class="sales-claim-history" {
                @for d in &claims.history {
                    li {
                        "#" (d.sequence) " at " (d.at) " \u{b7} " (d.actor) " \u{b7} "
                        (d.operation.replace('_', " ")) " " (d.subject)
                        @if let Some(r) = &d.reason { " \u{b7} " (label(r)) }
                    }
                }
            }
        }
    }
}

// ---- Scoped audit ----------------------------------------------------------

async fn audit(
    State(app): State<App>,
    headers: HeaderMap,
    Path((id, lead)): Path<(String, String)>,
) -> Response {
    let (context, standing) = match open(&app, &headers, &id).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    let top = heading(
        &id,
        "",
        &format!("Audit for record {}", short(&lead)),
        "Owner audit entries for this one record only. Entries for other customers, teams, and contacts are excluded by the sales owner before they leave it.",
    );
    if standing.role != Role::Owner {
        let content = html! {
            (top)
            p { "The audit is owner-only and is not shown to this principal." }
        };
        return shell(&context, &headers, content);
    }
    let body = match context.sales.audit(&context.viewer, &id, &lead).await {
        Ok(entries) if entries.is_empty() => html! { p { "No audit entry is recorded." } },
        Ok(entries) => {
            let mut table = ui::table("Audit entries").header([
                "Sequence",
                "At",
                "Actor",
                "Operation",
                "Reference",
            ]);
            for e in entries {
                table = table.row([
                    e.sequence.to_string(),
                    e.at.to_string(),
                    e.actor.clone(),
                    e.operation.replace('_', " "),
                    short(&e.reference_digest),
                ]);
            }
            html! { div class="sales-audit" { (table) } }
        }
        Err(error) => return answer(error),
    };
    let content = html! { (top) (body) };
    shell(&context, &headers, content)
}
