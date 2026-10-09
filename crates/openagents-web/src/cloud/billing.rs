//! Original billing statements and admitted commercial lanes (WEB-11).
//!
//! Every figure here is an original native record read under the viewer's
//! own current session and selected workspace: the joined
//! `openagents.joined-statement.v1` document from the gateway's shared
//! commercial owner (funding, holds, charges, refunds, reversals, author
//! fees, allocations, and payout references), the retail service's account
//! with this site's journaled invoices and each purchase's retained
//! first-observed record, and the gateway's own purchase context and
//! receipts for a decision resource. Nothing is re-derived: units stay
//! distinct (millisatoshis, satoshis, and currency millionths are never
//! added together), a missing cost or settlement is shown as unknown, and an
//! unused hold release is never shown as a refund.
//!
//! These pages grant nothing. Signing in, a mapping, or a statement row is
//! not a right to spend, invoke, or publish; purchases stay with their native
//! owners and their own quote, funding, and permission controls.

use super::retail::{self, Failure};
use super::session::{SessionError, Viewer};
use super::{failure, protect, refused, service, workspace_shell};
use crate::App;
use crate::layout::escape;
use axum::Router;
use axum::body::Body;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::get;
use receipts::funding_units::{Conversion, Unit};
use receipts::purchase::{CommercialProduct, CommercialRef, CommercialSource};
use serde::Deserialize;
use serde_json::Value;

const PAGE: &str = "/cloud/app/billing";
const STATEMENT_PAGE: usize = 50;

pub(super) fn routes() -> Router<App> {
    Router::new()
        .route(PAGE, get(index))
        .route("/cloud/app/billing/statements", get(statement))
        .route("/cloud/app/billing/statements/export", get(export))
        .route("/cloud/app/billing/decisions", get(decision))
        .route("/cloud/app/billing/decisions/{door}/receipt", get(receipt))
}

/// Billing is reachable whenever a workspace is selected; each lane says
/// whether its own native owner admits it.
pub(crate) fn available(viewer: &Viewer) -> bool {
    viewer.workspace.is_some()
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
            "Billing reads bind one selected workspace and its current membership.",
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

    fn shell(&self, headers: &HeaderMap, content: &str) -> Response {
        workspace_shell(
            self.app,
            headers,
            self.service,
            &self.viewer,
            "billing",
            Some(content),
            None,
        )
    }
}

/// A native refusal names no private response; a revoked session refuses.
enum Native {
    Session(SessionError),
    Unavailable,
}

fn native(error: &jev::Error) -> Native {
    match error {
        jev::Error::Api(api) if api.status == 401 => Native::Session(SessionError::Unauthenticated),
        _ => Native::Unavailable,
    }
}

// ---- Index -------------------------------------------------------------------

async fn index(State(app): State<App>, headers: HeaderMap) -> Response {
    let context = match context(&app, &headers).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    let retail = retail::available(context.app, &context.viewer);
    let content = format!(
        "<h2>Billing</h2><p>Each lane is read from its own native owner under your current session and the selected workspace <code>{}</code>. A page here grants no spending, invocation, or publication right; different units are never added together.</p><ul class=\"billing-lanes\"><li><a href=\"{PAGE}/statements\">Original statement</a>: funding, holds, charges, refunds, author fees, settlement, and payout references from the shared commercial owner.</li><li>{}</li></ul><section class=\"cloud-card\"><h3>Review a decision resource</h3><p>Read the current payer, price reference, and permission for one exact decision resource, and recover an earlier purchase by its original receipt.</p><form method=\"get\" action=\"{PAGE}/decisions\"><label>Resource <input name=\"door\" required maxlength=\"128\"></label> <button type=\"submit\">Review</button></form></section>",
        escape(&context.workspace),
        if retail {
            format!(
                "<a href=\"{PAGE}/retail\">Retail compute</a>: funding, quotes, purchases, and receipts through the delegated retail principal."
            )
        } else {
            "Retail compute · Unavailable: no retail delegation is provisioned for this account, workspace, and membership.".into()
        },
    );
    context.shell(&headers, &content)
}

// ---- Statement ----------------------------------------------------------------

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Page {
    #[serde(default)]
    cursor: Option<String>,
    #[serde(default)]
    after_earning: Option<i64>,
    #[serde(default)]
    after_payout: Option<i64>,
}

impl Page {
    fn query(&self) -> Result<jev::JoinedStatementQuery, SessionError> {
        if self.cursor.as_ref().is_some_and(|c| {
            c.is_empty() || c.len() > 4096 || !c.bytes().all(|b| b.is_ascii_hexdigit())
        }) || self.after_earning.is_some_and(|n| n < 0)
            || self.after_payout.is_some_and(|n| n < 0)
        {
            return Err(SessionError::InvalidRequest);
        }
        Ok(jev::JoinedStatementQuery {
            cursor: self.cursor.clone(),
            limit: Some(STATEMENT_PAGE),
            after_earning: self.after_earning,
            after_payout: self.after_payout,
        })
    }

    fn href(&self, path: &str) -> String {
        let mut pairs = url::form_urlencoded::Serializer::new(String::new());
        if let Some(c) = &self.cursor {
            pairs.append_pair("cursor", c);
        }
        if let Some(n) = self.after_earning {
            pairs.append_pair("after_earning", &n.to_string());
        }
        if let Some(n) = self.after_payout {
            pairs.append_pair("after_payout", &n.to_string());
        }
        let query = pairs.finish();
        if query.is_empty() {
            path.into()
        } else {
            format!("{path}?{query}")
        }
    }
}

/// One original statement row, typed in full: a row this page cannot read
/// exactly is refused rather than partly shown.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Row {
    key: String,
    kind: String,
    state: String,
    source: CommercialSource,
    commercial: CommercialRef,
    binding: String,
    unit: Unit,
    unit_scale: u64,
    conversion: Option<Conversion>,
    native_attempt: Option<String>,
    intent_digest: Option<String>,
    quote: Option<String>,
    execution: Option<String>,
    terms: Option<String>,
    payment_reference: Option<String>,
    units: Option<u64>,
    reserved_msat: i64,
    charged_msat: Option<i64>,
    credited_msat: Option<i64>,
    released_msat: i64,
    returned_msat: i64,
    loss_msat: i64,
    recovered_msat: i64,
    reduced_claim_msat: i64,
    fee_msat: Option<u64>,
    remainder: Option<u64>,
    denominator: Option<u64>,
    source_evidence: Option<String>,
    settlement_reference: Option<String>,
    allocation_rule_version: Option<i64>,
    allocations: Vec<Value>,
    disclosure: Vec<String>,
}

fn unit_name(unit: &Unit) -> String {
    match unit {
        Unit::Satoshis => "sat (BTC)".into(),
        Unit::Millisatoshis => "msat (BTC)".into(),
        Unit::CurrencyMillionths { currency } => format!("millionths of {currency}"),
    }
}

fn product(source: &CommercialSource) -> &'static str {
    match source.product {
        CommercialProduct::Gateway => "Decision or hosted resource (gateway)",
        CommercialProduct::Plugin => "Plugin release",
        CommercialProduct::Retail => "Retail compute",
    }
}

fn dd(out: &mut String, label: &str, value: &str) {
    out.push_str(&format!(
        "<dt>{}</dt><dd>{}</dd>",
        escape(label),
        escape(value)
    ));
}

fn msat(value: i64) -> String {
    format!("{value} msat")
}

fn known(value: Option<&str>) -> &str {
    value.unwrap_or("unknown")
}

fn render_row(row: &Row, projection: Option<&Value>) -> String {
    let mut out = format!(
        "<li class=\"statement-row\" id=\"row-{}\"><h4>{} · {} · {}</h4><dl>",
        escape(&row.key),
        escape(product(&row.source)),
        escape(&row.kind),
        escape(&row.state)
    );
    dd(&mut out, "Record", &row.key);
    dd(
        &mut out,
        "Native source",
        &format!(
            "{} / account {} / workspace {}",
            row.source.issuer,
            row.source.account,
            known(row.source.workspace.as_deref())
        ),
    );
    dd(
        &mut out,
        "Canonical mapping",
        &format!(
            "binding {} revision {} · customer {} · attribution only, no right",
            row.commercial.binding, row.commercial.revision, row.commercial.customer
        ),
    );
    dd(&mut out, "Source binding", &row.binding);
    dd(
        &mut out,
        "Source unit",
        &format!(
            "{} ({} per whole unit)",
            unit_name(&row.unit),
            row.unit_scale
        ),
    );
    dd(
        &mut out,
        "Source units",
        &row.units.map_or("unknown".into(), |n| {
            format!("{n} {}", unit_name(&row.unit))
        }),
    );
    if let Some(c) = &row.conversion {
        dd(
            &mut out,
            "Conversion",
            &format!(
                "{} · {} {} to {} {} · {:?} rounding · fee paid by {:?} · source {}",
                c.version,
                c.numerator,
                unit_name(&c.source),
                c.denominator,
                unit_name(&c.target),
                c.rounding,
                c.fee_payer,
                c.source_ref
            ),
        );
    }
    for (label, value) in [
        ("Quote", &row.quote),
        ("Execution", &row.execution),
        ("Terms", &row.terms),
        ("Native attempt", &row.native_attempt),
        ("Intent", &row.intent_digest),
        ("Payment", &row.payment_reference),
        ("Source evidence", &row.source_evidence),
        ("Settlement", &row.settlement_reference),
    ] {
        if let Some(value) = value {
            dd(&mut out, label, value);
        }
    }
    dd(&mut out, "Reserved", &msat(row.reserved_msat));
    dd(
        &mut out,
        "Charged",
        &row.charged_msat.map_or("unknown".into(), msat),
    );
    if let Some(credited) = row.credited_msat {
        dd(&mut out, "Credited", &msat(credited));
    }
    dd(
        &mut out,
        "Unused hold released",
        &format!("{} (not a refund)", msat(row.released_msat)),
    );
    for (label, value) in [
        ("Returned", row.returned_msat),
        ("Loss", row.loss_msat),
        ("Recovered", row.recovered_msat),
        ("Reduced claim", row.reduced_claim_msat),
    ] {
        if value != 0 {
            dd(&mut out, label, &msat(value));
        }
    }
    if let Some(fee) = row.fee_msat {
        dd(&mut out, "Conversion fee", &format!("{fee} msat"));
    }
    if let (Some(r), Some(d)) = (row.remainder, row.denominator) {
        dd(
            &mut out,
            "Remainder",
            &format!("{r}/{d} of one msat, never spendable credit"),
        );
    }
    if let Some(version) = row.allocation_rule_version {
        dd(&mut out, "Allocation rule", &format!("version {version}"));
    }
    out.push_str("</dl>");
    if !row.allocations.is_empty() {
        out.push_str("<ul class=\"statement-allocations\">");
        for allocation in &row.allocations {
            out.push_str(&format!(
                "<li>{}</li>",
                escape(&allocation_line(allocation))
            ));
        }
        out.push_str("</ul>");
    }
    if let Some(projection) = projection {
        out.push_str(&format!(
            "<p>Original gateway projection: {}</p>",
            escape(&projection_line(projection))
        ));
    }
    if !row.disclosure.is_empty() {
        out.push_str(&format!(
            "<p class=\"dim\">Disclosure: {}</p>",
            escape(&row.disclosure.join("; "))
        ));
    }
    out.push_str("</li>");
    out
}

fn text(value: &Value) -> String {
    match value {
        Value::Null => "unknown".into(),
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

fn allocation_line(value: &Value) -> String {
    let role = text(&value["role"]);
    let mut line = if role == "author" && value.get("release").is_some() {
        format!(
            "Author fee {} msat · plugin {} release {} · {}",
            text(&value["amount_msat"]),
            text(&value["plugin"]),
            text(&value["release"]),
            text(&value["state"])
        )
    } else {
        format!("Share {} msat · role {}", text(&value["amount_msat"]), role)
    };
    line.push_str(&format!(" · party {}", text(&value["party_reference"])));
    let payouts = value["payouts"].as_array().cloned().unwrap_or_default();
    if payouts.is_empty() {
        line.push_str(" · no payout recorded");
    }
    for payout in payouts {
        line.push_str(&format!(
            " · payout {} {}",
            text(&payout["reference"]),
            text(&payout["state"])
        ));
    }
    line
}

fn projection_line(value: &Value) -> String {
    let cost = &value["cost"];
    format!(
        "phase {} · reserved {} · retail charge {} at price version {} · receipt {} · read at journal head {}",
        text(&cost["phase"]),
        text(&cost["reserved"]),
        text(&cost["retail"]),
        text(&cost["price_version"]),
        text(&value["receipt"]),
        text(&value["source_head"])
    )
}

fn balance_lines(balance: &Value) -> Result<String, ()> {
    let object = balance.as_object().ok_or(())?;
    let mut out = String::from("<dl class=\"statement-balance\">");
    for (key, value) in object {
        let amount = value.as_i64().ok_or(())?;
        let label = key.strip_suffix("_msat").ok_or(())?.replace('_', " ");
        dd(
            &mut out,
            &label,
            &if label == "released" {
                format!("{} (already available; not a refund)", msat(amount))
            } else {
                msat(amount)
            },
        );
    }
    out.push_str("</dl>");
    Ok(out)
}

async fn statement(
    State(app): State<App>,
    headers: HeaderMap,
    Query(page): Query<Page>,
) -> Response {
    let context = match context(&app, &headers).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    let query = match page.query() {
        Ok(value) => value,
        Err(error) => return refused(error),
    };
    let read = context
        .viewer
        .client()
        .account()
        .joined_statement(&context.workspace, &query, false)
        .await;
    let mut content = format!(
        "<h2>Original statement</h2><p>Workspace <code>{}</code>. Read from the shared commercial owner with your current session; rows are the original native records, in their own units. <a href=\"{PAGE}\">Billing</a></p>",
        escape(&context.workspace)
    );
    match read {
        Ok(view) => match render_statement(&view, &page) {
            Ok(value) => content.push_str(&value),
            Err(()) => {
                return failure(
                    StatusCode::CONFLICT,
                    "Statement record unreadable",
                    "The native statement contained a record this page cannot show exactly. Nothing is shown in its place; export the original statement or retry.",
                );
            }
        },
        Err(error) => match native(&error) {
            Native::Session(error) => return refused(error),
            Native::Unavailable => content.push_str(
                "<section class=\"cloud-card\"><h3>Joined statement · Unavailable</h3><p>Original joined statements are not enabled for this workspace, your current statement review is absent or expired, or the source records changed. No figure is estimated in their place.</p></section>",
            ),
        },
    }
    content.push_str(&retail_statements(&context).await);
    if let Err(response) = context.still_current(&headers).await {
        return response;
    }
    context.shell(&headers, &content)
}

fn render_statement(view: &jev::JoinedStatementView, page: &Page) -> Result<String, ()> {
    let statement = &view.statement;
    if statement.unit != Unit::Millisatoshis || view.native_workspace.is_empty() {
        return Err(());
    }
    let rows: Vec<Row> = statement
        .rows
        .iter()
        .map(|row| serde_json::from_value(row.clone()).map_err(|_| ()))
        .collect::<Result<_, _>>()?;
    let mut out = format!("<section class=\"cloud-card\"><h3>Joined statement</h3><dl>",);
    dd(&mut out, "Origin", &statement.origin);
    dd(
        &mut out,
        "Customer",
        &format!("{} / workspace {}", statement.customer, statement.workspace),
    );
    dd(&mut out, "Snapshot", &statement.snapshot);
    dd(&mut out, "Records scanned", &statement.scanned.to_string());
    out.push_str("</dl>");
    match &statement.balance {
        Some(balance) => {
            out.push_str("<h4>Pool balance (msat)</h4>");
            out.push_str(&balance_lines(balance)?);
        }
        None => out.push_str(
            "<p>Pool balance: not shown. Member reads omit pool totals; only your exactly attributed original records appear.</p>",
        ),
    }
    out.push_str("</section>");
    for (label, wanted) in [
        ("Decision and hosted resources", CommercialProduct::Gateway),
        ("Plugin releases", CommercialProduct::Plugin),
        ("Retail compute", CommercialProduct::Retail),
    ] {
        let group: Vec<&Row> = rows.iter().filter(|r| r.source.product == wanted).collect();
        if group.is_empty() {
            continue;
        }
        out.push_str(&format!(
            "<section class=\"cloud-card\"><h3>{}</h3><ol class=\"statement-rows\">",
            escape(label)
        ));
        for row in group {
            let projection = view
                .native_projection
                .iter()
                .find(|p| p["key"].as_str() == Some(row.key.as_str()));
            out.push_str(&render_row(row, projection));
        }
        out.push_str("</ol></section>");
    }
    if rows.is_empty() {
        out.push_str("<p>No original records on this page.</p>");
    }
    let next = statement.next.as_ref().map(|cursor| Page {
        cursor: Some(cursor.clone()),
        after_earning: page.after_earning,
        after_payout: page.after_payout,
    });
    out.push_str("<p>");
    if let Some(next) = &next {
        out.push_str(&format!(
            "<a href=\"{}\">Next page</a> · ",
            escape(&next.href(&format!("{PAGE}/statements")))
        ));
    }
    out.push_str(&format!(
        "<a href=\"{}\">Export this page (NDJSON)</a></p>",
        escape(&page.href(&format!("{PAGE}/statements/export")))
    ));
    out.push_str(&format!(
        "<section class=\"cloud-card\"><h3>Source attribution</h3><ul>{}</ul><p class=\"dim\">{}</p></section>",
        view.source_attribution
            .iter()
            .map(|a| format!(
                "<li>{} · {}</li>",
                escape(&text(&a["binding"])),
                if a["current_original_mapping"] == Value::Bool(true) {
                    "current original mapping"
                } else {
                    "historical original source"
                }
            ))
            .collect::<String>(),
        escape(&view.attribution_disclosure)
    ));
    out.push_str("<section class=\"cloud-card\"><h3>Payee earnings</h3>");
    match &view.payee {
        Some(payee) => out.push_str(&format!(
            "<p>Party {} · unit msat. Earnings are separate from customer spending and are never netted against it.</p><pre class=\"statement-payee\">{}</pre>",
            escape(&text(&payee["party"])),
            escape(&serde_json::to_string_pretty(&payee["statement"]).unwrap_or_default())
        )),
        None => out.push_str("<p>No payee read is reviewed for this account.</p>"),
    }
    out.push_str(&format!(
        "<p class=\"dim\">{}</p></section>",
        escape(&view.payee_disclosure)
    ));
    out.push_str(&format!(
        "<p class=\"dim\">{} {}</p>",
        escape(&view.native_projection_disclosure),
        escape(&statement.disclosure.join("; "))
    ));
    Ok(out)
}

/// Retail compute keeps its own owner, unit, and records.
async fn retail_statements(context: &Context<'_>) -> String {
    let Some(delegations) = context.app.config.cloud_retail.as_deref() else {
        return String::new();
    };
    let mut out = String::new();
    for delegation in delegations.current(&context.viewer) {
        let id = delegation.id().to_owned();
        out.push_str(&format!(
            "<section class=\"cloud-card\" id=\"retail-statement-{}\"><h3>Retail compute · delegation {}</h3>",
            escape(&id),
            escape(&id)
        ));
        match delegations.statement(&context.viewer, &id).await {
            Ok(statement) => out.push_str(&render_retail(&id, &statement)),
            Err(Failure::Changed(part)) => out.push_str(&format!(
                "<p>Retail record changed: the retail service answered with a different {} than the one retained. Nothing is shown as current.</p>",
                escape(part)
            )),
            Err(_) => out.push_str(
                "<p>Retail service: Unavailable. No balance or charge is estimated in its place.</p>",
            ),
        }
        out.push_str("</section>");
    }
    out
}

fn render_retail(id: &str, statement: &retail::Statement) -> String {
    let mut out = format!(
        "<pre class=\"retail-account\">{}</pre><h4>Funding invoices</h4>",
        escape(&statement.account.lines())
    );
    if statement.funding.is_empty() {
        out.push_str("<p>No funding requested from this page.</p>");
    } else {
        out.push_str("<ul class=\"retail-funding\">");
        for funding in &statement.funding {
            let line = match (&funding.recorded, &funding.current) {
                (None, _) => format!(
                    "request {} · {} sats requested · Outcome unknown; retry the same request from Retail",
                    &funding.request[..8],
                    funding
                        .amount_sats
                        .map_or("unknown".into(), |n| n.to_string())
                ),
                (Some(original), current) => format!(
                    "invoice {} · {} msat · {}",
                    original.payment_hash,
                    original.amount_msat,
                    match current {
                        Some(now) => format!("state {}", now.state),
                        None => format!("recorded state {}; current state unknown", original.state),
                    }
                ),
            };
            out.push_str(&format!("<li>{}</li>", escape(&line)));
        }
        out.push_str("</ul>");
    }
    out.push_str("<h4>Purchases</h4>");
    if statement.purchases.is_empty() {
        out.push_str("<p>No funded purchases.</p>");
    } else {
        out.push_str("<ul class=\"retail-statement-purchases\">");
    }
    for (execution, binding) in &statement.purchases {
        let quote = binding.get("quote");
        let mut line = format!(
            "quote maximum {} sats",
            quote.map_or("unknown".into(), |q| text(&q["max_sats"]))
        );
        match binding.get("settlement") {
            Some(s) => line.push_str(&format!(
                " · charged {} msat · unused hold released {} msat (not a refund) · ending {} · usage {}",
                text(&s["charge_msat"]),
                text(&s["released_msat"]),
                text(&s["ending"]),
                text(&s["usage"])
            )),
            None if binding.is_empty() => line.push_str(
                " · not yet retained here; open the purchase to retain its original record",
            ),
            None => line.push_str(" · settlement unknown; the quote maximum may remain held"),
        }
        out.push_str(&format!(
            "<li><a href=\"{}\">Purchase {}</a> · {}</li>",
            retail::purchase_href(id, execution),
            escape(execution),
            escape(&line)
        ));
    }
    if !statement.purchases.is_empty() {
        out.push_str("</ul>");
    }
    if statement.more {
        out.push_str(&format!(
            "<p><a href=\"{PAGE}/retail/{}/purchases\">Older purchases</a></p>",
            escape(id)
        ));
    }
    out
}

async fn export(State(app): State<App>, headers: HeaderMap, Query(page): Query<Page>) -> Response {
    let context = match context(&app, &headers).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    let query = match page.query() {
        Ok(value) => value,
        Err(error) => return refused(error),
    };
    let view = match context
        .viewer
        .client()
        .account()
        .joined_statement(&context.workspace, &query, true)
        .await
    {
        Ok(value) => value,
        Err(error) => {
            return match native(&error) {
                Native::Session(error) => refused(error),
                Native::Unavailable => failure(
                    StatusCode::SERVICE_UNAVAILABLE,
                    "Statement export unavailable",
                    "The joined statement is not enabled, not currently reviewed, or its source records changed.",
                ),
            };
        }
    };
    if let Err(response) = context.still_current(&headers).await {
        return response;
    }
    let Ok(mut body) = serde_json::to_string(&view) else {
        return refused(SessionError::Unavailable);
    };
    body.push('\n');
    let mut response = Body::from(body).into_response();
    let headers = response.headers_mut();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/x-ndjson"),
    );
    headers.insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_static("attachment; filename=\"joined-statement.ndjson\""),
    );
    protect(response)
}

// ---- Decision and hosted resources ---------------------------------------------

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Door {
    door: String,
}

fn identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && !matches!(value, "." | "..")
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
}

fn digest(value: &str) -> bool {
    value.strip_prefix("sha256:").is_some_and(|hex| {
        hex.len() == 64
            && hex
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    })
}

/// The current purchase context must name exactly this viewer, workspace,
/// and membership; anything else is a changed qualification.
fn current_context(context: &Context<'_>, door: &str, read: &receipts::purchase::Context) -> bool {
    read.account == context.viewer.account_id
        && read.workspace == context.workspace
        && read.payer_workspace == context.workspace
        && read.door == door
        && read.workspace_members_epoch == context.epoch
}

async fn decision(
    State(app): State<App>,
    headers: HeaderMap,
    Query(door): Query<Door>,
) -> Response {
    let context = match context(&app, &headers).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    let door = door.door.trim().to_owned();
    if !identifier(&door) {
        return refused(SessionError::InvalidRequest);
    }
    let account = context.viewer.client().account();
    let read = account.purchase_context(&context.workspace, &door).await;
    let mut content = format!(
        "<h2>Decision resource {}</h2><p><a href=\"{PAGE}\">Billing</a> · <a href=\"{PAGE}/statements\">Original statement</a></p>",
        escape(&door)
    );
    match read {
        Ok(read) => {
            if !current_context(&context, &door, &read) {
                return refused(SessionError::Conflict);
            }
            content.push_str(&render_context(&read));
            content.push_str(&format!(
                "<section class=\"cloud-card\"><h3>Recover a purchase</h3><p>An earlier purchase is recovered by its original receipt; recovery never invokes, pays, or retries.</p><form method=\"get\" action=\"{PAGE}/decisions/{}/receipt\"><label>Receipt digest <input name=\"digest\" required maxlength=\"71\"></label> <button type=\"submit\">Read the original receipt</button></form></section>",
                escape(&door)
            ));
        }
        Err(error) => match native(&error) {
            Native::Session(error) => return refused(error),
            Native::Unavailable => content.push_str(
                "<section class=\"cloud-card\"><h3>Purchase · Unavailable</h3><p>This resource has no current purchase context for your account and workspace: the resource, its price, your membership, or the payer lane is not admitted. Nothing is offered in its place.</p></section>",
            ),
        },
    }
    if let Err(response) = context.still_current(&headers).await {
        return response;
    }
    context.shell(&headers, &content)
}

fn render_context(read: &receipts::purchase::Context) -> String {
    let mut out = String::from("<section class=\"cloud-card\"><h3>Payer and permission</h3><dl>");
    dd(&mut out, "Native account", &read.account);
    dd(&mut out, "Payer workspace", &read.payer_workspace);
    dd(
        &mut out,
        "Membership",
        &format!(
            "{} · membership epoch {} · workspace members epoch {}",
            read.role, read.membership_epoch, read.workspace_members_epoch
        ),
    );
    dd(&mut out, "Credential", &read.credential_reference);
    dd(
        &mut out,
        "Invocation right",
        if read.can_invoke {
            "Current: this account may invoke after approving an exact quote"
        } else {
            "Not admitted: this account cannot invoke this resource"
        },
    );
    dd(
        &mut out,
        "Team policy",
        &read.team_policy.as_ref().map_or("none".into(), |p| {
            format!("version {} · {}", p.version, p.digest)
        }),
    );
    dd(
        &mut out,
        "Canonical mapping",
        &read
            .commercial
            .as_ref()
            .map_or("none; attribution is not established".into(), |c| {
                format!(
                    "binding {} revision {} · customer {} · attribution only, no right",
                    c.binding, c.revision, c.customer
                )
            }),
    );
    out.push_str("</dl></section><section class=\"cloud-card\"><h3>Exact price reference</h3><dl>");
    dd(&mut out, "Resource", &read.door);
    dd(&mut out, "Artifact", &read.artifact_digest);
    dd(&mut out, "Registry", &read.registry_digest);
    dd(
        &mut out,
        "Price",
        &format!(
            "version {} · {} · policy {}",
            read.price.version, read.price.currency, read.price.policy
        ),
    );
    dd(
        &mut out,
        "Maximum charge",
        &format!(
            "{} integer units of {} under price version {}",
            read.price.maximum_charge, read.price.currency, read.price.version
        ),
    );
    dd(&mut out, "Terms", &read.price.terms_digest);
    dd(&mut out, "Maximum usage", &read.price.maximum_usage_digest);
    dd(&mut out, "Context digest", &read.digest());
    out.push_str("</dl><p>A quote freezes this exact context and request; approval, funding, and invocation stay on the installed customer client, which rechecks these rights before reservation. A changed resource, price, payer, or membership needs a new review.</p></section>");
    out
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Wanted {
    digest: String,
}

async fn receipt(
    State(app): State<App>,
    headers: HeaderMap,
    Path(door): Path<String>,
    Query(wanted): Query<Wanted>,
) -> Response {
    let wanted = wanted.digest.trim().to_owned();
    let context = match context(&app, &headers).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    if !identifier(&door) || !digest(&wanted) {
        return refused(SessionError::InvalidRequest);
    }
    let read = context
        .viewer
        .client()
        .account()
        .purchase_receipt(&context.workspace, &wanted)
        .await;
    let proof = match read {
        Ok(value) => value,
        Err(error) => {
            return match native(&error) {
                Native::Session(error) => refused(error),
                Native::Unavailable => failure(
                    StatusCode::NOT_FOUND,
                    "Receipt unavailable",
                    "No verifiable original receipt with this digest belongs to the selected workspace.",
                ),
            };
        }
    };
    let receipt = &proof.receipt;
    if receipt.requested.model != door && receipt.served.model != door {
        return failure(
            StatusCode::CONFLICT,
            "Receipt names another resource",
            "This original receipt belongs to a different decision resource.",
        );
    }
    let mut content = format!(
        "<h2>Receipt</h2><p><a href=\"{PAGE}/decisions?door={}\">Resource {}</a></p><section class=\"cloud-card\"><h3>Original receipt</h3><dl>",
        escape(&door),
        escape(&door)
    );
    dd(&mut content, "Digest", &receipt.digest);
    dd(
        &mut content,
        "Request",
        &format!("{} · attempt {}", receipt.request, receipt.attempt),
    );
    dd(
        &mut content,
        "Outcome",
        &text(&serde_json::to_value(&receipt.outcome).unwrap_or_default()),
    );
    dd(&mut content, "Served", &receipt.served.model);
    dd(&mut content, "Request digest", &receipt.request_digest);
    dd(
        &mut content,
        "Result digest",
        known(receipt.result_digest.as_deref()),
    );
    dd(
        &mut content,
        "Canonical mapping",
        &receipt.commercial.as_ref().map_or("none".into(), |c| {
            format!("binding {} revision {}", c.binding, c.revision)
        }),
    );
    content.push_str("</dl></section><section class=\"cloud-card\"><h3>Settlement</h3><dl>");
    match &proof.cost {
        Some(cost) => {
            dd(&mut content, "Phase", &cost.phase);
            dd(
                &mut content,
                "Reserved",
                &format!("{} at price version {}", cost.reserved, cost.price_version),
            );
            dd(
                &mut content,
                "Charge",
                &cost.retail.map_or("unknown".into(), |n| {
                    format!("{n} at price version {}", cost.price_version)
                }),
            );
        }
        None => dd(
            &mut content,
            "Charge",
            "unknown; no original hold is joined",
        ),
    }
    content.push_str("</dl><p class=\"dim\">This is the original receipt and its current settlement claim. Reading it never invokes, pays, or retries a purchase.</p></section>");
    if let Err(response) = context.still_current(&headers).await {
        return response;
    }
    context.shell(&headers, &content)
}
