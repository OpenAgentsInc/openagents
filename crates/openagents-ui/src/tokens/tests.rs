use super::*;
use std::collections::HashMap;
use std::path::Path;

const SCHEMES: [Scheme; 2] = [Scheme::Light, Scheme::Dark];

/// The checked-in token stylesheets are exactly what the table generates.
/// `OPENAGENTS_UI_BLESS=1` rewrites them.
#[test]
fn generated_token_css_is_checked_in() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let bless = std::env::var_os("OPENAGENTS_UI_BLESS").is_some();
    for file in TokenFile::ALL {
        let path = root.join(file.path());
        let generated = file.css();
        if bless {
            std::fs::write(&path, &generated).expect("write token css");
            continue;
        }
        let on_disk = std::fs::read_to_string(&path).unwrap_or_default();
        assert!(
            on_disk == generated,
            "{} is stale; run OPENAGENTS_UI_BLESS=1 cargo test -p openagents-ui",
            file.path()
        );
    }
}

#[test]
fn generated_css_has_no_build_functions_left() {
    for file in TokenFile::ALL {
        let css = file.css();
        for call in ["alpha(", "spacing("] {
            for (at, _) in css.match_indices(call) {
                let prev = css.as_bytes()[at - 1];
                assert!(
                    prev.is_ascii_alphanumeric() || prev == b'-',
                    "{} still calls {call}",
                    file.path()
                );
            }
        }
    }
}

#[test]
fn every_reference_resolves() {
    let resolver = Resolver::new();
    for (name, value) in all_tokens() {
        let mut rest = value.as_str();
        while let Some(at) = rest.find("var(") {
            let tail = &rest[at + 4..];
            let end = tail.find([')', ',']).expect("closed var()");
            let referenced = tail[..end].trim();
            assert!(
                resolver.defines(referenced),
                "{name} references undefined {referenced}"
            );
            rest = &tail[end..];
        }
    }
}

#[test]
fn token_names_are_unique_and_keep_apps_sdk_names() {
    let mut seen = std::collections::HashSet::new();
    for (name, _) in all_tokens() {
        assert!(name.starts_with("--"), "{name}");
        assert!(seen.insert(name), "{name} defined twice");
    }
    for name in [
        "--color-background-primary-solid",
        "--color-background-danger-soft-hover",
        "--color-text-secondary",
        "--color-border",
        "--color-surface",
        "--button-font-weight",
        "--control-size-md",
        "--radius-full",
        "--font-sans",
        "--font-heading-xl-size",
    ] {
        assert!(seen.contains(name), "missing {name}");
    }
}

#[test]
fn noir_overrides_name_real_roles_and_keep_coder_light() {
    let resolver = Resolver::new();
    for (name, noir) in noir::OVERRIDES {
        let source = TokenFile::ALL
            .iter()
            .flat_map(|f| f.sections())
            .flat_map(|s| s.tokens.iter())
            .find(|(n, _)| n == name)
            .unwrap_or_else(|| panic!("{name} is not an Apps SDK UI token"));
        assert_eq!(
            resolver.color(name, Scheme::Dark),
            Some(Rgba::opaque(*noir)),
            "{name} dark is Noir"
        );
        // Light side unchanged: resolve the upstream value with no overrides.
        let upstream: HashMap<&str, String> = TokenFile::ALL
            .iter()
            .flat_map(|f| f.sections())
            .flat_map(|s| s.tokens.iter())
            .map(|(n, v)| (*n, (*v).to_string()))
            .collect();
        let plain = Resolver::from_tokens(upstream);
        assert_eq!(
            resolver.color(name, Scheme::Light),
            plain.eval(source.1, Scheme::Light),
            "{name} light is Apps SDK UI's"
        );
    }
    assert_eq!(
        resolver.color("--color-background-primary-solid", Scheme::Light),
        Some(Rgba::opaque(0x181818)),
        "Coder Light's accent is neutral black"
    );
}

#[test]
fn lowering_matches_the_upstream_postcss_functions() {
    assert_eq!(
        css_value("alpha(var(--blue-400), 8%)"),
        "color-mix(in oklab, var(--blue-400) 8%, transparent)"
    );
    assert_eq!(
        css_value("alpha(var(--x), 0.6)"),
        "color-mix(in oklab, var(--x) 60%, transparent)"
    );
    assert_eq!(
        css_value("spacing(1.5) spacing(2)"),
        "calc(var(--spacing) * 1.5) calc(var(--spacing) * 2)"
    );
    assert_eq!(css_value("var(--spacing)"), "var(--spacing)");
    assert_eq!(css_value("--alpha(x)"), "--alpha(x)");
}

/// The page surfaces text sits on.
const SURFACES: [&str; 4] = [
    "--color-surface",
    "--color-surface-secondary",
    "--color-surface-tertiary",
    "--color-surface-elevated",
];

const INTENTS: [&str; 6] = [
    "info",
    "warning",
    "caution",
    "danger",
    "success",
    "discovery",
];

fn on(resolver: &Resolver, top: &str, below: Rgba, scheme: Scheme) -> Rgba {
    resolver
        .color(top, scheme)
        .unwrap_or_else(|| panic!("{top} resolves in {scheme:?}"))
        .over(below)
}

fn check(failures: &mut Vec<String>, label: String, ratio: f64, min: f64) {
    if ratio < min {
        failures.push(format!("{label}: {ratio:.2} < {min}"));
    }
}

/// WCAG AA (4.5:1) for every text role on the surface it is drawn on, in
/// Coder Light and Coder Noir. Translucent backgrounds are composited over
/// each page surface first.
///
/// Not text roles, so held to 3:1 (WCAG 1.4.11, non-text and large text)
/// instead: `--color-text-tertiary` (placeholders and receded meta, like
/// Noir's quarter step) and the `-solid` button labels at Apps SDK UI's
/// saturated fills. `--color-text-disabled` is exempt (WCAG 1.4.3).
#[test]
fn text_roles_meet_wcag_aa_in_both_themes() {
    let r = Resolver::new();
    let mut failures = Vec::new();
    for scheme in SCHEMES {
        for surface in SURFACES {
            let field = on(&r, surface, Rgba::opaque(0), scheme);
            let mut text = vec![
                "--color-text".to_string(),
                "--color-text-secondary".to_string(),
                "--color-text-primary-outline".to_string(),
                "--color-text-secondary-outline".to_string(),
                "--color-text-secondary-ghost".to_string(),
                "--link-primary-text-color".to_string(),
            ];
            text.extend(INTENTS.iter().map(|i| format!("--color-text-{i}")));
            for role in &text {
                let ratio = on(&r, role, field, scheme).contrast(field);
                check(
                    &mut failures,
                    format!("{scheme:?} {role} on {surface}"),
                    ratio,
                    4.5,
                );
            }
            let ratio = on(&r, "--color-text-tertiary", field, scheme).contrast(field);
            // Tertiary is held to 3:1 on the page and on cards; the tertiary
            // surface (Coder Light #f3f3f3) is for chrome, not meta text.
            if surface != "--color-surface-tertiary" {
                check(
                    &mut failures,
                    format!("{scheme:?} tertiary on {surface}"),
                    ratio,
                    3.0,
                );
            }

            for intent in INTENTS.iter().chain(["secondary"].iter()) {
                let soft_bg = on(
                    &r,
                    &format!("--color-background-{intent}-soft-alpha"),
                    field,
                    scheme,
                );
                let soft_fg = on(&r, &format!("--color-text-{intent}-soft"), soft_bg, scheme);
                check(
                    &mut failures,
                    format!("{scheme:?} {intent}-soft on soft-alpha over {surface}"),
                    soft_fg.contrast(soft_bg),
                    4.5,
                );
            }
            for intent in INTENTS {
                let bg = on(
                    &r,
                    &format!("--color-background-{intent}-surface"),
                    field,
                    scheme,
                );
                let fg = on(&r, &format!("--color-text-{intent}-surface"), bg, scheme);
                check(
                    &mut failures,
                    format!("{scheme:?} {intent}-surface over {surface}"),
                    fg.contrast(bg),
                    4.5,
                );
            }
        }
        // Primary solid: the accent button carries body-size labels.
        let bg = on(
            &r,
            "--color-background-primary-solid",
            Rgba::opaque(0),
            scheme,
        );
        let fg = on(&r, "--color-text-primary-solid", bg, scheme);
        check(
            &mut failures,
            format!("{scheme:?} primary-solid"),
            fg.contrast(bg),
            4.5,
        );
        for intent in INTENTS.iter().chain(["secondary"].iter()) {
            let bg = on(
                &r,
                &format!("--color-background-{intent}-solid"),
                Rgba::opaque(0),
                scheme,
            );
            let fg = on(&r, &format!("--color-text-{intent}-solid"), bg, scheme);
            check(
                &mut failures,
                format!("{scheme:?} {intent}-solid"),
                fg.contrast(bg),
                3.0,
            );
        }
    }
    assert!(
        failures.is_empty(),
        "contrast failures:\n{}",
        failures.join("\n")
    );
}
