//! The SDK docs at `/docs/sdk` (#12007): what you can do from your own
//! code, generated from the contracts and the SDK's source.
//!
//! - `/docs/sdk`: what the SDK is, adding it, signing in, and a first call.
//! - `/docs/sdk/api/{area}`: the HTTP API reference, one page per area,
//!   drawn at request time from the committed OpenAPI contracts
//!   ([`contract`]). Each operation has a stable anchor, its operation ID.
//! - `/docs/sdk/rust` and `/docs/sdk/rust/{module}`: the Rust SDK reference,
//!   from `docs/sdk/sdk-reference.json`, which the SDK crate's tests write
//!   from its source ([`rust`]).
//! - `/docs/sdk/search?q=...`: a search across both, answered by the server
//!   so the pages run no script.

mod contract;
mod rust;
mod schema;
#[cfg(test)]
mod tests;

use std::collections::HashMap;

use axum::Router;
use axum::extract::{Path, Query};
use axum::http::{HeaderMap, StatusCode};
use axum::response::Response;
use axum::routing::get;
use maud::{Markup, PreEscaped, html};
use openagents_ui::forms::{Input, InputType};
use openagents_ui::shell::Breadcrumb;
use serde_json::Value;

use crate::App;
use crate::markdown;
use crate::ui_page::{UiPage, action_link, problem, wide_prose};

use contract::{AREAS, Area, Operation};

/// The public API's address.
const API: &str = "https://api.openagents.com";
/// The first call, compiled as the SDK crate's `first_call` example.
const FIRST_CALL: &str = include_str!("../../../openagents-sdk/examples/first_call.rs");
/// Where the source lives, for links to contract files.
const REPOSITORY: &str = "https://github.com/OpenAgentsInc/openagents/blob/main";

pub(crate) fn routes() -> Router<App> {
    Router::new()
        .route("/docs/sdk", get(landing))
        .route("/docs/sdk/search", get(search))
        .route("/docs/sdk/api/{area}", get(area))
        .route("/docs/sdk/rust", get(rust_index))
        .route("/docs/sdk/rust/{module}", get(rust_module))
}

/// Every page address, for the sitemap.
pub(crate) fn paths() -> Vec<String> {
    let mut out = vec!["/docs/sdk".to_owned(), "/docs/sdk/rust".to_owned()];
    out.extend(AREAS.iter().map(|a| format!("/docs/sdk/api/{}", a.slug)));
    out.extend(rust::reference().modules.iter().map(rust::Module::href));
    out
}

fn page(title: &str, path: &str, trail: Breadcrumb, description: &str, body: Markup) -> UiPage {
    UiPage::new(title.to_owned())
        .breadcrumb(trail)
        .section("/docs")
        .path(path.to_owned())
        .description(description.to_owned())
        .scriptless()
        .content(wide_prose(body))
}

fn trail(title: &str) -> Breadcrumb {
    Breadcrumb::new(title.to_owned())
        .crumb("Docs", "/docs")
        .crumb("SDK", "/docs/sdk")
}

fn md(text: &str) -> PreEscaped<String> {
    PreEscaped(markdown::render(text))
}

fn code(language: &str, text: &str) -> PreEscaped<String> {
    PreEscaped(markdown::render_document(&format!(
        "```{language}\n{}\n```",
        text.trim_end()
    )))
}

fn search_form(query: &str) -> Markup {
    html! {
        form action="/docs/sdk/search" method="get" role="search" {
            (Input::new("q")
                .input_type(InputType::Search)
                .value(query)
                .placeholder("Search the SDK and API")
                .aria_label("Search the SDK and API"))
        }
    }
}

// ---------------------------------------------------------------------
// The landing page

async fn landing(headers: HeaderMap) -> Response {
    landing_page(&headers)
}

pub(crate) fn landing_page(headers: &HeaderMap) -> Response {
    let operations = contract::operations();
    let modules = &rust::reference().modules;
    let body = html! {
        h1 { "SDK" }
        p.oa-page-lead {
            "Everything you can do with OpenAgents from your own code: the HTTP API at "
            code { (API) "/v1" } " and the Rust SDK that calls it."
        }
        (search_form(""))
        h2 id="add" { "Add the SDK" }
        p { "The Rust SDK is the " code { "openagents-sdk" } " crate in our repository. \
    Add it to your " code { "Cargo.toml" } ":" }
        (code("toml", "[dependencies]\nopenagents-sdk = { git = \"https://github.com/OpenAgentsInc/openagents\" }\ntokio = { version = \"1\", features = [\"macros\", \"rt\"] }"))
        p { "Any language can call the HTTP API directly. Each operation below shows its \
    request and a " code { "curl" } " example." }
        h2 id="sign-in" { "Sign in" }
        p { "A call that acts for your account carries a credential in the "
            code { "Authorization: Bearer" } " header: an app session or a key you created \
    for your account. The SDK takes the credential you give it; it never reads one from your \
    computer on its own. "
            a href="/auth.md" { "Read how to get a credential" } "." }
        h2 id="first-call" { "A first call" }
        p { "Sign in with a credential and read your account:" }
        (code("rust", FIRST_CALL))
        p { "The same call over HTTP:" }
        (code("sh", &format!("curl {API}/v1/account \\\n  -H \"Authorization: Bearer $OPENAGENTS_TOKEN\"")))
        h2 id="api" { "API reference" }
        p.oa-page-meta { (operations.len()) " operations, generated from our OpenAPI contracts." }
        ol.oa-item-list {
            @for area in AREAS {
                @let count = contract::in_area(area.slug).count();
                @if count > 0 {
                    li { a href=(format!("/docs/sdk/api/{}", area.slug)) { (area.title) }
                        " \u{2014} " (area.lead) }
                }
            }
        }
        p.oa-page-meta { "Calling models through Open Responses or Chat Completions has its \
    own guides: " a href="/docs/api" { "API docs" } "." }
        h2 id="rust" { "Rust SDK reference" }
        p.oa-page-meta { "Every public module, type, and function, generated from the SDK's source." }
        ol.oa-item-list {
            @for module in modules.iter().filter(|m| !m.generated) {
                li { a href=(module.href()) { code { (module.rust_path()) } }
                    @let summary = module.summary();
                    @if !summary.is_empty() { " \u{2014} " (summary) } }
            }
        }
        h2 id="contracts" { "The contracts" }
        p { "These pages are drawn from the same files the API is tested against. Download them \
    to generate a client in another language:" }
        ul {
            @for source in contract::sources() {
                li { a href=(format!("{REPOSITORY}/{}", source.file)) { (source.title) } " (" code { (source.file) } ")" }
            }
        }
    };
    UiPage::new("SDK")
        .breadcrumb(Breadcrumb::new("SDK").crumb("Docs", "/docs"))
        .section("/docs")
        .path("/docs/sdk")
        .description(
            "Everything you can do with OpenAgents from your own code: the HTTP API and the \
Rust SDK, with every operation and function described.",
        )
        .scriptless()
        .content(wide_prose(body))
        .respond(headers)
}

// ---------------------------------------------------------------------
// The API reference

async fn area(Path(slug): Path<String>, headers: HeaderMap) -> Response {
    area_page(&headers, &slug)
}

pub(crate) fn area_page(headers: &HeaderMap, slug: &str) -> Response {
    let Some(area) = AREAS.iter().find(|a| a.slug == slug) else {
        return missing(headers);
    };
    let operations: Vec<&Operation> = contract::in_area(slug).collect();
    let index = AREAS.iter().position(|a| a.slug == slug).unwrap_or(0);
    let neighbours = |step: isize| {
        let mut at = index as isize + step;
        while at >= 0 && (at as usize) < AREAS.len() {
            let candidate = &AREAS[at as usize];
            if contract::in_area(candidate.slug).next().is_some() {
                return Some(candidate);
            }
            at += step;
        }
        None
    };
    let body = html! {
        h1 { (area.title) }
        p.oa-page-lead { (area.lead) }
        (search_form(""))
        @if operations.is_empty() {
            p { "This area has no public operations yet." }
        }
        table {
            thead { tr { th { "Operation" } th { "Request" } } }
            tbody {
                @for op in &operations {
                    tr {
                        td { a href=(format!("#{}", op.anchor())) { (op.title()) } }
                        td { code { (op.method) " " (op.path) } }
                    }
                }
            }
        }
        @for op in &operations { (operation(op)) }
        (area_nav(neighbours(-1), neighbours(1)))
    };
    page(
        area.title,
        &format!("/docs/sdk/api/{}", area.slug),
        trail(area.title),
        area.lead,
        body,
    )
    .respond(headers)
}

fn area_nav(previous: Option<&Area>, next: Option<&Area>) -> Markup {
    html! {
        nav.oa-page-actions aria-label="More SDK docs" {
            (action_link("SDK docs", "/docs/sdk"))
            @if let Some(area) = previous {
                (action_link(&format!("\u{2190} {}", area.title), &format!("/docs/sdk/api/{}", area.slug)))
            }
            @if let Some(area) = next {
                (action_link(&format!("{} \u{2192}", area.title), &format!("/docs/sdk/api/{}", area.slug)))
            }
        }
    }
}

/// The security an operation needs, as sentences.
fn security(op: &Operation) -> Markup {
    let document = op.document();
    let requirements = op
        .value
        .get("security")
        .or_else(|| document.get("security"))
        .and_then(Value::as_array);
    let Some(requirements) = requirements else {
        return html! {};
    };
    if requirements.is_empty()
        || requirements
            .iter()
            .all(|r| r.as_object().is_some_and(|o| o.is_empty()))
    {
        return html! { p { strong { "Sign-in: " } "none needed." } };
    }
    let names: Vec<&str> = requirements
        .iter()
        .filter_map(Value::as_object)
        .flat_map(|o| o.keys().map(String::as_str))
        .collect();
    html! {
        p { strong { "Sign-in: " } "a bearer credential in the " code { "Authorization" } " header." }
        @for name in names {
            @let scheme = &document["components"]["securitySchemes"][name];
            @if let Some(text) = scheme["description"].as_str() {
                (md(&format!("`{name}`: {text}")))
            }
        }
    }
}

fn field_table(document: &Value, schema: &Value) -> Markup {
    let fields = schema::fields(document, schema);
    if fields.is_empty() {
        return html! {};
    }
    html! {
        table {
            thead { tr { th { "Field" } th { "Type" } th { "Required" } th { "Description" } } }
            tbody {
                @for field in &fields {
                    tr {
                        td { code { (field.name) } }
                        td { (field.kind) }
                        td { @if field.required { "Yes" } @else { "No" } }
                        td { (md(&field.description)) }
                    }
                }
            }
        }
        @if fields.len() >= schema::MAX_FIELDS {
            p.oa-page-meta { "The first " (schema::MAX_FIELDS) " fields are shown; the contract has the rest." }
        }
    }
}

/// The JSON schema of a request or response body and its media type.
fn body_schema(value: &Value) -> Option<(&str, &Value)> {
    let content = value["content"].as_object()?;
    content
        .iter()
        .find(|(kind, _)| kind.contains("json"))
        .or_else(|| content.iter().next())
        .map(|(kind, media)| (kind.as_str(), &media["schema"]))
}

fn schema_block(op: &Operation, schema: &Value) -> Markup {
    let source = &contract::sources()[op.source];
    if let Some(file) = contract::external(source, schema) {
        return html! {
            p { "Schema: " a href=(format!("{REPOSITORY}/{file}")) { code { (file) } } }
        };
    }
    let document = op.document();
    html! {
        p { "Type: " (schema::kind(document, schema)) }
        (field_table(document, schema))
    }
}

fn operation(op: &Operation) -> Markup {
    let document = op.document();
    let parameters: Vec<&Value> = op.value["parameters"]
        .as_array()
        .map(|list| list.iter().map(|p| contract::deref(document, p)).collect())
        .unwrap_or_default();
    let request = op
        .value
        .get("requestBody")
        .map(|b| contract::deref(document, b));
    let mut responses: Vec<(String, &Value)> = op.value["responses"]
        .as_object()
        .map(|r| {
            r.iter()
                .map(|(status, value)| (status.clone(), contract::deref(document, value)))
                .collect()
        })
        .unwrap_or_default();
    responses.sort_by(|a, b| a.0.cmp(&b.0));
    let (ok, errors): (Vec<_>, Vec<_>) = responses.into_iter().partition(|(status, _)| {
        status.starts_with('1') || status.starts_with('2') || status.starts_with('3')
    });
    // Statuses with the same description and body share one row.
    let mut grouped: Vec<(Vec<String>, &Value)> = Vec::new();
    for (status, value) in errors {
        match grouped.iter_mut().find(|(_, v)| {
            v["description"] == value["description"]
                && body_schema(v).map(|b| b.1) == body_schema(value).map(|b| b.1)
        }) {
            Some((statuses, _)) => statuses.push(status),
            None => grouped.push((vec![status], value)),
        }
    }
    let callers = rust::callers(op);
    html! {
        section id=(op.anchor()) {
            h2 { (op.title()) }
            p { code { (op.method) " " (op.path) } " \u{b7} " a href=(format!("#{}", op.anchor())) { code { (op.id) } } }
            @if !op.description().is_empty() { (md(op.description())) }
            (security(op))
            @if !parameters.is_empty() {
                h3 { "Parameters" }
                table {
                    thead { tr { th { "Name" } th { "In" } th { "Type" } th { "Required" } th { "Description" } } }
                    tbody {
                        @for p in &parameters {
                            tr {
                                td { code { (p["name"].as_str().unwrap_or_default()) } }
                                td { (p["in"].as_str().unwrap_or_default()) }
                                td { (schema::kind(document, &p["schema"])) }
                                td { @if p["required"].as_bool().unwrap_or(false) { "Yes" } @else { "No" } }
                                td { (md(p["description"].as_str().unwrap_or_default())) }
                            }
                        }
                    }
                }
            }
            @if let Some(request) = request {
                h3 { "Request body" }
                @if let Some(text) = request["description"].as_str() { (md(text)) }
                @if let Some((kind, schema)) = body_schema(request) {
                    p { "Content type: " code { (kind) } }
                    (schema_block(op, schema))
                }
            }
            @if !ok.is_empty() {
                h3 { "Response" }
                @for (status, value) in &ok {
                    p { strong { (status) } " \u{2014} " (value["description"].as_str().unwrap_or_default()) }
                    @if let Some((kind, schema)) = body_schema(value) {
                        @if !kind.contains("json") { p { "Content type: " code { (kind) } } }
                        (schema_block(op, schema))
                    }
                }
            }
            @if !grouped.is_empty() {
                h3 { "Errors" }
                table {
                    thead { tr { th { "Status" } th { "Meaning" } th { "Body" } } }
                    tbody {
                        @for (statuses, value) in &grouped {
                            tr {
                                td { (statuses.join(", ")) }
                                td { (md(value["description"].as_str().unwrap_or_default())) }
                                td { @if let Some((_, schema)) = body_schema(value) { (schema::kind(document, schema)) } }
                            }
                        }
                    }
                }
            }
            h3 { "Example" }
            (code("sh", &example(op, &parameters, request)))
            @if !callers.is_empty() {
                h3 { "In the Rust SDK" }
                ul { @for (label, href) in &callers { li { a href=(href) { code { (label) } } } } }
            }
        }
    }
}

/// A `curl` request for `op`, with placeholders for its path parameters
/// and a body built from its request schema.
fn example(op: &Operation, parameters: &[&Value], request: Option<&Value>) -> String {
    let document = op.document();
    let mut url = format!("{API}{}", op.path);
    let query: Vec<String> = parameters
        .iter()
        .filter(|p| p["in"] == "query" && p["required"].as_bool().unwrap_or(false))
        .filter_map(|p| p["name"].as_str())
        .map(|name| format!("{name}=..."))
        .collect();
    if !query.is_empty() {
        url.push('?');
        url.push_str(&query.join("&"));
    }
    let mut lines = vec![if op.method == "GET" {
        format!("curl '{url}'")
    } else {
        format!("curl -X {} '{url}'", op.method)
    }];
    let signed_in = op
        .value
        .get("security")
        .or_else(|| document.get("security"))
        .and_then(Value::as_array)
        .is_some_and(|r| {
            r.iter()
                .any(|r| r.as_object().is_some_and(|o| !o.is_empty()))
        });
    if signed_in {
        lines.push("-H \"Authorization: Bearer $OPENAGENTS_TOKEN\"".to_owned());
    }
    for p in parameters
        .iter()
        .filter(|p| p["in"] == "header" && p["required"].as_bool().unwrap_or(false))
    {
        if let Some(name) = p["name"].as_str() {
            lines.push(format!("-H '{name}: ...'"));
        }
    }
    if let Some((kind, schema)) = request.and_then(body_schema) {
        let given = request.and_then(|r| r["content"][kind].get("example"));
        if kind.contains("json")
            && (given.is_some()
                || contract::external(&contract::sources()[op.source], schema).is_none())
        {
            let body = given
                .cloned()
                .unwrap_or_else(|| schema::example(document, schema));
            let text = serde_json::to_string_pretty(&body)
                .unwrap_or_default()
                .replace('\'', "'\\''");
            lines.push(format!("-H 'Content-Type: {kind}'"));
            lines.push(format!("-d '{text}'"));
        } else if kind.contains("json") {
            lines.push(format!("-H 'Content-Type: {kind}'"));
            lines.push("-d @request.json".to_owned());
        } else {
            lines.push(format!("-H 'Content-Type: {kind}'"));
            lines.push("--data-binary @file".to_owned());
        }
    }
    lines.join(" \\\n  ")
}

// ---------------------------------------------------------------------
// The Rust SDK reference

async fn rust_index(headers: HeaderMap) -> Response {
    rust_index_page(&headers)
}

pub(crate) fn rust_index_page(headers: &HeaderMap) -> Response {
    let reference = rust::reference();
    let body = html! {
        h1 { "Rust SDK" }
        p.oa-page-lead { "The " code { "openagents-sdk" } " crate: " (reference.description) }
        (search_form(""))
        table {
            thead { tr { th { "Module" } th { "What it does" } } }
            tbody {
                @for module in reference.modules.iter().filter(|m| !m.generated) {
                    tr {
                        td { a href=(module.href()) { code { (module.rust_path()) } } }
                        td { (module.summary()) }
                    }
                }
            }
        }
        @let generated: Vec<_> = reference.modules.iter().filter(|m| m.generated).collect();
        @if !generated.is_empty() {
            p.oa-page-meta { "The Computers wire types in "
                @for (i, module) in generated.iter().enumerate() { @if i > 0 { ", " } code { (module.rust_path()) } }
                " are generated from the Computers contract; the "
                a href="/docs/sdk/api/computers" { "Computers API reference" } " describes each of them." }
        }
        (area_nav(None, None))
    };
    page(
        "Rust SDK",
        "/docs/sdk/rust",
        trail("Rust SDK"),
        "Every public module, type, and function of the openagents-sdk crate.",
        body,
    )
    .respond(headers)
}

async fn rust_module(Path(slug): Path<String>, headers: HeaderMap) -> Response {
    rust_module_page(&headers, &slug)
}

fn calls_list(calls: &[rust::Call]) -> Markup {
    html! {
        @if !calls.is_empty() {
            p.oa-page-meta { "Calls "
                @for (i, call) in calls.iter().enumerate() {
                    @if i > 0 { ", " }
                    @match contract::operation_for(&call.method, &call.path) {
                        Some(op) => a href=(op.href()) { code { (op.method) " " (op.path) } },
                        None => code { (call.method) " " (call.path) },
                    }
                }
            }
        }
    }
}

pub(crate) fn rust_module_page(headers: &HeaderMap, slug: &str) -> Response {
    let reference = rust::reference();
    let Some(module) = reference
        .modules
        .iter()
        .find(|m| m.slug() == slug && !m.generated)
    else {
        return missing(headers);
    };
    let path = module.rust_path();
    let body = html! {
        h1 { code { (path) } }
        @if !module.doc.is_empty() { (md(&module.doc)) }
        (search_form(""))
        @if !module.items.is_empty() {
            table {
                thead { tr { th { "Item" } th { "What it does" } } }
                tbody {
                    @for item in &module.items {
                        tr { td { a href=(format!("#{}", item.name)) { code { (item.name) } } " " (item.kind) }
                             td { (rust::first_sentence(&item.doc)) } }
                    }
                }
            }
        }
        @for item in &module.items {
            section id=(item.name) {
                h2 { code { (item.name) } }
                @if let Some(feature) = &item.feature { p.oa-page-meta { "Needs the " code { (feature) } " feature." } }
                (code("rust", &item.declaration))
                @if !item.doc.is_empty() { (md(&item.doc)) }
                (calls_list(&item.calls))
                @for method in &item.methods {
                    section id=(format!("{}.{}", item.name, method.name)) {
                        h3 { code { (item.name) "::" (method.name) } }
                        @if let Some(feature) = &method.feature { p.oa-page-meta { "Needs the " code { (feature) } " feature." } }
                        (code("rust", &method.declaration))
                        @if !method.doc.is_empty() { (md(&method.doc)) }
                        (calls_list(&method.calls))
                    }
                }
            }
        }
        @if !module.reexports.is_empty() {
            h2 id="reexports" { "Re-exported from other crates" }
            ul {
                @for reexport in &module.reexports {
                    li { code { (reexport.name) } " from " code { (reexport.from) } }
                }
            }
        }
        nav.oa-page-actions aria-label="More SDK docs" {
            (action_link("Rust SDK", "/docs/sdk/rust"))
            (action_link("SDK docs", "/docs/sdk"))
        }
    };
    let description = module.summary();
    page(
        &path,
        &module.href(),
        trail(&path).crumb("Rust SDK", "/docs/sdk/rust"),
        &description,
        body,
    )
    .respond(headers)
}

// ---------------------------------------------------------------------
// Search

/// One search hit: its title, address, kind, and a line of context.
pub(crate) struct Hit {
    pub title: String,
    pub href: String,
    pub kind: String,
    pub context: String,
    score: usize,
}

fn score(terms: &[String], title: &str, rest: &str) -> Option<usize> {
    let title = title.to_lowercase();
    let rest = rest.to_lowercase();
    let mut total = 0;
    for term in terms {
        if title.contains(term.as_str()) {
            total += 3;
        } else if rest.contains(term.as_str()) {
            total += 1;
        } else {
            return None;
        }
    }
    Some(total)
}

/// What `query` finds across the API and the Rust SDK, best first.
pub(crate) fn find(query: &str) -> Vec<Hit> {
    let terms: Vec<String> = query
        .split_whitespace()
        .map(str::to_lowercase)
        .filter(|t| !t.is_empty())
        .take(8)
        .collect();
    if terms.is_empty() {
        return Vec::new();
    }
    let mut hits = Vec::new();
    for op in contract::operations() {
        let title = format!("{} {} {}", op.title(), op.id, op.path);
        let rest = format!("{} {} {}", op.method, op.tag, op.description());
        if let Some(score) = score(&terms, &title, &rest) {
            hits.push(Hit {
                title: op.title(),
                href: op.href(),
                kind: format!("{} {}", op.method, op.path),
                context: rust::first_sentence(op.description()),
                score,
            });
        }
    }
    for module in rust::reference().modules.iter().filter(|m| !m.generated) {
        for item in &module.items {
            let name = format!("{}::{}", module.rust_path(), item.name);
            if let Some(score) = score(&terms, &name, &item.doc) {
                hits.push(Hit {
                    title: name,
                    href: format!("{}#{}", module.href(), item.name),
                    kind: format!("Rust {}", item.kind),
                    context: rust::first_sentence(&item.doc),
                    score,
                });
            }
            for method in &item.methods {
                let name = format!("{}::{}", item.name, method.name);
                if let Some(score) = score(&terms, &name, &method.doc) {
                    hits.push(Hit {
                        title: name,
                        href: format!("{}#{}.{}", module.href(), item.name, method.name),
                        kind: "Rust method".to_owned(),
                        context: rust::first_sentence(&method.doc),
                        score,
                    });
                }
            }
        }
    }
    hits.sort_by(|a, b| b.score.cmp(&a.score).then_with(|| a.title.cmp(&b.title)));
    hits.truncate(60);
    hits
}

async fn search(Query(query): Query<HashMap<String, String>>, headers: HeaderMap) -> Response {
    let text = query.get("q").map(|q| q.trim()).unwrap_or_default();
    let text: String = text.chars().take(200).collect();
    let hits = find(&text);
    let body = html! {
        h1 { "Search the SDK" }
        (search_form(&text))
        @if text.is_empty() {
            p { "Search operations, types, and functions by name or by what they do." }
        } @else if hits.is_empty() {
            p { "Nothing matches \u{201c}" (text) "\u{201d}. Try fewer or shorter words." }
        } @else {
            p.oa-page-meta { (hits.len()) " results" @if hits.len() >= 60 { " (the first 60)" } }
            ol.oa-item-list {
                @for hit in &hits {
                    li {
                        a href=(hit.href) { (hit.title) } " " code { (hit.kind) }
                        @if !hit.context.is_empty() { br; span.oa-page-meta { (hit.context) } }
                    }
                }
            }
        }
    };
    page(
        "Search the SDK",
        "/docs/sdk/search",
        trail("Search"),
        "Search the OpenAgents API and Rust SDK.",
        body,
    )
    .respond(&headers)
}

fn missing(headers: &HeaderMap) -> Response {
    problem(
        headers,
        StatusCode::NOT_FOUND,
        "Page not found",
        "No SDK page has that name.",
        ("/docs/sdk", "SDK docs"),
    )
}
