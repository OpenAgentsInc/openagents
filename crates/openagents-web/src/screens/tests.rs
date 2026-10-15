use axum::body::to_bytes;
use axum::http::{HeaderMap, StatusCode, header};
use openagents_ui::shell::Theme;

use super::*;

async fn body(response: Response) -> String {
    String::from_utf8(
        to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap()
}

/// Every id is unique and every state renders as a whole page in both
/// themes, framable by this site, with no machine talk.
#[tokio::test]
async fn every_state_renders_in_both_themes() {
    let all = screens();
    let mut ids = std::collections::BTreeSet::new();
    for screen in &all {
        assert!(ids.insert(screen.id), "duplicate screen {}", screen.id);
        assert!(!screen.states.is_empty(), "{} has no states", screen.id);
        let mut states = std::collections::BTreeSet::new();
        for state in &screen.states {
            assert!(
                states.insert(state.id),
                "{}: duplicate {}",
                screen.id,
                state.id
            );
            for theme in [Theme::Light, Theme::Dark] {
                let route = format!("{PAGE}/{}/{}?theme={}", screen.id, state.id, theme.as_str());
                let response = render(state, &HeaderMap::new(), Some(theme)).await;
                assert!(
                    !response.status().is_redirection(),
                    "{route}: {}",
                    response.status()
                );
                // A sample can be framed here and can post or fetch nothing.
                let policy = response.headers()[header::CONTENT_SECURITY_POLICY]
                    .to_str()
                    .unwrap()
                    .to_owned();
                for inert in [
                    "frame-ancestors 'self'",
                    "form-action 'none'",
                    "connect-src 'none'",
                ] {
                    assert!(policy.contains(inert), "{route}: {policy}");
                }
                assert!(!policy.contains("'self' 'none'"), "{route}: {policy}");
                let html = body(response).await;
                assert!(
                    html.contains(&format!("data-theme=\"{}\"", theme.as_str())),
                    "{route}: theme"
                );
                assert!(html.contains("oa-sample-strip"), "{route}: sample strip");
                // A page keeps the allowances of the address it lives at.
                let guarded = match screen.routes {
                    [live, ..] if live.starts_with('/') => format!("{live}?{route}"),
                    _ => route.clone(),
                };
                crate::copy_guard::assert_plain(&guarded, &html);
            }
        }
    }
}

/// The index lists every screen, and a screen page frames every state.
#[tokio::test]
async fn the_index_and_screen_pages_list_everything() {
    let all = screens();
    let html = body(index(HeaderMap::new()).await).await;
    for screen in &all {
        assert!(
            html.contains(&screen_href(screen)),
            "index lacks {}",
            screen.id
        );
    }
    crate::copy_guard::assert_plain(PAGE, &html);
    let first = &all[0];
    let response = screen(HeaderMap::new(), Path(first.id.to_owned())).await;
    assert_eq!(response.status(), StatusCode::OK);
    let policy = response.headers()[header::CONTENT_SECURITY_POLICY]
        .to_str()
        .unwrap()
        .to_owned();
    assert!(policy.contains("frame-src 'self'"), "{policy}");
    let html = body(response).await;
    for state in &first.states {
        assert!(
            html.contains(&state_href(first, state, Theme::Dark)),
            "{}",
            state.id
        );
    }
    let missing = screen(HeaderMap::new(), Path("nope".to_owned())).await;
    assert_eq!(missing.status(), StatusCode::NOT_FOUND);
}

/// The site states show their sample data and answer with their status.
#[tokio::test]
async fn site_states_show_their_sample_data() {
    let all = screens();
    let cases = [
        ("download", "desktop", StatusCode::OK, "Download OpenAgents"),
        ("stats", "populated", StatusCode::OK, "sample-weather"),
        ("stats", "no-data", StatusCode::OK, "Stats"),
        ("live", "one", StatusCode::OK, "A Coder run started"),
        ("live", "waiting", StatusCode::OK, "None yet."),
        ("pilot", "received", StatusCode::OK, "SAMPLE-0001"),
        (
            "pilot",
            "invalid",
            StatusCode::BAD_REQUEST,
            "Nothing was sent.",
        ),
        ("docs", "missing", StatusCode::NOT_FOUND, "Page not found"),
        ("blog", "missing", StatusCode::NOT_FOUND, "Post not found"),
        ("blog", "post", StatusCode::OK, "Coder 1.0"),
        (
            "problem",
            "not-found",
            StatusCode::NOT_FOUND,
            "Nothing on this site",
        ),
        (
            "problem",
            "refused",
            StatusCode::FORBIDDEN,
            "Pick a workspace first.",
        ),
        ("problem", "gone", StatusCode::GONE, "This page has moved"),
        (
            "problem",
            "broken",
            StatusCode::INTERNAL_SERVER_ERROR,
            "Something went wrong",
        ),
        (
            "problem",
            "unavailable",
            StatusCode::SERVICE_UNAVAILABLE,
            "right now",
        ),
        (
            "problem",
            "rate-limited",
            StatusCode::TOO_MANY_REQUESTS,
            "Too many runs",
        ),
        (
            "connect-google",
            "connecting",
            StatusCode::OK,
            "Connecting Google",
        ),
    ];
    for (screen, state, status, text) in cases {
        let render = all
            .iter()
            .find(|s| s.id == screen)
            .and_then(|s| s.states.iter().find(|s| s.id == state))
            .unwrap_or_else(|| panic!("{screen}/{state}"))
            .render;
        let response = render(&HeaderMap::new());
        assert_eq!(response.status(), status, "{screen}/{state}");
        let html = body(response).await;
        assert!(html.contains(text), "{screen}/{state} lacks {text:?}");
    }
}

#[tokio::test]
async fn the_chat_states_show_their_sample_data() {
    let expected = [
        ("chat-thread", "long", "Read earlier messages"),
        ("chat-thread", "code-and-links", "items.reverse()"),
        ("chat-work", "approval", "Coder is waiting for your answer"),
        ("chat-work", "files", "sample-photo-2.jpg"),
        ("chat-work", "coder-offline", "/sync on"),
        ("chat-sidebar", "search-none", "No chats found"),
        ("chat-sidebar", "archived", "Archived chats"),
        ("chat-sidebar", "move", "Move here"),
        (
            "composer",
            "typing",
            "Plan a quiet weekend in the mountains",
        ),
        ("work-run", "finished", "The change"),
        ("work-run", "running", "Running the tests"),
        ("demo", "chat", "Fix the lease table reset"),
    ];
    let screens = super::screens();
    for (screen, id, text) in expected {
        let state = screens
            .iter()
            .find(|s| s.id == screen)
            .and_then(|s| s.states.iter().find(|st| st.id == id))
            .unwrap_or_else(|| panic!("{screen}/{id}"));
        let html = body((state.render)(&HeaderMap::new())).await;
        assert!(html.contains(text), "{screen}/{id} lacks {text:?}");
    }
}

/// The legal and project links (Docs, Pricing, Terms, ...) show only to a
/// signed-out visitor: under the home composer and in the phone drawer.
#[tokio::test]
async fn legal_links_show_only_when_signed_out() {
    let screens = super::screens();
    let render = |screen: &str, id: &str| {
        let state = screens
            .iter()
            .find(|s| s.id == screen)
            .and_then(|s| s.states.iter().find(|st| st.id == id))
            .unwrap_or_else(|| panic!("{screen}/{id}"));
        (state.render)(&HeaderMap::new())
    };
    for (screen, id, shown) in [
        ("home", "signed-out", true),
        ("home", "signed-in", false),
        ("app-shell", "signed-out", true),
        ("app-shell", "signed-in", false),
        ("app-shell", "admin", false),
        ("composer", "typing", false),
    ] {
        let html = body(render(screen, id)).await;
        assert_eq!(html.contains("href=\"/terms\""), shown, "{screen}/{id}");
        assert_eq!(
            html.contains("oa-sidebar-fine-print"),
            shown,
            "{screen}/{id} drawer"
        );
        if screen == "home" {
            assert_eq!(html.contains("oa-home-legal"), shown, "{screen}/{id}");
        }
    }
}

/// Every live GET address in this crate's routers, read from the source:
/// `.route(PATH, ...get(...)...)` where PATH is a literal, a const in the
/// same file, or a `format!` of consts.
fn live_get_routes() -> Vec<(String, String)> {
    fn walk(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                if !path.ends_with("screens") && !path.ends_with("tests") {
                    walk(&path, out);
                }
            } else if path.extension().is_some_and(|e| e == "rs") && !path.ends_with("tests.rs") {
                out.push(path);
            }
        }
    }
    fn consts(source: &str) -> std::collections::BTreeMap<String, String> {
        let mut found = std::collections::BTreeMap::new();
        for line in source.lines() {
            let Some(at) = line.find("const ") else {
                continue;
            };
            let rest = &line[at + 6..];
            let Some((name, value)) = rest.split_once(':') else {
                continue;
            };
            let Some((_, literal)) = value.split_once("= \"") else {
                continue;
            };
            let Some(literal) = literal.strip_suffix("\";") else {
                continue;
            };
            if value.trim_start().starts_with("&") {
                found.insert(name.trim().to_owned(), literal.to_owned());
            }
        }
        found
    }
    fn fill(template: &str, consts: &std::collections::BTreeMap<String, String>) -> String {
        let mut out = String::new();
        let mut rest = template;
        while let Some(open) = rest.find('{') {
            out.push_str(&rest[..open]);
            let tail = &rest[open..];
            if let Some(stripped) = tail.strip_prefix("{{") {
                let close = stripped.find("}}").unwrap();
                out.push('{');
                out.push_str(&stripped[..close]);
                out.push('}');
                rest = &stripped[close + 2..];
            } else {
                let close = tail.find('}').unwrap();
                let name = &tail[1..close];
                out.push_str(consts.get(name).map_or("?", String::as_str));
                rest = &tail[close + 1..];
            }
        }
        out.push_str(rest);
        out
    }
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    walk(&root, &mut files);
    let mut routes = Vec::new();
    for file in files {
        let source = std::fs::read_to_string(&file).unwrap();
        let source = source.split("#[cfg(test)]").next().unwrap();
        let consts = consts(source);
        let mut rest = source;
        while let Some(at) = rest.find(".route(") {
            let after = &rest[at + 7..];
            let mut depth = 1;
            let mut end = 0;
            for (i, c) in after.char_indices() {
                match c {
                    '(' => depth += 1,
                    ')' => depth -= 1,
                    _ => {}
                }
                if depth == 0 {
                    end = i;
                    break;
                }
            }
            let args = &after[..end];
            rest = &after[end..];
            let mut depth = 0;
            let mut comma = args.len();
            for (i, c) in args.char_indices() {
                match c {
                    '(' | '[' | '{' => depth += 1,
                    ')' | ']' | '}' => depth -= 1,
                    ',' if depth == 0 => {
                        comma = i;
                        break;
                    }
                    _ => {}
                }
            }
            let (path, handler) = (args[..comma].trim(), &args[comma..]);
            if !handler.contains("get(") {
                continue;
            }
            let route = if let Some(literal) = path.strip_prefix('"') {
                literal.trim_end_matches('"').to_owned()
            } else if let Some(template) = path.trim_start_matches('&').strip_prefix("format!(\"") {
                fill(
                    template.trim_end_matches(')').trim_end_matches('"'),
                    &consts,
                )
            } else {
                let name = path.rsplit("::").next().unwrap().trim_start_matches('&');
                consts
                    .get(name)
                    .cloned()
                    .unwrap_or_else(|| format!("?{path}"))
            };
            let file = file.strip_prefix(&root).unwrap().display().to_string();
            routes.push((route, file));
        }
    }
    routes.sort();
    routes.dedup();
    routes
}

fn allowed(route: &str) -> bool {
    // Text and files for programs and browsers, whatever their address.
    let file = [
        ".md", ".js", ".css", ".json", ".txt", ".xml", ".jpg", ".png",
    ]
    .iter()
    .any(|end| route.ends_with(end));
    file || NOT_SCREENS
        .iter()
        .any(|(pattern, _)| match pattern.strip_suffix('*') {
            Some(prefix) => route.starts_with(prefix),
            None => route == *pattern,
        })
}

/// Every live GET address is a screen in the gallery or named in
/// `NOT_SCREENS` with the reason it isn't one.
#[test]
fn every_live_page_is_a_screen() {
    let claimed: std::collections::BTreeSet<&str> = screens()
        .iter()
        .flat_map(|screen| screen.routes.iter().copied())
        .collect();
    let routes = live_get_routes();
    assert!(routes.len() > 50, "found only {} routes", routes.len());
    let missing: Vec<String> = routes
        .iter()
        .filter(|(route, _)| {
            !route.starts_with('?') && !claimed.contains(route.as_str()) && !allowed(route)
        })
        .map(|(route, file)| format!("{route} ({file})"))
        .collect();
    assert!(
        missing.is_empty(),
        "these pages are not in the gallery or NOT_SCREENS:\n{}",
        missing.join("\n")
    );
}
