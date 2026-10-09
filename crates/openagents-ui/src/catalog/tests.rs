use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use super::foundations::{COLOR_GROUPS, TOKENS_CSS_PATH, tokens_css};
use super::*;
use crate::tokens::Resolver;

fn catalog() -> String {
    render().into_string()
}

/// Values of `attr="..."` in `html`, in order.
fn attr_values<'a>(html: &'a str, attr: &str) -> Vec<&'a str> {
    let needle = format!(" {attr}=\"");
    html.match_indices(&needle)
        .map(|(at, _)| {
            let start = at + needle.len();
            let end = html[start..].find('"').map_or(html.len(), |i| start + i);
            &html[start..end]
        })
        .collect()
}

/// Builder names marked in `html`, with how often each appears.
fn markers(html: &str) -> BTreeMap<String, usize> {
    let mut out = BTreeMap::new();
    for value in attr_values(html, MARKER_ATTR) {
        for name in value.split_whitespace() {
            *out.entry(name.to_owned()).or_insert(0) += 1;
        }
    }
    out
}

#[test]
fn every_listed_builder_is_shown_in_both_themes() {
    let html = catalog();
    let found = markers(&html);
    for name in COMPONENTS {
        let count = found.get(*name).copied().unwrap_or(0);
        assert!(count >= 2, "{name} is not in the catalog in both themes");
        assert_eq!(count % 2, 0, "{name} shows in one theme only");
    }
    for name in found.keys() {
        assert!(
            COMPONENTS.contains(&name.as_str()),
            "{name} is marked but not in COMPONENTS"
        );
    }
}

/// `COMPONENTS` names every public builder: each `pub struct` in the
/// component modules, apart from the plain data types listed here.
#[test]
fn components_lists_every_public_builder() {
    const NOT_BUILDERS: [&str; 1] = ["FieldAria"];
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut public = BTreeSet::new();
    for dir in ["actions", "forms", "content", "overlays", "shell", "icons"] {
        for entry in std::fs::read_dir(src.join(dir)).expect("module dir") {
            let path = entry.expect("entry").path();
            if path.extension().is_none_or(|e| e != "rs") {
                continue;
            }
            let text = std::fs::read_to_string(&path).expect("source");
            for line in text.lines() {
                let rest = line
                    .strip_prefix("pub struct ")
                    .or_else(|| line.strip_prefix("pub enum Icon "));
                if let Some(rest) = rest {
                    let name: String = rest
                        .chars()
                        .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                        .collect();
                    public.insert(if name.is_empty() {
                        "Icon".to_owned()
                    } else {
                        name
                    });
                }
            }
        }
    }
    for name in &public {
        assert!(
            COMPONENTS.contains(&name.as_str()) || NOT_BUILDERS.contains(&name.as_str()),
            "pub struct {name} is not in catalog::COMPONENTS; add a specimen for it"
        );
    }
    for name in COMPONENTS {
        assert!(
            public.contains(*name),
            "{name} in COMPONENTS is not a public builder"
        );
    }
}

#[test]
fn output_is_stable() {
    let first = catalog();
    assert_eq!(first, catalog());
    assert!(first.len() > 100_000, "catalog is unexpectedly small");
}

#[test]
fn every_section_is_in_the_nav_and_shows_both_themes() {
    let html = catalog();
    let mut ids = BTreeSet::new();
    for section in SECTIONS {
        assert!(ids.insert(section.id), "duplicate section {}", section.id);
        assert!(
            GROUPS.contains(&section.group),
            "{} has an unknown group",
            section.id
        );
        assert!(
            html.contains(&format!("href=\"#{}\"", section.id)),
            "{} not in nav",
            section.id
        );
        let start = html
            .find(&format!(
                "<section class=\"oa-catalog-section\" id=\"{}\"",
                section.id
            ))
            .expect("section rendered");
        let end = html[start + 1..]
            .find("<section class=\"oa-catalog-section\"")
            .map_or(html.len(), |i| start + 1 + i);
        let body = &html[start..end];
        assert!(
            body.contains("data-theme=\"light\""),
            "{} has no light pane",
            section.id
        );
        assert!(
            body.contains("data-theme=\"dark\""),
            "{} has no dark pane",
            section.id
        );
    }
    for group in GROUPS {
        assert!(
            SECTIONS.iter().any(|s| s.group == group),
            "{group} is empty"
        );
    }
}

#[test]
fn ids_are_unique_and_never_clash_with_the_page_shell() {
    let html = catalog();
    let mut seen = BTreeSet::new();
    for id in attr_values(&html, "id") {
        // Brand icons carry their own clip-path ids, identical in each copy.
        if id.starts_with("oa-icon-") {
            continue;
        }
        assert!(seen.insert(id), "duplicate id {id}");
    }
    for page_id in ["content", "oa-left-panel"] {
        assert!(
            !seen.contains(page_id),
            "catalog reuses the page id {page_id}"
        );
    }
    assert!(!html.contains("<main"), "the page has the only <main>");
}

#[test]
fn csp_safe_no_inline_styles_or_scripts() {
    let html = catalog().to_ascii_lowercase();
    // The catalog adds no inline styles. Textarea and Slider (UI-03) still
    // set their custom properties through `style` (`--textarea-*-rows`,
    // `--oa-slider-fill`), which `style-src 'self'` drops; those are the
    // only ones allowed here until the components move them to classes.
    for style in attr_values(&html, "style") {
        assert!(
            style.starts_with("--textarea-min-rows:") || style.starts_with("--oa-slider-fill:"),
            "inline style {style}"
        );
    }
    assert!(!html.contains("<style"), "style element");
    assert!(!html.contains("<script"), "script element");
    for handler in [
        " onclick=",
        " onload=",
        " onerror=",
        " oninput=",
        " onchange=",
    ] {
        assert!(!html.contains(handler), "inline handler {handler}");
    }
}

#[test]
fn swatch_tokens_are_defined_and_swatch_css_is_checked_in() {
    let resolver = Resolver::new();
    for (_, tokens) in COLOR_GROUPS {
        for token in *tokens {
            assert!(
                resolver.defines(&format!("--{token}")),
                "--{token} is not a token"
            );
        }
    }
    for token in foundations::RADII.iter().chain(foundations::SHADOWS) {
        assert!(
            resolver.defines(&format!("--{token}")),
            "--{token} is not a token"
        );
    }
    for name in foundations::TYPE_SCALE {
        for part in ["size", "line-height", "weight", "tracking"] {
            let token = format!("--font-{name}-{part}");
            assert!(resolver.defines(&token), "{token} is not a token");
        }
    }
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(TOKENS_CSS_PATH);
    let generated = tokens_css();
    if std::env::var_os("OPENAGENTS_UI_BLESS").is_some() {
        std::fs::write(&path, &generated).expect("write catalog token css");
        return;
    }
    let on_disk = std::fs::read_to_string(&path).unwrap_or_default();
    assert!(
        on_disk == generated,
        "{TOKENS_CSS_PATH} is stale; run OPENAGENTS_UI_BLESS=1 cargo test -p openagents-ui"
    );
    assert!(crate::stylesheet().contains("/* static/components/catalog-tokens.css */"));
    assert!(crate::stylesheet().contains("/* static/components/catalog.css */"));
}

#[test]
fn swatches_show_each_themes_resolved_value() {
    let html = catalog();
    // --color-surface is white in Coder Light and Noir's surface in Coder Noir.
    assert!(html.contains("#ffffff"), "light surface value");
    assert!(
        html.contains(&format!("#{:06x}", crate::tokens::noir::SURFACE)),
        "noir surface value"
    );
}
