//! Public-source Coder fixtures assembled from reusable presentation components.

use super::{CatalogEntry, CatalogIntent, FixtureState, SourceRef, entry};
use crate::{
    components::{self as c, Component, conversation as chat},
    source_theme as t,
};
use chat::{Agent, Parameter, Status, ToolKind};
use rust_native::{
    style::{Color, Viewport},
    view::Element,
};

const UI: &str = "crates/coder-new/src/ui.rs";
const TOOLS: &str = "crates/coder-new/src/tools.rs";
const MD: &str = "crates/coder-terminal/src/markdown.rs";
const DIFF: &str = "crates/coder-terminal/src/components/diff.rs";
const MAIN_DIFF: &str = "@@ -48 +48 @@\n-let cursor = (\"▏\", SetCursorStyle::SteadyBar);\n+let cursor = (\"█\", SetCursorStyle::BlinkingBlock);";
const SAMPLE_MD: &str = "# Heading one\n\n## Heading two\n\n### Heading three\n\n#### Heading four\n\n##### Heading five\n\n###### Heading six\n\nA paragraph with **bold**, _italic_, ~~removed~~, `inline code`, and [a link](https://example.com).\n\n> A quote.\n>\n> A second paragraph.\n\n1. Ordered item\n2. Another item\n   - Nested item\n\n- [x] Completed task\n- [ ] Pending task\n\n---\n\n```rust\nfn main() {\n    println!(\"Hello, world\");\n}\n```\n\n![Image alternative](https://example.com/image.png)\n\n<div>Literal HTML</div>";
const TABLE: &str = "| Component | Status | Notes |\n| :--- | :---: | ---: |\n| Composer | **Ready** | Keeps graphemes and independent drafts |\n| Transcript | Streaming | Selectable text, bounded pages, source cursors |";

pub(super) fn entries() -> Vec<CatalogEntry> {
    let specs = &[
        (
            "foundations.theme",
            "Foundations",
            "Source theme",
            "crates/coder-new/src/theme.rs",
            "usgc_lines,usgc_style,usgc_color",
            vec!["tokens", "modifiers", "remapping", "resolved-styles"],
        ),
        (
            "foundations.text",
            "Foundations",
            "Text and cell geometry",
            UI,
            "span,truncate,wrap_display",
            vec![
                "plain", "rich", "unicode", "ellipsis", "wrapping", "empty", "narrow",
            ],
        ),
        (
            "screen.main",
            "Full screens",
            "Coder conversation",
            UI,
            "render,conversation,live_conversation,live_lines,composer_view,agent_rail",
            vec![
                "demo",
                "child-claude-code",
                "child-codex",
                "child-devin-cli",
                "child-grok-build",
                "live-empty",
                "live",
                "live-child",
                "streaming",
                "stopped",
                "nested",
                "error",
                "restored",
                "pending",
                "long",
                "slash",
                "multiline",
                "overflow",
                "narrow",
                "minimum",
                "tiny",
            ],
        ),
        (
            "conversation.context",
            "Conversation",
            "Header and repository context",
            UI,
            "header_view,context_view",
            vec![
                "main",
                "child",
                "default",
                "absent",
                "long-directory",
                "long-branch",
                "narrow",
            ],
        ),
        (
            "conversation.prompt",
            "Conversation",
            "User prompt",
            UI,
            "prompt,message_body",
            vec![
                "plain",
                "empty",
                "multiline",
                "markdown",
                "nested",
                "unicode",
                "wrapping",
                "narrow",
            ],
        ),
        (
            "conversation.reply",
            "Conversation",
            "Assistant reply and attribution",
            UI,
            "entry_lines,reply_lines,live_lines",
            vec![
                "complete",
                "streaming",
                "partial",
                "stopped",
                "model-only",
                "elapsed-only",
                "both",
                "absent",
                "long-model",
                "narrow",
            ],
        ),
        (
            "conversation.markdown",
            "Conversation",
            "Markdown body",
            MD,
            "wrapped,blocks_lines,block_lines,item_lines,inlines",
            vec![
                "all",
                "headings",
                "inline",
                "lists",
                "quotes",
                "rule",
                "code-rust",
                "code-unknown",
                "code-open",
                "indented-code",
                "links-images",
                "literal-html",
                "narrow",
            ],
        ),
        (
            "conversation.tables",
            "Conversation",
            "Markdown tables",
            MD,
            "table_lines,table_widths,stacked_table",
            vec![
                "boxed",
                "wrapped",
                "marked-header",
                "long-values",
                "stacked",
                "empty",
                "narrow",
            ],
        ),
        (
            "tools.demo",
            "Tools",
            "Demo native tools",
            TOOLS,
            "tool_lines",
            vec![
                "read.done",
                "read.running",
                "read.failed",
                "search.done",
                "search.running",
                "search.failed",
                "edit.done",
                "edit.running",
                "edit.failed",
                "run.done",
                "run.running",
                "run.failed",
                "multiline",
                "long-input",
                "narrow",
            ],
        ),
        (
            "tools.diff",
            "Tools",
            "Diff and syntax rows",
            DIFF,
            "hunks,lines,assemble_rows,render_gutter,render_content_spans",
            vec![
                "edit",
                "equal",
                "insert",
                "delete",
                "multiple-hunks",
                "wrapped",
                "long-word",
                "narrow",
                "unknown-language",
                "separate-syntax",
                "empty",
            ],
        ),
        (
            "tools.plugin",
            "Tools",
            "Demo plugin calls",
            TOOLS,
            "plugin_lines",
            vec![
                "done",
                "running",
                "failed",
                "no-input",
                "long-operation",
                "grouped",
                "narrow",
            ],
        ),
        (
            "tools.live",
            "Tools",
            "Live tools and plugin results",
            UI,
            "entry_lines",
            vec![
                "run",
                "read",
                "edit",
                "search",
                "plugin",
                "running",
                "failed",
                "brainstorm-search",
                "brainstorm-rank",
                "brainstorm-observation",
                "brainstorm-partial",
                "brainstorm-expired",
                "brainstorm-discovery",
                "brainstorm-unavailable",
                "brainstorm-error",
                "brainstorm-error-no-recipient",
                "brainstorm-enrichment-error",
                "narrow",
            ],
        ),
        (
            "tools.parameters",
            "Tools",
            "Parameters and results",
            TOOLS,
            "parameter_lines",
            vec![
                "null",
                "empty",
                "scalar",
                "string",
                "newline",
                "nested",
                "deep",
                "array",
                "five",
                "omitted",
                "long-key",
                "long-value",
                "narrow",
            ],
        ),
        (
            "agents.delegation",
            "Agents",
            "Delegation activity",
            TOOLS,
            "delegation_lines,pulse",
            vec![
                "demo",
                "live-running",
                "live-done",
                "live-failed",
                "nested",
                "narrow",
                "long-task",
            ],
        ),
        (
            "agents.rail",
            "Agents",
            "Agent rail",
            UI,
            "agent_rail,token_count",
            vec![
                "four",
                "empty",
                "one",
                "many",
                "selected",
                "last-selected",
                "narrow",
                "long-name",
                "long-task",
                "elapsed",
                "finished",
                "token-counts",
            ],
        ),
        (
            "composer.input",
            "Composer",
            "Framed composer",
            UI,
            "composer_view",
            vec![
                "empty",
                "single",
                "multiline",
                "overflow",
                "child",
                "caret-off",
                "unicode",
                "paste",
                "per-chat",
                "narrow",
                "minimum",
            ],
        ),
        (
            "composer.rails",
            "Composer",
            "Composer contributions",
            UI,
            "composer_rail_text,composer_view",
            vec![
                "top",
                "bottom",
                "both",
                "selected-model",
                "fallback",
                "absent",
                "disabled",
                "priority",
                "conflict",
                "suffix",
                "narrow",
            ],
        ),
        (
            "pickers.slash",
            "Pickers",
            "Slash command suggestions",
            "crates/coder-new/src/slash.rs",
            "matches,render,help",
            vec![
                "all",
                "production",
                "prefix",
                "selected",
                "window",
                "empty",
                "invalid-prefix",
                "demo-off",
                "narrow",
                "short",
                "multiline",
            ],
        ),
        (
            "status.notice",
            "Status",
            "Status and animation",
            UI,
            "conversation,live_conversation",
            vec![
                "spinner",
                "pulse",
                "caret",
                "elapsed",
                "tokens",
                "working",
                "notice",
                "error",
                "stopped",
                "unavailable",
                "exported",
                "copied",
                "clipboard-error",
                "demo-acknowledgment",
            ],
        ),
        (
            "evidence.export",
            "Evidence",
            "Export and restored conversation",
            "crates/coder-new/src/trajectory.rs",
            "document,main_document,export_app,read,restore_app",
            vec![
                "parent",
                "child",
                "pending",
                "partial",
                "complete",
                "failed",
                "redacted",
                "validation-error",
                "restored",
            ],
        ),
    ];
    specs
        .iter()
        .map(|(id, family, title, path, symbols, variants)| {
            let mut item = entry(id, family, title, path, symbols, variants);
            item.sources = symbols
                .split(',')
                .flat_map(|symbol| {
                    variants.iter().map(move |variant| SourceRef {
                        path: (*path).into(),
                        symbol: symbol.into(),
                        branch: format!("fixture:{variant}; {}", branch(id, variant)),
                    })
                })
                .collect();
            if *id == "tools.live" {
                item.sources.extend(
                    variants
                        .iter()
                        .filter(|variant| variant.starts_with("brainstorm-"))
                        .map(|variant| SourceRef {
                            path: "crates/coder-new/src/brainstorm.rs".into(),
                            symbol: "summary".into(),
                            branch: format!(
                                "fixture:{variant}; typed {} result projection",
                                variant.trim_start_matches("brainstorm-")
                            ),
                        }),
                );
            }
            item
        })
        .collect()
}

fn branch(id: &str, variant: &str) -> String {
    match id {
        "screen.main" => format!(
            "render viewport/layout and {} conversation composition",
            variant
        ),
        "tools.demo" => format!("tool_lines kind/state/output branch {variant}"),
        "conversation.markdown" | "conversation.tables" => {
            format!("Markdown block/layout {variant}")
        }
        "tools.parameters" => format!("parameter_lines value/depth/omission {variant}"),
        "conversation.reply" => format!("reply_lines text/model/elapsed {variant}"),
        _ => format!("visible presentation branch {variant}"),
    }
}

pub(super) fn defaults(state: &mut FixtureState) {
    match state.variant.as_str() {
        "child-claude-code" => state.selected = 1,
        "child-codex" => state.selected = 2,
        "child-devin-cli" => state.selected = 3,
        "child-grok-build" => state.selected = 4,
        "live-child" => state.selected = 1,
        "single" => state.draft = "Review the shared Coder components.".into(),
        "unicode" => state.draft = "日本語 · café · 👩🏽‍💻 · 🇯🇵".into(),
        "paste" => state.draft = "Pasted text stays local.\nNo implicit submission.".into(),
        "slash" => state.draft = "/".into(),
        "caret-off" => {
            state.flags.insert("caret-hidden".into(), true);
        }
        _ => {}
    }
    if state.component == "pickers.slash" {
        state.draft = match state.variant.as_str() {
            "prefix" => "/m",
            "empty" => "/unknown",
            "invalid-prefix" => "/Help",
            _ => "/",
        }
        .into();
    }
    if state.variant == "stacked" {
        state.width = 8;
    }
    if state.variant == "last-selected" {
        state.selected = 4;
    }
    if state.variant == "narrow"
        && matches!(
            state.component.as_str(),
            "agents.rail" | "agents.delegation"
        )
    {
        state.width = 24;
    }
}

pub(super) fn render(id: &str, variant: &str, state: &FixtureState) -> Option<Component> {
    if !entries()
        .iter()
        .any(|entry| entry.id == id && entry.variants.iter().any(|v| v.id == variant))
    {
        return None;
    }
    let w = usize::from(state.width);
    let phase = state.phase;
    Some(match id {
        "foundations.theme" => theme(variant, w),
        "foundations.text" => text_fixture(variant, w),
        "screen.main" => screen(variant, state),
        "conversation.context" => {
            let title = if variant == "child" {
                c::rich(
                    "context-header",
                    vec![c::bold(c::run("claude-code", t::ACCENT_MODEL))],
                )
            } else {
                c::blank("context-header")
            };
            let directory = if variant == "long-directory" {
                "a-very-long-repository-directory-name-that-preserves-the-branch"
            } else {
                "openagents"
            };
            let branch = if variant == "long-branch" {
                "codex/a-branch-longer-than-the-entire-context-row"
            } else {
                "main"
            };
            c::column(
                "context",
                vec![title, chat::context("context-footer", directory, branch, w)],
            )
        }
        "conversation.prompt" => chat::prompt(
            "prompt",
            match variant {
                "empty" => "",
                "multiline" => "First line.\n\nThird line.",
                "markdown" => "**Review** the `composer` and [contract](https://example.com).",
                "nested" => "> Quoted input\n\n- [ ] A nested task\n  - Additional context",
                "unicode" => "日本語 · café · 👩🏽‍💻 · 🇯🇵",
                "wrapping" => {
                    "Review this long prompt across whole words, preserving inline styles, continuation indentation, and the full-width raised band."
                }
                _ => "Review the terminal with four agents.",
            },
            w,
        ),
        "conversation.reply" => {
            let model = if matches!(variant, "absent" | "elapsed-only") {
                None
            } else if variant == "long-model" {
                Some("provider/a-very-long-actual-served-model-identifier-that-does-not-fit")
            } else {
                Some("grok-build")
            };
            let elapsed = if matches!(variant, "absent" | "model-only" | "streaming" | "partial") {
                None
            } else {
                Some(1250 + state.elapsed * 1000)
            };
            chat::reply(
                "reply",
                if variant == "streaming" || variant == "partial" {
                    "A reply still **streaming\n\n```rust\nlet partial ="
                } else if variant == "stopped" {
                    "This reply stopped after its partial result."
                } else {
                    "The input keeps pasted text local and restores each conversation's draft. Left and Right move through whole graphemes."
                },
                model,
                elapsed,
                w,
            )
        }
        "conversation.markdown" => {
            c::rows("markdown", c::markdown::lines(markdown_source(variant), w))
        }
        "conversation.tables" => c::rows(
            "table",
            c::markdown::lines(
                match variant {
                    "empty" => "| Name | Value |\n| --- | --- |",
                    "long-values" => {
                        "| Name | Value |\n| --- | --- |\n| VeryLongUnbrokenLabel | 日本語日本語日本語日本語 and a value that wraps |"
                    }
                    _ => TABLE,
                },
                w,
            ),
        ),
        "tools.demo" => {
            let kind = if variant.starts_with("read") {
                ToolKind::Read
            } else if variant.starts_with("search") {
                ToolKind::Search
            } else if variant.starts_with("edit") {
                ToolKind::Edit
            } else {
                ToolKind::Run
            };
            let status = if variant.ends_with("running") {
                Status::Running
            } else if variant.ends_with("failed") {
                Status::Failed
            } else {
                Status::Done
            };
            let input = if variant == "long-input" {
                "cargo test -p coder-new switching_restores_each_conversations_draft_cursor_messages_and_scroll"
            } else {
                match kind {
                    ToolKind::Read => "docs/coder-new/",
                    ToolKind::Search => "\"composer|agent_rail\" crates/coder-new/src/",
                    ToolKind::Edit => "crates/coder-new/src/main.rs",
                    ToolKind::Run => "cargo fmt -p coder-new --check",
                }
            };
            let output = if kind == ToolKind::Edit && status == Status::Done {
                MAIN_DIFF
            } else if variant == "multiline" {
                "First result row\nSecond result row\nThird result row"
            } else if status == Status::Failed {
                "The synthetic command returned an error."
            } else if status == Status::Running {
                "Checking source behavior"
            } else {
                match kind {
                    ToolKind::Read => "4 documents reviewed",
                    ToolKind::Search => "8 matches in 3 files",
                    _ => "Exit 0 · formatting passed",
                }
            };
            chat::demo_tool("tool", kind, input, output, status, phase, w)
        }
        "tools.diff" => c::rows(
            "diff",
            c::diff::lines(
                diff_source(variant),
                if variant == "unknown-language" {
                    "unknown"
                } else {
                    "sample.rs"
                },
                w,
            ),
        ),
        "tools.plugin" => {
            let status = if variant == "running" {
                Status::Running
            } else if variant == "failed" {
                Status::Failed
            } else {
                Status::Done
            };
            let call = chat::plugin(
                "plugin",
                if variant == "long-operation" {
                    "plugin-with-a-long-name"
                } else {
                    "terminal-inspector"
                },
                if variant == "long-operation" {
                    "namespace.operation.with.a.long.name"
                } else {
                    "layout.inspect"
                },
                if variant == "no-input" {
                    ""
                } else {
                    "110×36 · composer and agent rail"
                },
                if status == Status::Failed {
                    "Synthetic tool error"
                } else {
                    "4 agent rows · aligned names, tasks, and tokens"
                },
                status,
                phase,
                w,
            );
            if variant == "grouped" {
                c::column(
                    "grouped-tools",
                    vec![
                        chat::demo_tool(
                            "grouped-read",
                            ToolKind::Read,
                            "docs/coder-new/",
                            "4 documents reviewed",
                            Status::Done,
                            phase,
                            w,
                        ),
                        call,
                    ],
                )
            } else {
                call
            }
        }
        "tools.live" => live_fixture(variant, state),
        "tools.parameters" => chat::parameters("parameters", &parameters(variant), w),
        "agents.delegation" => {
            let mut agent = agents(state.elapsed)[0].clone();
            agent.status = match variant {
                "live-done" => Status::Done,
                "live-failed" => Status::Failed,
                _ => Status::Running,
            };
            if variant == "long-task" {
                agent.task="A very long delegated task with a name that must truncate while keeping status and tokens visible".into();
            }
            let call = chat::delegation(
                "delegate",
                &agent,
                matches!(variant, "demo" | "narrow" | "long-task"),
                phase,
                w,
            );
            if variant == "nested" {
                let mut nested = agent.clone();
                nested.name = "codex child".into();
                nested.task = "Nested task linked to parent delegation".into();
                c::column(
                    "nested-delegation",
                    vec![
                        call,
                        chat::delegation("nested-child", &nested, false, phase, w),
                    ],
                )
            } else {
                call
            }
        }
        "agents.rail" => {
            let mut agents = agents(state.elapsed);
            if variant == "empty" {
                agents.clear();
            }
            if variant == "one" {
                agents.truncate(1);
            }
            if variant == "many" {
                for i in 0..12 {
                    let mut agent = agents[i % 4].clone();
                    agent.name = format!("agent-{i}");
                    agents.push(agent);
                }
            }
            if variant == "long-name" {
                agents[0].name = "a-very-long-agent-display-name".into();
            }
            if variant == "long-task" {
                agents[0].task =
                    "A long task that always ends with an ellipsis before the aligned token suffix"
                        .into();
            }
            if variant == "token-counts" {
                for (agent, count) in agents.iter_mut().zip([0, 999, 1000, 1_000_000]) {
                    agent.tokens = count;
                }
            }
            if variant == "finished" {
                for agent in &mut agents {
                    agent.elapsed = 45;
                    agent.status = Status::Done;
                }
            }
            chat::agent_rail(
                "agents",
                &agents,
                state.selected,
                w,
                if variant == "many" { 4 } else { agents.len() },
            )
        }
        "composer.input" => {
            let main = variant != "child";
            let mut composer = chat::composer(
                "composer",
                &state.draft,
                main,
                Some("openrouter/free"),
                None,
                w,
                6,
            );
            if variant == "per-chat" {
                composer = c::column(
                    "per-chat",
                    vec![
                        chat::agent_rail("draft-agents", &agents(0), state.selected, w, 4),
                        composer,
                    ],
                );
            }
            composer
        }
        "composer.rails" => {
            let top = match variant {
                "absent" | "disabled" | "bottom" => None,
                "fallback" => Some("auto"),
                "suffix" => {
                    Some("very-long-provider/very-long-model-name:reasoning=high,output=4096")
                }
                "priority" | "conflict" => Some("host-selected-provider/model:high"),
                _ => Some("openrouter/free:high"),
            };
            let bottom = if matches!(variant, "bottom" | "both") {
                Some("Synthetic bottom contribution")
            } else {
                None
            };
            chat::composer("rails", &state.draft, true, top, bottom, w, 6)
        }
        "pickers.slash" => {
            let picker = chat::slash(
                "slash",
                &state.draft,
                if variant == "selected" {
                    2
                } else if variant == "window" {
                    6
                } else {
                    state.selected
                },
                variant != "production",
                w,
                if matches!(variant, "window" | "short") {
                    3
                } else {
                    7
                },
            );
            if variant == "multiline" {
                c::column(
                    "slash-compose",
                    vec![
                        picker,
                        chat::composer(
                            "slash-composer",
                            "/\nsecond draft row",
                            true,
                            None,
                            None,
                            w,
                            6,
                        ),
                    ],
                )
            } else {
                picker
            }
        }
        "status.notice" => status_fixture(variant, state),
        "evidence.export" => evidence(variant, state),
        _ => return None,
    })
}

fn theme(variant: &str, width: usize) -> Component {
    let children = match variant {
        "tokens" => t::TOKENS
            .iter()
            .enumerate()
            .map(|(i, (name, color))| {
                let mut value = c::run(
                    format!(
                        " ■ {name:<24} #{:02x}{:02x}{:02x}",
                        color.red, color.green, color.blue
                    ),
                    *color,
                );
                if name.starts_with("BG_") {
                    value.foreground = Some(t::TEXT_PRIMARY);
                    value.background = Some(*color);
                }
                c::rich(format!("token-{i}"), vec![value])
            })
            .collect(),
        "modifiers" => {
            let mut runs = Vec::new();
            for (i, name) in ["plain", "bold", "italic", "underline", "strike", "dim"]
                .iter()
                .enumerate()
            {
                let mut run = c::run(format!(" {name} "), t::TEXT_PRIMARY);
                run.bold = i == 1;
                run.italic = i == 2;
                run.underline = i == 3;
                run.strike = i == 4;
                run.dim = i == 5;
                runs.push(run);
            }
            vec![c::rich("modifiers", runs)]
        }
        "remapping" => [
            Color::rgb(58, 149, 171),
            Color::rgb(157, 124, 216),
            Color::rgb(158, 206, 106),
            Color::rgb(247, 118, 142),
        ]
        .iter()
        .enumerate()
        .map(|(i, color)| {
            c::rich(
                format!("remap-{i}"),
                vec![c::run(
                    format!(
                        "Imported #{:02x}{:02x}{:02x} → source role",
                        color.red, color.green, color.blue
                    ),
                    t::remap(*color),
                )],
            )
        })
        .collect(),
        _ => {
            let mut a = c::text(
                "style-reset",
                "Explicit neutral defaults: source styles do not inherit accent roles.",
                t::TEXT_SECONDARY,
            );
            a.style.background = Some(t::BG_DARK);
            vec![
                c::rich(
                    "style-accent",
                    vec![c::run(
                        c::truncate(
                            "Resolved leaf: cyan → explicit neutral → reset background",
                            width,
                        ),
                        t::ACCENT_SKILL,
                    )],
                ),
                a,
            ]
        }
    };
    c::column("theme", children)
}

fn text_fixture(variant: &str, width: usize) -> Component {
    let text = match variant {
        "empty" => "",
        "unicode" => "日本語 · café · 👩🏽‍💻 · 🇯🇵 · e\u{301}",
        "wrapping" => "the quick brown fox and a wordlongerthantheavailablecellwidth",
        _ => "Coder selectable fixed-cell text",
    };
    if variant == "ellipsis" {
        c::text(
            "ellipsis",
            c::truncate("A source string longer than the width", 12),
            t::TEXT_PRIMARY,
        )
    } else if variant == "rich" {
        c::rich(
            "rich",
            vec![
                c::bold(c::run("Coder ", t::ACCENT_SKILL)),
                c::run("selectable ", t::TEXT_SECONDARY),
                c::run("rich text", t::PATH),
            ],
        )
    } else {
        c::rows("text", c::wrap(&[c::run(text, t::TEXT_PRIMARY)], width))
    }
}

fn markdown_source(variant: &str) -> &'static str {
    match variant {
        "headings" => "# One\n\n## Two\n\n### Three\n\n#### Four\n\n##### Five\n\n###### Six",
        "inline" => {
            "**Bold** _italic_ **_both_** ~~strike~~ `inline code` and [link](https://example.com)."
        }
        "lists" => {
            "1. First ordered item with a long wrapped continuation\n2. Second item\n   - Nested bullet\n\n- [x] Complete\n- [ ] Pending"
        }
        "quotes" => "> First paragraph.\n>\n> Second paragraph.\n>> Nested quote.",
        "rule" => "Above\n\n---\n\nBelow",
        "code-rust" => {
            "```rust\nfn main() {\n    let value = \"hello\";\n    println!(\"{value}\");\n}\n```"
        }
        "code-unknown" => "```unknown\nunknown language stays plain\nwith a quiet code band\n```",
        "code-open" => "A **streaming reply\n\n```rust\nfn partial() {\n    let value = \"half",
        "indented-code" => "    Indented code\n    Another row",
        "links-images" => {
            "[link](https://example.com) and ![alternative](https://example.com/image.png)"
        }
        "literal-html" => {
            "<script>Literal HTML</script>\n\n<div>Selectable source, escaped by the adapter.</div>"
        }
        _ => SAMPLE_MD,
    }
}

fn diff_source(variant: &str) -> &'static str {
    match variant {
        "empty" => "--- a/file\n+++ b/file",
        "equal" => "@@ -1,2 +1,2 @@\n fn main() {\n }",
        "insert" => "@@ -1 +1,2 @@\n+let added = true;\n existing",
        "delete" => "@@ -1,2 +1 @@\n-let removed = true;\n existing",
        "multiple-hunks" => "@@ -1 +1 @@\n-old\n+new\n@@ -10 +10 @@\n-before\n+after",
        "wrapped" | "long-word" => {
            "@@ -99 +99 @@\n-let long_value = \"abcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghij\";\n+let long_value = \"a changed line that wraps without losing gutter alignment or its background\";"
        }
        "separate-syntax" => {
            "@@ -1,3 +1,3 @@\n /*\n-old comment\n+new comment\n */\n@@ -8 +8 @@\n-let next = false;\n+let next = true;"
        }
        _ => MAIN_DIFF,
    }
}

fn parameters(variant: &str) -> Parameter {
    let value = |value: &str| Parameter::Value(value.into());
    let object = |entries: Vec<(&str, Parameter)>| {
        Parameter::Object(
            entries
                .into_iter()
                .map(|(key, value)| (key.into(), value))
                .collect(),
        )
    };
    match variant {
        "null" => Parameter::Null,
        "empty" => Parameter::Object(Vec::new()),
        "scalar" => Parameter::Number(42),
        "string" => value("A plain scalar string"),
        "newline" => object(vec![("text", value("first\nsecond\nthird"))]),
        "nested" => object(vec![(
            "request",
            object(vec![
                ("command", value("cargo fmt")),
                ("directory", value("openagents")),
            ]),
        )]),
        "deep" => object(vec![(
            "request",
            object(vec![(
                "options",
                object(vec![("bounded", value("true")), ("limit", value("5"))]),
            )]),
        )]),
        "array" => object(vec![(
            "args",
            Parameter::Array(vec![value("one"), value("two"), Parameter::Number(3)]),
        )]),
        "five" | "omitted" => Parameter::Object(
            (0..if variant == "five" { 5 } else { 9 })
                .map(|i| (format!("field_{i}"), value(&format!("Value {i}"))))
                .collect(),
        ),
        "long-key" => object(vec![(
            "a_very_long_parameter_name_that_must_truncate",
            value("still visible"),
        )]),
        "long-value" => object(vec![(
            "value",
            value(
                "A very long value whose visible prefix fits while preserving the parameter label and its branch gutter",
            ),
        )]),
        _ => object(vec![
            ("command", value("cargo fmt -p coder-new --check")),
            ("exit_code", value("0")),
            ("output", value("formatting passed")),
        ]),
    }
}

fn live_fixture(variant: &str, state: &FixtureState) -> Component {
    let name = match variant {
        "run" => "Run",
        "read" => "Read",
        "edit" => "Edit",
        "search" => "Search",
        "brainstorm-rank" => "brainstorm_rank",
        _ if variant.starts_with("brainstorm-") => "brainstorm_search_people",
        _ => "terminal-inspector.layout.inspect",
    };
    let status = if variant == "running" {
        Status::Running
    } else if matches!(
        variant,
        "failed" | "brainstorm-error" | "brainstorm-error-no-recipient"
    ) {
        Status::Failed
    } else {
        Status::Done
    };
    let input = Parameter::Object(vec![(
        "query".into(),
        Parameter::Value("public coding profiles".into()),
    )]);
    let output = if status == Status::Failed {
        Parameter::Object(vec![(
            "error".into(),
            Parameter::Value("Synthetic failure".into()),
        )])
    } else {
        parameters("nested")
    };
    let summary = variant
        .starts_with("brainstorm-")
        .then(|| c::brainstorm::summary(&brainstorm_result(variant)));
    chat::live_tool(
        "live-tool",
        name,
        &input,
        &output,
        status,
        state.phase,
        usize::from(state.width),
        summary.as_deref(),
    )
}

fn brainstorm_result(variant: &str) -> c::brainstorm::ResultDisplay {
    use c::brainstorm::{Coverage, Observation, Response, ResultDisplay, Subject};
    let origin = "https://house.example.test";
    let house_key = "f".repeat(64);
    match variant {
        "brainstorm-search" | "brainstorm-rank" => ResultDisplay::Demo,
        "brainstorm-discovery" => ResultDisplay::Discovery {
            origin: origin.into(),
            house_key,
            search: true,
            rank: false,
            discovered_ms: 1_700_000_000_000,
            expires_ms: 1_700_000_300_000,
        },
        "brainstorm-unavailable" => ResultDisplay::Unavailable,
        "brainstorm-error" | "brainstorm-error-no-recipient" => ResultDisplay::Error {
            recipient: (variant == "brainstorm-error").then(|| origin.into()),
            message: "Synthetic lookup unavailable".into(),
        },
        _ => ResultDisplay::Observation(Observation {
            origin: origin.into(),
            house_key,
            discovered_ms: 1_700_000_000_000,
            expires_ms: 1_700_000_300_000,
            fresh: variant != "brainstorm-expired",
            partial: variant == "brainstorm-partial",
            subjects: vec![
                Subject {
                    pubkey: "a".repeat(64),
                    relevance: Some(0.8),
                    influence: if variant == "brainstorm-partial" {
                        None
                    } else {
                        Some(0.0)
                    },
                    coverage: if variant == "brainstorm-partial" {
                        None
                    } else {
                        Some(Coverage::Unknown)
                    },
                    profile_url: "https://profiles.example.test/alice".into(),
                },
                Subject {
                    pubkey: "b".repeat(64),
                    relevance: None,
                    influence: Some(42.0),
                    coverage: Some(Coverage::Reported),
                    profile_url: "https://profiles.example.test/devin".into(),
                },
            ],
            enrichment_error: (variant == "brainstorm-enrichment-error")
                .then(|| "Synthetic rank observation unavailable".into()),
            responses: vec![Response {
                endpoint: origin.into(),
                status: 200,
                algorithm: "Some(Relevance)".into(),
                fetched_ms: 1_700_000_000_001,
                expires_ms: 1_700_000_300_000,
                input_digest: "1".repeat(64),
                output_digest: "2".repeat(64),
            }],
        }),
    }
}

fn agents(elapsed: u64) -> Vec<Agent> {
    [
        ("claude-code", "Reviewing keyboard navigation", 8200, 4358),
        ("codex", "Checking the agent rail placement", 12400, 967),
        ("devin-cli", "Checking conversation switching", 4700, 271),
        ("grok-build", "Verifying the preview colors", 3100, 45),
    ]
    .into_iter()
    .map(|(name, task, tokens, time)| Agent {
        name: name.into(),
        task: task.into(),
        tokens,
        elapsed: time + elapsed,
        status: Status::Running,
    })
    .collect()
}

fn status_fixture(variant: &str, state: &FixtureState) -> Component {
    let w = usize::from(state.width);
    match variant {
        "spinner" => c::rich(
            "spinner",
            vec![c::run(
                format!(" {} Running", c::spinner(state.phase)),
                t::ACCENT_SKILL,
            )],
        ),
        "pulse" => c::rich("pulse", vec![c::run(" ◆ Delegate", c::pulse(state.phase))]),
        "caret" => c::text(
            "caret",
            if state.phase % 2 == 0 {
                "Draft █"
            } else {
                "Draft  "
            },
            t::TEXT_PRIMARY,
        ),
        "elapsed" => c::text("elapsed", c::elapsed(4358 + state.elapsed), t::GRAY),
        "tokens" => c::text(
            "tokens",
            [0, 999, 1000, 8200, 1_000_000]
                .iter()
                .map(|n| chat::token_count(*n))
                .collect::<Vec<_>>()
                .join(" · "),
            t::GRAY,
        ),
        "working" => c::rich(
            "working",
            vec![
                c::run(
                    format!(" {} Working", c::spinner(state.phase)),
                    t::ACCENT_SKILL,
                ),
                c::run(format!(" · {}", c::elapsed(state.elapsed)), t::GRAY),
            ],
        ),
        _ => chat::notice(
            "notice",
            if !state.notice.is_empty() {
                &state.notice
            } else {
                match variant {
                    "error" => "The synthetic reply failed.",
                    "stopped" => "Request stopped.",
                    "unavailable" => "No agent is connected.",
                    "exported" => "Exported this conversation as ATIF.",
                    "copied" => "Copied this conversation as ATIF.",
                    "clipboard-error" => "Could not copy this conversation to the clipboard.",
                    "demo-acknowledgment" => "Preview message added. No agent is connected.",
                    _ => "This is a source-equivalent synthetic notice.",
                }
            },
            variant == "error",
            w,
        ),
    }
}

fn evidence(variant: &str, state: &FixtureState) -> Component {
    let w = usize::from(state.width);
    let mut children = vec![chat::prompt(
        "evidence-prompt",
        "Review the saved conversation fixture.",
        w,
    )];
    if matches!(variant, "pending" | "partial") {
        children.push(chat::live_tool(
            "evidence-pending",
            "Run",
            &parameters("nested"),
            &Parameter::Null,
            Status::Running,
            state.phase,
            w,
            None,
        ));
    } else if variant == "failed" {
        children.push(chat::notice(
            "evidence-failed",
            "The synthetic restored command failed.",
            true,
            w,
        ));
    } else {
        children.push(chat::reply(
            "evidence-reply",
            "Saved conversation restored, showing the model that answered.",
            Some("grok-build"),
            Some(1250),
            w,
        ));
    }
    let notice = match variant {
        "child" => "Selected child export: linked to its parent delegation.",
        "pending" => "Pending command keeps its arguments and has no result yet.",
        "partial" => "Partial reply saved before interruption.",
        "redacted" => "Export omits credentials and private values.",
        "validation-error" => "Could not read this ATIF document: invalid trajectory.",
        "restored" => "Resumed conversation. Saved folder differs from the current folder.",
        _ => "Synthetic ATIF fixture ready. No private session was read.",
    };
    children.extend([
        c::blank("evidence-gap"),
        chat::notice("evidence-notice", notice, variant == "validation-error", w),
        c::action(
            "evidence-export",
            "Export synthetic fixture",
            CatalogIntent::Action {
                name: "export".into(),
            },
        ),
    ]);
    c::column("evidence", children)
}

fn screen(variant: &str, state: &FixtureState) -> Component {
    if let Some(overlay) = state.fields.get("overlay") {
        let mut projected = state.clone();
        projected.component = overlay.clone();
        projected.variant = state
            .fields
            .get("overlay-variant")
            .cloned()
            .unwrap_or_else(|| "default".into());
        if let Some(view) =
            super::settings::render(&projected.component, &projected.variant, &projected)
        {
            if overlay == "models.picker" {
                let mut underlay = state.clone();
                underlay.fields.remove("overlay");
                underlay.selected = state
                    .fields
                    .get("main.selected")
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(0);
                underlay.draft = state.fields.get("main.draft").cloned().unwrap_or_default();
                if let Some(mode) = state.fields.get("main.mode") {
                    underlay.fields.insert("mode".into(), mode.clone());
                } else {
                    underlay.fields.remove("mode");
                }
                let background = if let Some(previous) = state.fields.get("overlay-underlay") {
                    underlay.component = previous.clone();
                    underlay.selected = state
                        .fields
                        .get("underlay.selected")
                        .and_then(|v| v.parse().ok())
                        .unwrap_or(0);
                    underlay.stage = state
                        .fields
                        .get("underlay.stage")
                        .cloned()
                        .unwrap_or_default();
                    super::settings::render(previous, "default", &underlay)
                        .unwrap_or_else(|| screen(variant, &underlay))
                } else {
                    screen(variant, &underlay)
                };
                return c::column("main-overlay", vec![background, view]);
            }
            return view;
        }
    }
    if state.width < 24 || state.height < 12 {
        return c::column(
            "main-screen",
            vec![
                c::text("small-title", "Coder", t::TEXT_SECONDARY),
                c::text("small-resize", "Resize to continue.", t::TEXT_SECONDARY),
            ],
        );
    }
    let w = usize::from(state.width);
    let content_width = w.saturating_sub(4);
    let area = usize::from(state.height).saturating_sub(2);
    let live = state.fields.get("mode").map_or(
        variant.starts_with("live")
            || matches!(
                variant,
                "streaming" | "stopped" | "nested" | "error" | "restored" | "pending" | "long"
            ),
        |mode| mode == "live",
    );
    let agents = if variant == "live-empty" {
        Vec::new()
    } else {
        agents(state.elapsed)
    };
    let rail_height = agents.len().min(area.saturating_sub(6));
    let draft_rows = c::wrap_ranges(&state.draft, w.saturating_sub(3))
        .len()
        .clamp(1, 6);
    let composer_height = (draft_rows + 2).min(area.saturating_sub(rail_height + 3));
    let body_height = area
        .saturating_sub(1 + composer_height + 1 + rail_height)
        .max(1);
    let mut body = if variant == "long" {
        large_transcript_window(state.scroll, content_width, body_height)
    } else if live {
        live_conversation(variant, state, content_width)
    } else if state.selected > 0 {
        demo_child(state.selected, state.phase, content_width)
    } else {
        demo_main(state.phase, content_width, &agents)
    };
    if let Some(message) = state.fields.get("last_message") {
        body.extend([
            chat::prompt("main-added-prompt", message, content_width),
            c::blank("main-added-gap"),
            chat::notice(
                "main-added-ack",
                "Preview message added. No agent is connected.",
                false,
                content_width,
            ),
        ]);
    }
    if !state.notice.is_empty() {
        body.extend([
            chat::notice("main-notice", &state.notice, false, content_width),
            c::blank("main-notice-gap"),
        ]);
    }
    let mut flat = Vec::new();
    for node in body {
        flatten(node, &mut flat);
    }
    let start = if variant == "long" {
        flat.len().saturating_sub(body_height)
    } else {
        flat.len()
            .saturating_sub(body_height)
            .saturating_sub(state.scroll)
    };
    let stop = (start + body_height).min(flat.len());
    let mut body = flat[start..stop].to_vec();
    while body.len() < body_height {
        body.push(c::blank(format!("main-body-pad-{}", body.len())));
    }
    let mut body = c::column("main-body", body);
    body.style.viewport = Some(Viewport {
        max_height: (body_height * 20) as u16,
        offset: 0,
        fade: 0,
    });
    let header = if let Some(agent) = state.selected.checked_sub(1).and_then(|i| agents.get(i)) {
        c::rich(
            "main-header",
            vec![c::bold(c::run(&agent.name, t::ACCENT_MODEL))],
        )
    } else {
        c::blank("main-header")
    };
    let inset = |mut node: Component| {
        node.style.padding_points = Some([0, 18, 0, 18]);
        node
    };
    let mut children = vec![c::blank("main-top-margin"), inset(header), inset(body)];
    if state.draft.starts_with('/') {
        children.push(chat::slash(
            "main-slash",
            &state.draft,
            state
                .fields
                .get("slash.selected")
                .and_then(|v| v.parse().ok())
                .unwrap_or(0),
            true,
            w,
            body_height.min(7),
        ));
    }
    children.push(chat::composer(
        "main-composer",
        &state.draft,
        state.selected == 0,
        if live { Some("auto") } else { None },
        None,
        w,
        composer_height.saturating_sub(2),
    ));
    let mut rail = chat::agent_rail(
        "main-agents",
        &agents,
        state.selected,
        content_width,
        rail_height,
    );
    if let Element::Stack { children, .. } = &mut rail.element {
        for row in children {
            row.style.padding_points = Some([0, 18, 0, 18]);
        }
    }
    children.extend([
        inset(chat::context(
            "main-context",
            "openagents",
            "main",
            content_width,
        )),
        rail,
        c::blank("main-bottom-margin"),
    ]);
    let mut root = c::column("main-screen", children);
    root.style.min_height = Some(state.height * 20);
    root
}

fn flatten(node: Component, out: &mut Vec<Component>) {
    match node.element {
        Element::Stack { children, .. } => {
            for child in children {
                flatten(child, out);
            }
        }
        _ => out.push(node),
    }
}

fn retained_turn(index: usize, width: usize) -> Vec<Component> {
    let (prompt, reply) = super::retained_fixture_turn(index).expect("bounded synthetic index");
    let mut out = Vec::new();
    for node in [
        chat::prompt(&format!("retained-user-{index}"), &prompt, width),
        c::blank(format!("retained-user-gap-{index}")),
        chat::reply(
            &format!("retained-reply-{index}"),
            &reply,
            Some("grok-build"),
            Some(1250),
            width,
        ),
        c::blank(format!("retained-reply-gap-{index}")),
    ] {
        flatten(node, &mut out);
    }
    out
}

pub(super) fn maximum_scroll(state: &FixtureState) -> Option<usize> {
    if state.component != "screen.main" || state.variant != "long" {
        return None;
    }
    if state.width < 24 || state.height < 12 {
        return Some(0);
    }
    let width = usize::from(state.width);
    let area = usize::from(state.height).saturating_sub(2);
    let rail_height = 4.min(area.saturating_sub(6));
    let draft_rows = c::wrap_ranges(&state.draft, width.saturating_sub(3))
        .len()
        .clamp(1, 6);
    let composer = (draft_rows + 2).min(area.saturating_sub(rail_height + 3));
    let body = area.saturating_sub(1 + composer + 1 + rail_height).max(1);
    Some(
        (retained_turn(0, width.saturating_sub(4)).len() * super::LARGE_TRANSCRIPT_TURNS)
            .saturating_sub(body),
    )
}

/// Generate only the retained records intersecting the viewport. Equal-width
/// numbered records make the source cursor arithmetic deterministic.
fn large_transcript_window(scroll: usize, width: usize, height: usize) -> Vec<Component> {
    let rows_per_turn = retained_turn(0, width).len();
    let total = rows_per_turn * super::LARGE_TRANSCRIPT_TURNS;
    let stop = total.saturating_sub(scroll.min(total.saturating_sub(height)));
    let start = stop.saturating_sub(height);
    let first = start / rows_per_turn;
    let last = stop.div_ceil(rows_per_turn);
    let mut rows = Vec::new();
    for index in first..last {
        rows.extend(retained_turn(index, width));
    }
    let offset = start - first * rows_per_turn;
    rows.into_iter().skip(offset).take(stop - start).collect()
}

fn demo_main(phase: u8, width: usize, agents: &[Agent]) -> Vec<Component> {
    let mut rows = vec![
        chat::prompt(
            "main-prompt",
            "Review the terminal with four agents.",
            width,
        ),
        c::blank("main-prompt-gap"),
    ];
    for (key, kind, input, output) in [
        (
            "read",
            ToolKind::Read,
            "docs/coder-new/",
            "4 documents reviewed",
        ),
        (
            "search",
            ToolKind::Search,
            "\"composer|agent_rail\" crates/coder-new/src/",
            "8 matches in 3 files",
        ),
        (
            "edit",
            ToolKind::Edit,
            "crates/coder-new/src/main.rs",
            MAIN_DIFF,
        ),
        (
            "run",
            ToolKind::Run,
            "cargo fmt -p coder-new --check",
            "Exit 0 · formatting passed",
        ),
    ] {
        rows.push(chat::demo_tool(
            &format!("main-tool-{key}"),
            kind,
            input,
            output,
            Status::Done,
            phase,
            width,
        ));
    }
    rows.extend([
        chat::plugin(
            "main-plugin-inspector",
            "terminal-inspector",
            "layout.inspect",
            "110×36 · composer and agent rail",
            "4 agent rows · aligned names, tasks, and tokens",
            Status::Done,
            phase,
            width,
        ),
        chat::plugin(
            "main-plugin-palette",
            "palette-audit",
            "colors.check",
            "USGC · #0a0a0a",
            "Comparing tool accents and diff backgrounds",
            Status::Running,
            phase,
            width,
        ),
        c::blank("main-tool-gap"),
    ]);
    for (i, agent) in agents.iter().enumerate() {
        rows.push(chat::delegation(
            &format!("main-delegate-{i}"),
            agent,
            true,
            phase,
            width,
        ));
    }
    rows.push(c::blank("main-delegate-gap"));
    rows
}

fn demo_child(selected: usize, phase: u8, width: usize) -> Vec<Component> {
    let (
        prompt,
        read_output,
        search_input,
        search_output,
        plugin,
        operation,
        plugin_input,
        plugin_output,
        reply,
        next,
        run,
    ) = match selected {
        1 => (
            "Review the composer keyboard navigation.",
            "Draft editing and per-conversation state",
            "\"KeyCode|KeyEventKind\" crates/coder-new/src/lib.rs",
            "18 matches · editing, selection, and key releases",
            "keyboard-audit",
            "navigation.check",
            "",
            "Draft and cursor restored across conversation switches",
            "The input keeps pasted text local and restores each conversation's draft. Left and Right move through whole graphemes.",
            "Check cursor restoration while switching agents.",
            "cargo test -p coder-new switching_restores_each_conversations_draft_cursor_messages_and_scroll",
        ),
        2 => (
            "Check the agent rail at wide and narrow terminal sizes.",
            "Composer rules, agent columns, and token alignment",
            "cargo test -p coder-new selected_agent_keeps_the_rail_visible_and_tokens_aligned_after_resize",
            "Exit 0 · 80- and 24-column views checked",
            "terminal-inspector",
            "layout.inspect",
            "80 and 24 columns · composer and agent rail",
            "4 consecutive agent rows · token counts aligned",
            "Agent names and token counts stay visible. Long tasks end with an ellipsis, and narrow terminals shorten the token label.",
            "Verify there are no blank rows between the input and the rail.",
            "cargo test -p coder-new header_and_rail_keep_compact_spacing_above_the_bottom_margin",
        ),
        3 => (
            "Keep each agent conversation's draft independent.",
            "Conversation selection and saved draft state",
            "\"saved_chats|select_agent\" crates/coder-new/src/lib.rs",
            "5 conversation slots · main and four agents",
            "conversation-audit",
            "state.check",
            "Main and four agents · multiline drafts",
            "5 independent drafts · cursor, messages, and scroll kept",
            "Each conversation keeps its own draft, messages, and scroll position. Switching restores the cursor where you left it.",
            "Exercise a switch away from a multiline draft.",
            "cargo test -p coder-new switching_restores_each_conversations_draft_cursor_messages_and_scroll",
        ),
        _ => (
            "Verify the terminal preview colors.",
            "USGC accents and the shared Coder background",
            "crates/coder-new/src/theme.rs",
            "@@ -8,3 +8,4 @@\n fn diff_style() -> Style {\n-    Style::default().fg(Color::Green)\n+    let background = Color::Rgb(0, 41, 17);\n+    Style::default().bg(background)\n }",
            "palette-audit",
            "colors.check",
            "Terminal and SVG · USGC · #0a0a0a",
            "Shared background and tool accents match the exported cells",
            "The terminal and SVG export use the same colors. The composer shares Coder's near-black background and uses quiet horizontal rules.",
            "Compare the terminal and exported preview.",
            "cargo run -p coder-new -- --snapshot",
        ),
    };
    let second_kind = match selected {
        2 => ToolKind::Run,
        4 => ToolKind::Edit,
        _ => ToolKind::Search,
    };
    let read_input = if selected == 2 {
        "crates/coder-new/src/ui.rs"
    } else if selected == 4 {
        "crates/coder-new/src/theme.rs"
    } else {
        "crates/coder-new/src/lib.rs"
    };
    vec![
        chat::prompt("child-prompt", prompt, width),
        c::blank("child-prompt-gap"),
        chat::demo_tool(
            "child-read",
            ToolKind::Read,
            read_input,
            read_output,
            Status::Done,
            phase,
            width,
        ),
        chat::demo_tool(
            "child-second",
            second_kind,
            search_input,
            search_output,
            Status::Done,
            phase,
            width,
        ),
        chat::plugin(
            "child-plugin",
            plugin,
            operation,
            plugin_input,
            plugin_output,
            Status::Done,
            phase,
            width,
        ),
        c::blank("child-tools-gap"),
        chat::reply("child-reply", reply, None, None, width),
        c::blank("child-reply-gap"),
        chat::prompt("child-next", next, width),
        c::blank("child-next-gap"),
        chat::demo_tool(
            "child-running",
            ToolKind::Run,
            run,
            match selected {
                1 => "Checking draft, cursor, messages, and scroll restoration",
                2 => "Checking consecutive rows and the bottom margin",
                3 => "Switching across all four agents and back to main",
                _ => "Rendering tool rows, delegation components, and the composer",
            },
            Status::Running,
            phase,
            width,
        ),
        c::blank("child-last-gap"),
    ]
}

fn live_conversation(variant: &str, state: &FixtureState, width: usize) -> Vec<Component> {
    if variant == "live-empty" {
        return Vec::new();
    }
    let mut body = Vec::new();
    let count = 1;
    for i in 0..count {
        body.extend([
            chat::prompt(
                &format!("live-prompt-{i}"),
                if variant == "live-child" {
                    "Review the selected child's component."
                } else {
                    "Review the shared Rust Native components."
                },
                width,
            ),
            c::blank(format!("live-user-gap-{i}")),
            chat::reply(
                &format!("live-reply-{i}"),
                "The shared components keep presentation independent of task execution.",
                Some("grok-build"),
                Some(1250),
                width,
            ),
            c::blank(format!("live-reply-gap-{i}")),
        ]);
    }
    body.extend([
        chat::live_tool(
            "live-call",
            "Read",
            &Parameter::Object(vec![(
                "path".into(),
                Parameter::Value("docs/coder/rust-native/architecture.md".into()),
            )]),
            &Parameter::Object(vec![(
                "content".into(),
                Parameter::Value("Shared Rust application presentation".into()),
            )]),
            if variant == "pending" {
                Status::Running
            } else {
                Status::Done
            },
            state.phase,
            width,
            None,
        ),
        c::blank("live-call-gap"),
    ]);
    if variant == "nested" {
        let agent = agents(0)[0].clone();
        body.extend([
            chat::delegation("live-nested-parent", &agent, false, state.phase, width),
            chat::delegation(
                "live-nested-child",
                &Agent {
                    name: "codex child".into(),
                    task: "Nested work linked to parent".into(),
                    ..agent
                },
                false,
                state.phase,
                width,
            ),
            c::blank("live-nested-gap"),
        ]);
    }
    if variant == "streaming" || state.flags.get("busy") == Some(&true) {
        body.extend([
            chat::reply(
                "live-partial",
                "A reply still **streaming**.\n\n```rust\nlet partial =",
                Some("grok-build"),
                None,
                width,
            ),
            c::rich(
                "live-working",
                vec![
                    c::run(
                        format!(" {} Working", c::spinner(state.phase)),
                        t::ACCENT_SKILL,
                    ),
                    c::run(format!(" · {}", c::elapsed(state.elapsed)), t::GRAY),
                ],
            ),
        ]);
    }
    if matches!(variant, "error" | "stopped" | "restored") {
        body.push(chat::notice(
            "live-state-notice",
            match variant {
                "error" => "The synthetic command failed.",
                "stopped" => "Request stopped.",
                _ => "Resumed conversation. Saved folder differs from the current folder.",
            },
            variant == "error",
            width,
        ));
    }
    body
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn source_main_fixture_has_exact_baseline_rows_labels_and_colors() {
        let state = FixtureState::default_for("screen.main", "demo");
        let scene = screen("demo", &state);
        let source = super::super::plain(&scene);
        for value in [
            "Review the terminal with four agents.",
            "Read docs/coder-new/",
            "8 matches in 3 files",
            "Edit crates/coder-new/src/main.rs +1 -1",
            "Exit 0 · formatting passed",
            "Plugin terminal-inspector.layout.inspect",
            "Running · Comparing tool accents and diff backgrounds",
            "claude-code",
            "12.4k tokens ↓",
            "openagents / main",
        ] {
            assert!(
                source.contains(value),
                "missing source fixture text {value}"
            );
        }
        let mut body = demo_main(0, 106, &agents(0));
        let mut rows = Vec::new();
        for row in body.drain(..) {
            flatten(row, &mut rows);
        }
        assert_eq!(rows.len(), 25, "source110x36 body allocation");
        let rendered = c::diff::lines(MAIN_DIFF, "crates/coder-new/src/main.rs", 106);
        assert!(
            rendered
                .iter()
                .flatten()
                .any(|r| r.background == Some(t::DIFF_DELETE_BG))
        );
        assert!(
            rendered
                .iter()
                .flatten()
                .any(|r| r.background == Some(t::DIFF_INSERT_BG))
        );
    }
    #[test]
    fn all_conversation_variants_render_and_pin_source_branches() {
        for entry in entries() {
            for variant in entry.variants {
                let state = FixtureState::default_for(&entry.id, &variant.id);
                let view = super::super::view(&entry.id, &variant.id, &state);
                view.validate()
                    .unwrap_or_else(|e| panic!("{} / {}: {e}", entry.id, variant.id));
                assert!(
                    entry
                        .sources
                        .iter()
                        .any(|s| s.branch.starts_with(&format!("fixture:{};", variant.id)))
                );
            }
        }
    }
    #[test]
    fn live_failure_uses_source_cyan_glyph_and_argument_bands() {
        let state = FixtureState::default_for("tools.live", "failed");
        let node = live_fixture("failed", &state);
        let text = super::super::plain(&node);
        assert!(text.contains("×"));
        assert!(text.contains("error:"));
        fn cyan_cross(node: &Component) -> bool {
            match &node.element {
                Element::RichText { runs, .. } => runs
                    .iter()
                    .any(|r| r.text.contains('×') && r.foreground == Some(t::ACCENT_SKILL)),
                Element::Stack { children, .. } => children.iter().any(cyan_cross),
                _ => false,
            }
        }
        assert!(cyan_cross(&node));
    }

    #[test]
    fn retained_source_exceeds_view_limits_but_windows_keep_original_records_reachable() {
        let stats = super::super::transcript_fixture_stats();
        assert_eq!(stats.messages, 10_000);
        assert!(stats.original_text_bytes > rust_native::view::MAX_VIEW_BYTES);
        let mut state = FixtureState::default_for("screen.main", "long");
        let end = super::super::plain(&screen("long", &state));
        assert!(end.contains("04999"));
        for _ in 0..20 {
            super::super::reduce(&mut state, CatalogIntent::Scroll { delta: i16::MAX }, None)
                .unwrap();
        }
        let beginning = super::super::plain(&screen("long", &state));
        assert!(beginning.contains("00000"));
        let oldest = state.scroll;
        super::super::reduce(&mut state, CatalogIntent::Scroll { delta: -10 }, None).unwrap();
        assert_eq!(state.scroll, oldest - 10);
        for source in [0, 4999] {
            let (prompt, reply) = super::super::retained_fixture_turn(source).unwrap();
            assert!(prompt.contains(&format!("{source:05}")));
            assert!(reply.contains(&format!("{source:05}")));
        }
        let view = super::super::view("screen.main", "long", &state)
            .validate()
            .unwrap();
        let bytes = view.to_json().unwrap().len();
        let baseline = super::super::view(
            "screen.main",
            "demo",
            &FixtureState::default_for("screen.main", "demo"),
        )
        .validate()
        .unwrap()
        .to_json()
        .unwrap()
        .len();
        assert!(
            bytes <= baseline * 2 && bytes < rust_native::view::MAX_VIEW_BYTES / 4,
            "bounded viewport encoded {bytes} bytes, baseline {baseline}"
        );
        super::super::reduce(
            &mut state,
            CatalogIntent::Action {
                name: "latest".into(),
            },
            None,
        )
        .unwrap();
        assert!(super::super::plain(&screen("long", &state)).contains("04999"));
    }
}
