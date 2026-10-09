use super::*;
use std::path::Path;

/// The site CSP is `style-src 'self'`, which drops inline `style` attributes,
/// so no component source may emit one: every rendering path is scanned
/// (tests excluded), covering maud `style=` attributes and string-built
/// markup alike. Runtime values go through data-attribute presets in CSS or
/// through the CSSOM from the component scripts.
#[test]
fn no_component_source_emits_an_inline_style_attribute() {
    fn visit(dir: &Path, hits: &mut Vec<String>) {
        for entry in std::fs::read_dir(dir).expect("read src dir").flatten() {
            let path = entry.path();
            if path.is_dir() {
                visit(&path, hits);
                continue;
            }
            let name = path.file_name().unwrap().to_string_lossy();
            if !name.ends_with(".rs") || name == "tests.rs" {
                continue;
            }
            let source = std::fs::read_to_string(&path).expect("read source");
            for (index, line) in source.lines().enumerate() {
                let code = line.trim_start();
                if code.starts_with("//") {
                    continue;
                }
                let code = code.split(" //").next().unwrap_or(code);
                if code.starts_with("style=")
                    || code.contains(" style=")
                    || code.contains("\"style\"")
                {
                    hits.push(format!("{}:{}: {}", path.display(), index + 1, line.trim()));
                }
            }
        }
    }
    let mut hits = Vec::new();
    visit(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("src"),
        &mut hits,
    );
    assert!(
        hits.is_empty(),
        "inline style attributes:\n{}",
        hits.join("\n")
    );
}

/// Every component, in every variant the catalog shows, renders without an
/// inline `style` attribute (`style-src 'self'` would drop it).
#[test]
fn every_rendered_component_carries_no_inline_style() {
    let markup = crate::catalog::render().into_string().to_ascii_lowercase();
    assert!(
        !markup.contains(" style="),
        "inline style attribute in catalog"
    );
    assert!(!markup.contains("<style"), "style element in catalog");
}

#[test]
fn base_reset_is_scoped_to_oa_elements_in_the_base_layer() {
    let css =
        std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("static/base.css"))
            .expect("base.css");
    let body = css.split_once("@layer base {").expect("base layer").1;
    for rule in [
        "box-sizing: border-box;",
        "font: inherit;",
        "background: transparent;",
        "border: 0;",
        "appearance: none;",
    ] {
        assert!(body.contains(rule), "reset sets {rule}");
    }
    // Zero specificity and scoped: every reset selector is a :where() on oa-
    // classes, and checkbox, radio and range inputs keep native appearance.
    assert!(body.contains(r#":where([class^="oa-"], [class*=" oa-"])"#));
    assert!(body.contains(r#"input:not([type="checkbox"], [type="radio"], [type="range"])"#));
}

#[test]
fn stylesheet_bundles_tokens_base_and_every_component_file() {
    let css = stylesheet();
    assert!(css.starts_with("@layer theme, base, components;"));
    for file in [
        "static/tokens-primitive.css",
        "static/tokens-semantic.css",
        "static/tokens-components.css",
        "static/base.css",
    ] {
        assert!(css.contains(&format!("/* {file} */")), "{file} bundled");
    }
    // Every component stylesheet on disk is in the bundle, with no list to edit.
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("static/components");
    let mut on_disk: Vec<String> = std::fs::read_dir(dir)
        .expect("components dir")
        .filter_map(Result::ok)
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.ends_with(".css"))
        .collect();
    on_disk.sort();
    assert_eq!(component_stylesheets(), on_disk.as_slice());
    for name in &on_disk {
        assert!(css.contains(&format!(
            "/* static/components/{name} */\n@layer components {{"
        )));
    }
    assert!(css.contains("--color-background-primary-solid:"));
    assert!(css.contains("color-scheme: light dark"));
    assert_eq!(stylesheet_version().len(), 16);
}

#[test]
fn bundled_css_has_no_unlowered_build_functions_outside_comments() {
    let mut css = stylesheet();
    let mut code = String::new();
    while let Some(start) = css.find("/*") {
        code.push_str(&css[..start]);
        css = css[start..].split_once("*/").map_or("", |(_, rest)| rest);
    }
    code.push_str(css);
    for call in ["alpha(", "spacing("] {
        for (at, _) in code.match_indices(call) {
            let prev = code.as_bytes()[at - 1];
            assert!(
                prev.is_ascii_alphanumeric() || prev == b'-',
                "unlowered {call}"
            );
        }
    }
}

/// Theme colors come from `light-dark()`, resolved by `color-scheme`, so
/// "follow the system" works with no `data-theme` set. A rule keyed on
/// `[data-theme="dark"]` only applies to an explicit choice, so system-dark
/// users would get light colors. Such selectors may only toggle `display`
/// (for example, which theme-toggle icon shows).
#[test]
fn component_css_themes_through_light_dark_not_data_theme_selectors() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("static/components");
    let mut failures = Vec::new();
    for name in component_stylesheets() {
        let text = std::fs::read_to_string(dir.join(name)).expect("component css");
        for needle in [
            "[data-theme=\"dark\"]",
            "[data-theme=dark]",
            "[data-theme='dark']",
            "[data-theme=\"light\"]",
            "[data-theme=light]",
            "[data-theme='light']",
        ] {
            for (at, _) in text.match_indices(needle) {
                let Some(open) = text[at..].find('{').map(|i| at + i) else {
                    continue;
                };
                let close = text[open..].find('}').map_or(text.len(), |i| open + i);
                let body = &text[open + 1..close];
                let properties: Vec<&str> = body
                    .split(';')
                    .filter_map(|decl| decl.split_once(':').map(|(p, _)| p.trim()))
                    .filter(|p| !p.is_empty() && !p.starts_with("/*"))
                    .collect();
                if properties.iter().any(|p| *p != "display") {
                    let line = text[..at].lines().count() + 1;
                    failures.push(format!("{name}:{line}: {needle} sets {properties:?}"));
                }
            }
        }
    }
    assert!(
        failures.is_empty(),
        "use light-dark() instead of data-theme selectors:\n{}",
        failures.join("\n")
    );
}

#[test]
fn script_is_csp_safe_and_matches_the_shell_contract() {
    let js = script();
    assert!(js.starts_with("/* static/theme-toggle.js */"));
    // Component scripts (Alpine.data registrations) ride in the bundle that
    // loads before Alpine.
    assert!(js.contains("/* static/components/forms.js */"));
    assert_eq!(assets::SCRIPT_LOAD_ORDER[1], assets::ALPINE_CSP_FILE);
    for banned in [
        "eval(",
        "new Function",
        "setTimeout(\"",
        "setInterval(\"",
        "innerHTML",
    ] {
        assert!(!js.contains(banned), "script uses {banned}");
    }
    let toggle = assets::THEME_TOGGLE_JS;
    assert!(toggle.contains(&format!("[{}]", shell::THEME_TOGGLE_ATTR)));
    assert!(toggle.contains(&format!("\"{}\"", shell::THEME_COOKIE)));
    assert!(toggle.contains("prefers-color-scheme: dark"));
    assert!(toggle.contains("SameSite=Lax"));
    assert!(toggle.contains("preventDefault"));
    assert_eq!(script_version().len(), 16);
}

#[test]
fn alpine_csp_build_matches_its_pinned_checksum() {
    assert_eq!(
        sha256_hex(assets::ALPINE_CSP_JS.as_bytes()),
        assets::ALPINE_CSP_SHA256
    );
    assert!(assets::ALPINE_CSP_FILE.contains(assets::ALPINE_CSP_VERSION));
    // The CSP build evaluates expressions with its own parser.
    assert!(!assets::ALPINE_CSP_JS.contains("new Function"));
    assert!(!assets::ALPINE_CSP_JS.contains("eval("));
}

#[test]
fn sha256_known_answers() {
    assert_eq!(
        sha256_hex(b""),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
    assert_eq!(
        sha256_hex(b"abc"),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
    assert_eq!(
        sha256_hex(&[b'a'; 1000]),
        "41edece42d63e8d9bf515a9ba6932e1c20cbc9f5a5d134645adb5db1b9737ea3"
    );
}

#[test]
fn bundled_stylesheet_stays_within_its_byte_budget() {
    use crate::css_classes::STYLESHEET_BUDGET_BYTES;
    let size = stylesheet().len();
    assert!(
        size <= STYLESHEET_BUDGET_BYTES,
        "openagents-ui.css is {size} bytes, over the {STYLESHEET_BUDGET_BYTES}-byte budget"
    );
}

/// No component, in any variant the catalog shows, carries a class that no
/// rule in the bundled stylesheet styles: a dead class is either a missing
/// rule or a leftover to delete.
#[test]
fn every_class_the_catalog_renders_has_a_rule() {
    let defined = crate::css_classes::selector_classes(stylesheet());
    let used = crate::css_classes::markup_classes(&crate::catalog::render().into_string());
    let missing: Vec<_> = used
        .difference(&defined)
        .filter(|class| crate::css_classes::needs_rule(class))
        .collect();
    assert!(missing.is_empty(), "classes with no rule: {missing:?}");
}

/// Dependency-free SHA-256 (FIPS 180-4), for the vendored asset check.
fn sha256_hex(data: &[u8]) -> String {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];
    let mut h: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];
    let mut message = data.to_vec();
    let bit_len = (data.len() as u64).wrapping_mul(8);
    message.push(0x80);
    while message.len() % 64 != 56 {
        message.push(0);
    }
    message.extend_from_slice(&bit_len.to_be_bytes());
    for block in message.chunks(64) {
        let mut w = [0u32; 64];
        for (i, word) in block.chunks(4).enumerate() {
            w[i] = u32::from_be_bytes([word[0], word[1], word[2], word[3]]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh] = h;
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ (!e & g);
            let t1 = hh
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let t2 = s0.wrapping_add(maj);
            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(t1);
            d = c;
            c = b;
            b = a;
            a = t1.wrapping_add(t2);
        }
        for (slot, value) in h.iter_mut().zip([a, b, c, d, e, f, g, hh]) {
            *slot = slot.wrapping_add(value);
        }
    }
    h.iter().map(|word| format!("{word:08x}")).collect()
}
