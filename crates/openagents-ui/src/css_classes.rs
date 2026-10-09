//! Style audits shared by this crate's tests and `openagents-web`'s page
//! tests (UI-13): the stylesheet byte budget, the class names a stylesheet
//! defines, and the class names a page uses. A page class with no rule is
//! a missing rule or a leftover to delete.

use std::collections::BTreeSet;

/// The byte budget for [`crate::stylesheet()`], the only stylesheet a
/// `UiPage` page links. Set about 10% above its size when the legacy web
/// stylesheets were removed (262,873 bytes); raising it is a decision to
/// make in review, not a drift. Raised to 300,000 on 2026-10-09 for the
/// sidebar project groups, row menus, and plugin cards.
pub const STYLESHEET_BUDGET_BYTES: usize = 300_000;

/// Classes that components emit on purpose with no rule of their own:
/// script hooks (`querySelector`, Alpine roots) and structural wrappers
/// whose look comes from a sibling class or their children.
pub const UNSTYLED_HOOKS: &[&str] = &[
    "oa-catalog-intro",
    "oa-catalog-nav__group",
    "oa-catalog-specimen__body",
    "oa-catalog-swatches",
    "oa-dialog-body",
    "oa-menu",
    "oa-popover-root",
    "oa-slider__value-text",
];

/// Class prefixes that follow outside conventions and carry no rule here:
/// `language-*` names a code block's language and `hljs-*` marks
/// highlighted tokens a highlighter emits.
pub const UNSTYLED_PREFIXES: &[&str] = &["language-", "hljs-"];

/// Whether `class` must have a rule: anything but [`UNSTYLED_HOOKS`] and
/// [`UNSTYLED_PREFIXES`].
#[must_use]
pub fn needs_rule(class: &str) -> bool {
    !UNSTYLED_HOOKS.contains(&class) && !UNSTYLED_PREFIXES.iter().any(|p| class.starts_with(p))
}

/// Every class name a selector in `css` names, comments skipped.
#[must_use]
pub fn selector_classes(css: &str) -> BTreeSet<String> {
    let mut code = String::with_capacity(css.len());
    let mut rest = css;
    while let Some(start) = rest.find("/*") {
        code.push_str(&rest[..start]);
        rest = rest[start..].split_once("*/").map_or("", |(_, tail)| tail);
    }
    code.push_str(rest);
    let bytes = code.as_bytes();
    let mut classes = BTreeSet::new();
    for (at, _) in code.match_indices('.') {
        // Skip decimals such as `0.5rem` and `.5rem`.
        if at > 0 && bytes[at - 1].is_ascii_digit() {
            continue;
        }
        let name: String = code[at + 1..]
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
            .collect();
        if name.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_') {
            classes.insert(name);
        }
    }
    classes
}

/// Every class name in the `class="..."` attributes of `markup`.
#[must_use]
pub fn markup_classes(markup: &str) -> BTreeSet<String> {
    let mut classes = BTreeSet::new();
    for (at, _) in markup.match_indices(" class=\"") {
        let value = &markup[at + " class=\"".len()..];
        let value = &value[..value.find('"').unwrap_or(value.len())];
        classes.extend(value.split_ascii_whitespace().map(str::to_owned));
    }
    classes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selectors_and_markup_classes_are_read_exactly() {
        let css = "/* .not-me */ .oa-a:hover,.oa-b .oa-c>p{margin:0.5rem .25rem}\
                   @media(min-width:1px){.oa-d{x:1}}";
        let names: Vec<_> = selector_classes(css).into_iter().collect();
        assert_eq!(names, ["oa-a", "oa-b", "oa-c", "oa-d"]);
        let html = "<p class=\"oa-a  oa-b\"><span class=\"x\">.y</span></p>";
        let names: Vec<_> = markup_classes(html).into_iter().collect();
        assert_eq!(names, ["oa-a", "oa-b", "x"]);
    }
}
