//! Shell: Composer, AppShell with Sidebar, ThemeToggle, Document.

use maud::{Markup, PreEscaped, Render, html};

use super::{Pane, row, specimen, stack};
use crate::actions::{Button, ButtonVariant, Color, ControlSize};
use crate::content::CodeBlock;
use crate::icons::Icon;
use crate::overlays::MenuItem;
use crate::shell::{
    AccountMenu, AppShell, Breadcrumb, ChatList, Composer, ComposerAction, ComposerDropdown,
    ComposerPanel, Document, HxGet, LegalLinks, MainMode, Message, ModelPickerTrigger, NavItem,
    ScrollToBottom, Sidebar, SidebarSection, Theme, ThemeToggle,
};

/// The id `AppShell` gives its left panel. The page around the catalog has
/// its own shell with that id, so preview copies get a pane-local one.
const SHELL_LEFT_PANEL_ID: &str = "oa-left-panel";

pub(super) fn composer(pane: Pane) -> Markup {
    let full = Composer::new(pane.id("composer"), "/ui")
        .label("Task composer")
        .enhanced(false)
        .placeholder("Describe a task")
        .input_label("Task")
        .max_chars(4000)
        .rows(2)
        .hidden(html! { input type="hidden" name="source" value="catalog"; })
        .dropdown(
            ComposerDropdown::new("Repository", "openagents")
                .icon(Icon::Folder)
                .popover(pane.id("composer-repo")),
        )
        .dropdown(
            ComposerDropdown::new("Branch", "main")
                .icon(Icon::Branch)
                .show_label(true)
                .hx(HxGet::new("/composer/branches")
                    .target(format!("#{}", pane.id("composer-status")))
                    .include(format!("#{}", pane.id("composer")))
                    .swap("innerHTML")
                    .sync("this:replace")),
        )
        .model_picker(
            ModelPickerTrigger::new("Max")
                .effort("High")
                .popover(pane.id("composer-model")),
        )
        .leading(
            Button::icon(Icon::Paperclip, "Attach files")
                .variant(ButtonVariant::Ghost)
                .color(Color::Secondary)
                .size(ControlSize::Sm),
        )
        .trailing(
            Button::icon(Icon::Mic, "Dictate")
                .variant(ButtonVariant::Ghost)
                .color(Color::Secondary)
                .size(ControlSize::Sm),
        )
        .status(html! { "Ready" });
    let draft = Composer::new(pane.id("composer-draft"), "/ui")
        .enhanced(false)
        .draft("Fix the flaky login test")
        .model_picker(ModelPickerTrigger::new("Mini").hx(HxGet::new("/composer/models")))
        .send_label("Start task");
    let rich = Composer::new(pane.id("composer-rich"), "/ui")
        .enhanced(true)
        .hx_include(format!("#{}", pane.id("composer-rich")))
        .input_id(pane.id("composer-rich-text"))
        .body_id(pane.id("composer-rich-body"))
        .name("prompt")
        .autofocus(false)
        .selectors(html! { span class="oa-catalog-caption" { "Selectors slot" } })
        .attachments(html! { span class="oa-catalog-caption" { "notes.md attached" } })
        .send(Button::icon(Icon::ArrowRight, "Send").size(ControlSize::Sm));
    let off = Composer::new(pane.id("composer-off"), "/ui")
        .enhanced(false)
        .placeholder("Sign in to start a task")
        .disabled(true);
    html! {
        (specimen("Composer ComposerDropdown ModelPickerTrigger HxGet", "Full composer", full))
        (specimen("Composer ModelPickerTrigger HxGet", "Draft and send label", draft))
        (specimen("Composer", "HTMX-enhanced, custom slots", rich))
        (specimen("Composer", "Disabled", off))
        (specimen("ComposerAction ComposerPanel", "Footer actions and a panel", stack(html! {
            (row(html! {
                (ComposerAction::new(Icon::Plus, "Add context").title("Add context"))
                (ComposerAction::new(Icon::Mic, "Dictate"))
            }))
            (ComposerPanel::new("Repository").body(html! { p { "Choose a repository for this task." } }))
        })))
        (specimen("Message", "Thread messages", stack(html! {
            (Message::user("Add a light theme to the web app"))
            (Message::assistant(html! { p { "Coder Light is on. The toggle follows your system setting." } }).author("OpenAgents"))
            (Message::status("Working…"))
        })))
        (specimen("ComposerDropdown ModelPickerTrigger", "Triggers alone", row(html! {
            (ComposerDropdown::new("Environment", "Cloud").icon(Icon::Globe).show_label(true))
            (ModelPickerTrigger::new("Max").effort("Medium"))
        })))
    }
}

pub(super) fn app_shell(pane: Pane) -> Markup {
    let sidebar = Sidebar::new()
        .label("Preview sidebar")
        .brand(html! { a class="oa-wordmark" href="/ui" { "OpenAgents" } })
        .nav(
            NavItem::new("New chat", "/ui")
                .icon(Icon::ComposeEditSquare)
                .shortcut("Control+N", "⌃N"),
        )
        .section(
            ChatList::new()
                .chat("Catalog review", "/ui#app-shell", true)
                .chat("Token audit", "/ui#colors", false)
                .chat("Icon sweep", "/ui#icons", false),
        )
        .section(SidebarSection::new("Pinned").empty("Nothing pinned yet"))
        .bottom(
            NavItem::new("Docs", "/docs")
                .icon(Icon::Code)
                .trailing(html! { span class="oa-catalog-caption" { "3" } }),
        )
        .footer(
            AccountMenu::new("Ada Lovelace")
                .id(pane.id("account-menu"))
                .item(MenuItem::link("Settings", "/ui#app-shell"))
                .item(MenuItem::separator())
                .item(MenuItem::link("Docs", "/docs")),
        );
    let shell = AppShell::new()
        .sidebar(sidebar)
        .breadcrumb(Breadcrumb::new("A long chat title that truncates")
                .crumb("Chats", "/ui")
                .id(pane.id("breadcrumb")),
        )
        .actions(
            Button::new("Share")
                .size(ControlSize::Sm)
                .variant(ButtonVariant::Outline)
                .color(Color::Secondary),
        )
        .content(html! {
            p { "The main frame scrolls this content." }
            (LegalLinks::new().link("Terms", "/terms").link("Privacy", "/privacy").note("Legal links"))
        })
        .footer(html! { p class="oa-catalog-caption" { "Footer slot" } })
        .mode(MainMode::Scroll)
        .main_id(pane.id("shell-main"));
    let app = AppShell::new()
        .breadcrumb(Breadcrumb::new("App mode, no sidebar").id(pane.id("app-breadcrumb")))
        .content(html! {
            div id=(pane.id("shell-thread-view")) class="oa-thread-view" {
                section id=(pane.id("shell-thread")) class="oa-thread" aria-label="Preview thread" {
                    div class="oa-thread-column" { p { "App pages fill the frame and dock the composer." } }
                }
                (ScrollToBottom::new(format!("#{}", pane.id("shell-thread"))))
            }
        })
        .composer(
            Composer::new(pane.id("shell-composer"), "/ui")
                .enhanced(false)
                .placeholder("Ask OpenAgents anything"),
        )
        .mode(MainMode::App)
        .main_id(pane.id("shell-app-main"));
    html! {
        (specimen("AppShell Sidebar SidebarSection NavItem ChatList LegalLinks Breadcrumb AccountMenu", "Scroll mode with sidebar", preview(pane, &shell)))
        (specimen("AppShell Composer ScrollToBottom", "App mode with docked composer", preview(pane, &app)))
    }
}

/// An app shell in a fixed-height frame. The shell's `<main>` becomes a
/// `div` (the page has its own `<main>`), and its left panel gets a
/// pane-local id.
fn preview(pane: Pane, shell: &AppShell) -> Markup {
    let panel = pane.id("left-panel");
    let html = shell
        .render()
        .into_string()
        .replace(
            &format!("id=\"{SHELL_LEFT_PANEL_ID}\""),
            &format!("id=\"{panel}\""),
        )
        .replace(
            &format!("popovertarget=\"{SHELL_LEFT_PANEL_ID}\""),
            &format!("popovertarget=\"{panel}\""),
        )
        .replace(
            &format!("aria-controls=\"{SHELL_LEFT_PANEL_ID}\""),
            &format!("aria-controls=\"{panel}\""),
        )
        .replace(
            "<main id=",
            "<div role=\"region\" aria-label=\"Preview content\" id=",
        )
        .replace("</main>", "</div>");
    html! { div class="oa-catalog-frame" { (PreEscaped(html)) } }
}

pub(super) fn theme(_pane: Pane) -> Markup {
    html! {
        (specimen("ThemeToggle", "Theme toggle", row(html! {
            (ThemeToggle::new())
            (ThemeToggle::new().label("Switch theme").light_icon(Icon::Star).dark_icon(Icon::Sparkles))
            (ThemeToggle::new().fallback_action("/theme").return_to("/ui"))
        })))
    }
}

pub(super) fn document(_pane: Pane) -> Markup {
    let document = Document::new("Components")
        .theme(Some(Theme::Dark))
        .head(html! { link rel="stylesheet" href="/static/ui.css"; })
        .body(html! { p { "Page body" } });
    let system = Document::new("OpenAgents").theme(None);
    html! {
        (specimen("Document", "The <html> element, rendered as source", stack(html! {
            (CodeBlock::new(document.render().into_string()).language("html").wrap(true))
            (CodeBlock::new(system.render().into_string()).language("html").wrap(true))
        })))
    }
}
