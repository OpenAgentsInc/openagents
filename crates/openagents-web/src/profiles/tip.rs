//! Explicit gifts use a separate callback so income deposits are not counted twice.
use super::*;
use axum::http::Method;
use openagents_ui::content::PageColumn;

#[derive(Deserialize)]
pub(super) struct Amount {
    amount: Option<u64>,
}
pub(super) async fn page(
    State(app): State<App>,
    Path(handle): Path<String>,
    Query(amount): Query<Amount>,
    headers: HeaderMap,
) -> Response {
    let Ok(who) = who(&app, &handle).await else {
        return missing(&headers);
    };
    let loaded = load(&app.config.profile_relays, &who.pubkey).await;
    let address = loaded
        .metadata
        .as_ref()
        .and_then(|m| m.lud16.as_deref())
        .unwrap_or("");
    let Some(name) = address
        .strip_suffix("@openagents.com")
        .filter(|n| tenancy::accounts::names::valid_name(n))
    else {
        return missing(&headers);
    };
    let Some(service) = app.config.cloud.as_deref() else {
        return unreachable(&headers);
    };
    let terms = service
        .lightning_call(
            Method::GET,
            &format!("/.well-known/lnurlp/{name}"),
            None,
            &[],
            &HeaderMap::new(),
        )
        .await;
    let Ok(terms) = terms else {
        return unreachable(&headers);
    };
    let (Some(min), Some(max)) = (
        terms.body["minSendable"].as_u64(),
        terms.body["maxSendable"].as_u64(),
    ) else {
        return unreachable(&headers);
    };
    if terms.status != 200 || min == 0 || max < min {
        return unreachable(&headers);
    }
    let minimum = min.saturating_add(999) / 1000;
    let maximum = max / 1000;
    let mut invoice = None;
    let mut notice = None;
    if let Some(units) = amount.amount {
        if units < minimum || units > maximum {
            notice = Some("Choose an amount in the range shown.");
        } else {
            let path = format!(
                "/lnurlp/{name}/tip?amount={}&recipient={}",
                units * 1000,
                who.pubkey
            );
            match service
                .lightning_call(Method::GET, &path, None, &[], &HeaderMap::new())
                .await
            {
                Ok(answer) if answer.status == 200 => {
                    invoice = answer.body["pr"]
                        .as_str()
                        .filter(|s| s.starts_with("lnbc") && s.len() < 16384)
                        .map(str::to_owned);
                    if invoice.is_none() {
                        notice = Some("We couldn't create that payment. Try again later.");
                    }
                }
                _ => notice = Some("We couldn't create that payment. Try again later."),
            }
        }
    }
    let content = html! {
        section.oa-card { (openagents_ui::content::MarkdownRoot::new(html! {
            h1 { "Send a tip" }
            p { (address) }
            @if let Some(invoice) = &invoice {
                (qr(invoice))
                p { a.oa-button href=(format!("lightning:{invoice}")) { "Open wallet" } }
                p { "Scan the code or open your Lightning wallet to pay." }
            } @else {
                form method="get" {
                    label for="tip-amount" { "Amount (₿)" }
                    input id="tip-amount" name="amount" type="number" min=(minimum) max=(maximum) step="1" required value=(amount.amount.unwrap_or(minimum));
                    p { (bitcoin_amount::Format::Bip177.show(minimum)) "–" (bitcoin_amount::Format::Bip177.show(maximum)) }
                    button.oa-button type="submit" { "Create payment" }
                }
            }
            @if let Some(notice) = notice { p role="status" { (notice) } }
            p { a href=(format!("/@{}", npub(&who.pubkey))) { "Back to profile" } }
        })) }
    };
    let mut response = UiPage::new("Send a tip")
        .path(&format!("/tip/{handle}"))
        .scriptless()
        .content(PageColumn::new(content))
        .respond(&headers);
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("no-store, private"),
    );
    response
}
fn qr(invoice: &str) -> Markup {
    let Ok(code) =
        qrcodegen::QrCode::encode_text(&invoice.to_ascii_uppercase(), qrcodegen::QrCodeEcc::Medium)
    else {
        return html! {};
    };
    let mut path = String::new();
    for y in 0..code.size() {
        for x in 0..code.size() {
            if code.get_module(x, y) {
                use std::fmt::Write;
                let _ = write!(path, "M{},{}h1v1h-1z", x + 4, y + 4);
            }
        }
    }
    let size = code.size() + 8;
    html! { svg xmlns="http://www.w3.org/2000/svg" viewBox=(format!("0 0 {size} {size}")) width="280" height="280" shape-rendering="crispEdges" role="img" aria-label="Lightning payment code" {
        path fill="#fff" d=(format!("M0,0h{size}v{size}H0z"));
        path fill="#000" d=(path);
    } }
}
