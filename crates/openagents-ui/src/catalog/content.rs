//! Content: the Markdown root and its elements, code, tables, sources.

use maud::{Markup, html};

use super::{Pane, row, specimen, stack};
use crate::actions::{Button, ButtonVariant, Color, ControlSize};
use crate::content::{
    ActivityStatus, CodeBlock, ColSize, Facts, Favicon, FileChanges, Heading, InlineCode, LinkCard,
    LinkCards, List, ListItem, MarkdownRoot, MarkdownSize, PageColumn, Paragraph, PluginCard,
    PluginCards, ResultCard, Source, SourceVariant, Step, Steps, StickyActionBar, Table, ToolCall,
    ToolGroup,
};
use crate::icons::Icon;

const RUST: &str = "fn main() {\n    let catalog = openagents_ui::catalog::render();\n    println!(\"{}\", catalog.into_string());\n}";

pub(super) fn markdown(pane: Pane) -> Markup {
    let body = html! {
        (Heading::new(2, "Shipping the catalog").id(pane.id("md-heading")))
        (Paragraph::new(html! {
            "The catalog renders every builder. Run " (InlineCode::new("cargo test -p openagents-ui"))
            " before you push."
        }))
        (Heading::new(3, "Checklist"))
        (List::unordered().items(["Tokens", "Components", "Both themes"]))
        (Heading::new(4, "Order"))
        (List::ordered().start(3).item("Rebase").item("Test").item("Push"))
        (Heading::new(5, "Level five"))
        (Heading::new(6, "Level six"))
        ul class="oa-list" data-variant="unordered" {
            (ListItem::new(html! { "A lone " strong { "ListItem" } }))
        }
    };
    html! {
        (specimen("MarkdownRoot Heading Paragraph List ListItem InlineCode", "Markdown, md", MarkdownRoot::new(body).label("Sample message")))
        (specimen("MarkdownRoot", "Markdown, sm", MarkdownRoot::new(html! {
            p { "Dense panels use the small size. Plain " code { "<p>" } " and " a href="/docs" { "links" } " from the renderer are styled too." }
            blockquote { p { "A quote from the Markdown renderer." } }
        }).size(MarkdownSize::Sm)))
        (specimen("Heading", "Heading levels", stack(html! {
            @for level in 1..=6_u8 {
                (Heading::new(level, format!("Heading {level}")))
            }
        })))
    }
}

pub(super) fn code(_pane: Pane) -> Markup {
    html! {
        (specimen("CodeBlock", "Code block with language", CodeBlock::new(RUST).language("rust")))
        (specimen("CodeBlock", "Wrapped, custom copy icon", CodeBlock::new("openagents issue claim 11021 && cargo test -p openagents-ui --quiet -- --nocapture --test-threads 1")
            .language("sh").wrap(true).copy_icon(Icon::Copy)))
        (specimen("CodeBlock", "Not copyable, no language", CodeBlock::new("plain output\nsecond line").copyable(false)))
        (specimen("CodeBlock", "Pre-highlighted markup", CodeBlock::new("let x = 1;").language("rust")
            .highlighted(html! { span class="hljs-keyword" { "let" } " x = " span class="hljs-number" { "1" } ";" })))
        (specimen("StickyActionBar", "Sticky action bar", StickyActionBar::new().label("diff.patch")
            .aria_label("Patch actions")
            .action(Button::icon(Icon::Copy, "Copy patch").size(ControlSize::Xs).variant(ButtonVariant::Ghost).color(Color::Secondary))
            .action(Button::icon(Icon::Download, "Download patch").size(ControlSize::Xs).variant(ButtonVariant::Ghost).color(Color::Secondary))))
    }
}

pub(super) fn table(_pane: Pane) -> Markup {
    html! {
        (specimen("Table", "Table", Table::new().label("Agent runs")
            .caption("Runs in the last day")
            .header(["Agent", "Status", "Tokens", "Cost"])
            .row(["Coder", "Done", "12,480", "$0.42"])
            .row(["Reviewer", "Running", "3,102", "$0.08"])
            .row(["Planner", "Queued", "0", "$0.00"])
            .numeric(2).numeric(3)
            .col_size(0, ColSize::Lg).col_size(1, ColSize::Sm).col_size(2, ColSize::Md)))
        (specimen("PageColumn Facts", "Reading column with facts", PageColumn::new(
            Facts::new()
                .fact("Jobs", html! { "12" })
                .fact("Median time", html! { "4.2 s" })
                .fact("Pass rate", html! { "97%" }),
        )))
    }
}

pub(super) fn sources(_pane: Pane) -> Markup {
    html! {
        (specimen("Source Favicon", "Compact", row(html! {
            (Source::new("Apps SDK UI", "https://github.com/openai/apps-sdk-ui"))
            (Source::new("Docs", "/docs").favicon(Favicon::new("openagents.com").src("/favicon.svg")))
            (Source::new("Rust book", "https://doc.rust-lang.org/book/").extra(3))
            (Source::new("Unsafe link", "javascript:alert(1)"))
        })))
        (specimen("Source Favicon", "Leading", stack(html! {
            (Source::new("Apps SDK UI", "https://github.com/openai/apps-sdk-ui").variant(SourceVariant::Leading))
            (Source::new("OpenAgents docs", "/docs").variant(SourceVariant::Leading)
                .favicon(Favicon::new("openagents.com").src("/favicon.svg")).extra(2))
        })))
    }
}

pub(super) fn activity(_pane: Pane) -> Markup {
    let calls = ToolGroup::new("Explored the repository")
        .call(ToolCall::new(Icon::Search, "Searched").detail("rust-toolchain* in /workspace"))
        .call(ToolCall::new(Icon::FileDocument, "Read").detail("Cargo.toml"))
        .open(true);
    let failed = ToolCall::new(Icon::Terminal, "Ran")
        .detail("cargo fetch --locked")
        .status(ActivityStatus::Failed)
        .status_label("Exit 101")
        .body(CodeBlock::new("error: the lock file needs to be updated").copyable(false));
    html! {
        (specimen("ToolCall", "Tool calls", stack(html! {
            (ToolCall::new(Icon::FileDocument, "Read").detail("AGENTS.md"))
            (ToolCall::new(Icon::Terminal, "Running").detail("cargo build --release").status(ActivityStatus::Running))
            (failed)
        })))
        (specimen("ToolGroup ToolCall", "Grouped calls", calls))
        (specimen("Steps Step", "Progress steps", Steps::new("Environment setup")
            .step(Step::new("Discover the repository", ActivityStatus::Done))
            .step(Step::new("Build a clean image", ActivityStatus::Running).detail("Installing from the trusted base"))
            .step(Step::new("Verify a fresh machine", ActivityStatus::Waiting))))
        (specimen("ResultCard", "Result card", ResultCard::new("Root Rust v1")
            .subtitle("Ready to save")
            .badge(ActivityStatus::Done.badge("Verified"))
            .fact("Toolchain", "Rust 1.97.1")
            .fact("Checks", "4 passed")
            .footer("Saving selects this version for new tasks.")))
        (specimen("FileChanges", "Changed files", FileChanges::new()
            .file("crates/coder-lease/src/table.rs", 18, 4)
            .file("crates/coder-lease/tests/corrupt.rs", 31, 0)))
    }
}

/// Plugin cards as a chat answer shows them: Coder's built-in plugins.
pub(super) fn plugins(_pane: Pane) -> Markup {
    let card = |name: &str, icon: Icon, summary: &str| {
        PluginCard::new(name, icon, summary)
            .runs_on("With Coder on your computer")
            .action("Get Coder", "/download")
    };
    let cards = PluginCards::new("Plugins").cards([
        card(
            "Claude Code",
            Icon::Assistant,
            "Coder hands a task to Claude Code on your computer and shows its progress as it \
             works.",
        ),
        card(
            "Codex",
            Icon::Code,
            "Coder hands a task to Codex on your computer and shows its progress as it works.",
        ),
        card(
            "Cursor",
            Icon::Cursor,
            "Coder hands a task to Cursor's agent on your computer and shows its progress as it \
             works.",
        ),
    ]);
    html! {
        (specimen("PluginCards PluginCard", "Plugins in an answer", cards))
        (specimen("PluginCard", "A card with no action", PluginCard::new(
            "OpenRouter",
            Icon::ApiKey,
            "Use OpenRouter models in Coder with your own API key.",
        )))
    }
}

/// Link cards as the new chat shows them: the whole card is the link.
pub(super) fn link_cards(_pane: Pane) -> Markup {
    let cards = LinkCards::new("Learn about OpenAgents").cards([
        LinkCard::new(
            "Explore the Verse",
            "A shared world you walk around in.",
            "/docs/verse",
        )
        .icon(Icon::EarthTravelWorld),
        LinkCard::new(
            "Meet Coder",
            "An AI coding assistant in your terminal.",
            "/docs/coder",
        )
        .icon(Icon::Terminal),
        LinkCard::new(
            "Tour the codebase",
            "Everything we build, open on GitHub.",
            "https://github.com/OpenAgentsInc/openagents",
        )
        .icon(Icon::Code),
        LinkCard::new(
            "Start with the basics",
            "What OpenAgents is and how to get it.",
            "/docs/what-is-openagents",
        )
        .icon(Icon::BookOpen),
    ]);
    html! {
        (specimen("LinkCards LinkCard", "Learn about cards", cards))
        (specimen("LinkCard", "A card without an icon", LinkCard::new(
            "Read the docs",
            "Guides for the website, apps, and Coder.",
            "/docs",
        )))
    }
}
