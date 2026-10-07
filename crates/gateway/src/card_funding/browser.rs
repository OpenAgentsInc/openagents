//! The selected Rust browser client uses the same native billing authority.
use super::{
    browser_csrf::{self, FundingForm},
    controller::{self, Request},
};
use crate::{
    dashboard::{esc, page, page_error},
    serve::ServeState,
};
use axum::{
    Json,
    extract::{Form, Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use jev::CardFundingView;
use std::sync::Arc;

fn unavailable() -> Response {
    page_error(
        StatusCode::SERVICE_UNAVAILABLE,
        "Checkout unavailable",
        "Original checkout references remain retained. Refresh or resume the original reference when billing is available.",
    )
}
fn valid(v: &str) -> bool {
    !v.is_empty()
        && v.len() <= 128
        && v.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
        && !matches!(v, "." | "..")
}
fn money(units: u64) -> String {
    format!("{}.{:06} USD", units / 1_000_000, units % 1_000_000)
}
fn form(
    headers: &HeaderMap,
    workspace: &str,
    action: &str,
    door: &str,
    id: &str,
    approved: &str,
    label: &str,
) -> Result<String, Response> {
    let mut f = FundingForm {
        action: action.into(),
        door: door.into(),
        id: id.into(),
        gross_cents: 0,
        approved: approved.into(),
        expires: crate::accounts::unix_now().saturating_add(300),
        csrf: String::new(),
    };
    f.csrf = browser_csrf::sign(headers, workspace, &f)?;
    let amount = if action == "quote" {
        r#"<label>Amount in USD cents <input name="gross_cents" type="number" min="1" step="1" required></label>"#
    } else {
        ""
    };
    Ok(format!(
        r#"<form method="post" action="/dashboard/w/{}/funding"><input type="hidden" name="action" value="{}"><input type="hidden" name="door" value="{}"><input type="hidden" name="id" value="{}"><input type="hidden" name="approved" value="{}"><input type="hidden" name="expires" value="{}"><input type="hidden" name="csrf" value="{}">{}<button type="submit">{}</button></form>"#,
        esc(workspace),
        esc(action),
        esc(door),
        esc(id),
        esc(approved),
        f.expires,
        f.csrf,
        amount,
        esc(label)
    ))
}
async fn call(
    state: Arc<ServeState>,
    headers: &HeaderMap,
    workspace: &str,
    door: &str,
    request: Request,
) -> Result<CardFundingView, Response> {
    let forwarded = browser_csrf::authorization(headers, workspace)?;
    let response = controller::handle(
        State(state),
        Path((workspace.into(), door.into())),
        forwarded,
        Json(request),
    )
    .await;
    if !response.status().is_success() {
        return Err(page_error(
            response.status(),
            "Checkout unavailable",
            "Current billing authority or original provider evidence is unavailable. Original obligations remain retained.",
        ));
    }
    let bytes = axum::body::to_bytes(response.into_body(), 256 * 1024)
        .await
        .map_err(|_| unavailable())?;
    serde_json::from_slice(&bytes).map_err(|_| unavailable())
}
fn status(view: &CardFundingView) -> &'static str {
    if view.record.applying.is_some() {
        return "Application pending";
    }
    if let Some(o) = &view.record.applied {
        if o.snapshot.reconciliation_pending {
            return "Unknown; credit restricted pending reconciliation";
        }
        if o.snapshot.refunded_source_units > 0 || o.snapshot.disputed_source_units > 0 {
            return "Reversed or disputed";
        }
        return match o.snapshot.finality {
            jev::CardFundingFinality::Pending => "Collection pending",
            jev::CardFundingFinality::Confirmed => "Confirmed",
            jev::CardFundingFinality::Final => "Funded",
        };
    }
    match view.record.unpaid_status.as_deref() {
        Some("pending") => "Pending payment",
        Some("expired") => "Expired unpaid; no funding",
        _ => "Unknown; no verified collection",
    }
}
async fn render(
    state: &ServeState,
    headers: &HeaderMap,
    workspace: &str,
    view: &CardFundingView,
) -> Result<String, Response> {
    let b = &view.record.binding;
    let fresh = browser_csrf::authorization(headers, workspace)?;
    let current = controller::current(
        state,
        &fresh,
        &b.context.door,
        &serde_json::from_value(serde_json::to_value(b).map_err(|_| unavailable())?)
            .map_err(|_| unavailable())?,
    )
    .is_ok();
    let mut html = format!(
        r#"<section><h2>Original checkout <code>{}</code></h2><p><strong>{}</strong></p><dl><dt>Payer workspace</dt><dd>{}</dd><dt>Resource</dt><dd>{}</dd><dt>Gross amount</dt><dd>{}</dd><dt>Maximum customer fee</dt><dd>{}</dd><dt>Conversion</dt><dd>USD to USD, exact 1:1; {}</dd><dt>Price version</dt><dd>{}</dd><dt>Quote expires (Unix seconds)</dt><dd>{}</dd><dt>Terms approval digest</dt><dd><code>{}</code></dd></dl><p>Available commercial credit: {}. Reserved: {}. Restricted: {}. Spend limit remaining: {}.</p><p>Processor and Lightning wallet liquidity: unknown. Commercial credit does not establish wallet liquidity.</p>"#,
        esc(&b.quote.id),
        status(view),
        esc(&b.context.payer_workspace),
        esc(&b.context.door),
        money(b.quote.gross_units),
        money(b.quote.maximum_fee_units),
        esc(&b.quote.conversion),
        esc(&b.context.price.version),
        b.quote.expires_at,
        esc(&view.approval_digest),
        money(view.balance.available),
        money(view.balance.reserved),
        money(view.balance.restricted_credit),
        money(view.balance.spend_remaining)
    );
    html.push_str(&format!("<p>Funding policy: {} (<code>{}</code>). Resource maximum charge: {}. Resource policy: {} (<code>{}</code>). Credit requires final verified collection.</p>",esc(&b.quote.policy),esc(&b.policy_digest),money(b.context.price.maximum_charge),esc(&b.context.price.policy),esc(&b.context.price.terms_digest)));
    if let Some(o) = &view.record.applied {
        html.push_str(&format!("<p>Verified collection fee: {}. Returned principal: {}. Disputed principal: {}. Finality: {:?}.</p>",money(o.snapshot.funding.fee_units),money(o.snapshot.refunded_source_units),money(o.snapshot.disputed_source_units),o.snapshot.finality));
    }
    html.push_str(&format!(
        "<p>Outstanding holds: {}.</p><ul>",
        view.outstanding_count
            .map(|n| n.to_string())
            .unwrap_or_else(|| "detail unavailable".into())
    ));
    if let Some(holds) = &view.outstanding {
        for h in holds {
            html.push_str(&format!(
                "<li><code>{}</code>: {} ({})</li>",
                esc(&h.attempt),
                money(h.reserved),
                esc(&h.phase)
            ));
        }
    }
    html.push_str("</ul>");
    if let (Some(holds), Some(total)) = (&view.outstanding, view.outstanding_count)
        && total > holds.len() as u64
    {
        html.push_str(&format!(
            "<p>Showing {} of {} outstanding holds.</p>",
            holds.len(),
            total
        ));
    }

    if current && crate::accounts::unix_now() < b.quote.expires_at && view.record.applied.is_none()
    {
        if let Some(url) = &view.record.hosted_url {
            if let Ok(u) = reqwest::Url::parse(url)
                && u.scheme() == "https"
                && u.host_str() == Some("checkout.stripe.com")
                && u.username().is_empty()
                && u.password().is_none()
            {
                html.push_str(&format!(r#"<p><a rel="noreferrer" href="{}">Continue the original hosted checkout</a></p>"#,esc(url)));
            }
        } else {
            html.push_str(&form(
                headers,
                workspace,
                "checkout",
                &b.context.door,
                &b.quote.id,
                &view.approval_digest,
                "Approve these terms and prepare checkout",
            )?);
        }
    }
    if view
        .record
        .checkout
        .as_ref()
        .and_then(|c| c.native.as_ref())
        .is_some()
    {
        html.push_str(&form(
            headers,
            workspace,
            "reconcile",
            &b.context.door,
            &b.quote.id,
            "",
            "Check original payment status",
        )?);
    }
    html.push_str(&format!(
        r#"<p><a href="/dashboard/funding/{}">Resume original checkout</a></p></section>"#,
        esc(&b.quote.id)
    ));
    Ok(html)
}

pub(crate) async fn billing(
    state: Arc<ServeState>,
    workspace: String,
    headers: HeaderMap,
) -> Response {
    if !valid(&workspace) {
        return unavailable();
    }
    let forwarded = match browser_csrf::authorization(&headers, &workspace) {
        Ok(h) => h,
        Err(r) => return r,
    };
    let account = match controller::view_authority(&state, &forwarded, &workspace) {
        Ok(a) => a,
        Err(r) => return r,
    };
    let mut html = String::from(
        "<h1>Prepaid billing</h1><p>Review a quote before approving checkout. Production card funding requires owner activation O5. Credit requires final verified collection; applicable native customer fees reduce the credited amount. A return from the provider changes no credit.</p>",
    );
    for door in state.config.doors.keys() {
        if !valid(door) {
            continue;
        }
        if crate::purchase::current(&state, &forwarded, door)
            .is_ok_and(|c| c.can_invoke && c.price.currency == "USD")
        {
            let Ok(id) = tenancy::billing::fresh_ref() else {
                return unavailable();
            };
            html.push_str(&format!("<h2>{}</h2>", esc(door)));
            match form(&headers, &workspace, "quote", door, &id, "", "Review quote") {
                Ok(f) => html.push_str(&f),
                Err(r) => return r,
            }
        }
    }
    let store = match state.card_billing.as_ref().and_then(|b| b.store().ok()) {
        Some(s) => s,
        None => return unavailable(),
    };
    let refs = store
        .book
        .prepaid
        .checkouts
        .values()
        .filter(|r| {
            r.binding.context.workspace == workspace && r.binding.context.account == account
        })
        .map(|r| (r.binding.context.door.clone(), r.binding.quote.id.clone()))
        .collect::<Vec<_>>();
    for (door, id) in refs {
        let v = match call(
            state.clone(),
            &headers,
            &workspace,
            &door,
            Request::Read { id },
        )
        .await
        {
            Ok(v) => v,
            Err(r) => return r,
        };
        match render(&state, &headers, &workspace, &v).await {
            Ok(s) => html.push_str(&s),
            Err(r) => return r,
        }
    }
    if controller::view_authority(&state, &forwarded, &workspace).is_err() {
        return unavailable();
    }
    private(page("Prepaid billing", Some(&workspace), &html).into_response())
}
fn private(mut response: Response) -> Response {
    response
        .headers_mut()
        .insert("cache-control", "no-store".parse().unwrap());
    response
        .headers_mut()
        .insert("referrer-policy", "no-referrer".parse().unwrap());
    response
}
pub(crate) async fn submit(
    State(state): State<Arc<ServeState>>,
    Path(workspace): Path<String>,
    headers: HeaderMap,
    Form(f): Form<FundingForm>,
) -> Response {
    if !valid(&workspace) || !valid(&f.door) || !valid(&f.id) {
        return unavailable();
    }
    if let Err(r) = browser_csrf::check(&headers, &workspace, &f, crate::accounts::unix_now()) {
        return r;
    }
    let request = match f.action.as_str() {
        "quote" if f.approved.is_empty() => match f.gross_cents.checked_mul(10_000) {
            Some(gross_units) => Request::Quote {
                id: f.id,
                gross_units,
            },
            None => return unavailable(),
        },
        "checkout"
            if f.gross_cents == 0
                && f.approved
                    .strip_prefix("sha256:")
                    .is_some_and(|h| h.len() == 64 && h.bytes().all(|b| b.is_ascii_hexdigit())) =>
        {
            Request::Checkout {
                id: f.id,
                approved: f.approved,
            }
        }
        "reconcile" if f.gross_cents == 0 && f.approved.is_empty() => {
            Request::Reconcile { id: f.id }
        }
        _ => return unavailable(),
    };
    let v = match call(state.clone(), &headers, &workspace, &f.door, request).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    match render(&state, &headers, &workspace, &v).await {
        Ok(body) => private(page("Original checkout", Some(&workspace), &body).into_response()),
        Err(r) => r,
    }
}
pub(crate) async fn resume(
    State(state): State<Arc<ServeState>>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Response {
    if !valid(&id) {
        return unavailable();
    }
    // Retained routing is never payment authority. Current authentication is checked by Read.
    if crate::dashboard::principal_of(&state, &headers).is_err() {
        return page_error(
            StatusCode::UNAUTHORIZED,
            "Sign-in required",
            "Sign in before resuming the original checkout.",
        );
    }
    let Some(r) = state
        .card_billing
        .as_ref()
        .and_then(|b| b.store().ok())
        .and_then(|s| s.book.prepaid.checkouts.get(&id).cloned())
    else {
        return unavailable();
    };
    let workspace = &r.binding.context.workspace;
    let v = match call(
        state.clone(),
        &headers,
        workspace,
        &r.binding.context.door,
        Request::Read { id },
    )
    .await
    {
        Ok(v) => v,
        Err(r) => return r,
    };
    match render(&state, &headers, workspace, &v).await {
        Ok(body) => private(page("Original checkout", Some(workspace), &body).into_response()),
        Err(r) => r,
    }
}
