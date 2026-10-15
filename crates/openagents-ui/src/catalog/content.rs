//! Content: the Markdown root and its elements, code, tables, sources.

use maud::{Markup, html};

use super::{Pane, row, specimen, stack};
use crate::actions::CopyButton;
use crate::actions::{Button, ButtonVariant, Color, ControlSize};
use crate::content::{
    ActivityStatus, CodeBlock, ColSize, Facts, Favicon, FileChanges, Heading, InlineCode, LinkCard,
    LinkCards, List, ListItem, MarkdownRoot, MarkdownSize, PageColumn, Paragraph, PlanCard,
    PlanCards, PluginCard, PluginCards, ResultCard, Source, SourceVariant, Step, Steps,
    StickyActionBar, Table, ToolCall, ToolGroup,
};
use crate::content::{
    Attachment, Attachments, Bubble, CodeSize, Marker, MarkerTone, MessageScroller, Question,
    Questionnaire, Trace, TraceKind, TraceStatus, TraceStep, Turn,
};
use crate::icons::Icon;
use crate::overlays::Dialog;

const DIFF: &str = "@@ -12,4 +12,4 @@ fn login_button() {\n     let page = open(\"/login\");\n-    page.click(\"#login\");\n+    page.click(\"[data-test=login]\");\n     assert!(page.signed_in());\n";

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
        (specimen("CodeBlock", "Title, line numbers, a marked line, Wrap switch", CodeBlock::new(RUST)
            .title("src/main.rs").language("rust").line_numbers(true).highlight_lines([2]).wrap_toggle(true)))
        (specimen("CodeBlock", "Diff", CodeBlock::new(DIFF).title("crates/web/src/login.rs").language("diff").diff(true).line_numbers(true)))
        (specimen("CodeBlock", "Small, terminal output", CodeBlock::new("$ cargo test -p sample-app\ntest result: ok. 48 passed; 0 failed")
            .title("cargo test -p sample-app").size(CodeSize::Sm)))
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

/// Plan cards as the pricing page shows them.
pub(super) fn plans(_pane: Pane) -> Markup {
    let cards = PlanCards::new("Plans")
        .lead("Start free. Upgrade when you need the cloud.")
        .cards([
            PlanCard::new("Free", "Free")
                .lead("Everything you need to use Coder:")
                .item("Coder on terminal, desktop, web and phone")
                .item_with("Every coding agent", ["Use your own subscriptions"])
                .action("Get Coder", "/download"),
            PlanCard::new("Pro", "$20")
                .per("/mo.")
                .note("3-day free trial")
                .lead("Everything in Free, and:")
                .item_with(
                    "6,000 minutes of cloud time a month",
                    ["2 vCPU, 8 GB machines \u{b7} 2 at once"],
                )
                .action("Start 3-day free trial", "/settings")
                .featured(true),
        ]);
    html! {
        (specimen("PlanCards PlanCard", "Plans side by side", cards))
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
            "The code for this site and our apps, on GitHub.",
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

/// What an answer draws inline: how to connect a codebase, two ways side by
/// side, with tabs for each system's install command and numbered steps.
pub(super) fn answers(pane: Pane) -> Markup {
    use crate::actions::ButtonLink;
    use crate::content::{AnswerCard, AnswerColumns, AnswerSteps, Tabs};
    use maud::Render;
    let tabs = Tabs::new(
        &format!("catalog-{}-os", pane.name().replace(' ', "-")),
        "Install command",
    )
    .tab_for(
        "unix",
        "macOS and Linux",
        CodeBlock::new("curl -fsSL https://openagents.com/cli/install.sh | bash").language("bash"),
    )
    .tab_for(
        "windows",
        "Windows",
        CodeBlock::new("irm https://openagents.com/cli/install.ps1 | iex").language("powershell"),
    );
    let steps = AnswerSteps::new()
        .step("Install Coder", vec![tabs.render()])
        .step(
            "Log in",
            vec![
                CodeBlock::new("coder login").language("bash").render(),
                ButtonLink::new("Approve login", "/device")
                    .variant(ButtonVariant::Outline)
                    .color(Color::Secondary)
                    .render(),
            ],
        );
    let columns = AnswerColumns::new()
        .child(
            AnswerCard::new("On the web")
                .child(html! { p { "Connect GitHub and pick a repository." } })
                .child(ButtonLink::new("Connect GitHub", "/projects")),
        )
        .child(AnswerCard::new("On your computer").child(steps));
    html! {
        (specimen("AnswerColumns AnswerCard AnswerSteps Tabs", "Connect a codebase", MarkdownRoot::new(columns)))
    }
}

pub(super) fn trace(pane: Pane) -> Markup {
    let dialog = pane.id("trace-plan");
    let steps = [
        TraceStep::new(TraceKind::Reasoning, "Thought for 4s")
            .description("The button moved, so the selector is stale.")
            .body(MarkdownRoot::new(html! { p { "The login button moved into the header, so the test clicks nothing. Update the selector, then run the suite." } })),
        TraceStep::new(TraceKind::Read, "Read").detail("tests/login.rs").meta("42 lines"),
        TraceStep::new(TraceKind::Edit, "Edited").detail("tests/login.rs").meta("+1 −1")
            .body(CodeBlock::new(DIFF).language("diff").diff(true).size(CodeSize::Sm)),
        TraceStep::new(TraceKind::Command, "Ran").detail("cargo test -p sample-app").meta("12s")
            .body(CodeBlock::new("test result: ok. 48 passed; 0 failed").title("Output").size(CodeSize::Sm)),
        TraceStep::new(TraceKind::Agent, "Asked Claude Code")
            .description("Check the other login tests for the same selector")
            .meta("3 steps")
            .child(TraceStep::new(TraceKind::Search, "Searched").detail("#login").meta("2 files"))
            .child(TraceStep::new(TraceKind::Command, "Ran").detail("cargo test -p sample-web").status(TraceStatus::Error)
                .description("2 tests failed").body(CodeBlock::new("test result: FAILED. 10 passed; 2 failed").size(CodeSize::Sm)))
            .child(TraceStep::new(TraceKind::Reasoning, "Thinking").status(TraceStatus::Running)),
        TraceStep::new(TraceKind::Source, "Read the testing guide").detail("example.com/testing").href("https://example.com/testing"),
        TraceStep::new(TraceKind::Plan, "Plan").meta("3 steps").dialog(dialog.clone()),
        TraceStep::new(TraceKind::Command, "Run the full suite").status(TraceStatus::Pending),
    ];
    html! {
        (specimen("Trace TraceStep", "Collapsed", Trace::new(steps.clone()).label("What the agent did")))
        (specimen("Trace TraceStep", "A summary opened to its steps", Trace::new([
            TraceStep::new(TraceKind::Summary, "Worked for 1m 12s").meta("8 steps").open(true).children(steps),
        ])))
        (Dialog::new(dialog, "Plan", Trace::new([
            TraceStep::new(TraceKind::Plan, "Update the selector"),
            TraceStep::new(TraceKind::Plan, "Run the suite").status(TraceStatus::Running),
            TraceStep::new(TraceKind::Plan, "Push").status(TraceStatus::Pending),
        ])))
    }
}

pub(super) fn transcript(pane: Pane) -> Markup {
    let questions = |id: &str| {
        Questionnaire::new(pane.id(id), "A few questions first")
            .question(
                Question::new("scope", "Which tests should I fix?")
                    .option("Only the login test", Some("Fastest"))
                    .option("Every test that uses the old button", None)
                    .answer(["Every test that uses the old button"]),
            )
            .question(
                Question::new("push", "Push when they pass?")
                    .option("Yes", None)
                    .option("No", None)
                    .other(true),
            )
    };
    html! {
        (specimen("MessageScroller Turn Bubble Marker Attachment Attachments", "A conversation", MessageScroller::new()
            .label("Conversation")
            .turn(Marker::divider("October 14"))
            .turn(Bubble::new("The login test fails since the redesign. Can you fix it?")
                .attachment(Attachment::new("failure.png").meta("Picture"))
                .attachment(Attachment::new("tests/login.rs").meta("Code")))
            .turn(Turn::new("Coder")
                .model("sample-model")
                .time("2026-10-14T13:02:00Z", "13:02")
                .part(Trace::new([
                    TraceStep::new(TraceKind::Summary, "Worked for 1m 12s").meta("4 steps"),
                ]))
                .part(MarkdownRoot::new(html! { p { "The button moved, so the test clicked nothing. I updated the selector. " strong { "All 48 tests pass." } } }))
                .part(Attachments::new([Attachment::new("tests/login.rs").meta("+1 −1")]).label("Files changed"))
                .action(CopyButton::new("All 48 tests pass.").label("Copy").aria_label("Copy answer").size(ControlSize::Xs).variant(ButtonVariant::Ghost).color(Color::Secondary))
                .meta("1m 12s · 28K tokens"))
            .turn(Marker::note("This chat was shortened here").icon(Icon::InfoCircle))
            .turn(Marker::note("The agent stopped before it finished").tone(MarkerTone::Danger))))
        (specimen("Questionnaire Question", "Questions, answered", questions("questions-shown")))
        (specimen("Questionnaire Question", "Questions, to answer", questions("questions-form").action("#")))
    }
}
