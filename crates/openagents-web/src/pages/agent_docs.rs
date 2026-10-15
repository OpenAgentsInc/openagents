//! The docs for agents, and for the people who build them: `/docs/agents`
//! and its pages, each also served as Markdown at
//! `/docs/agents/{slug}.md`, with an `llms.txt` index at
//! `/docs/agents/llms.txt`.
//!
//! The pages are Markdown in `content/docs/agents/`, grouped into a start
//! page, task tutorials, and reference. Every endpoint, field, and command
//! they name is one the site, the gateway, or the `openagents` command
//! serves today; the endpoint catalog is checked against the OpenAPI
//! description in this module's tests. The site's `llms.txt`,
//! `llms-full.txt`, sitemap, Markdown twins, and docs MCP server list them
//! with the other guides (`crate::agent_ready`, `crate::docs_mcp`).

use axum::Router;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use maud::{PreEscaped, html};
use openagents_ui::content::{MarkdownRoot, PageColumn};
use openagents_ui::shell::Breadcrumb;

use crate::App;
use crate::layout::escape;
use crate::markdown;
use crate::ui_page::{UiPage, action_link, problem, prose};

/// The pages, by slug, in reading order.
pub(crate) const AGENT_DOCS: [(&str, &str); 16] = [
    (
        "start-here",
        include_str!("../../content/docs/agents/start-here.md"),
    ),
    (
        "sign-up",
        include_str!("../../content/docs/agents/sign-up.md"),
    ),
    (
        "buy-credits",
        include_str!("../../content/docs/agents/buy-credits.md"),
    ),
    (
        "plan-and-usage",
        include_str!("../../content/docs/agents/plan-and-usage.md"),
    ),
    (
        "chat-and-stream",
        include_str!("../../content/docs/agents/chat-and-stream.md"),
    ),
    (
        "decisions",
        include_str!("../../content/docs/agents/decisions.md"),
    ),
    (
        "projects",
        include_str!("../../content/docs/agents/projects.md"),
    ),
    (
        "coder-headless",
        include_str!("../../content/docs/agents/coder-headless.md"),
    ),
    ("mcp", include_str!("../../content/docs/agents/mcp.md")),
    (
        "webmcp",
        include_str!("../../content/docs/agents/webmcp.md"),
    ),
    ("x402", include_str!("../../content/docs/agents/x402.md")),
    (
        "endpoints",
        include_str!("../../content/docs/agents/endpoints.md"),
    ),
    (
        "errors",
        include_str!("../../content/docs/agents/errors.md"),
    ),
    (
        "scopes",
        include_str!("../../content/docs/agents/scopes.md"),
    ),
    (
        "webhooks",
        include_str!("../../content/docs/agents/webhooks.md"),
    ),
    ("cli", include_str!("../../content/docs/agents/cli.md")),
];

/// The index's groups: each one's title, a line saying what it covers,
/// and the slug of its first page. A group runs until the next one's
/// first page, in [`AGENT_DOCS`] order.
pub(crate) const GROUPS: [(&str, &str, &str); 3] = [
    (
        "Start here",
        "What we offer an agent, and how it signs itself up.",
        "start-here",
    ),
    (
        "Tutorials",
        "One task each, with the exact requests and commands.",
        "sign-up",
    ),
    (
        "Reference",
        "Every endpoint, error, scope, and command.",
        "endpoints",
    ),
];

/// What the agent docs are, in one line.
pub(crate) const LEAD: &str = "Docs for agents that use OpenAgents, and for the people who \
build them: sign up, pay in bitcoin, call the API, and use our MCP server and command.";

pub(crate) fn routes() -> Router<App> {
    Router::new()
        .route("/docs/agents", get(index))
        .route("/docs/agents/{slug}", get(page))
}

/// The group a page sits in.
pub(crate) fn group_of(slug: &str) -> Option<&'static str> {
    let at = AGENT_DOCS.iter().position(|(name, _)| *name == slug)?;
    GROUPS
        .iter()
        .filter_map(|(title, _, first)| {
            AGENT_DOCS
                .iter()
                .position(|(name, _)| name == first)
                .map(|start| (start, *title))
        })
        .filter(|(start, _)| *start <= at)
        .max_by_key(|(start, _)| *start)
        .map(|(_, title)| title)
}

/// `/docs/agents`: every page, in reading order, under its group.
async fn index(headers: HeaderMap) -> Response {
    index_page(&headers)
}

/// The agent docs index for `headers`.
pub(crate) fn index_page(headers: &HeaderMap) -> Response {
    let mut body = format!(
        "<h1>For agents</h1><p class=\"oa-page-lead\">{}</p>",
        escape(LEAD)
    );
    let mut open = false;
    for (slug, source) in AGENT_DOCS {
        if let Some((title, lead, _)) = GROUPS.iter().find(|(_, _, first)| *first == slug) {
            if open {
                body.push_str("</ol>");
            }
            body.push_str(&format!(
                "<h2 id=\"{}\">{}</h2><p class=\"oa-page-meta\">{}</p><ol class=\"oa-item-list\">",
                crate::pages::content::section_anchor(title),
                escape(title),
                escape(lead)
            ));
            open = true;
        }
        body.push_str(&format!(
            "<li><a href=\"/docs/agents/{slug}\">{}</a></li>",
            escape(&markdown::title(source, slug))
        ));
    }
    if open {
        body.push_str("</ol>");
    }
    body.push_str(
        "<p class=\"oa-page-meta\">Each page is also plain Markdown: add <code>.md</code> to its \
address, or start from <a href=\"/docs/agents/llms.txt\">llms.txt</a>.</p>",
    );
    UiPage::new("For agents")
        .breadcrumb(Breadcrumb::new("For agents").crumb("Docs", "/docs"))
        .section("/docs")
        .path("/docs/agents")
        .description(LEAD)
        .scriptless()
        .content(prose(PreEscaped(body)))
        .respond(headers)
}

/// The agent docs as Markdown list items, grouped, with each page's
/// summary. `absolute` writes full addresses.
pub(crate) fn list(absolute: bool) -> String {
    let base = if absolute {
        crate::agent_ready::SITE
    } else {
        ""
    };
    let mut out = String::new();
    for (slug, source) in AGENT_DOCS {
        if let Some((title, lead, _)) = GROUPS.iter().find(|(_, _, first)| *first == slug) {
            out.push_str(&format!("\n### {title}\n\n{lead}\n\n"));
        }
        out.push_str(&format!(
            "- [{}]({base}/docs/agents/{slug}.md): {}\n",
            markdown::title(source, slug),
            crate::agent_ready::summary(source)
        ));
    }
    out
}

/// `/docs/agents.md`: the index as Markdown.
pub(crate) fn index_markdown() -> String {
    format!("# For agents\n\n{LEAD}\n{}", list(false))
}

/// `/docs/agents/llms.txt`.
fn llms() -> String {
    format!(
        "# OpenAgents for agents\n\n> {LEAD}\n{}\n## More\n\n\
- [Every page on the site]({site}/llms.txt)\n\
- [OpenAPI 3.1 description of the API](https://api.openagents.com/v1/openapi.json)\n",
        list(true),
        site = crate::agent_ready::SITE
    )
}

fn text(body: String, kind: &'static str) -> Response {
    let mut response = (StatusCode::OK, body).into_response();
    response
        .headers_mut()
        .insert(header::CONTENT_TYPE, HeaderValue::from_static(kind));
    response
}

/// `/docs/agents/{slug}`, `/docs/agents/{slug}.md`, and
/// `/docs/agents/llms.txt`.
async fn page(State(app): State<App>, Path(slug): Path<String>, headers: HeaderMap) -> Response {
    if slug == "llms.txt" {
        return text(llms(), "text/plain; charset=utf-8");
    }
    let (name, raw) = match slug.strip_suffix(".md") {
        Some(name) => (name, true),
        None => (slug.as_str(), false),
    };
    let Some(index) = AGENT_DOCS.iter().position(|(slug, _)| *slug == name) else {
        return problem(
            &headers,
            StatusCode::NOT_FOUND,
            "Page not found",
            "No page for agents has that name.",
            ("/docs/agents", "For agents"),
        );
    };
    let (_, stored) = AGENT_DOCS[index];
    let source = crate::pages::api_docs::source(&app, stored).await;
    if raw {
        return crate::agent_ready::doc_md(&format!("/docs/agents/{name}"), &source, name);
    }
    page_view(&headers, index, &source)
}

/// The page at `index` in [`AGENT_DOCS`], drawn from `source`.
pub(crate) fn page_view(headers: &HeaderMap, index: usize, source: &str) -> Response {
    let (name, stored) = AGENT_DOCS[index];
    let previous = index.checked_sub(1).map(|i| AGENT_DOCS[i]);
    let next = AGENT_DOCS.get(index + 1).copied();
    let content = PageColumn::new(html! {
        (MarkdownRoot::new(PreEscaped(markdown::render_document(source))))
        nav.oa-page-actions aria-label="More docs for agents" {
            (action_link("For agents", "/docs/agents"))
            @if let Some((name, text)) = previous {
                (action_link(
                    &format!("\u{2190} {}", markdown::title(text, name)),
                    &format!("/docs/agents/{name}"),
                ))
            }
            @if let Some((name, text)) = next {
                (action_link(
                    &format!("{} \u{2192}", markdown::title(text, name)),
                    &format!("/docs/agents/{name}"),
                ))
            }
        }
    });
    let title = markdown::title(stored, name);
    let mut trail = Breadcrumb::new(title.clone())
        .crumb("Docs", "/docs")
        .crumb("For agents", "/docs/agents");
    if let Some(group) = group_of(name) {
        trail = trail.crumb(
            group,
            format!(
                "/docs/agents#{}",
                crate::pages::content::section_anchor(group)
            ),
        );
    }
    UiPage::new(title)
        .breadcrumb(trail)
        .section("/docs")
        .path(format!("/docs/agents/{name}"))
        .description(crate::agent_ready::summary(stored))
        .scriptless()
        .content(content)
        .respond(headers)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every page has a title, the groups start at pages in reading order,
    /// and every page resolves to its group.
    #[test]
    fn the_groups_cover_the_pages_in_order() {
        assert_eq!(GROUPS[0].2, AGENT_DOCS[0].0);
        let mut last = None;
        for (title, _, first) in GROUPS {
            let index = AGENT_DOCS.iter().position(|(slug, _)| *slug == first);
            assert!(index.is_some(), "{title} starts at {first}");
            assert!(index > last, "{title} is out of order");
            last = index;
        }
        for (slug, source) in AGENT_DOCS {
            assert!(source.starts_with("# "), "{slug} has a title");
            assert!(group_of(slug).is_some(), "{slug} has a group");
        }
        assert_eq!(group_of("cli"), Some("Reference"));
        assert_eq!(group_of("x402"), Some("Tutorials"));
    }

    /// The endpoints the agent docs name, as `METHOD /path` in code spans.
    fn named_endpoints() -> Vec<(String, String, String)> {
        let mut found = Vec::new();
        for (slug, source) in AGENT_DOCS {
            for span in source.split('`').skip(1).step_by(2) {
                let mut words = span.split_whitespace();
                let (Some(method), Some(path), None) = (words.next(), words.next(), words.next())
                else {
                    continue;
                };
                if !["GET", "POST", "PUT", "PATCH", "DELETE"].contains(&method)
                    || !path.starts_with('/')
                {
                    continue;
                }
                let path = path.split('?').next().unwrap_or(path).to_owned();
                found.push((slug.to_owned(), method.to_owned(), path));
            }
        }
        found
    }

    /// Every `METHOD /path` the agent docs name is a route the code mounts:
    /// a route in the gateway's route table (`crates/gateway/src/
    /// audience.rs`, the table the OpenAPI document is made from), or a
    /// route this website registers. A docs page can't name an endpoint
    /// nobody serves.
    #[test]
    fn every_named_endpoint_is_a_mounted_route() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let gateway = std::fs::read_to_string(root.join("../gateway/src/audience.rs"))
            .expect("the gateway's route table");
        let mut site = String::new();
        fn read(dir: &std::path::Path, out: &mut String) {
            for entry in std::fs::read_dir(dir).expect("the source directory") {
                let path = entry.expect("an entry").path();
                let name = path.file_name().unwrap().to_string_lossy().into_owned();
                if path.is_dir() {
                    if name != "tests" {
                        read(&path, out);
                    }
                } else if name.ends_with(".rs")
                    && name != "tests.rs"
                    && !name.ends_with("_tests.rs")
                {
                    out.push_str(&std::fs::read_to_string(&path).unwrap_or_default());
                }
            }
        }
        read(&root.join("src"), &mut site);
        let endpoints = named_endpoints();
        assert!(endpoints.len() > 40, "{}", endpoints.len());
        let mut missing = Vec::new();
        for (slug, method, path) in endpoints {
            // `/api/v1/...` is the public API on the website's address.
            let api = path.strip_prefix("/api").unwrap_or(&path);
            let quoted = format!("\"{api}\"");
            let mounted =
                gateway.contains(&format!("route({quoted}, Public")) || site.contains(&quoted);
            if !mounted {
                missing.push(format!("{slug}: {method} {path}"));
            }
        }
        assert!(
            missing.is_empty(),
            "endpoints no route serves:\n{}",
            missing.join("\n")
        );
    }

    /// The catalog's credential labels and hosts are the ones it defines.
    #[test]
    fn the_endpoint_catalog_names_a_host_for_every_row() {
        let (_, catalog) = AGENT_DOCS
            .iter()
            .find(|(slug, _)| *slug == "endpoints")
            .unwrap();
        for row in catalog.lines().filter(|line| line.starts_with("| `")) {
            let cells: Vec<&str> = row.split('|').map(str::trim).collect();
            if cells.len() < 5 || !cells[1].contains(" /") {
                continue;
            }
            assert!(
                [
                    "`api.openagents.com`",
                    "`openagents.com`",
                    "`openagents.com/api`"
                ]
                .contains(&cells[2]),
                "{row}"
            );
        }
    }
}
