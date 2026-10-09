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
use super::ui;
use super::{failure, protect, refused, service, workspace_shell};
use crate::App;
use axum::Router;
use axum::body::Body;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::get;
use maud::{Markup, html};
use openagents_ui::forms::{Field, Input};
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

    fn shell(&self, headers: &HeaderMap, content: Markup) -> Response {
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
    let door = Field::new("billing-door", "Resource").required(true);
    let content = html! {
        h2 { "Billing" }
        p {
            "Each lane is read from its own native owner under your current session and the selected workspace "
            code { (context.workspace) }
            ". A page here grants no spending, invocation, or publication right; different units are never added together."
        }
        ul class="billing-lanes" {
            li {
                a href=(format!("{PAGE}/statements")) { "Original statement" }
                ": funding, holds, charges, refunds, author fees, settlement, and payout references from the shared commercial owner."
            }
            li {
                @if retail {
                    a href=(format!("{PAGE}/retail")) { "Retail compute" }
                    ": funding, quotes, purchases, and receipts through the delegated retail principal."
                } @else {
                    "Retail compute · Unavailable: no retail delegation is provisioned for this account, workspace, and membership."
                }
            }
        }
        (ui::card(html! {
            h3 { "Review a decision resource" }
            p { "Read the current payer, price reference, and permission for one exact decision resource, and recover an earlier purchase by its original receipt." }
            form class="cloud-form" method="get" action=(format!("{PAGE}/decisions")) {
                (door.clone().control(Input::new("door").aria(door.aria()).required(true).maxlength(128)))
                div class="cloud-form-actions" { (ui::submit("Review", true)) }
            }
        }))
    };
    context.shell(&headers, content)
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

fn msat(value: i64) -> String {
    format!("{value} msat")
}

fn known(value: Option<&str>) -> &str {
    value.unwrap_or("unknown")
}

fn render_row(row: &Row, projection: Option<&Value>) -> Markup {
    let mut details = ui::Details::new()
        .row("Record", row.key.as_str())
        .row(
            "Native source",
            format!(
                "{} / account {} / workspace {}",
                row.source.issuer,
                row.source.account,
                known(row.source.workspace.as_deref())
            ),
        )
        .row(
            "Canonical mapping",
            format!(
                "binding {} revision {} · customer {} · attribution only, no right",
                row.commercial.binding, row.commercial.revision, row.commercial.customer
            ),
        )
        .row("Source binding", row.binding.as_str())
        .row(
            "Source unit",
            format!(
                "{} ({} per whole unit)",
                unit_name(&row.unit),
                row.unit_scale
            ),
        )
        .row(
            "Source units",
            row.units.map_or("unknown".into(), |n| {
                format!("{n} {}", unit_name(&row.unit))
            }),
        );
    if let Some(c) = &row.conversion {
        details = details.row(
            "Conversion",
            format!(
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
            details = details.row(label, value.as_str());
        }
    }
    details = details
        .row("Reserved", msat(row.reserved_msat))
        .row("Charged", row.charged_msat.map_or("unknown".into(), msat));
    if let Some(credited) = row.credited_msat {
        details = details.row("Credited", msat(credited));
    }
    details = details.row(
        "Unused hold released",
        format!("{} (not a refund)", msat(row.released_msat)),
    );
    for (label, value) in [
        ("Returned", row.returned_msat),
        ("Loss", row.loss_msat),
        ("Recovered", row.recovered_msat),
        ("Reduced claim", row.reduced_claim_msat),
    ] {
        if value != 0 {
            details = details.row(label, msat(value));
        }
    }
    if let Some(fee) = row.fee_msat {
        details = details.row("Conversion fee", format!("{fee} msat"));
    }
    if let (Some(r), Some(d)) = (row.remainder, row.denominator) {
        details = details.row(
            "Remainder",
            format!("{r}/{d} of one msat, never spendable credit"),
        );
    }
    if let Some(version) = row.allocation_rule_version {
        details = details.row("Allocation rule", format!("version {version}"));
    }
    html! {
        li class="statement-row" id=(format!("row-{}", row.key)) {
            h4 { (product(&row.source)) " · " (row.kind) " · " (row.state) }
            (details)
            @if !row.allocations.is_empty() {
                ul class="statement-allocations" {
                    @for allocation in &row.allocations {
                        li { (allocation_line(allocation)) }
                    }
                }
            }
            @if let Some(projection) = projection {
                p { "Original gateway projection: " (projection_line(projection)) }
            }
            @if !row.disclosure.is_empty() {
                p class="dim" { "Disclosure: " (row.disclosure.join("; ")) }
            }
        }
    }
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

fn balance_lines(balance: &Value) -> Result<Markup, ()> {
    let object = balance.as_object().ok_or(())?;
    let mut rows = Vec::with_capacity(object.len());
    for (key, value) in object {
        let amount = value.as_i64().ok_or(())?;
        let label = key.strip_suffix("_msat").ok_or(())?.replace('_', " ");
        let shown = if label == "released" {
            format!("{} (already available; not a refund)", msat(amount))
        } else {
            msat(amount)
        };
        rows.push((label, shown));
    }
    Ok(html! {
        dl class="statement-balance" {
            @for (label, shown) in &rows {
                dt { (label) }
                dd { (shown) }
            }
        }
    })
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
    let body = match read {
        Ok(view) => match render_statement(&view, &page) {
            Ok(value) => value,
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
            Native::Unavailable => ui::card(html! {
                h3 { "Joined statement · Unavailable" }
                p { "Original joined statements are not enabled for this workspace, your current statement review is absent or expired, or the source records changed. No figure is estimated in their place." }
            }),
        },
    };
    let retail = retail_statements(&context).await;
    if let Err(response) = context.still_current(&headers).await {
        return response;
    }
    let content = html! {
        h2 { "Original statement" }
        p {
            "Workspace " code { (context.workspace) }
            ". Read from the shared commercial owner with your current session; rows are the original native records, in their own units. "
            a href=(PAGE) { "Billing" }
        }
        (body)
        (retail)
    };
    context.shell(&headers, content)
}

fn render_statement(view: &jev::JoinedStatementView, page: &Page) -> Result<Markup, ()> {
    let statement = &view.statement;
    if statement.unit != Unit::Millisatoshis || view.native_workspace.is_empty() {
        return Err(());
    }
    let rows: Vec<Row> = statement
        .rows
        .iter()
        .map(|row| serde_json::from_value(row.clone()).map_err(|_| ()))
        .collect::<Result<_, _>>()?;
    let balance = statement.balance.as_ref().map(balance_lines).transpose()?;
    let mut groups = Vec::new();
    for (label, wanted) in [
        ("Decision and hosted resources", CommercialProduct::Gateway),
        ("Plugin releases", CommercialProduct::Plugin),
        ("Retail compute", CommercialProduct::Retail),
    ] {
        let group: Vec<Markup> = rows
            .iter()
            .filter(|r| r.source.product == wanted)
            .map(|row| {
                let projection = view
                    .native_projection
                    .iter()
                    .find(|p| p["key"].as_str() == Some(row.key.as_str()));
                render_row(row, projection)
            })
            .collect();
        if !group.is_empty() {
            groups.push((label, group));
        }
    }
    let next = statement.next.as_ref().map(|cursor| Page {
        cursor: Some(cursor.clone()),
        after_earning: page.after_earning,
        after_payout: page.after_payout,
    });
    Ok(html! {
        (ui::card(html! {
            h3 { "Joined statement" }
            (ui::Details::new()
                .row("Origin", statement.origin.as_str())
                .row(
                    "Customer",
                    format!("{} / workspace {}", statement.customer, statement.workspace),
                )
                .row("Snapshot", statement.snapshot.as_str())
                .row("Records scanned", statement.scanned.to_string()))
            @match &balance {
                Some(balance) => {
                    h4 { "Pool balance (msat)" }
                    (balance)
                }
                None => {
                    p { "Pool balance: not shown. Member reads omit pool totals; only your exactly attributed original records appear." }
                }
            }
        }))
        @for (label, group) in &groups {
            (ui::card(html! {
                h3 { (label) }
                ol class="statement-rows" {
                    @for row in group { (row) }
                }
            }))
        }
        @if rows.is_empty() {
            p { "No original records on this page." }
        }
        p {
            @if let Some(next) = &next {
                a href=(next.href(&format!("{PAGE}/statements"))) { "Next page" }
                " · "
            }
            a href=(page.href(&format!("{PAGE}/statements/export"))) { "Export this page (NDJSON)" }
        }
        (ui::card(html! {
            h3 { "Source attribution" }
            ul {
                @for a in &view.source_attribution {
                    li {
                        (text(&a["binding"])) " · "
                        @if a["current_original_mapping"] == Value::Bool(true) {
                            "current original mapping"
                        } @else {
                            "historical original source"
                        }
                    }
                }
            }
            p class="dim" { (view.attribution_disclosure) }
        }))
        (ui::card(html! {
            h3 { "Payee earnings" }
            @match &view.payee {
                Some(payee) => {
                    p {
                        "Party " (text(&payee["party"]))
                        " · unit msat. Earnings are separate from customer spending and are never netted against it."
                    }
                    pre class="statement-payee" {
                        (serde_json::to_string_pretty(&payee["statement"]).unwrap_or_default())
                    }
                }
                None => {
                    p { "No payee read is reviewed for this account." }
                }
            }
            p class="dim" { (view.payee_disclosure) }
        }))
        p class="dim" {
            (view.native_projection_disclosure) " " (statement.disclosure.join("; "))
        }
    })
}

/// Retail compute keeps its own owner, unit, and records.
async fn retail_statements(context: &Context<'_>) -> Markup {
    let Some(delegations) = context.app.config.cloud_retail.as_deref() else {
        return html! {};
    };
    let mut sections = Vec::new();
    for delegation in delegations.current(&context.viewer) {
        let id = delegation.id().to_owned();
        let body = match delegations.statement(&context.viewer, &id).await {
            Ok(statement) => render_retail(&id, &statement),
            Err(Failure::Changed(part)) => html! {
                p {
                    "Retail record changed: the retail service answered with a different "
                    (part)
                    " than the one retained. Nothing is shown as current."
                }
            },
            Err(_) => html! {
                p { "Retail service: Unavailable. No balance or charge is estimated in its place." }
            },
        };
        sections.push(html! {
            section class="cloud-card" id=(format!("retail-statement-{id}")) {
                h3 { "Retail compute · delegation " (id) }
                (body)
            }
        });
    }
    html! { @for part in &sections { (part) } }
}

fn render_retail(id: &str, statement: &retail::Statement) -> Markup {
    let funding: Vec<String> = statement
        .funding
        .iter()
        .map(|funding| match (&funding.recorded, &funding.current) {
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
        })
        .collect();
    let mut purchases = Vec::with_capacity(statement.purchases.len());
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
        purchases.push((retail::purchase_href(id, execution), execution, line));
    }
    html! {
        pre class="retail-account" { (statement.account.lines()) }
        h4 { "Funding invoices" }
        @if funding.is_empty() {
            p { "No funding requested from this page." }
        } @else {
            ul class="retail-funding" {
                @for line in &funding { li { (line) } }
            }
        }
        h4 { "Purchases" }
        @if purchases.is_empty() {
            p { "No funded purchases." }
        } @else {
            ul class="retail-statement-purchases" {
                @for (href, execution, line) in &purchases {
                    li {
                        a href=(href) { "Purchase " (execution) }
                        " · " (line)
                    }
                }
            }
        }
        @if statement.more {
            p { a href=(format!("{PAGE}/retail/{id}/purchases")) { "Older purchases" } }
        }
    }
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
    let body = match read {
        Ok(read) => {
            if !current_context(&context, &door, &read) {
                return refused(SessionError::Conflict);
            }
            let wanted = Field::new("billing-receipt-digest", "Receipt digest").required(true);
            html! {
                (render_context(&read))
                (ui::card(html! {
                    h3 { "Recover a purchase" }
                    p { "An earlier purchase is recovered by its original receipt; recovery never invokes, pays, or retries." }
                    form class="cloud-form" method="get" action=(format!("{PAGE}/decisions/{door}/receipt")) {
                        (wanted.clone().control(Input::new("digest").aria(wanted.aria()).required(true).maxlength(71)))
                        div class="cloud-form-actions" { (ui::submit("Read the original receipt", true)) }
                    }
                }))
            }
        }
        Err(error) => match native(&error) {
            Native::Session(error) => return refused(error),
            Native::Unavailable => ui::card(html! {
                h3 { "Purchase · Unavailable" }
                p { "This resource has no current purchase context for your account and workspace: the resource, its price, your membership, or the payer lane is not admitted. Nothing is offered in its place." }
            }),
        },
    };
    if let Err(response) = context.still_current(&headers).await {
        return response;
    }
    let statements = format!("{PAGE}/statements");
    let content = html! {
        h2 { "Decision resource " (door) }
        (ui::links([(PAGE, "Billing"), (statements.as_str(), "Original statement")]))
        (body)
    };
    context.shell(&headers, content)
}

fn render_context(read: &receipts::purchase::Context) -> Markup {
    let membership = ui::Details::new()
        .row("Native account", read.account.as_str())
        .row("Payer workspace", read.payer_workspace.as_str())
        .row(
            "Membership",
            format!(
                "{} · membership epoch {} · workspace members epoch {}",
                read.role, read.membership_epoch, read.workspace_members_epoch
            ),
        )
        .row("Credential", read.credential_reference.as_str())
        .row(
            "Invocation right",
            if read.can_invoke {
                "Current: this account may invoke after approving an exact quote"
            } else {
                "Not admitted: this account cannot invoke this resource"
            },
        )
        .row(
            "Team policy",
            read.team_policy.as_ref().map_or("none".into(), |p| {
                format!("version {} · {}", p.version, p.digest)
            }),
        )
        .row(
            "Canonical mapping",
            read.commercial
                .as_ref()
                .map_or("none; attribution is not established".into(), |c| {
                    format!(
                        "binding {} revision {} · customer {} · attribution only, no right",
                        c.binding, c.revision, c.customer
                    )
                }),
        );
    let price = ui::Details::new()
        .row("Resource", read.door.as_str())
        .row("Artifact", read.artifact_digest.as_str())
        .row("Registry", read.registry_digest.as_str())
        .row(
            "Price",
            format!(
                "version {} · {} · policy {}",
                read.price.version, read.price.currency, read.price.policy
            ),
        )
        .row(
            "Maximum charge",
            format!(
                "{} integer units of {} under price version {}",
                read.price.maximum_charge, read.price.currency, read.price.version
            ),
        )
        .row("Terms", read.price.terms_digest.as_str())
        .row("Maximum usage", read.price.maximum_usage_digest.as_str())
        .row("Context digest", read.digest());
    html! {
        (ui::card(html! {
            h3 { "Payer and permission" }
            (membership)
        }))
        (ui::card(html! {
            h3 { "Exact price reference" }
            (price)
            p { "A quote freezes this exact context and request; approval, funding, and invocation stay on the installed customer client, which rechecks these rights before reservation. A changed resource, price, payer, or membership needs a new review." }
        }))
    }
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
    let original = ui::Details::new()
        .row("Digest", receipt.digest.as_str())
        .row(
            "Request",
            format!("{} · attempt {}", receipt.request, receipt.attempt),
        )
        .row(
            "Outcome",
            text(&serde_json::to_value(&receipt.outcome).unwrap_or_default()),
        )
        .row("Served", receipt.served.model.as_str())
        .row("Request digest", receipt.request_digest.as_str())
        .row("Result digest", known(receipt.result_digest.as_deref()))
        .row(
            "Canonical mapping",
            receipt.commercial.as_ref().map_or("none".into(), |c| {
                format!("binding {} revision {}", c.binding, c.revision)
            }),
        );
    let settlement = match &proof.cost {
        Some(cost) => ui::Details::new()
            .row("Phase", cost.phase.as_str())
            .row(
                "Reserved",
                format!("{} at price version {}", cost.reserved, cost.price_version),
            )
            .row(
                "Charge",
                cost.retail.map_or("unknown".into(), |n| {
                    format!("{n} at price version {}", cost.price_version)
                }),
            ),
        None => ui::Details::new().row("Charge", "unknown; no original hold is joined"),
    };
    if let Err(response) = context.still_current(&headers).await {
        return response;
    }
    let content = html! {
        h2 { "Receipt" }
        p { a href=(format!("{PAGE}/decisions?door={door}")) { "Resource " (door) } }
        (ui::card(html! {
            h3 { "Original receipt" }
            (original)
        }))
        (ui::card(html! {
            h3 { "Settlement" }
            (settlement)
            p class="dim" { "This is the original receipt and its current settlement claim. Reading it never invokes, pays, or retries a purchase." }
        }))
    };
    context.shell(&headers, content)
}
