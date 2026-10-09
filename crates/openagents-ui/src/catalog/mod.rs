//! The component catalog (UI-07): every builder in every meaningful variant,
//! grouped like Apps SDK UI's Storybook (Foundations, Actions, Forms,
//! Content, Overlays, Shell), each section side by side in Coder Light and
//! Coder Noir.
//!
//! [`render()`] returns the catalog body: an in-page section nav and the
//! sections. `openagents-web` mounts it at `/ui` inside its page shell.
//!
//! Each theme pane is a `data-theme="light"` or `data-theme="dark"` wrapper,
//! so the same tokens resolve to each theme's values through `light-dark()`.
//! Every element id in a pane carries the pane's prefix (`catalog-light-`,
//! `catalog-dark-`), and so do form control names, so the two copies never
//! collide with each other or with the page around them.
//!
//! Every specimen is marked with [`MARKER_ATTR`], naming the builders it
//! shows; a test checks every name in [`COMPONENTS`] is marked, and that
//! [`COMPONENTS`] lists every public builder in the crate. Catalog-only
//! styles live in `static/components/catalog.css`; the per-token swatch
//! rules in `static/components/catalog-tokens.css` are generated from
//! [`foundations`]' tables (`OPENAGENTS_UI_BLESS=1 cargo test -p openagents-ui`).
//! No inline styles or scripts: the site CSP is `style-src 'self'` and
//! `script-src 'self'`.

mod actions;
mod content;
mod forms;
pub mod foundations;
mod overlays;
mod shell;

#[cfg(test)]
mod tests;

use maud::{Markup, Render, html};

use crate::shell::Theme;
use crate::tokens::Scheme;

/// The attribute naming the builders a specimen shows (space-separated).
pub const MARKER_ATTR: &str = "data-catalog-component";

/// Every public builder the catalog shows, by type name.
pub const COMPONENTS: &[&str] = &[
    // Foundations
    "Icon",
    "SizedIcon",
    // Actions
    "Button",
    "ButtonLink",
    "CopyButton",
    "TextLink",
    "Badge",
    "LoadingIndicator",
    "Busy",
    "LoadingDots",
    "CircularProgress",
    "Avatar",
    "AvatarGroup",
    "Alert",
    "EmptyMessage",
    "Image",
    "ShimmerText",
    // Forms
    "Field",
    "Input",
    "Textarea",
    "Checkbox",
    "RadioGroup",
    "Switch",
    "SegmentedControl",
    "Slider",
    "Select",
    "DatePicker",
    "DateRangePicker",
    "TagInput",
    // Content
    "MarkdownRoot",
    "Paragraph",
    "Heading",
    "List",
    "ListItem",
    "InlineCode",
    "CodeBlock",
    "StickyActionBar",
    "Table",
    "PageColumn",
    "Facts",
    "Source",
    "Favicon",
    "ToolCall",
    "ToolGroup",
    "Steps",
    "Step",
    "ResultCard",
    "FileChanges",
    "PluginCard",
    "PluginCards",
    "LinkCard",
    "LinkCards",
    // Overlays
    "Popover",
    "Menu",
    "MenuItem",
    "Tooltip",
    "SelectControl",
    "Dialog",
    "DialogTrigger",
    // Shell
    "Composer",
    "ComposerAction",
    "ComposerPanel",
    "Message",
    "TaskRow",
    "SuggestionChips",
    "SuggestionChip",
    "ComposerDropdown",
    "ModelPickerTrigger",
    "HxGet",
    "AppShell",
    "Sidebar",
    "SidebarSection",
    "ChatList",
    "ChatGroup",
    "ChatSearch",
    "RowMenu",
    "RowAction",
    "RowRename",
    "LegalLinks",
    "Breadcrumb",
    "AccountMenu",
    "ScrollToBottom",
    "NavItem",
    "ThemeToggle",
    "Document",
];

/// One catalog section.
#[derive(Clone, Copy, Debug)]
pub struct Section {
    /// The Storybook group: Foundations, Actions, Forms, Content, Overlays
    /// or Shell.
    pub group: &'static str,
    /// The anchor id of the section.
    pub id: &'static str,
    pub title: &'static str,
    body: fn(Pane) -> Markup,
}

/// Every section, in page order.
pub const SECTIONS: &[Section] = &[
    section("Foundations", "colors", "Colors", foundations::colors),
    section(
        "Foundations",
        "typography",
        "Typography",
        foundations::typography,
    ),
    section(
        "Foundations",
        "radius-shadow",
        "Radius and shadow",
        foundations::radius_shadow,
    ),
    section("Foundations", "icons", "Icons", foundations::icons),
    section("Actions", "buttons", "Buttons", actions::buttons),
    section("Actions", "links", "Links", actions::links),
    section("Actions", "badges", "Badges", actions::badges),
    section("Actions", "indicators", "Indicators", actions::indicators),
    section("Actions", "avatars", "Avatars", actions::avatars),
    section("Actions", "alerts", "Alerts", actions::alerts),
    section(
        "Actions",
        "empty-message",
        "Empty message",
        actions::empty_message,
    ),
    section("Actions", "media", "Image and shimmer", actions::media),
    section("Forms", "text-fields", "Text fields", forms::text_fields),
    section("Forms", "choices", "Choices", forms::choices),
    section("Forms", "select", "Select", forms::select),
    section("Forms", "slider", "Slider", forms::slider),
    section("Forms", "dates", "Dates", forms::dates),
    section("Forms", "tags", "Tag input", forms::tags),
    section("Content", "markdown", "Markdown", content::markdown),
    section("Content", "code", "Code", content::code),
    section("Content", "table", "Table", content::table),
    section("Content", "sources", "Sources", content::sources),
    section("Content", "activity", "Agent activity", content::activity),
    section("Content", "plugins", "Plugin cards", content::plugins),
    section("Content", "link-cards", "Link cards", content::link_cards),
    section("Overlays", "popover", "Popover", overlays::popover),
    section("Overlays", "menu", "Menu", overlays::menu),
    section("Overlays", "tooltip", "Tooltip", overlays::tooltip),
    section(
        "Overlays",
        "select-control",
        "SelectControl",
        overlays::select_control,
    ),
    section("Overlays", "dialog", "Dialog", overlays::dialog),
    section("Shell", "composer", "Composer", shell::composer),
    section("Shell", "app-shell", "App shell", shell::app_shell),
    section("Shell", "theme", "Theme toggle", shell::theme),
    section("Shell", "document", "Document", shell::document),
];

const fn section(
    group: &'static str,
    id: &'static str,
    title: &'static str,
    body: fn(Pane) -> Markup,
) -> Section {
    Section {
        group,
        id,
        title,
        body,
    }
}

/// The Storybook groups, in page order.
pub const GROUPS: [&str; 6] = [
    "Foundations",
    "Actions",
    "Forms",
    "Content",
    "Overlays",
    "Shell",
];

/// One theme pane of a section.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pane {
    pub theme: Theme,
}

/// The two panes every section shows, Coder Light first.
pub const PANES: [Pane; 2] = [
    Pane {
        theme: Theme::Light,
    },
    Pane { theme: Theme::Dark },
];

impl Pane {
    /// The theme's display name.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self.theme {
            Theme::Light => "Coder Light",
            Theme::Dark => "Coder Noir",
        }
    }

    /// The token scheme the pane resolves.
    #[must_use]
    pub const fn scheme(self) -> Scheme {
        match self.theme {
            Theme::Light => Scheme::Light,
            Theme::Dark => Scheme::Dark,
        }
    }

    /// `local`, prefixed so it is unique to this pane.
    #[must_use]
    pub fn id(self, local: &str) -> String {
        format!("catalog-{}-{local}", self.theme.as_str())
    }
}

/// The whole catalog: the section nav and every section in both themes.
#[must_use]
pub fn render() -> Markup {
    html! {
        div class="oa-catalog" {
            header class="oa-catalog-intro" {
                h1 class="oa-catalog-intro__title" { "Components" }
                p class="oa-catalog-intro__lede" {
                    "Every openagents-ui builder in its variants, in Coder Light and Coder Noir."
                }
            }
            (nav())
            (foundations::icon_sprite())
            @for group in GROUPS {
                section class="oa-catalog-group" aria-labelledby=(format!("group-{}", slug(group))) {
                    h2 class="oa-catalog-group__title" id=(format!("group-{}", slug(group))) { (group) }
                    @for section in SECTIONS.iter().filter(|s| s.group == group) {
                        (render_section(section))
                    }
                }
            }
        }
    }
}

fn nav() -> Markup {
    html! {
        nav class="oa-catalog-nav" aria-label="Catalog sections" {
            @for group in GROUPS {
                div class="oa-catalog-nav__group" {
                    a class="oa-catalog-nav__heading" href=(format!("#group-{}", slug(group))) { (group) }
                    ul class="oa-catalog-nav__list" role="list" {
                        @for section in SECTIONS.iter().filter(|s| s.group == group) {
                            li { a class="oa-catalog-nav__link" href=(format!("#{}", section.id)) { (section.title) } }
                        }
                    }
                }
            }
        }
    }
}

fn render_section(section: &Section) -> Markup {
    let heading = format!("{}-title", section.id);
    html! {
        section class="oa-catalog-section" id=(section.id) aria-labelledby=(heading) {
            h3 class="oa-catalog-section__title" id=(heading) { (section.title) }
            div class="oa-catalog-themes" {
                @for pane in PANES {
                    div class="oa-catalog-theme" data-theme=(pane.theme.as_str())
                        role="group" aria-label=(format!("{} in {}", section.title, pane.name())) {
                        p class="oa-catalog-theme__name" { (pane.name()) }
                        div class="oa-catalog-theme__body" { ((section.body)(pane)) }
                    }
                }
            }
        }
    }
}

fn slug(text: &str) -> String {
    text.to_ascii_lowercase().replace(' ', "-")
}

/// One labelled specimen, marked with the builders it shows.
pub(crate) fn specimen(components: &str, title: &str, body: impl Render) -> Markup {
    html! {
        div class="oa-catalog-specimen" data-catalog-component=(components) {
            p class="oa-catalog-specimen__title" { (title) }
            div class="oa-catalog-specimen__body" { (body.render()) }
        }
    }
}

/// A wrapping row of items.
pub(crate) fn row(items: Markup) -> Markup {
    html! { div class="oa-catalog-row" { (items) } }
}

/// A vertical stack of items.
pub(crate) fn stack(items: Markup) -> Markup {
    html! { div class="oa-catalog-stack" { (items) } }
}

/// A small caption under or beside an item.
pub(crate) fn caption(text: &str) -> Markup {
    html! { span class="oa-catalog-caption" { (text) } }
}
