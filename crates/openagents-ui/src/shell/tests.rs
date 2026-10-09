use maud::{Render, html};

use super::*;

fn chat_composer() -> Composer {
    Composer::new("chat-form", "/chat/abc")
        .label("Continue the chat")
        .input_id("chat-input")
        .body_id("chat-card")
        .placeholder("Ask OpenAgents anything")
        .max_chars(8000)
        .hidden(html! { input type="hidden" name="csrf" value="t"; })
        .selectors(
            ComposerDropdown::new("Repository", "openagents")
                .hx(HxGet::new("/composer/repository").target("#composer-panel")),
        )
        .model_picker(ModelPickerTrigger::new("Auto").hx(HxGet::new("/composer/model")))
}

#[test]
fn composer_is_a_plain_post_form_without_javascript() {
    let html = chat_composer().render().into_string();
    assert!(html.contains(
        r#"<form id="chat-form" class="oa-composer-root" action="/chat/abc" method="post""#
    ));
    assert!(html.contains(r#"name="q""#));
    assert!(html.contains(r#"type="submit" class="oa-composer-send""#));
    assert!(html.contains(r#"<label class="oa-visually-hidden" for="chat-input">Message</label>"#));
    assert!(html.contains(r#"id="chat-card""#));
    assert!(html.contains(r#"maxlength="8000""#));
    assert!(html.contains(" required"));
    assert!(!html.to_ascii_lowercase().contains("<script"));
}

#[test]
fn enhanced_composer_posts_with_htmx_and_never_swaps_the_draft() {
    let html = chat_composer().render().into_string();
    assert!(html.contains(r#"hx-post="/chat/abc""#));
    assert!(html.contains(r#"hx-swap="none""#));
    // Absolute, so the selector and picker buttons that inherit it from the
    // form still name the form's own submit button.
    assert!(html.contains(r##"hx-disabled-elt="#chat-form button[type=submit]""##));
    assert!(!html.contains("find button"));
    assert!(html.contains(r#"hx-sync="this:drop""#));
    let plain = chat_composer().enhanced(false).render().into_string();
    assert!(!plain.contains("hx-post") && !plain.contains(r#"hx-swap="none""#));
    assert!(plain.contains(r#"method="post""#));
}

#[test]
fn draft_is_restored_and_escaped() {
    let html = chat_composer()
        .draft("fix </textarea><script>x</script> & go")
        .render()
        .into_string();
    assert!(
        html.contains("fix &lt;/textarea&gt;&lt;script&gt;x&lt;/script&gt; &amp; go</textarea>")
    );
}

#[test]
fn status_region_is_live_and_described() {
    let composer = chat_composer().status(html! { p { "Not sent. Try again." } });
    let html = composer.render().into_string();
    assert_eq!(composer.status_id(), "chat-form-status");
    assert!(html.contains(
        r#"id="chat-form-status" class="oa-composer-status" role="status" aria-live="polite""#
    ));
    assert!(html.contains(r#"aria-describedby="chat-form-status""#));
    assert!(html.contains("Not sent. Try again."));
}

#[test]
fn selectors_and_picker_load_panels_with_htmx() {
    let html = chat_composer().render().into_string();
    assert!(html.contains(r#"<div class="oa-composer-selectors">"#));
    assert!(html.contains(r#"aria-label="Repository: openagents""#));
    assert!(html.contains(r#"hx-get="/composer/repository""#));
    assert!(html.contains(r##"hx-target="#composer-panel""##));
    assert!(html.contains(r#"aria-label="Model: Auto""#));
    assert!(html.contains(r#"hx-get="/composer/model""#));
    // Selectors and picker are buttons, so they never submit the form.
    assert_eq!(html.matches(r#"type="submit""#).count(), 1);
}

#[test]
fn default_ids_derive_from_the_form_id() {
    let html = Composer::new("ask", "/ask").render().into_string();
    assert!(html.contains(r#"id="ask-input""#));
    assert!(html.contains(r#"id="ask-body""#));
    assert!(html.contains(r#"id="ask-status""#));
}

#[test]
fn disabled_composer_disables_input_and_send() {
    let html = Composer::new("c", "/c")
        .disabled(true)
        .render()
        .into_string();
    assert_eq!(html.matches(" disabled").count(), 2);
}

#[test]
fn sidebar_has_one_toggle_that_collapses_or_opens_the_drawer() {
    let shell = |collapsed| {
        AppShell::new()
            .sidebar(
                Sidebar::new()
                    .brand(html! { a href="/" { "OpenAgents" } })
                    .nav(NavItem::new("New chat", "/").current(true))
                    .section(SidebarSection::new("Chats").item(NavItem::new("<b>", "/chat/1")))
                    .bottom(NavItem::new("Docs", "/docs"))
                    .footer(LegalLinks::new().link("Terms", "/terms")),
            )
            .sidebar_collapsed(collapsed)
            .content(html! { p { "x" } })
            .render()
            .into_string()
    };
    let html = shell(false);
    assert!(html.contains(
        r#"<aside id="oa-left-panel" class="oa-left-panel" popover="auto" aria-label="Sidebar">"#
    ));
    assert_eq!(html.matches("data-oa-sidebar-toggle").count(), 1, "{html}");
    assert!(html.contains(
        r#"popovertarget="oa-left-panel" aria-controls="oa-left-panel" aria-label="Toggle sidebar""#
    ));
    assert!(
        !html.contains("popovertargetaction"),
        "one toggle both opens and closes"
    );
    // The toggle sits in the panel's header, not the main header.
    let header = html.find("oa-sidebar-header").unwrap();
    let toggle = html.find("oa-sidebar-toggle").unwrap();
    let main = html.find("oa-main-header").unwrap();
    assert!(header < toggle && toggle < main);
    assert!(html.contains(r#"data-sidebar="expanded""#));
    assert!(shell(true).contains(r#"data-sidebar="collapsed""#));
    assert!(html.contains(r#"<nav class="oa-navigation" aria-label="Main">"#));
    assert!(html.contains(r#"href="/" aria-current="page""#));
    assert!(html.contains("&lt;b&gt;"));
    // Docs and the legal links sit at the bottom, after the sections.
    let chats = html.find("/chat/1").unwrap();
    let docs = html.find(r#"href="/docs""#).unwrap();
    let terms = html.find(r#"href="/terms""#).unwrap();
    assert!(chats < docs && docs < terms);
    assert!(html.contains(r#"<nav class="oa-sidebar-bottom" aria-label="More">"#));
    assert!(html.contains(r##"<a class="oa-skip-link" href="#content">"##));
    assert!(
        html.contains(r#"<main id="content" class="oa-workspace" tabindex="-1"><p>x</p></main>"#)
    );
    let css = SHELL_CSS;
    assert!(css.contains(".oa-left-panel[popover]:not(:popover-open)"));
    assert!(css.contains(r#".oa-layout[data-sidebar="collapsed"] .oa-left-panel"#));
    assert!(css.contains(":root:not([data-oa-sidebar-ready]) .oa-sidebar-toggle"));
    assert!(css.contains("@media (max-width: 47.999rem)"));
    let script = crate::script();
    assert!(script.contains("[data-oa-sidebar-toggle]") && script.contains("oa_sidebar"));
    assert!(SIDEBAR_COOKIE == "oa_sidebar" && SIDEBAR_TOGGLE_ATTR == "data-oa-sidebar-toggle");
    assert!(sidebar_collapsed_from_cookie(" collapsed") && !sidebar_collapsed_from_cookie("x"));
}

#[test]
fn chat_list_marks_the_open_chat_and_stays_quiet_when_empty() {
    let list = ChatList::new()
        .id("chat-sidebar")
        .chat("First <chat>", "/chat/1", true)
        .chat("Second", "/chat/2", false)
        .render()
        .into_string();
    assert!(list.contains(r#"id="chat-sidebar""#));
    assert!(list.contains(r#"<h2 class="oa-sidebar-section-title">Chats</h2>"#));
    assert!(list.contains(r#"href="/chat/1" aria-current="page""#));
    assert!(list.contains("First &lt;chat&gt;"));
    assert!(list.find("/chat/1").unwrap() < list.find("/chat/2").unwrap());
    let empty = ChatList::new().id("chat-sidebar").swap_oob(true);
    assert!(empty.is_empty());
    let empty = empty.render().into_string();
    assert_eq!(
        empty,
        r#"<section class="oa-sidebar-section oa-chat-list" id="chat-sidebar" hx-swap-oob="outerHTML" aria-label="Chats"></section>"#
    );
}

#[test]
fn chat_rows_carry_a_detail_line_and_a_plain_status() {
    let list = ChatList::new()
        .item(
            NavItem::new("Fix <the> build", "/chat/1")
                .detail("acme/app · main")
                .trailing(ChatStatus::Working),
        )
        .item(NavItem::new("Plain", "/chat/2").trailing(ChatStatus::PausedUntil("3:40 PM".into())))
        .render()
        .into_string();
    assert!(list.contains(
        r#"<span class="oa-nav-item-label oa-nav-item-label--stacked"><span class="oa-nav-item-title">Fix &lt;the&gt; build</span><span class="oa-nav-item-detail">acme/app · main</span></span>"#
    ));
    assert!(list.contains(r#"<span class="oa-nav-item-label">Plain</span>"#));
    // Working is a spinner with no words beside it; screen readers hear it.
    assert!(list.contains(r#"data-status="working""#) && list.contains(r#"aria-label="Working""#));
    assert!(!list.contains(">Working<") && list.contains("oa-loading-indicator"));
    assert!(list.contains(r#"data-status="paused""#) && list.contains("Paused until 3:40 PM"));
    let labels: Vec<String> = [
        ChatStatus::Working,
        ChatStatus::WaitingForYou,
        ChatStatus::PausedUntil("3:40 PM".into()),
        ChatStatus::Done,
        ChatStatus::Failed,
    ]
    .iter()
    .map(ChatStatus::label)
    .collect();
    assert_eq!(
        labels,
        [
            "Working",
            "Waiting for you",
            "Paused until 3:40 PM",
            "Done",
            "Failed"
        ]
    );
    for class in [
        ".oa-nav-item-label--stacked",
        ".oa-nav-item-detail",
        ".oa-chat-status-dot",
        r#".oa-chat-status[data-status="failed"]"#,
    ] {
        assert!(SHELL_CSS.contains(class), "{class}");
    }
}

#[test]
fn legal_links_are_quiet_and_mark_external_links() {
    let html = LegalLinks::new()
        .link("Terms", "/terms")
        .link("GitHub", "https://github.com/x")
        .note("(c) 2026")
        .render()
        .into_string();
    assert!(html.contains(r#"<nav class="oa-sidebar-legal" aria-label="Legal and links">"#));
    assert!(html.contains(r#"<a href="/terms">Terms</a>"#));
    assert!(html.contains(r#"<a href="https://github.com/x" rel="noopener">GitHub</a>"#));
    assert!(html.contains(r#"<span class="oa-sidebar-legal-note">(c) 2026</span>"#));
}

#[test]
fn composer_script_submits_on_enter_unless_an_adapter_handled_it() {
    let script = crate::script();
    assert!(script.contains("form[data-oa-composer]") || script.contains("data-oa-composer"));
    assert!(script.contains("event.defaultPrevented"));
    assert!(script.contains("event.shiftKey") && script.contains("isComposing"));
    assert!(script.contains("requestSubmit"));
    assert!(!script.contains("eval(") && !script.contains("innerHTML"));
}

#[test]
fn shell_without_sidebar_has_no_sidebar_toggle() {
    let html = AppShell::new().render().into_string();
    assert!(!html.contains("oa-sidebar-toggle") && !html.contains("popover"));
    assert!(html.contains(r#"data-sidebar="none""#));
}

#[test]
fn app_mode_docks_the_composer_and_drops_the_footer() {
    let shell = AppShell::new()
        .footer(html! { footer { "legal" } })
        .composer(Composer::new("chat-form", "/chat"));
    let page = shell.clone().render().into_string();
    assert!(page.contains(r#"data-mode="scroll""#) && page.contains("legal"));
    let app = shell.mode(MainMode::App).render().into_string();
    assert!(app.contains(r#"data-mode="app""#) && !app.contains("legal"));
    assert!(app.contains(r#"<div class="oa-main-composer"><section class="oa-composer""#));
    assert!(app.contains(r#"class="oa-main-top-fade" aria-hidden="true""#));
}

#[test]
fn theme_toggle_carries_the_script_hook() {
    let html = ThemeToggle::new().render().into_string();
    assert!(html.contains(&format!(r#"{THEME_TOGGLE_ATTR}="""#)));
    assert!(html.contains(r#"type="button""#));
    assert!(html.contains(r#"aria-label="Toggle light and dark theme""#));
    let fallback = ThemeToggle::new()
        .fallback_action("/theme")
        .return_to("/chat")
        .render()
        .into_string();
    assert!(
        fallback.contains(r#"<form class="oa-theme-toggle-form" method="post" action="/theme">"#)
    );
    assert!(fallback.contains(r#"type="submit""#) && fallback.contains(r#"value="toggle""#));
    assert!(fallback.contains(r#"name="return_to" value="/chat""#));
}

#[test]
fn document_sets_the_cookie_theme_or_follows_the_system() {
    let system = Document::new("Chat").render().into_string();
    assert!(system.starts_with("<!DOCTYPE html><html lang=\"en\"><head>"));
    assert!(system.contains("<title>Chat \u{b7} OpenAgents</title>"));
    assert!(system.contains(r#"content="light dark""#));
    let dark = Document::new("OpenAgents")
        .theme(Theme::from_cookie("dark"))
        .render()
        .into_string();
    assert!(dark.contains(r#"<html lang="en" data-theme="dark">"#));
    assert!(dark.contains("<title>OpenAgents</title>"));
    assert_eq!(Theme::from_cookie("system"), None);
    assert_eq!(THEME_COOKIE, "oa_theme");
}

#[test]
fn product_roles_are_defined_in_the_apps_sdk_naming_scheme() {
    for role in [
        "--color-background-composer-surface:",
        "--color-background-user-message:",
        "--color-text-user-message:",
        "--color-text-composer-primary:",
    ] {
        assert!(COMPOSER_CSS.contains(role), "{role}");
    }
}

#[test]
fn every_rendered_class_has_a_rule() {
    let shell = AppShell::new()
        .sidebar(
            Sidebar::new()
                .nav(
                    NavItem::new("Home", "/")
                        .icon(html! { "i" })
                        .trailing(html! { "1" }),
                )
                .section(SidebarSection::new("Chats").empty("No chats yet"))
                .footer(ThemeToggle::new().fallback_action("/theme")),
        )
        .actions(ThemeToggle::new())
        .composer(
            chat_composer()
                .attachments(html! {})
                .dropdown(
                    ComposerDropdown::new("Branch", "main")
                        .show_label(true)
                        .icon(html! { "b" }),
                )
                .model_picker(ModelPickerTrigger::new("Auto").effort("High")),
        )
        .render()
        .into_string();
    let extra = html! {
        (ComposerAction::new(html! { "+" }, "Add").hx(HxGet::new("/x")))
        (composer_panel_host("composer-panel"))
        (ComposerPanel::new("Model").close(HxGet::new("/close")).body(html! { p { "x" } }))
        (Message::user("hi"))
        (Message::assistant(html! { p { "hello" } }))
        (Message::status("failed"))
    };
    let shell = format!("{shell}{}", extra.into_string());
    let css = format!("{SHELL_CSS}{COMPOSER_CSS}{THREAD_CSS}");
    let mut missing = Vec::new();
    for chunk in shell.split("class=\"").skip(1) {
        let classes = chunk.split('"').next().unwrap();
        for class in classes.split_whitespace() {
            if !css.contains(&format!(".{class}")) {
                missing.push(class.to_owned());
            }
        }
    }
    assert!(missing.is_empty(), "classes without rules: {missing:?}");
}

#[test]
fn composer_actions_and_panels_load_with_htmx_and_never_submit() {
    let html = chat_composer()
        .leading(
            ComposerAction::new(html! { "+" }, "Add context and tools").hx(HxGet::new(
                "/composer/context",
            )
            .target("#composer-panel")
            .swap("innerHTML")),
        )
        .trailing(
            ComposerAction::new(html! { "m" }, "Voice input").title("Voice input availability"),
        )
        .after(composer_panel_host("composer-panel"))
        .render()
        .into_string();
    assert!(html.contains(r#"class="oa-composer-action" aria-label="Add context and tools""#));
    assert!(html.contains(r#"hx-get="/composer/context""#));
    assert!(html.contains(r#"title="Voice input availability""#));
    assert!(html.contains(r#"<div id="composer-panel" class="oa-composer-panel-host"></div>"#));
    assert_eq!(html.matches(r#"type="submit""#).count(), 1);

    let panel = ComposerPanel::new("Model")
        .close(HxGet::new("/composer/close").target("#composer-panel"))
        .close_label("Close selection")
        .body(html! { p { "Auto <b>" } })
        .render()
        .into_string();
    assert!(panel.contains(r#"<section class="oa-composer-panel" aria-label="Model">"#));
    assert!(panel.contains(r#"aria-label="Close selection""#));
    assert!(panel.contains(r#"hx-get="/composer/close""#));
    assert!(panel.contains("Auto &lt;b&gt;"));
    assert!(!panel.contains("aria-haspopup"), "close opens nothing");
}

#[test]
fn sidebar_sections_can_be_replaced_and_rows_load_with_htmx() {
    let section = |oob| {
        SidebarSection::new("Chats")
            .id("chat-sidebar")
            .swap_oob(oob)
            .item(
                NavItem::new("First <chat>", "/chat/1")
                    .current(true)
                    .hx(HxGet::new("/chat/1/workspace").target("#chat-content")),
            )
            .after(html! { p { "note" } })
            .render()
            .into_string()
    };
    let page = section(false);
    assert!(page.contains(r#"id="chat-sidebar""#));
    assert!(!page.contains("hx-swap-oob"));
    assert!(
        page.contains(r##"href="/chat/1" hx-get="/chat/1/workspace" hx-target="#chat-content""##)
    );
    assert!(page.contains(r#"aria-current="page""#));
    assert!(page.contains("First &lt;chat&gt;"));
    assert!(page.find("</ul>").unwrap() < page.find("note").unwrap());
    assert!(section(true).contains(r#"hx-swap-oob="outerHTML""#));
    let plain = NavItem::new("Home", "/").render().into_string();
    assert!(!plain.contains("hx-"));
}

#[test]
fn messages_escape_text_and_keep_authors_for_screen_readers() {
    let user = Message::user("<script>x</script>\nline")
        .id("m-0")
        .render()
        .into_string();
    assert!(user.contains(r#"<article class="oa-message" data-role="user" id="m-0">"#));
    assert!(user.contains(r#"<h2 class="oa-message-author oa-visually-hidden">You</h2>"#));
    assert!(user.contains("&lt;script&gt;x&lt;/script&gt;\nline"));
    let assistant = Message::assistant(maud::PreEscaped("<p>ok</p>"))
        .author("OpenAgents")
        .render()
        .into_string();
    assert!(assistant.contains(r#"data-role="assistant""#));
    assert!(assistant.contains("OpenAgents</h2>"));
    assert!(assistant.contains(r#"<div class="oa-message-content"><p>ok</p></div>"#));
    let status = Message::status("Failed").render().into_string();
    assert!(status.contains(r#"<h2 class="oa-message-author">Status</h2>"#));
    assert_eq!(Message::status("x").role(), MessageRole::Status);
}

#[test]
fn stylesheets_are_balanced_and_use_the_oa_prefix() {
    for css in [SHELL_CSS, COMPOSER_CSS, THREAD_CSS] {
        assert_eq!(css.matches('{').count(), css.matches('}').count());
        for line in css.lines() {
            let line = line.trim_start();
            if line.starts_with('.') {
                assert!(line.starts_with(".oa-"), "{line}");
            }
        }
    }
}

#[test]
fn breadcrumb_sits_in_the_header_row_before_the_actions() {
    let html = AppShell::new()
        .sidebar(Sidebar::new())
        .breadcrumb(Breadcrumb::new("so does <this> work").crumb("Chats", "/chats"))
        .actions(html! { a href="/download" { "Download" } })
        .render()
        .into_string();
    let crumb = html
        .find(r#"<nav id="oa-breadcrumb" class="oa-breadcrumb" aria-label="Breadcrumb">"#)
        .expect("breadcrumb in the header");
    let header = html
        .find(r#"<div class="oa-main-header-content">"#)
        .unwrap();
    let actions = html
        .find(r#"<div class="oa-main-header-actions">"#)
        .unwrap();
    assert!(header < crumb && crumb < actions);
    assert!(html.contains(r#"<a class="oa-breadcrumb-link" href="/chats">Chats</a>"#));
    assert!(html.contains(
        r#"<span class="oa-breadcrumb-current" aria-current="page" title="so does &lt;this&gt; work">so does &lt;this&gt; work</span>"#
    ));
    let oob = Breadcrumb::new("x").swap_oob(true).render().into_string();
    assert!(oob.contains(r#"hx-swap-oob="outerHTML""#) && BREADCRUMB_ID == "oa-breadcrumb");
    assert!(
        SHELL_CSS.contains(".oa-breadcrumb-current {")
            && SHELL_CSS.contains("text-overflow: ellipsis")
    );
}

#[test]
fn scroll_to_bottom_is_an_icon_button_hidden_until_the_script_shows_it() {
    let html = ScrollToBottom::new("#chat-thread").render().into_string();
    assert!(html.starts_with(
        r##"<button type="button" class="oa-scroll-bottom" data-oa-scroll-bottom="#chat-thread" aria-label="Scroll to bottom" title="Scroll to bottom" hidden>"##
    ));
    assert!(html.contains("oa-icon") && !html.contains(">Latest<"));
    assert!(
        SCROLL_TO_BOTTOM_ATTR == "data-oa-scroll-bottom"
            && SCROLL_TAIL_ATTR == "data-oa-scroll-tail"
    );
    let script = crate::script();
    assert!(script.contains("IntersectionObserver") && script.contains("[data-oa-scroll-tail]"));
    assert!(script.contains(r#"behavior: reduce ? "auto" : "smooth""#));
    let css = crate::stylesheet();
    assert!(css.contains(".oa-scroll-bottom[hidden]"));
    assert!(THREAD_CSS.contains(".oa-thread-view {\n  position: relative;"));
}

#[test]
fn root_layout_is_one_fixed_height_screen_that_never_bounces() {
    let css = SHELL_CSS;
    let root = &css[css.find("html:has(> body.oa-body) {").expect("root rule")..];
    let root = &root[..root.find('}').unwrap()];
    assert!(root.contains("height: 100dvh;") && root.contains("overflow: hidden;"));
    assert!(root.contains("overscroll-behavior: none;"));
    let body = &css[css.find(".oa-body {").unwrap()..];
    let body = &body[..body.find('}').unwrap()];
    assert!(
        body.contains("height: 100vh;\n  height: 100dvh;") && body.contains("overflow: hidden;")
    );
    for region in ["\n.oa-conversation-sidebar {", "\n.oa-main-viewport {"] {
        let rule = &css[css.find(region).unwrap()..];
        let rule = &rule[..rule.find('}').unwrap()];
        assert!(rule.contains("overscroll-behavior: contain;"), "{region}");
    }
    assert!(THREAD_CSS.contains("overscroll-behavior: contain;"));
    let doc = Document::new("x").render().into_string();
    assert!(doc.contains("interactive-widget=resizes-content"));
}

#[test]
fn new_chat_row_carries_its_shortcut_and_the_send_button_follows_the_text() {
    let row = NavItem::new("New chat", "/")
        .shortcut("Control+N", "⌃N")
        .render()
        .into_string();
    assert!(row.contains(r#"aria-keyshortcuts="Control+N" title="New chat (⌃N)""#));
    assert!(row.contains(r#"<kbd class="oa-nav-shortcut" aria-hidden="true">⌃N</kbd>"#));
    let script = crate::script();
    assert!(script.contains(r#""Control+" + event.key.toUpperCase()"#));
    assert!(script.contains("event.metaKey") && script.contains("parts.send.disabled = true"));
    // Without the script the send button is enabled; the server rejects an
    // empty message.
    let composer = chat_composer().render().into_string();
    assert!(composer.contains(
        r#"<button type="submit" class="oa-composer-send" aria-label="Send" title="Send">"#
    ));
    assert!(composer.contains(r#"rows="1""#));
    assert!(COMPOSER_CSS.contains("background: var(--color-background-disabled);"));
}

#[test]
fn account_menu_opens_above_the_account_button() {
    let html = AccountMenu::new("ada@example.com")
        .item(crate::overlays::MenuItem::link(
            "Settings",
            "/cloud/app/settings",
        ))
        .render()
        .into_string();
    assert!(html.contains(r#"<div class="oa-account">"#));
    assert!(html.contains(r#"aria-label="Account: ada@example.com""#));
    assert!(html.contains(r#"data-side="top""#) && html.contains(r#"role="menu""#));
    assert!(html.contains(r#"<span class="oa-account-name">ada@example.com</span>"#));
    assert!(html.contains(r#"href="/cloud/app/settings""#));
}

#[test]
fn chat_rows_organize_with_plain_forms_and_a_search_box() {
    let menu = RowMenu::new("chat-menu-1", "Fix <it>")
        .action(
            RowAction::post("Pin", "/chat/1/pin")
                .icon(crate::icons::Icon::Pin)
                .field("pinned", "1")
                .target("#chat-sidebar")
                .swap("outerHTML"),
        )
        .action(RowAction::get("Rename", "/chat/1/rename").target("#chat-row-1"))
        .action(RowAction::post("Archive", "/chat/1/archive").confirm("Archive it?"));
    let list = ChatList::new()
        .id("chat-sidebar")
        .search(ChatSearch::new("/chat/list", "#chat-sidebar-rows").field("current", "1"))
        .notice(html! { "Chat archived." })
        .pinned([NavItem::new("Pinned one", "/chat/2")])
        .item(
            NavItem::new("Fix <it>", "/chat/1")
                .row_id("chat-row-1")
                .menu(menu),
        )
        .after(html! { p class="oa-chat-list-more" { a class="oa-chat-list-link" href="/a" { "Archived" } } })
        .render()
        .into_string();
    // Pinned sits above the chats, in one box a search replaces.
    let pinned = list.find(">Pinned<").unwrap();
    assert!(pinned < list.find(">Chats<").unwrap());
    assert!(list.contains(r#"id="chat-sidebar-rows""#));
    assert!(list.find("role=\"search\"").unwrap() < list.find("chat-sidebar-rows").unwrap());
    assert!(list.contains(r##"hx-select="#chat-sidebar-rows""##));
    assert!(list.contains(r#"<input type="hidden" name="current" value="1">"#));
    // Each menu entry submits its own hidden form, which works without HTMX.
    assert!(list.contains(r#"<li class="oa-nav-row oa-nav-row--menu" id="chat-row-1">"#));
    assert!(list.contains(r#"form="chat-menu-1-0""#) && list.contains(r#"form="chat-menu-1-2""#));
    assert!(list.contains(
        r##"<form id="chat-menu-1-0" hidden method="post" action="/chat/1/pin" hx-post="/chat/1/pin" hx-target="#chat-sidebar" hx-swap="outerHTML">"##
    ));
    assert!(list.contains(r#"method="get" action="/chat/1/rename" hx-get="/chat/1/rename""#));
    assert!(list.contains(r#"hx-confirm="Archive it?""#));
    assert!(list.contains(r#"aria-label="Options for Fix &lt;it&gt;""#));
    assert!(list.contains("Chat archived.") && list.contains(">Archived<"));
    // Nothing found: the plain line, still inside the box.
    let none = ChatList::new()
        .id("chat-sidebar")
        .search(ChatSearch::new("/chat/list", "#chat-sidebar-rows").value("zz"))
        .empty("No chats found")
        .render()
        .into_string();
    assert!(none.contains(r#"<p class="oa-sidebar-empty">No chats found</p>"#));
    assert!(none.contains(r#"value="zz""#));

    let rename = RowRename::new("chat-row-1", "/chat/1/rename", "Fix it", "/chat/1")
        .cancel_hx("/chat/list?current=1")
        .field("csrf", "token")
        .target("#chat-sidebar")
        .swap("outerHTML")
        .render()
        .into_string();
    assert!(rename.contains(r#"<li id="chat-row-1" class="oa-nav-row oa-nav-row--editing">"#));
    assert!(rename.contains(r#"method="post" action="/chat/1/rename" hx-post="/chat/1/rename""#));
    assert!(rename.contains(r#"name="title" value="Fix it" required maxlength="120""#));
    assert!(rename.contains(r#"href="/chat/1" hx-get="/chat/list?current=1""#));

    let css = crate::stylesheet();
    let html = format!("{list}{none}{rename}");
    for chunk in html.split("class=\"").skip(1) {
        for class in chunk.split('"').next().unwrap().split_whitespace() {
            assert!(css.contains(&format!(".{class}")), "{class} has no rule");
        }
    }
    let script = crate::script();
    for hook in [
        "[data-oa-chat-rows]",
        "[data-oa-rename-cancel]",
        "BracketLeft",
        "key.toLowerCase() === \"k\"",
    ] {
        assert!(script.contains(hook), "{hook}");
    }
}
