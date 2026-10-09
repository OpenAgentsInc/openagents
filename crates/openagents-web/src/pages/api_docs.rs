//! The API docs: `/docs/api` and its guides, each also served as Markdown
//! at `/docs/api/{slug}.md`, with an `llms.txt` index at
//! `/docs/api/llms.txt` (OpenAgents API design, D15).
//!
//! The guides are Markdown in `content/docs/api/`. The models page's rate
//! card and model table are drawn from the inference gateway's own card:
//! `GET /v1/rates` on the gateway this site is pointed at (`--inference`),
//! or, when there is none or it doesn't answer in time, the card the
//! gateway's adapters publish (`inference::rates::Card::published`), which
//! is what a gateway with no rate overrides serves.

use std::time::Duration;

use axum::Router;
use axum::body::{Body, to_bytes};
use axum::extract::{Path, State};
use axum::http::{HeaderMap, HeaderValue, Method, Request, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use inference::rates::{Card, Kind, Row};
use maud::{PreEscaped, html};
use openagents_ui::content::{MarkdownRoot, PageColumn};
use openagents_ui::shell::Breadcrumb;

use crate::App;
use crate::layout::escape;
use crate::markdown;
use crate::ui_page::{UiPage, action_link, problem, prose};

/// The guides, by slug, in reading order.
pub(crate) const API_DOCS: [(&str, &str); 12] = [
    (
        "quickstart",
        include_str!("../../content/docs/api/quickstart.md"),
    ),
    ("models", include_str!("../../content/docs/api/models.md")),
    (
        "decisions",
        include_str!("../../content/docs/api/decisions.md"),
    ),
    (
        "responses",
        include_str!("../../content/docs/api/responses.md"),
    ),
    (
        "chat-completions",
        include_str!("../../content/docs/api/chat-completions.md"),
    ),
    ("routing", include_str!("../../content/docs/api/routing.md")),
    (
        "bring-your-own-key",
        include_str!("../../content/docs/api/bring-your-own-key.md"),
    ),
    (
        "pay-per-request",
        include_str!("../../content/docs/api/pay-per-request.md"),
    ),
    (
        "for-agents",
        include_str!("../../content/docs/api/for-agents.md"),
    ),
    ("errors", include_str!("../../content/docs/api/errors.md")),
    ("limits", include_str!("../../content/docs/api/limits.md")),
    ("privacy", include_str!("../../content/docs/api/privacy.md")),
];

/// Where the models page draws the rate card.
const RATE_CARD: &str = "{{rate card}}";
/// Where the models page draws the model table.
const MODEL_TABLE: &str = "{{model table}}";
const TOKENS: &str = "{{tokens served}}";

/// How long the models page waits for the gateway's card.
const WAIT: Duration = Duration::from_secs(2);
const LIMIT: usize = 1024 * 1024;

pub(crate) fn routes() -> Router<App> {
    Router::new()
        .route("/docs/api", get(index))
        .route("/docs/api/{slug}", get(guide))
}

/// The gateway's card, or the published one.
pub(crate) async fn card(app: &App) -> Card {
    match read(app).await {
        Some(card) => card,
        None => Card::published(None),
    }
}

async fn read(app: &App) -> Option<Card> {
    let gateway = app.config.inference.as_ref()?;
    let request = Request::builder()
        .method(Method::GET)
        .uri("/v1/rates")
        .header(header::ACCEPT, "application/json")
        .body(Body::empty())
        .ok()?;
    let response = tokio::time::timeout(WAIT, gateway.forward(request))
        .await
        .ok()?;
    if response.status() != StatusCode::OK {
        return None;
    }
    let body = tokio::time::timeout(WAIT, to_bytes(response.into_body(), LIMIT))
        .await
        .ok()?
        .ok()?;
    serde_json::from_slice(&body).ok()
}

/// Reads the meter's public counts without a key or a local estimate.
async fn served(app: &App) -> String {
    let unavailable = "Token totals are unavailable right now.".to_owned();
    let Some(gateway) = app.config.inference.as_ref() else {
        return unavailable;
    };
    let Ok(request) = Request::builder()
        .method(Method::GET)
        .uri("/v1/usage/tokens-served")
        .header(header::ACCEPT, "application/json")
        .body(Body::empty())
    else {
        return unavailable;
    };
    let Ok(response) = tokio::time::timeout(WAIT, gateway.forward(request)).await else {
        return unavailable;
    };
    if response.status() != StatusCode::OK {
        return unavailable;
    }
    let Ok(Ok(body)) = tokio::time::timeout(WAIT, to_bytes(response.into_body(), LIMIT)).await
    else {
        return unavailable;
    };
    let Ok(report) = serde_json::from_slice::<inference::meter::served::Report>(&body) else {
        return unavailable;
    };
    if report.v != inference::meter::served::SCHEMA {
        return unavailable;
    }
    served_table(&report)
}

fn served_table(report: &inference::meter::served::Report) -> String {
    let counts = &report.totals;
    let mut out = format!(
        "**{} tokens served** (input and output).\n\n",
        thousands(counts.all.total)
    );
    if let Some(since) = report.since_ms {
        out.push_str(&format!(
            "Counting since {} (UTC).\n\n",
            inference::meter::store::date(since / inference::meter::store::DAY_MS)
        ));
    }
    if !report.persistent {
        out.push_str("These totals cover this server's current run.\n\n");
    }
    out.push_str("| Caller | Free calls | Paid calls |\n| --- | ---: | ---: |\n");
    out.push_str(&format!(
        "| Our services | {} | {} |\n| Outside callers | {} | {} |\n\n",
        thousands(counts.internal.free.total),
        thousands(counts.internal.paid.total),
        thousands(counts.outside.free.total),
        thousands(counts.outside.paid.total)
    ));
    let own = counts
        .internal
        .own_key
        .total
        .saturating_add(counts.outside.own_key.total);
    out.push_str(&format!(
        "Paid calls include **{} tokens on callers' own keys**.\n\n",
        thousands(own)
    ));
    if !report.days.is_empty() {
        out.push_str("| Day (UTC) | Our services, free | Our services, paid | Outside callers, free | Outside callers, paid | Total |\n| --- | ---: | ---: | ---: | ---: | ---: |\n");
        for (day, totals) in report.days.iter().rev().take(7) {
            // The date comes from the gateway, so escape it as Markdown text.
            let day: String = day
                .chars()
                .filter(|c| c.is_ascii_digit() || *c == '-')
                .collect();
            out.push_str(&format!(
                "| {day} | {} | {} | {} | {} | {} |\n",
                thousands(totals.internal.free.total),
                thousands(totals.internal.paid.total),
                thousands(totals.outside.free.total),
                thousands(totals.outside.paid.total),
                thousands(totals.all.total)
            ));
        }
        out.push('\n');
    }
    out.push_str("[Read the same counts as JSON](/api/v1/usage/tokens-served). Only answers with reported token counts are included.");
    out
}

fn dollars(amount: &str) -> String {
    format!("${amount}")
}

fn cell(amount: &inference::rates::Amount, kind: Kind) -> String {
    let sats = amount
        .price_sats
        .map(|sats| format!(" ({sats} sats)"))
        .unwrap_or_default();
    match kind {
        Kind::List => format!(
            "{} + {} = **{}**{sats}",
            dollars(&amount.list_usd),
            dollars(&amount.margin_usd),
            dollars(&amount.price_usd)
        ),
        Kind::Promotion => format!(
            "~~{}~~ **{}**{sats}",
            dollars(&amount.list_usd),
            dollars(&amount.price_usd)
        ),
    }
}

/// Markdown table cells can't hold a pipe.
fn plain(text: &str) -> String {
    text.replace('|', "/")
}

/// The rate card as Markdown: a line on how to read it, then one table
/// row per model and provider, a promotion on its own row.
pub(crate) fn rate_card(card: &Card) -> String {
    let margins: Vec<&str> = card
        .rows
        .iter()
        .filter(|row| row.kind == Kind::List)
        .map(|row| row.margin_percent.as_str())
        .collect();
    let margin = match margins.first() {
        Some(first) if margins.iter().all(|m| m == first) => format!("our {first}%"),
        _ => "our margin".to_owned(),
    };
    let sats = match &card.sats_rate {
        Some(rate) => format!(
            " Sats are at ${} per bitcoin ({}).",
            thousands(rate.usd_per_btc),
            plain(&rate.as_of)
        ),
        None => String::new(),
    };
    let mut out = format!(
        "Each price is per million tokens, in US dollars: the provider's list price, plus \
{margin}, equals what you pay. You're charged in sats from your balance.{sats}\n\n\
| Model | Provider | Input | Cached input | Output |\n| --- | --- | --- | --- | --- |\n"
    );
    for row in &card.rows {
        out.push_str(&table_row(row));
    }
    out
}

fn table_row(row: &Row) -> String {
    let model = match (&row.kind, &row.label) {
        (Kind::Promotion, Some(label)) => {
            format!("`{}`, promotion: {}", plain(&row.model), plain(label))
        }
        (Kind::Promotion, None) => format!("`{}`, promotion", plain(&row.model)),
        (Kind::List, _) => format!("`{}`", plain(&row.model)),
    };
    format!(
        "| {model} | {} | {} | {} | {} |\n",
        plain(&row.provider),
        cell(&row.input, row.kind),
        cell(&row.cached_input, row.kind),
        cell(&row.output, row.kind)
    )
}

fn thousands(value: u64) -> String {
    let digits = value.to_string();
    let mut out = String::new();
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index) % 3 == 0 {
            out.push(',');
        }
        out.push(digit);
    }
    out
}

/// What each model can do, from the adapters' own rows.
pub(crate) fn model_table() -> String {
    let mut out = String::from(
        "| Model | Context | Most output | Tools | Images | JSON schema |\n\
| --- | --- | --- | --- | --- | --- |\n",
    );
    let mut seen = Vec::new();
    for (_, row) in inference::rates::published_models() {
        if seen.contains(&row.id) {
            continue;
        }
        seen.push(row.id.clone());
        let caps = row.capabilities;
        let yes = |on: bool| if on { "Yes" } else { "No" };
        out.push_str(&format!(
            "| `{}` | {} | {} | {} | {} | {} |\n",
            plain(&row.id),
            thousands(caps.context),
            thousands(caps.max_output),
            yes(caps.tools),
            yes(caps.images),
            yes(caps.json_schema),
        ));
    }
    out
}

/// A guide's Markdown with the models page's tables and the ways to pay
/// this API takes now drawn in.
pub(crate) async fn source(app: &App, text: &str) -> String {
    let mut out = text.to_owned();
    if out.contains(RATE_CARD) {
        let card = card(app).await;
        out = out
            .replace(RATE_CARD, &rate_card(&card))
            .replace(MODEL_TABLE, &model_table());
    }
    if out.contains(TOKENS) {
        out = out.replace(TOKENS, &served(app).await);
    }
    if out.contains(crate::payments::TABLE) || out.contains(crate::payments::SENTENCE) {
        let methods = crate::payments::live(app).await;
        out = out
            .replace(crate::payments::TABLE, &crate::payments::table(&methods))
            .replace(
                crate::payments::SENTENCE,
                &crate::payments::sentence(&methods),
            );
    }
    out
}

/// `/docs/api`: every guide, in reading order.
async fn index(headers: HeaderMap) -> Response {
    let mut body = String::from(
        "<h1>API</h1><p class=\"oa-page-lead\">One API for many models: Open Responses and \
Chat Completions at <code>https://api.openagents.com/v1</code>. Beta.</p>\
<ol class=\"oa-item-list\">",
    );
    for (slug, text) in API_DOCS {
        body.push_str(&format!(
            "<li><a href=\"/docs/api/{slug}\">{}</a></li>",
            escape(&markdown::title(text, slug))
        ));
    }
    body.push_str(
        "</ol><p class=\"oa-page-meta\">Each guide is also plain Markdown: add <code>.md</code> \
to its address, or start from <a href=\"/docs/api/llms.txt\">llms.txt</a>. Every route is \
described in OpenAPI at <code>https://api.openagents.com/v1/openapi.json</code>.</p>",
    );
    UiPage::new("API")
        .breadcrumb(Breadcrumb::new("API").crumb("Docs", "/docs"))
        .section("/docs")
        .path("/docs/api")
        .scriptless()
        .content(prose(PreEscaped(body)))
        .respond(&headers)
}

/// The `llms.txt` index: every guide's Markdown address and title.
fn llms() -> String {
    let mut out = String::from(
        "# OpenAgents API\n\n> One API for many models: Open Responses and Chat Completions at \
https://api.openagents.com/v1 (also https://openagents.com/api/v1). Beta.\n\n## Guides\n\n",
    );
    for (slug, text) in API_DOCS {
        out.push_str(&format!(
            "- [{}](https://openagents.com/docs/api/{slug}.md)\n",
            markdown::title(text, slug)
        ));
    }
    out.push_str(
        "\n## Reference\n\n- [OpenAPI 3.1 description of every route](https://api.openagents.com/v1/openapi.json)\n",
    );
    out
}

fn text(body: String, kind: &'static str) -> Response {
    let mut response = (StatusCode::OK, body).into_response();
    response
        .headers_mut()
        .insert(header::CONTENT_TYPE, HeaderValue::from_static(kind));
    response
}

/// `/docs/api/{slug}`, `/docs/api/{slug}.md`, and `/docs/api/llms.txt`.
async fn guide(State(app): State<App>, Path(slug): Path<String>, headers: HeaderMap) -> Response {
    if slug == "llms.txt" {
        return text(llms(), "text/plain; charset=utf-8");
    }
    let (name, raw) = match slug.strip_suffix(".md") {
        Some(name) => (name, true),
        None => (slug.as_str(), false),
    };
    let Some(index) = API_DOCS.iter().position(|(slug, _)| *slug == name) else {
        return problem(
            &headers,
            StatusCode::NOT_FOUND,
            "Page not found",
            "No API guide has that name.",
            ("/docs/api", "API docs"),
        );
    };
    let (_, stored) = API_DOCS[index];
    let source = source(&app, stored).await;
    if raw {
        return text(source, "text/markdown; charset=utf-8");
    }
    let previous = index.checked_sub(1).map(|i| API_DOCS[i]);
    let next = API_DOCS.get(index + 1).copied();
    let content = PageColumn::new(html! {
        (MarkdownRoot::new(PreEscaped(markdown::render_document(&source))))
        nav.oa-page-actions aria-label="More API docs" {
            (action_link("API docs", "/docs/api"))
            @if let Some((name, text)) = previous {
                (action_link(
                    &format!("\u{2190} {}", markdown::title(text, name)),
                    &format!("/docs/api/{name}"),
                ))
            }
            @if let Some((name, text)) = next {
                (action_link(
                    &format!("{} \u{2192}", markdown::title(text, name)),
                    &format!("/docs/api/{name}"),
                ))
            }
        }
    });
    let title = markdown::title(stored, name);
    UiPage::new(title.clone())
        .breadcrumb(
            Breadcrumb::new(title)
                .crumb("Docs", "/docs")
                .crumb("API", "/docs/api"),
        )
        .section("/docs")
        .path(format!("/docs/api/{name}"))
        .description(crate::agent_ready::summary(stored))
        .scriptless()
        .content(content)
        .respond(&headers)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_guide_has_a_title_and_the_models_page_has_its_tables() {
        for (slug, text) in API_DOCS {
            assert!(text.starts_with("# "), "{slug} has a title");
        }
        let models = API_DOCS.iter().find(|(slug, _)| *slug == "models").unwrap();
        assert!(models.1.contains(RATE_CARD) && models.1.contains(MODEL_TABLE));
    }

    #[test]
    fn the_rate_card_shows_list_margin_and_price_on_every_row() {
        let card = Card::published(Some(&inference::rates::SatsRate {
            usd_per_btc: 100_000,
            as_of: "2026-10-09".into(),
        }));
        let table = rate_card(&card);
        assert!(table.contains("plus our 5%"), "{table}");
        assert!(table.contains("$100,000 per bitcoin"), "{table}");
        assert!(
            table.contains(
                "| `zai/glm-5.3-flash` | Z.ai | $0.15 + $0.0075 = **$0.1575** (158 sats) |"
            ),
            "{table}"
        );
        assert_eq!(table.lines().count(), 4 + card.rows.len());
        assert!(!table.to_lowercase().contains("stripe"));
        assert_eq!(thousands(1_048_576), "1,048,576");
        assert_eq!(thousands(400), "400");
    }
}
