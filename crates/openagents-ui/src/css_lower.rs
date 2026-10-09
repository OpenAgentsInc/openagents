//! Lowering of Apps SDK UI's PostCSS build functions to plain CSS.
//!
//! Shared by the token generator and `build.rs`, which applies it to every
//! bundled stylesheet, so component CSS may keep upstream's `alpha()` and
//! `spacing()` calls. Keep this file free of `crate::` imports: `build.rs`
//! includes it with `#[path]` (`oa-tokens` is a build dependency too).

/// Lower Apps SDK UI's build-time functions to plain CSS, as its PostCSS
/// plugin does: `alpha(c, n%)` becomes `color-mix(in oklab, c n%, transparent)`
/// and `spacing(n)` becomes `calc(var(--spacing) * n)`.
#[must_use]
pub fn css_value(value: &str) -> String {
    let mut out = String::new();
    let mut rest = value;
    while let Some((name, start)) = find_call(rest, &["alpha", "spacing"]) {
        let open = start + name.len();
        let Some(close) = matching_paren(rest, open) else {
            break;
        };
        let args = split_args(&rest[open + 1..close]);
        let lowered = match (name, args.as_slice()) {
            ("alpha", [color, amount]) => {
                let amount = amount.trim();
                let amount = if amount.ends_with('%') {
                    amount.to_string()
                } else if amount.starts_with('.') || amount.starts_with("0.") {
                    match amount.parse::<f64>() {
                        Ok(fraction) => format!("{}%", fraction * 100.0),
                        Err(_) => amount.to_string(),
                    }
                } else {
                    format!("{amount}%")
                };
                format!(
                    "color-mix(in oklab, {} {amount}, transparent)",
                    css_value(color.trim())
                )
            }
            ("spacing", [count]) => format!("calc(var(--spacing) * {})", count.trim()),
            _ => rest[start..=close].to_string(),
        };
        out.push_str(&rest[..start]);
        out.push_str(&lowered);
        rest = &rest[close + 1..];
    }
    out.push_str(rest);
    out
}

/// Find the first call of one of `names` that is not part of a longer
/// identifier. Returns the name and its byte offset.
fn find_call<'a>(text: &str, names: &[&'a str]) -> Option<(&'a str, usize)> {
    let bytes = text.as_bytes();
    let mut best: Option<(&str, usize)> = None;
    for name in names {
        let pattern = format!("{name}(");
        let mut from = 0;
        while let Some(found) = text[from..].find(&pattern) {
            let at = from + found;
            let prev = at.checked_sub(1).map(|i| bytes[i]);
            let ident = prev.is_some_and(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_');
            if !ident {
                if best.is_none_or(|(_, b)| at < b) {
                    best = Some((name, at));
                }
                break;
            }
            from = at + 1;
        }
    }
    best
}

pub(crate) use oa_tokens::{matching_paren, split_args};
