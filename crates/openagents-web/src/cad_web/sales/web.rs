//! Shop, checkout, receipts, and authenticated file downloads.
use super::*;
use axum::{
    Json,
    extract::{DefaultBodyLimit, Request},
    middleware::{self, Next},
};
use openagents_ui::forms::Slider;
const SCRIPT: &str = "/chat/cad/shop.js";
async fn private(request: Request, next: Next) -> Response {
    let mut reply = next.run(request).await;
    reply.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("private, no-store"),
    );
    reply
        .headers_mut()
        .insert("referrer-policy", HeaderValue::from_static("same-origin"));
    reply
}
fn page(headers: &HeaderMap, title: &str, content: Markup) -> Response {
    let mut response=crate::ui_page::UiPage::new(title).head(html!{link rel="stylesheet" href=(crate::cad_lab::ASSET_CSS);script src=(crate::cad_lab::ASSET_JS) defer {} script src=(SCRIPT) defer {}}).content(PageColumn::new(content).wide()).respond(headers);
    response.headers_mut().insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(crate::pages::cad::CAD_POLICY),
    );
    response
}
async fn buyer(app: &App, headers: &HeaderMap) -> Option<String> {
    page_scope(app, headers, None).await.map(|s| s.actor)
}
async fn listing(app: &App, id: &str) -> Option<Listing> {
    if !valid(id) {
        return None;
    }
    read::<Listing>(&app.config.chat_store, &listing_key(id))
        .await
        .ok()
        .flatten()
        .map(|(l, _)| l)
        .filter(|l| l.active)
}
async fn shop(State(app): State<App>, headers: HeaderMap) -> Response {
    let ids = read::<Vec<String>>(&app.config.chat_store, CATALOG)
        .await
        .ok()
        .flatten()
        .map(|(ids, _)| ids)
        .unwrap_or_default();
    let mut items = Vec::new();
    for id in ids.into_iter().take(500) {
        if valid(&id) {
            if let Some((brief, _)) = read::<Brief>(
                &app.config.chat_store,
                &format!("cad-products/{id}/public.json"),
            )
            .await
            .ok()
            .flatten()
            .filter(|(b, _)| b.active)
            {
                items.push((id, brief));
            }
        }
    }
    page(
        &headers,
        "CAD parts",
        html! {h1 {"CAD parts"}a href="/cad-purchases" {"Your purchases"} @if items.is_empty(){p {"No parts are listed yet."}} @for (id,l) in items {article.oa-cad-config-option {h2 {a href=(format!("/cad-shop/{id}")){(l.title)}}p {(l.sats) " sats"}}}},
    )
}
async fn product(
    State(app): State<App>,
    headers: HeaderMap,
    UrlPath(id): UrlPath<String>,
) -> Response {
    let Some(l) = listing(&app, &id).await else {
        return crate::not_found_for(&headers);
    };
    let buyer = buyer(&app, &headers).await;
    page(
        &headers,
        &l.title,
        html! {
            section data-cad-shop="" {a href="/cad-shop" {"CAD parts"}h1 {(&l.title)}p {(l.sats) " sats · paid directly to " (&l.address)}
                (crate::cad_lab::viewer(&format!("/cad-shop/{id}/view"),None,&l.title,"shop",l.data["version"].as_u64().unwrap_or(1)))
                @if let Some(buyer)=&buyer {
                    form method="post" action=(format!("/cad-shop/{id}/checkout")) data-shop-form="" hx-boost="false" {
                        input type="hidden" name="csrf" value=(crate::pages::chat::csrf(&app,buyer));input type="hidden" name="order" value=(token());input type="hidden" name="values" value=(l.data["values"].to_string());
                        @for (i,c) in l.data["controls"].as_array().into_iter().flatten().enumerate(){
                            (Slider::new(format!("dimension-{i}")).id(format!("dimension-{i}")).label(c["label"].as_str().unwrap_or("Dimension")).range(c["min"].as_f64().unwrap_or(0.),c["max"].as_f64().unwrap_or(1.)).step(c["step"].as_f64().unwrap_or(0.1)).value(l.data["values"][i].as_f64().unwrap_or(0.)))
                        }
                        div.oa-cad-files {(Button::new("Update preview").kind(ButtonType::Submit).variant(ButtonVariant::Outline).attr("name","intent").attr("value","preview")) (Button::new(format!("Buy for {} sats",l.sats)).kind(ButtonType::Submit).attr("name","intent").attr("value","buy"))}
                        p data-shop-status="" role="status" aria-live="polite" {}
                    }
                } @else {(ButtonLink::new("Sign in to customize and buy",crate::auth::login_href(&format!("/cad-shop/{id}"),false)))}
                p {"Includes STEP, STL, and PDF and DXF drawings for your dimensions."}
            }
        },
    )
}
async fn view(State(app): State<App>, UrlPath(id): UrlPath<String>) -> Response {
    match listing(&app, &id).await {
        Some(l) => Json(l.data["view"].clone()).into_response(),
        None => crate::not_found().await,
    }
}
#[derive(Deserialize)]
struct Checkout {
    csrf: String,
    order: String,
    values: String,
    #[serde(default)]
    intent: String,
}
async fn checkout(
    State(app): State<App>,
    headers: HeaderMap,
    UrlPath(id): UrlPath<String>,
    Form(form): Form<Checkout>,
) -> Response {
    if crate::pages::chat::validate_form(&app, &headers, &form.csrf)
        .await
        .is_err()
    {
        return StatusCode::FORBIDDEN.into_response();
    }
    let Some(buyer) = buyer(&app, &headers).await else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    let Some(l) = listing(&app, &id).await else {
        return crate::not_found_for(&headers);
    };
    let values: Vec<f64> = match serde_json::from_str(&form.values) {
        Ok(values) => values,
        Err(_) => return (StatusCode::BAD_REQUEST, "Check the chosen dimensions.").into_response(),
    };
    if let Err(error) = l.values(&values) {
        return (StatusCode::BAD_REQUEST, error).into_response();
    }
    if form.intent == "preview" {
        let Some(engine) = engine() else {
            return StatusCode::SERVICE_UNAVAILABLE.into_response();
        };
        let result=engine.run(&app.config.chat_store,&l.scope(&buyer),"cad_product",&json!({"doc":l.doc,"version":l.data["version"],"controls":l.data["controls"],"values":values,"preview":true})).await;
        if let Some(error) = result["error"].as_str() {
            return (StatusCode::BAD_REQUEST, error.to_owned()).into_response();
        }
        return Json(result["view"].clone()).into_response();
    }
    match prepare(&app.config.chat_store, &buyer, &id, &form.order, values).await {
        Ok(_) => Redirect::to(&format!("/cad-order/{}", form.order)).into_response(),
        Err(error) => (StatusCode::CONFLICT, error).into_response(),
    }
}
async fn owned(app: &App, headers: &HeaderMap, id: &str) -> Option<Order> {
    if !valid(id) {
        return None;
    }
    let who = buyer(app, headers).await?;
    read::<Order>(&app.config.chat_store, &order_key(id))
        .await
        .ok()
        .flatten()
        .map(|(o, _)| o)
        .filter(|o| o.buyer == who)
}
#[derive(Default, Deserialize)]
struct Status {
    pending: Option<u8>,
}
async fn order(
    State(app): State<App>,
    headers: HeaderMap,
    UrlPath(id): UrlPath<String>,
    Query(query): Query<Status>,
) -> Response {
    let Some(order) = owned(&app, &headers, &id).await else {
        return crate::not_found_for(&headers);
    };
    let qr_available = order.invoice.as_ref().is_some_and(|i| {
        qrcodegen::QrCode::encode_text(
            &format!("lightning:{}", i.bolt11).to_uppercase(),
            qrcodegen::QrCodeEcc::Low,
        )
        .is_ok()
    });
    let can_resume = read::<Brief>(
        &app.config.chat_store,
        &format!("cad-products/{}/public.json", order.listing),
    )
    .await
    .ok()
    .flatten()
    .is_some_and(|(b, _)| b.active);
    page(
        &headers,
        &order.title,
        html! {
            a href="/cad-purchases" {"Your purchases"}h1 {(&order.title)}
            p {"Version " (order.version) " · " (order.sats) " sats"}
            dl.oa-cad-list {@for (label,value) in order.labels.iter().zip(&order.values){dt {(label)}dd {(value)}}}
            @if order.paid {
                h2 {"Your files are ready"}p {"Paid to " (&order.address)}
                div.oa-cad-files {@for (format,label) in [("zip","All files (ZIP)"),("step","STEP"),("stl","STL"),("pdf","Drawing (PDF)"),("dxf","Drawing (DXF)")] {(ButtonLink::new(label,format!("/cad-order/{id}/file/{format}")).variant(ButtonVariant::Outline))}}
            } @else if let Some(invoice)=&order.invoice {
                h2 {"Pay with Lightning"}
                @if invoice.expires>now(){@if qr_available {img.oa-cad-payment-qr src=(format!("/cad-order/{id}/qr")) width="260" height="260" alt="Lightning invoice QR code";}
                p {(ButtonLink::new(format!("Pay {} sats in your wallet",order.sats),format!("lightning:{}",invoice.bolt11)))}
                label {"Invoice" textarea data-shop-invoice="" readonly rows="4" {(&invoice.bolt11)}}
                (Button::new("Copy invoice").variant(ButtonVariant::Outline).attr("data-shop-copy",""))
                p data-shop-status="" role="status" aria-live="polite" {}
                } @else {p {"This invoice has expired. If you paid it, check payment to get your files."}(ButtonLink::new("Return to the listing",format!("/cad-shop/{}",order.listing)).variant(ButtonVariant::Outline))}
                @if query.pending.is_some(){p {"Payment hasn't arrived yet. You can check again."}}
                form method="post" action=(format!("/cad-order/{id}/check")) {input type="hidden" name="csrf" value=(crate::pages::chat::csrf(&app,&order.buyer));(Button::new("Check payment").kind(ButtonType::Submit))}
            } @else if can_resume {
                form method="post" action=(format!("/cad-shop/{}/checkout",order.listing)) {input type="hidden" name="csrf" value=(crate::pages::chat::csrf(&app,&order.buyer));input type="hidden" name="order" value=(&id);input type="hidden" name="values" value=(json!(order.values).to_string());(Button::new("Continue checkout").kind(ButtonType::Submit))}
            } @else {p {"This design is no longer for sale."}}
        },
    )
}
#[derive(Deserialize)]
struct Check {
    csrf: String,
}
async fn check(
    State(app): State<App>,
    headers: HeaderMap,
    UrlPath(id): UrlPath<String>,
    Form(form): Form<Check>,
) -> Response {
    if crate::pages::chat::validate_form(&app, &headers, &form.csrf)
        .await
        .is_err()
    {
        return StatusCode::FORBIDDEN.into_response();
    }
    let Some(order) = owned(&app, &headers, &id).await else {
        return crate::not_found_for(&headers);
    };
    match pay_check(&app.config.chat_store, &order.buyer, &id).await {
        Ok(paid) => Redirect::to(&format!(
            "/cad-order/{id}{}",
            if paid { "" } else { "?pending=1" }
        ))
        .into_response(),
        Err(error) => (StatusCode::CONFLICT, error).into_response(),
    }
}
async fn download(
    State(app): State<App>,
    headers: HeaderMap,
    UrlPath((id, format)): UrlPath<(String, String)>,
) -> Response {
    let Some(order) = owned(&app, &headers, &id).await else {
        return crate::not_found_for(&headers);
    };
    if !order.paid {
        return (
            StatusCode::PAYMENT_REQUIRED,
            "Pay for this order to download its files.",
        )
            .into_response();
    }
    let Some((bytes, _)) = app
        .config
        .chat_store
        .read_key(&package_key(&id))
        .await
        .ok()
        .flatten()
    else {
        return crate::not_found_for(&headers);
    };
    if Some(hex(&Sha256::digest(&bytes))) != order.package_hash {
        return (
            StatusCode::CONFLICT,
            "The saved files need to be restored. Keep this order number.",
        )
            .into_response();
    }
    if format == "zip" {
        return file_response("design-files.zip", bytes, Some("design-files.zip"));
    }
    let Some((_, name)) = FILES.iter().find(|(f, _)| *f == format) else {
        return crate::not_found_for(&headers);
    };
    use std::io::Read as _;
    let result = (|| {
        let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).ok()?;
        let file = zip.by_name(name).ok()?;
        if file.size() > 15 << 20 {
            return None;
        }
        let mut out = Vec::new();
        file.take(15 * 1024 * 1024 + 1).read_to_end(&mut out).ok()?;
        (out.len() <= 15 << 20).then_some(out)
    })();
    match result {
        Some(bytes) => file_response(name, bytes, Some(name)),
        None => (StatusCode::CONFLICT, "The saved file couldn't be opened.").into_response(),
    }
}
async fn qr(State(app): State<App>, headers: HeaderMap, UrlPath(id): UrlPath<String>) -> Response {
    let Some(order) = owned(&app, &headers, &id).await else {
        return crate::not_found_for(&headers);
    };
    let Some(invoice) = order.invoice else {
        return crate::not_found_for(&headers);
    };
    let code = match qrcodegen::QrCode::encode_text(
        &format!("lightning:{}", invoice.bolt11).to_uppercase(),
        qrcodegen::QrCodeEcc::Low,
    ) {
        Ok(code) => code,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };
    let mut path = String::new();
    use std::fmt::Write as _;
    for y in 0..code.size() {
        for x in 0..code.size() {
            if code.get_module(x, y) {
                let _ = write!(path, "M{},{}h1v1h-1z", x + 4, y + 4);
            }
        }
    }
    file_response("invoice.svg",format!("<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {0} {0}\" shape-rendering=\"crispEdges\"><path fill=\"white\" d=\"M0,0h{0}v{0}H0z\"/><path fill=\"black\" d=\"{path}\"/></svg>",code.size()+8).into_bytes(),None)
}
async fn purchases(State(app): State<App>, headers: HeaderMap) -> Response {
    let Some(buyer) = buyer(&app, &headers).await else {
        return Redirect::to(&crate::auth::login_href("/cad-purchases", false)).into_response();
    };
    let scope = Scope::personal(&buyer).unwrap();
    let prefix = scope.key("purchases").unwrap();
    let keys = app
        .config
        .chat_store
        .folder_keys(&prefix)
        .await
        .unwrap_or_default();
    let mut orders = Vec::new();
    for key in keys.into_iter().rev().take(200) {
        if let Some(id) = key
            .rsplit('/')
            .next()
            .and_then(|s| s.strip_suffix(".json"))
            .filter(|s| valid(s))
        {
            if let Some(order) = owned(&app, &headers, id).await {
                orders.push((id.to_owned(), order));
            }
        }
    }
    page(
        &headers,
        "Your CAD purchases",
        html! {h1 {"Your CAD purchases"}a href="/cad-shop" {"Browse parts"}@if orders.is_empty(){p {"You haven't bought a part yet."}} @for (id,o) in orders {article.oa-cad-config-option {h2 {a href=(format!("/cad-order/{id}")){(o.title)}}p {(o.sats) " sats · " @if o.paid {"Paid"}@else {"Checkout"}}}}},
    )
}
#[derive(Deserialize)]
struct Stop {
    csrf: String,
    #[serde(default)]
    project: String,
    token: String,
}
async fn unlist(
    State(app): State<App>,
    headers: HeaderMap,
    UrlPath(doc): UrlPath<String>,
    Form(form): Form<Stop>,
) -> Response {
    if crate::pages::chat::validate_form(&app, &headers, &form.csrf)
        .await
        .is_err()
    {
        return StatusCode::FORBIDDEN.into_response();
    }
    let Some(scope) = page_scope(&app, &headers, Some(&form.project)).await else {
        return crate::not_found_for(&headers);
    };
    match stop(&app.config.chat_store, &scope, &doc, &form.token).await {
        Ok(_) => Redirect::to(&format!("/cad/{doc}/configurator?project={}", form.project))
            .into_response(),
        Err(error) => (StatusCode::FORBIDDEN, error).into_response(),
    }
}
pub(super) fn routes() -> Router<App> {
    Router::new()
        .route("/cad-shop", get(shop))
        .route("/cad-shop/{token}", get(product))
        .route("/cad-shop/{token}/view", get(view))
        .route("/cad-shop/{token}/checkout", post(checkout))
        .route("/cad-purchases", get(purchases))
        .route("/cad-order/{order}", get(order))
        .route("/cad-order/{order}/check", post(check))
        .route("/cad-order/{order}/qr", get(qr))
        .route("/cad-order/{order}/file/{format}", get(download))
        .route("/cad/{file}/unlist", post(unlist))
        .route(
            SCRIPT,
            get(|| async {
                (
                    [(header::CONTENT_TYPE, "text/javascript; charset=utf-8")],
                    include_str!("../../../static/cad-shop.js"),
                )
            }),
        )
        .layer(DefaultBodyLimit::max(16 * 1024))
        .layer(middleware::from_fn(private))
}
