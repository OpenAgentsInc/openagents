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
use coder::task::sales::remote::{Claims, Delivery, RecordView, Weekly};
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
pub(super) fn nav(id: &str, current: &str) -> String {
    let id = escape(id);
    let mut out = String::from("<nav class=\"sales-modules\" aria-label=\"Sales modules\"><ul>");
    for (suffix, title) in MODULES {
        let href = if suffix.is_empty() {
            format!("{PAGE}#sales-{id}")
        } else {
            format!("{PAGE}/{id}/{suffix}")
        };
        if suffix == current {
            out.push_str(&format!(
                "<li><a href=\"{href}\" aria-current=\"page\">{title}</a></li>"
            ));
        } else {
            out.push_str(&format!("<li><a href=\"{href}\">{title}</a></li>"));
        }
    }
    out.push_str("</ul></nav>");
    out
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

fn heading(id: &str, current: &str, title: &str, intro: &str) -> String {
    format!(
        "<p><a href=\"{PAGE}\">Private sales</a></p>{}<h2>{title}</h2><p>{intro}</p>",
        nav(id, current)
    )
}

fn short(value: &str) -> String {
    escape(&value[..value.len().min(16)])
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
    escape(&raw.replace('_', " "))
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
            escape(currency),
            minor / scale,
            minor % scale,
            width = digits
        )
    } else {
        format!("{} {minor} (scale {scale})", escape(currency))
    }
}

fn record_heading(id: &str, record: &RecordView) -> String {
    let lead = &record.summary.id;
    format!(
        "<h3><a href=\"{PAGE}/{}/leads/{}\">Record {}</a> · {} · responsible {}</h3>",
        escape(id),
        escape(lead),
        short(lead),
        stage_label(record.summary.stage),
        escape(&record.summary.responsible_human),
    )
}

fn module_error(error: Failure) -> Result<String, Response> {
    match error {
        Failure::Session(error) => Err(refused(error)),
        Failure::Owner(Code::AccessDenied) => Ok(
            "<p>Access refused for this binding's current credential or role. Nothing is shown.</p>"
                .into(),
        ),
        Failure::Owner(Code::Stale) => Ok(
            "<p>The sources changed or exceed current custody. Nothing stale is shown; rebuild them with the sales owner.</p>"
                .into(),
        ),
        _ => Ok("<p>Sales owner: Unavailable. Nothing is shown.</p>".into()),
    }
}

// ---- Pilots and delivery ---------------------------------------------------

async fn pilots(State(app): State<App>, headers: HeaderMap, Path(id): Path<String>) -> Response {
    let (context, _) = match open(&app, &headers, &id).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    let mut content = heading(
        &id,
        "pilots",
        "Pilots and delivery",
        "Each accepted pilot is shown from the sales owner's retained service record: workflow, exact source task and checks, the customer's decision maker, candidate, runbook, baseline comparison, and the separate agreement, review, handoff, customer acceptance, and support acceptance. This page records nothing; it signs, books, qualifies, accepts, and cleans up nothing.",
    );
    match context.sales.records(&context.viewer, &id).await {
        Ok(records) => {
            let mut shown = 0;
            for record in &records {
                for sale in &record.services {
                    shown += 1;
                    content.push_str(&pilot(&id, record, sale));
                }
            }
            let open: Vec<&RecordView> = records
                .iter()
                .filter(|r| r.summary.stage == Stage::Pilot && r.services.is_empty())
                .collect();
            if !open.is_empty() {
                content.push_str("<h3>Pilots without an accepted service record</h3><p>These records are at the Pilot stage. No accepted delivery or service record is retained for them yet; attempts, cost, and review stay with the owner until one is admitted.</p><ul>");
                for record in open {
                    content.push_str(&format!(
                        "<li><a href=\"{PAGE}/{}/leads/{}\">Record {}</a> · responsible {} · workflow {}</li>",
                        escape(&id),
                        escape(&record.summary.id),
                        short(&record.summary.id),
                        escape(&record.summary.responsible_human),
                        escape(&record.workflow),
                    ));
                }
                content.push_str("</ul>");
            }
            if shown == 0 {
                content.push_str("<p>No accepted service record is visible to this principal.</p>");
            }
        }
        Err(error) => match module_error(error) {
            Ok(text) => content.push_str(&text),
            Err(response) => return response,
        },
    }
    content.push_str(&kits());
    shell(&context, &headers, &content)
}

fn pilot(id: &str, record: &RecordView, sale: &Sale) -> String {
    let facts = &sale.facts;
    let sources = &sale.admission.sources;
    let list = |refs: &[Reference]| -> String {
        refs.iter().map(reference).collect::<Vec<_>>().join(", ")
    };
    let mut out = format!(
        "<section class=\"cloud-card sales-pilot\">{}<dl><dt>Service record</dt><dd>{}</dd><dt>Account</dt><dd>{}</dd><dt>Workflow</dt><dd>{}</dd><dt>Offer version</dt><dd>{}</dd><dt>Task digest</dt><dd>{}</dd><dt>Frozen checks</dt><dd>{}</dd><dt>Accepted candidate</dt><dd>sha256 {}</dd><dt>Runbook</dt><dd>{}</dd><dt>Baseline comparison</dt><dd>manifest {} · report {} · all failed, repair, and retry attempts included at review</dd><dt>Accepted checks</dt><dd>{}</dd><dt>Deliverables</dt><dd>{}</dd><dt>Customer decision maker</dt><dd>{}</dd><dt>Customer accepted at</dt><dd>{}</dd><dt>Support owner</dt><dd>{}</dd><dt>Admitted by</dt><dd>{} at {}</dd><dt>Data use</dt><dd>{} · recipients {} · retained until {}</dd><dt>Reuse and marketing</dt><dd>No separate reuse, training, public example, or marketing permission is part of this record.</dd></dl>",
        record_heading(id, record),
        escape(&sale.admission.id),
        escape(&sale.account),
        escape(&record.workflow),
        escape(&sale.admission.offer_version),
        short(&facts.task_digest),
        list(&facts.frozen_checks),
        short(&facts.candidate_sha256),
        reference(&facts.runbook),
        reference(&facts.comparison_manifest),
        reference(&facts.comparison_report),
        list(&facts.accepted_checks),
        list(&facts.deliverables),
        escape(&facts.customer_decision_maker),
        facts.accepted_at,
        escape(&facts.support_human),
        escape(&sale.admitted_by),
        sale.admitted_at,
        escape(&record.data.permitted_use),
        escape(&sale.admitted_recipients.join(", ")),
        sale.retain_until,
    );
    out.push_str("<h4>Separate records</h4><ul class=\"sales-separate\">");
    for (name, r) in [
        ("Agreement", &sources.agreement),
        ("Agreement acceptance", &sources.agreement_acceptance),
        ("Pilot review", &sources.pilot_review),
        ("Delivery handoff", &sources.handoff),
        ("Customer acceptance", &sources.customer_acceptance),
        ("Support acceptance", &sources.support_acceptance),
        (
            "Customer decision evidence",
            &facts.customer_decision_evidence,
        ),
    ] {
        out.push_str(&format!("<li>{name}: {}</li>", reference(r)));
    }
    out.push_str(&format!(
        "</ul><p><a href=\"{PAGE}/{}/leads/{}/services/{}\">Delivery, support, and offboarding</a></p></section>",
        escape(id),
        escape(&record.summary.id),
        escape(&sale.admission.id),
    ));
    out
}

/// The pinned requirement documents, by exact digest. A kit is a document
/// requirement, never proof that a record satisfies it.
fn kits() -> String {
    let mut out = String::from(
        "<h3>Pinned kits</h3><p>Records are compared against these exact documents. A kit is a requirement, not proof; filling one in records nothing.</p><ul class=\"sales-kits\">",
    );
    for (name, text) in KITS {
        let doc: Value = serde_json::from_str(text).unwrap_or(Value::Null);
        let digest = hex(&Sha256::digest(text.as_bytes()));
        let required = [
            "required_before_start",
            "required_before_customer_acceptance",
        ]
        .iter()
        .find_map(|f| doc[*f].as_array())
        .map_or_else(String::new, |r| format!(" · {} required fields", r.len()));
        let flags = match name {
            "Install qualification" => format!(
                " · real customer qualified: {} · commercial activation: {}",
                doc["scope"]["real_customer_qualified"], doc["scope"]["commercial_activation"]
            ),
            "Team qualification" => format!(
                " · result {} · production qualification: {}",
                escape(doc["result"].as_str().unwrap_or("unknown")),
                doc["production_qualification"]
            ),
            _ => String::new(),
        };
        out.push_str(&format!(
            "<li>{name}: {} · sha256 {}{required}{flags}</li>",
            escape(doc["schema"].as_str().unwrap_or("unknown")),
            &digest[..16],
        ));
    }
    out.push_str("</ul>");
    out
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
    let mut content = heading(
        &id,
        "pilots",
        &format!("Delivery for service record {}", escape(&found.sale)),
        "Read from the retained delivery handoff by its exact digest. The handoff plans support and offboarding; acceptance and verified cleanup are separate records. This page performs no cleanup and accepts nothing.",
    );
    content.push_str(&format!(
        "<p>Handoff sha256 {} · <a href=\"{PAGE}/{}/leads/{}\">Record {}</a></p>",
        short(&found.handoff_sha256),
        escape(&id),
        escape(&lead),
        short(&lead),
    ));
    let Some(handoff) = found.handoff else {
        content.push_str(match found.unavailable.as_deref() {
            Some("not_configured") => "<p>The sales owner has no readable delivery evidence configured for this binding. No handoff detail is shown.</p>",
            _ => "<p>The retained handoff is missing or no longer matches its recorded digest. No handoff detail is shown.</p>",
        });
        return shell(&context, &headers, &content);
    };
    content.push_str(&format!(
        "<dl><dt>Handoff</dt><dd>{} {}</dd><dt>Delivered at</dt><dd>{}</dd><dt>Reuse default</dt><dd>{}</dd></dl>",
        escape(&handoff.id),
        escape(&handoff.version),
        when(handoff.delivered_at),
        escape(&handoff.reuse_default),
    ));
    content.push_str("<h3>Dependencies</h3>");
    if handoff.dependencies.is_empty() {
        content.push_str("<p>None recorded.</p>");
    } else {
        content.push_str("<table><thead><tr><th>Dependency</th><th>Version or digest</th><th>Scope</th><th>Readiness</th><th>Unavailable reason</th></tr></thead><tbody>");
        for d in &handoff.dependencies {
            content.push_str(&format!(
                "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
                escape(&d.id),
                escape(&d.version_or_digest),
                escape(&d.scope),
                escape(&d.readiness),
                escape(&d.unavailable_reason),
            ));
        }
        content.push_str("</tbody></table>");
    }
    content.push_str("<h3>Known limits</h3>");
    if handoff.known_limits.is_empty() {
        content.push_str("<p>None recorded.</p>");
    } else {
        content.push_str("<ul>");
        for limit in &handoff.known_limits {
            content.push_str(&format!("<li>{}</li>", escape(limit)));
        }
        content.push_str("</ul>");
    }
    content.push_str("<h3>Retained artifacts</h3>");
    if handoff.retained_artifacts.is_empty() {
        content.push_str("<p>None recorded.</p>");
    } else {
        content.push_str("<ul>");
        for a in &handoff.retained_artifacts {
            content.push_str(&format!(
                "<li>{} · controller {} · retained until {} · {}</li>",
                escape(&a.id),
                escape(&a.controller),
                when(a.retain_until),
                escape(&a.purpose),
            ));
        }
        content.push_str("</ul>");
    }
    let support = &handoff.support;
    content.push_str(&format!(
        "<h3>Support</h3><dl><dt>Responsible human</dt><dd>{}</dd><dt>Business hours</dt><dd>{}</dd><dt>Response boundary</dt><dd>{}</dd><dt>Included work</dt><dd>{}</dd><dt>Out of scope</dt><dd>{}</dd><dt>Ends at</dt><dd>{}</dd></dl><p>Support acceptance is the separate support-owner record listed with the pilot.</p>",
        escape(&support.responsible_human),
        escape(&support.business_hours),
        escape(&support.response_boundary),
        escape(&support.included_work),
        escape(&support.out_of_scope_route),
        when(support.ends_at),
    ));
    content.push_str("<h3>Offboarding</h3>");
    if handoff.cleanup_plan.is_empty() {
        content.push_str(
            "<p>No cleanup plan is recorded in this handoff. Offboarding is unverified.</p>",
        );
    } else {
        content.push_str("<p>Planned items. None is shown as done: no verified cleanup report is recorded with the sales owner, so each item remains open.</p><table><thead><tr><th>Item</th><th>Class</th><th>Responsible human</th><th>Due</th><th>Verification</th></tr></thead><tbody>");
        for item in &handoff.cleanup_plan {
            content.push_str(&format!(
                "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>Unverified</td></tr>",
                escape(&item.id),
                escape(&item.class.replace('_', " ")),
                escape(&item.responsible_human),
                when(item.due_at),
            ));
        }
        content.push_str("</tbody></table>");
    }
    shell(&context, &headers, &content)
}

// ---- Invoices and fulfillment ----------------------------------------------

async fn invoices(State(app): State<App>, headers: HeaderMap, Path(id): Path<String>) -> Response {
    let (context, _) = match open(&app, &headers, &id).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    let mut content = heading(
        &id,
        "invoices",
        "Invoices and fulfillment",
        "Invoices, verified payments, and separately priced fulfillment as the sales owner retains them. An invoice is not a payment, a payment funds no product usage, and only the owner's verification against receiver or processor evidence records one. Unknown, disputed, and reversed outcomes stay listed. This page records nothing.",
    );
    match context.sales.records(&context.viewer, &id).await {
        Ok(records) => {
            let mut shown = 0;
            for record in &records {
                for sale in &record.services {
                    shown += 1;
                    content.push_str(&invoice(&id, record, sale));
                }
            }
            if shown == 0 {
                content.push_str("<p>No invoice is visible to this principal.</p>");
            }
        }
        Err(error) => match module_error(error) {
            Ok(text) => content.push_str(&text),
            Err(response) => return response,
        },
    }
    shell(&context, &headers, &content)
}

fn invoice(id: &str, record: &RecordView, sale: &Sale) -> String {
    let i = &sale.admission.invoice;
    let mut out = format!(
        "<section class=\"cloud-card sales-invoice\">{}<dl><dt>Invoice</dt><dd>{} · external {}</dd><dt>Amount</dt><dd>{}</dd><dt>Issued</dt><dd>{}</dd><dt>Due</dt><dd>{}</dd><dt>Payment route</dt><dd>{}</dd><dt>Invoice evidence</dt><dd>{}</dd><dt>Invoice retention</dt><dd>{}</dd></dl>",
        record_heading(id, record),
        escape(&i.id),
        escape(&i.external_reference),
        amount(&i.currency, i.currency_scale, i.amount_minor),
        i.issued_at,
        i.due_at,
        escape(&i.payment_route_reference),
        reference(&i.evidence),
        escape(&sale.facts.invoice_retention_reference),
    );
    match sale.summary() {
        Ok(summary) => out.push_str(&format!(
            "<p>Verified collection: paid {} · refunded {} · refund reversals {} · {}</p>",
            amount(&i.currency, i.currency_scale, summary.paid_minor),
            amount(&i.currency, i.currency_scale, summary.refunded_minor),
            amount(&i.currency, i.currency_scale, summary.refund_reversals_minor),
            if summary.unresolved {
                "Unresolved: the latest outcome is pending, unknown, or disputed"
            } else {
                "Resolved"
            },
        )),
        Err(_) => out.push_str("<p>Verified collection: the payment history does not reconcile. Treat it as unknown.</p>"),
    }
    out.push_str("<h4>Payment verifications</h4>");
    if sale.payments.is_empty() {
        out.push_str("<p>No payment is verified. The invoice remains unpaid until the owner records receiver or processor evidence.</p>");
    } else {
        out.push_str("<table><thead><tr><th>Outcome</th><th>Paid</th><th>Reversed</th><th>External reference</th><th>Evidence</th><th>Verified by</th><th>At</th></tr></thead><tbody>");
        for v in &sale.payments {
            let outcome = match v.input.disposition {
                Disposition::Pending => "Pending",
                Disposition::Unknown => "Unknown",
                Disposition::Paid => "Paid",
                Disposition::Reversed => "Reversed",
                Disposition::Disputed => "Disputed",
            };
            out.push_str(&format!(
                "<tr><td>{outcome}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
                v.input.paid_minor.map_or_else(|| "None".into(), |m| amount(&i.currency, i.currency_scale, m)),
                v.input.reversed_minor.map_or_else(|| "None".into(), |m| amount(&i.currency, i.currency_scale, m)),
                v.input.external_reference.as_deref().map_or_else(|| "None".into(), escape),
                reference(&v.input.evidence),
                escape(&v.verified_by),
                v.verified_at,
            ));
        }
        out.push_str("</tbody></table>");
    }
    out.push_str("<h4>Fulfillment</h4>");
    match sale.effective_fulfillment() {
        Ok(None) => out.push_str("<p>No separately priced fulfillment obligation is admitted.</p>"),
        Ok(Some(f)) => {
            out.push_str(&format!(
                "<dl><dt>Obligation</dt><dd>{}</dd><dt>Responsible human</dt><dd>{}</dd><dt>Amount</dt><dd>{}</dd><dt>Trigger</dt><dd>{}</dd><dt>Agreement</dt><dd>{}</dd><dt>Acceptance</dt><dd>{}</dd><dt>Bill</dt><dd>{}</dd><dt>Payment</dt><dd>{}</dd></dl>",
                escape(&f.id),
                escape(&f.responsible_human),
                amount(&f.currency, f.currency_scale, f.amount_minor),
                match f.trigger {
                    FulfillmentTrigger::AcceptedDelivery => "Accepted delivery",
                    FulfillmentTrigger::VerifiedServicePayment => "Verified service payment",
                },
                reference(&f.agreement),
                reference(&f.acceptance),
                f.bill.as_ref().map_or_else(|| "Not billed".into(), reference),
                f.payment.as_ref().map_or_else(|| "Not paid".into(), reference),
            ));
            for r in &sale.fulfillment_reconciliations {
                out.push_str(&format!(
                    "<p>Reconciled by {} at {}: bill {}{}</p>",
                    escape(&r.verified_by),
                    r.verified_at,
                    reference(&r.input.bill),
                    r.input
                        .payment
                        .as_ref()
                        .map_or_else(String::new, |p| format!(" · payment {}", reference(p))),
                ));
            }
        }
        Err(_) => {
            out.push_str("<p>The fulfillment history does not reconcile. Treat it as unknown.</p>")
        }
    }
    out.push_str("</section>");
    out
}

// ---- Journeys and weekly review --------------------------------------------

async fn journeys(State(app): State<App>, headers: HeaderMap, Path(id): Path<String>) -> Response {
    let (context, standing) = match open(&app, &headers, &id).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    let mut content = heading(
        &id,
        "journeys",
        "Journeys and weekly review",
        "Consented journey measurement, separate from the assisted pipeline. Every recorded event and failure stays listed, resolved or not. Measurement grants no outreach or tracking authority.",
    );
    match context.sales.records(&context.viewer, &id).await {
        Ok(records) => {
            let mut shown = 0;
            for record in &records {
                for j in &record.journeys {
                    shown += 1;
                    content.push_str(&journey(&id, record, j));
                }
            }
            if shown == 0 {
                content.push_str("<p>No consented journey is visible to this principal.</p>");
            }
        }
        Err(error) => match module_error(error) {
            Ok(text) => content.push_str(&text),
            Err(response) => return response,
        },
    }
    content.push_str("<h3>Weekly review</h3>");
    if standing.role != Role::Owner {
        content
            .push_str("<p>The weekly review is owner-only and is not shown to this principal.</p>");
    } else {
        match context.sales.weekly(&context.viewer, &id).await {
            Ok(None) => content
                .push_str("<p>No weekly review sources are configured with the sales owner.</p>"),
            Ok(Some(weekly)) => content.push_str(&review(&weekly)),
            Err(error) => match module_error(error) {
                Ok(text) => content.push_str(&text),
                Err(response) => return response,
            },
        }
    }
    shell(&context, &headers, &content)
}

fn journey(id: &str, record: &RecordView, j: &Journey) -> String {
    let a = &j.admission;
    let mut out = format!(
        "<section class=\"cloud-card sales-journey\">{}<dl><dt>Journey</dt><dd>{}</dd><dt>Offer and cohort</dt><dd>{} · {}</dd><dt>Lane</dt><dd>{} · {}</dd><dt>Consent</dt><dd>at {} · expires {} · aggregate counts {} · {}</dd><dt>Retained until</dt><dd>{}</dd></dl>",
        record_heading(id, record),
        escape(&a.id),
        escape(&a.offer_version),
        escape(&a.cohort),
        label(&a.lane),
        label(&a.classification),
        a.consent.at,
        a.consent.expires_at,
        if a.consent.aggregate_counts {
            "permitted"
        } else {
            "not permitted"
        },
        reference(&a.consent.evidence),
        j.retain_until,
    );
    if j.events.is_empty() {
        out.push_str("<p>No event is recorded.</p>");
    } else {
        out.push_str("<h4>Events</h4><ul>");
        for e in &j.events {
            out.push_str(&format!(
                "<li>{} · {} at {} · recorded by {}</li>",
                escape(&e.input.id),
                label(&e.input.kind),
                e.input.at,
                escape(&e.recorded_by),
            ));
        }
        out.push_str("</ul>");
    }
    if !j.failures.is_empty() {
        out.push_str("<h4>Failures and repairs</h4><ul>");
        for f in &j.failures {
            out.push_str(&format!(
                "<li>{} · {} · {} · responsible {} · next {} due {} · {}</li>",
                escape(&f.input.event),
                label(&f.input.reason),
                if f.input.resolved { "Resolved" } else { "Open" },
                escape(&f.input.responsible_human),
                escape(&f.input.next_action),
                f.input.due_at,
                reference(&f.input.evidence),
            ));
        }
        out.push_str("</ul>");
    }
    out.push_str("</section>");
    out
}

fn counts(value: &Value) -> String {
    value
        .as_object()
        .map(|map| {
            map.iter()
                .map(|(k, v)| format!("{} {}", escape(&k.replace('_', " ")), v))
                .collect::<Vec<_>>()
                .join(" · ")
        })
        .unwrap_or_default()
}

fn review(weekly: &Weekly) -> String {
    let mut out = format!(
        "<dl><dt>Period</dt><dd>{} to {}</dd><dt>Generated</dt><dd>{}</dd><dt>Manifest</dt><dd>{}</dd><dt>Finance</dt><dd>{}</dd><dt>Commercial activation attested</dt><dd>{}</dd><dt>Contribution scope</dt><dd>{}</dd></dl>",
        weekly.period_start,
        weekly.period_end,
        weekly.generated_at,
        short(&weekly.manifest_digest),
        if weekly.finance_included {
            "Included"
        } else {
            "Not included"
        },
        if weekly.commercial_activation_attested {
            "Yes"
        } else {
            "No"
        },
        escape(&weekly.contribution_scope),
    );
    for (title, items) in [("Gaps", &weekly.gaps), ("Limitations", &weekly.limitations)] {
        if !items.is_empty() {
            out.push_str(&format!("<h4>{title}</h4><ul>"));
            for item in items {
                out.push_str(&format!("<li>{}</li>", escape(item)));
            }
            out.push_str("</ul>");
        }
    }
    if weekly.cohorts.is_empty() {
        out.push_str("<p>No cohort is in this period.</p>");
    } else {
        out.push_str("<h4>Cohorts</h4><ul>");
        for c in &weekly.cohorts {
            out.push_str(&format!(
                "<li>{} · {} · {} · {}: period {} ; cumulative {}</li>",
                escape(c["lane"].as_str().unwrap_or("unknown")),
                escape(c["classification"].as_str().unwrap_or("unknown")),
                escape(c["offer_version"].as_str().unwrap_or("")),
                escape(c["cohort"].as_str().unwrap_or("")),
                counts(&c["period"]),
                counts(&c["cumulative"]),
            ));
        }
        out.push_str("</ul>");
    }
    out.push_str(&format!(
        "<p>{} journeys in this review, failed and unknown history included.</p>",
        weekly.journeys.len()
    ));
    out
}

// ---- Evidence and claims ---------------------------------------------------

async fn evidence(State(app): State<App>, headers: HeaderMap, Path(id): Path<String>) -> Response {
    let (context, standing) = match open(&app, &headers, &id).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    let mut content = heading(
        &id,
        "evidence",
        "Evidence and claims",
        "Reviewed claims and price references with their review pins, expiry, withdrawal, and history. A verdict is the one recorded at review; a later read against current sources can still invalidate it. No claim of savings stands on an unverified demonstration. This page publishes and approves nothing.",
    );
    if standing.role != Role::Owner {
        content.push_str(
            "<p>The claim register and its history are owner-only and are not shown to this principal.</p>",
        );
        return shell(&context, &headers, &content);
    }
    match context.sales.claims(&context.viewer, &id).await {
        Ok(claims) => content.push_str(&register(&claims)),
        Err(error) => match module_error(error) {
            Ok(text) => content.push_str(&text),
            Err(response) => return response,
        },
    }
    shell(&context, &headers, &content)
}

fn register(claims: &Claims) -> String {
    let mut out = String::new();
    if claims.register.is_empty() {
        out.push_str("<p>No claim is reviewed.</p>");
    } else {
        out.push_str("<table class=\"sales-claims\"><thead><tr><th>Claim</th><th>Purpose</th><th>Source</th><th>Verdict</th><th>Price reference</th><th>Reviewed</th><th>Expires</th><th>State</th></tr></thead><tbody>");
        for c in &claims.register {
            let (verdict, price) = match &c.verdict {
                Verdict::Allowed { claim } => (
                    format!("Allowed: {}", escape(&claim.wording)),
                    claim.price.as_ref(),
                ),
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
                    "Service {} · {}",
                    escape(&terms.offer_version),
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
            out.push_str(&format!(
                "<tr><td>{} r{}</td><td>{}</td><td>{} r{}</td><td>{verdict}</td><td>{price}</td><td>{} at {} · review sha256 {}</td><td>{}</td><td>{}</td></tr>",
                escape(&c.pin.id),
                c.pin.revision,
                match c.purpose {
                    Purpose::Capability => "Capability",
                    Purpose::Price => "Price",
                    Purpose::Comparison => "Comparison",
                    Purpose::Launch => "Launch",
                },
                escape(&c.source.id),
                c.source.revision,
                escape(&c.reviewer),
                c.reviewed_at,
                short(&c.review_sha256),
                c.expires_at,
                state.join(" · "),
            ));
        }
        out.push_str("</tbody></table>");
    }
    out.push_str("<h3>Review history</h3>");
    if claims.history.is_empty() {
        out.push_str("<p>No review decision is recorded.</p>");
    } else {
        out.push_str("<ol class=\"sales-claim-history\">");
        for d in &claims.history {
            out.push_str(&format!(
                "<li>#{} at {} · {} · {} {}{}</li>",
                d.sequence,
                d.at,
                escape(&d.actor),
                escape(&d.operation.replace('_', " ")),
                escape(&d.subject),
                d.reason
                    .as_ref()
                    .map_or_else(String::new, |r| format!(" · {}", label(r))),
            ));
        }
        out.push_str("</ol>");
    }
    out
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
    let mut content = heading(
        &id,
        "",
        &format!("Audit for record {}", short(&lead)),
        "Owner audit entries for this one record only. Entries for other customers, teams, and contacts are excluded by the sales owner before they leave it.",
    );
    if standing.role != Role::Owner {
        content.push_str("<p>The audit is owner-only and is not shown to this principal.</p>");
        return shell(&context, &headers, &content);
    }
    match context.sales.audit(&context.viewer, &id, &lead).await {
        Ok(entries) if entries.is_empty() => content.push_str("<p>No audit entry is recorded.</p>"),
        Ok(entries) => {
            content.push_str("<table class=\"sales-audit\"><thead><tr><th>Sequence</th><th>At</th><th>Actor</th><th>Operation</th><th>Reference</th></tr></thead><tbody>");
            for e in entries {
                content.push_str(&format!(
                    "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
                    e.sequence,
                    e.at,
                    escape(&e.actor),
                    escape(&e.operation.replace('_', " ")),
                    short(&e.reference_digest),
                ));
            }
            content.push_str("</tbody></table>");
        }
        Err(error) => return answer(error),
    }
    shell(&context, &headers, &content)
}
