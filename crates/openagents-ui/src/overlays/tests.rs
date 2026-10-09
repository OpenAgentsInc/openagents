use maud::{Render, html};

use super::*;

fn render(item: impl Render) -> String {
    item.render().into_string()
}

#[test]
fn popover_opens_natively_and_names_its_alpine_component() {
    let html = render(
        Popover::new("share", "Share", html! { p { "Copied" } })
            .label("Share options")
            .side(Side::Top)
            .align(Align::End),
    );
    assert!(html.contains(r#"x-data="oaPopover""#));
    assert!(html.contains(
        r#"<button type="button" class="oa-overlay-trigger" popovertarget="share" aria-haspopup="dialog" aria-controls="share" data-oa-trigger="" data-state="closed">Share</button>"#
    ));
    assert!(html.contains(r#"id="share" class="oa-popover" popover="auto" role="dialog" aria-label="Share options" tabindex="-1" data-oa-panel data-state="closed" data-side="top" data-align="end""#));
    assert!(html.contains("<p>Copied</p>"));
}

#[test]
fn menu_has_menu_roles_and_a_no_js_baseline_of_links_and_buttons() {
    let html = render(
        Menu::new("account", html! { span { "Account" } })
            .label("Account")
            .item(MenuItem::heading("You"))
            .item(MenuItem::link("Settings", "/settings").icon(html! { svg {} }))
            .item(MenuItem::link("Billing", "/billing").disabled(true))
            .item(MenuItem::separator())
            .item(MenuItem::button("Compact").checked(true).value("compact"))
            .item(
                MenuItem::button("Sign out")
                    .submit()
                    .form("logout")
                    .name("action")
                    .value("out"),
            ),
    );
    assert!(html.contains(r#"x-data="oaMenu""#));
    assert!(html.contains(r#"aria-haspopup="menu""#));
    assert!(html.contains(r#"popovertarget="account""#));
    assert!(html.contains(r#"class="oa-menu-list" popover="auto" role="menu""#));
    assert!(
        html.contains(r#"<a class="oa-menu-item" role="menuitem" tabindex="-1" href="/settings">"#)
    );
    // A disabled link loses its href.
    assert!(html.contains(r#"<a class="oa-menu-item" role="menuitem" tabindex="-1" aria-disabled="true" data-disabled="">"#));
    assert!(html.contains(r#"role="separator""#));
    assert!(html.contains(r#"role="menuitemcheckbox" tabindex="-1" aria-checked="true""#));
    assert!(html.contains(r#"class="oa-menu-check""#));
    assert!(html.contains(r#"type="submit" role="menuitem" tabindex="-1" name="action" value="out" form="logout" data-value="out""#));
    // aria-expanded is left to the script: the native invoker already
    // exposes the expanded state without it.
    assert!(!html.contains("aria-expanded"));
}

#[test]
fn trigger_slots_take_classes_and_attributes_but_not_handlers_or_overrides() {
    let html = render(
        Menu::new("m", "More")
            .trigger_class("oa-button")
            .trigger_label("More actions")
            .trigger_attr("data-variant", "soft")
            .trigger_attr("hx-get", "/x?a=1&b=\"2\"")
            .trigger_attr("onclick", "alert(1)")
            .trigger_attr("popovertarget", "elsewhere")
            .trigger_attr("bad name", "x"),
    );
    assert!(html.contains(r#"class="oa-overlay-trigger oa-button""#));
    assert!(html.contains(r#"aria-label="More actions""#));
    assert!(html.contains(r#"data-variant="soft""#));
    assert!(html.contains(r#"hx-get="/x?a=1&amp;b=&quot;2&quot;""#));
    assert!(!html.contains("onclick"));
    assert!(!html.contains("elsewhere"));
    assert!(!html.contains("bad name"));
}

#[test]
fn text_is_escaped() {
    let html = render(
        Menu::new("m", "<b>")
            .item(MenuItem::link("<script>", "/a?b=\"c\""))
            .label("\"x\""),
    );
    assert!(html.contains("&lt;script&gt;"));
    assert!(html.contains(r#"href="/a?b=&quot;c&quot;""#));
    assert!(!html.contains("<script>"));
    // Trigger content is a Render slot: a &str renders escaped.
    assert!(html.contains("&lt;b&gt;</button>"));
}

#[test]
fn tooltip_is_a_manual_popover_with_role_tooltip() {
    let html = render(
        Tooltip::new(
            "tip",
            html! { button type="button" { "Copy" } },
            "Copy link",
        )
        .compact(true)
        .delay_ms(300),
    );
    assert!(html.contains(r#"x-data="oaTooltip""#));
    assert!(html.contains(r#"data-delay="300""#));
    assert!(html.contains(r#"<button type="button">Copy</button>"#));
    assert!(html.contains(r#"id="tip" class="oa-tooltip" popover="manual" role="tooltip""#));
    assert!(html.contains(
        r#"data-side="top" data-align="center" data-compact="true" data-gutter-size="sm""#
    ));
}

#[test]
fn select_control_keeps_the_native_select_and_adds_a_hidden_listbox() {
    let html = render(
        SelectControl::new("repo", "repo")
            .placeholder("Repository")
            .option("oa", "openagents")
            .option("ps", "psionic")
            .option("pr", "probe")
            .selected("ps")
            .selected("pr")
            .multiple(true)
            .searchable("Search repos")
            .empty_text("Nothing matches"),
    );
    assert!(html.contains(r#"x-data="oaSelect""#));
    // The no-JS form: a native multiple select carrying the value.
    assert!(html.contains(r#"<span class="oa-select-native" data-oa-native><span class="oa-select" data-variant="outline" data-size="md" data-block="false" data-pill>"#));
    assert!(html.contains(r#"<select class="oa-select__control" id="repo" name="repo" multiple>"#));
    assert!(html.contains(r#"<option value="ps" selected>psionic</option>"#));
    assert!(html.contains(r#"<option value="pr" selected>probe</option>"#));
    // The enhanced form ships hidden.
    assert!(html.contains(r#"<button type="button" class="oa-select-control" hidden data-oa-trigger popovertarget="repo-panel" aria-haspopup="listbox" aria-controls="repo-panel""#));
    assert!(html.contains(r#"data-selected="true""#));
    assert!(html.contains(r#"data-oa-text>psionic, probe</span>"#));
    assert!(html.contains(r#"id="repo-panel" class="oa-select-list" popover="auto""#));
    assert!(html.contains(r#"role="combobox" aria-expanded="true" aria-controls="repo-list""#));
    assert!(html.contains(r#"id="repo-list" class="oa-select-options" role="listbox" tabindex="-1" aria-multiselectable="true" aria-label="Repository""#));
    assert!(html.contains(r#"id="repo-opt-0" class="oa-select-option" role="option" data-value="oa" data-label="openagents" aria-selected="false""#));
    assert!(html.contains(r#"id="repo-opt-1" class="oa-select-option" role="option" data-value="ps" data-label="psionic" aria-selected="true""#));
    assert!(html.contains(r#"data-oa-empty hidden>Nothing matches</div>"#));
}

#[test]
fn single_select_control_shows_the_placeholder_until_chosen() {
    let html = render(
        SelectControl::new("s", "s")
            .placeholder("Choose")
            .option("a", "A"),
    );
    assert!(html.contains(r#"data-oa-text>Choose</span>"#));
    assert!(html.contains(r#"data-placeholder="Choose" data-selected="false""#));
    assert!(html.contains(r#"<option value="" disabled selected>Choose</option>"#));
    assert!(!html.contains("aria-multiselectable"));
    assert!(!html.contains("data-oa-search"));
    assert!(!html.contains("<select class=\"oa-select__control\" id=\"s\" name=\"s\" multiple"));
}

#[test]
fn select_control_passes_field_wiring_to_both_forms() {
    let field = crate::forms::Field::new("repo", "Repository").error("Pick one");
    let html = render(
        SelectControl::new("repo", "repo")
            .option("a", "A")
            .aria(field.aria())
            .variant(SelectVariant::Ghost)
            .disabled(true),
    );
    assert!(html.contains(r#"aria-describedby="repo-error""#));
    assert!(html.matches(r#"aria-invalid="true""#).count() == 2);
    // Ghost is a trigger look; the native fallback shows it as outline.
    assert!(html.contains(r#"data-variant="ghost""#));
    assert!(html.contains(r#"class="oa-select" data-variant="outline""#));
    assert!(html.contains("disabled data-state"));
}

#[test]
fn dialog_is_native_and_closes_without_script() {
    let html = render(
        Dialog::new("confirm", "Delete run?", html! { p { "Gone for good." } })
            .description("This cannot be undone.")
            .footer(html! { button { "Delete" } })
            .size(DialogSize::Sm),
    );
    assert!(html.contains(r#"<dialog id="confirm" class="oa-dialog" x-data="oaDialog" data-state="closed" aria-labelledby="confirm-title" aria-describedby="confirm-description" closedby="any" data-size="sm">"#));
    assert!(html.contains(r#"<h2 id="confirm-title" class="oa-dialog-title">Delete run?</h2>"#));
    assert!(html.contains(r#"<form method="dialog" class="oa-dialog-close-form"><button type="submit" class="oa-dialog-close" aria-label="Close">"#));
    assert!(html.contains(r#"class="oa-dialog-footer"><button>Delete</button>"#));

    let strict = render(Dialog::new("d", "T", "body").dismissible(false));
    assert!(strict.contains(r#"closedby="closerequest""#));
    assert!(!strict.contains("aria-describedby"));

    let open = render(DialogTrigger::new("confirm", "Delete").class("oa-button"));
    assert_eq!(
        open,
        r#"<button type="button" class="oa-overlay-trigger oa-button" commandfor="confirm" command="show-modal" aria-haspopup="dialog" aria-controls="confirm" data-oa-dialog="confirm" data-state="closed">Delete</button>"#
    );
}

#[test]
fn the_script_registers_every_component_without_inline_code() {
    // The bundle carries the overlay script.
    assert!(crate::script().contains(SCRIPT));
    for name in ["oaPopover", "oaMenu", "oaTooltip", "oaSelect", "oaDialog"] {
        assert!(
            SCRIPT.contains(&format!("{name}: ")),
            "{name} is not registered"
        );
    }
    assert!(SCRIPT.contains("Alpine.data(name, components[name])"));
    assert!(SCRIPT.contains("alpine:init"));
    // CSP: nothing that needs 'unsafe-eval'.
    for banned in ["eval(", "new Function", "innerHTML", "setTimeout(\""] {
        assert!(!SCRIPT.contains(banned), "overlays.js uses {banned}");
    }
    // The markup names components and never carries expressions or inline
    // handlers.
    let page = [
        render(Popover::new("p", "P", "x")),
        render(Menu::new("m", "M").item(MenuItem::button("B"))),
        render(Tooltip::new("t", "T", "x")),
        render(SelectControl::new("s", "s").option("a", "A")),
        render(Dialog::new("d", "D", "x")),
        render(DialogTrigger::new("d", "Open")),
    ]
    .concat();
    for banned in [
        " @", " x-on", " x-bind", " :", " onclick", "<script", " style=",
    ] {
        assert!(!page.contains(banned), "markup contains {banned:?}");
    }
}

#[test]
fn stylesheets_use_the_oa_prefix_and_state_attributes() {
    let names: Vec<_> = STYLESHEETS.iter().map(|(name, _)| *name).collect();
    assert_eq!(
        names,
        [
            "popover.css",
            "menu.css",
            "tooltip.css",
            "select-control.css",
            "dialog.css"
        ]
    );
    for name in names {
        assert!(
            crate::component_stylesheets().contains(&name),
            "{name} not bundled"
        );
    }
    let all: String = STYLESHEETS.iter().map(|(_, css)| *css).collect();
    assert!(all.contains("position-area"));
    assert!(all.contains(":popover-open"));
    assert!(all.contains("@starting-style"));
    assert!(all.contains(r#"[data-state="open"]"#));
    assert!(all.contains("--menu-item-background-color"));
    assert!(all.contains("--select-control-size"));
    assert!(
        !all.contains("spacing("),
        "postcss spacing() left unexpanded"
    );
    assert!(!all.contains("@mixin"), "postcss mixin left unexpanded");
    for (name, css) in STYLESHEETS {
        // Every class selector the file defines is oa- prefixed.
        for (i, _) in css.match_indices('.') {
            let rest = &css[i + 1..];
            let starts_class = rest.starts_with(|c: char| c.is_ascii_alphabetic())
                && css[..i]
                    .chars()
                    .last()
                    .is_some_and(|c| c.is_whitespace() || ",(>&:".contains(c));
            if starts_class {
                assert!(
                    rest.starts_with("oa-"),
                    "{name}: class .{}",
                    &rest[..12.min(rest.len())]
                );
            }
        }
    }
}

/// The script's pure helpers (step, type-ahead, filter, fallback placement)
/// under node's test runner, when node is installed.
#[test]
fn overlays_js_helpers_pass_node_tests() {
    let crate_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    match std::process::Command::new("node")
        .arg("--test")
        .arg(crate_dir.join("tests/js/overlays.test.js"))
        .output()
    {
        Ok(out) => assert!(
            out.status.success(),
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        ),
        Err(_) => {
            eprintln!("node is not installed; tests/js/overlays.test.js did not run")
        }
    }
}
