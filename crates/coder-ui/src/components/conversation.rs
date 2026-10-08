//! Reusable Coder conversation components, independent of agent execution.

use crate::{
    catalog::CatalogIntent,
    components::{self as c, Component},
    source_theme as t,
};
use rust_native::{
    style::{Color, TextAlign},
    view::RichRun,
};
use unicode_width::UnicodeWidthStr;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Running,
    Done,
    Failed,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolKind {
    Read,
    Search,
    Edit,
    Run,
}
impl ToolKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Read => "Read",
            Self::Search => "Search",
            Self::Edit => "Edit",
            Self::Run => "Run",
        }
    }
}

pub fn prompt(key: &str, value: &str, width: usize) -> Component {
    let mut rows = c::markdown::lines(value, width.saturating_sub(3));
    if rows.is_empty() {
        rows.push(Vec::new());
    }
    c::column(
        key,
        rows.into_iter()
            .enumerate()
            .map(|(i, mut runs)| {
                runs.insert(
                    0,
                    c::run(if i == 0 { " ❯ " } else { "   " }, t::TEXT_SECONDARY),
                );
                let used = runs.iter().map(|r| r.text.width()).sum::<usize>();
                runs.push(c::run(
                    " ".repeat(width.saturating_sub(used)),
                    t::TEXT_SECONDARY,
                ));
                let mut row = c::rich(format!("{key}-{i}"), runs);
                row.style.background = Some(t::BG_LIGHT);
                row
            })
            .collect(),
    )
}

pub fn reply(
    key: &str,
    value: &str,
    model: Option<&str>,
    elapsed_ms: Option<u64>,
    width: usize,
) -> Component {
    let mut rows = c::markdown::lines(value, width);
    let time = elapsed_ms.map(|ms| format!("{:.1}s", ms as f64 / 1000.0));
    let footer = match (model, time) {
        (Some(model), Some(time)) => {
            let suffix = format!(" · {time}");
            Some(format!(
                "{}{suffix}",
                c::truncate(model, width.saturating_sub(suffix.width()))
            ))
        }
        (Some(model), None) => Some(c::truncate(model, width)),
        (None, Some(time)) => Some(time),
        (None, None) => None,
    };
    if let Some(footer) = footer {
        rows.push(vec![c::run(
            format!(
                "{}{footer}",
                " ".repeat(width.saturating_sub(footer.width()))
            ),
            t::GRAY,
        )]);
    }
    c::rows(key, rows)
}

pub fn demo_tool(
    key: &str,
    kind: ToolKind,
    input: &str,
    output: &str,
    status: Status,
    phase: u8,
    width: usize,
) -> Component {
    let accent = if matches!(kind, ToolKind::Read | ToolKind::Search) {
        t::ACCENT_SKILL
    } else {
        t::ACCENT_SUCCESS
    };
    let glyph = match status {
        Status::Running => c::spinner(phase),
        Status::Done => "◆",
        Status::Failed => "×",
    };
    let mut header = vec![
        c::run(
            format!(" {glyph} "),
            if status == Status::Failed {
                t::DIFF_DELETE_FG
            } else {
                accent
            },
        ),
        c::bold(c::run(kind.label(), accent)),
        c::run(format!(" {input}"), t::TEXT_SECONDARY),
    ];
    let mut lines = Vec::new();
    if kind == ToolKind::Edit && status == Status::Done {
        let (added, removed) = c::diff::counts(output);
        header.extend([
            c::run(format!(" +{added}"), t::DIFF_INSERT_FG),
            c::run(format!(" -{removed}"), t::DIFF_DELETE_FG),
        ]);
        lines.extend(c::wrap(&header, width));
        lines.extend(c::diff::lines(output, input, width));
    } else {
        lines.extend(c::wrap(&header, width));
        match status {
            Status::Done => {
                for (i, line) in output.lines().enumerate() {
                    lines.extend(c::wrap(
                        &[
                            c::run(if i == 0 { "   ╰ " } else { "     " }, t::GRAY_DIM),
                            c::run(line, t::GRAY_BRIGHT),
                        ],
                        width,
                    ));
                }
            }
            Status::Running | Status::Failed => lines.extend(c::wrap(
                &[
                    c::run("   ╰ ", t::GRAY_DIM),
                    c::run(
                        if status == Status::Running {
                            "Running"
                        } else {
                            "Failed"
                        },
                        if status == Status::Running {
                            accent
                        } else {
                            t::DIFF_DELETE_FG
                        },
                    ),
                    c::run(
                        format!(" · {output}"),
                        if status == Status::Running {
                            t::GRAY_BRIGHT
                        } else {
                            t::DIFF_DELETE_FG
                        },
                    ),
                ],
                width,
            )),
        }
    }
    c::rows(key, lines)
}

pub fn plugin(
    key: &str,
    name: &str,
    operation: &str,
    input: &str,
    output: &str,
    status: Status,
    phase: u8,
    width: usize,
) -> Component {
    let glyph = match status {
        Status::Done => "◆",
        Status::Running => c::spinner(phase),
        Status::Failed => "×",
    };
    let mut header = vec![
        c::run(
            format!(" {glyph} "),
            if status == Status::Failed {
                t::DIFF_DELETE_FG
            } else {
                t::ACCENT_SKILL
            },
        ),
        c::bold(c::run("Plugin", t::ACCENT_SKILL)),
        c::run(format!(" {name}.{operation}"), t::TEXT_PRIMARY),
    ];
    if !input.is_empty() {
        header.push(c::run(format!(" · {input}"), t::GRAY_BRIGHT));
    }
    let mut lines = c::wrap(&header, width);
    let mut result = vec![c::run("   ╰ ", t::GRAY_DIM)];
    match status {
        Status::Done => result.push(c::run(output, t::GRAY_BRIGHT)),
        Status::Running => result.extend([
            c::run("Running", t::ACCENT_SKILL),
            c::run(format!(" · {output}"), t::GRAY_BRIGHT),
        ]),
        Status::Failed => result.extend([
            c::run("Failed", t::DIFF_DELETE_FG),
            c::run(format!(" · {output}"), t::DIFF_DELETE_FG),
        ]),
    };
    lines.extend(c::wrap(&result, width));
    c::rows(key, lines)
}

#[derive(Clone, Debug, PartialEq)]
pub enum Parameter {
    Null,
    Value(String),
    Number(i64),
    Boolean(bool),
    Array(Vec<Parameter>),
    Object(Vec<(String, Parameter)>),
}

fn parameter_values(
    value: &Parameter,
    prefix: &str,
    depth: usize,
    result: &mut Vec<(String, String)>,
) {
    match value {
        Parameter::Object(object) if depth < 2 => {
            for (key, value) in object {
                let key = if prefix.is_empty() {
                    key.clone()
                } else {
                    format!("{prefix}.{key}")
                };
                parameter_values(value, &key, depth + 1, result);
            }
        }
        Parameter::Null if prefix.is_empty() => {}
        Parameter::Null => result.push((prefix.into(), "null".into())),
        Parameter::Value(value) => result.push((
            if prefix.is_empty() {
                "value".into()
            } else {
                prefix.into()
            },
            value.replace('\n', " ↵ "),
        )),
        Parameter::Number(_) | Parameter::Boolean(_) | Parameter::Array(_) => result.push((
            if prefix.is_empty() {
                "value".into()
            } else {
                prefix.into()
            },
            parameter_json(value),
        )),
        Parameter::Object(object) => {
            let value = object
                .iter()
                .map(|(k, v)| format!("\"{k}\":{}", parameter_json(v)))
                .collect::<Vec<_>>()
                .join(",");
            result.push((prefix.into(), format!("{{{value}}}")));
        }
    }
}
fn parameter_json(value: &Parameter) -> String {
    match value {
        Parameter::Null => "null".into(),
        Parameter::Value(v) => serde_json::to_string(v).expect("a string serializes"),
        Parameter::Number(value) => value.to_string(),
        Parameter::Boolean(value) => value.to_string(),
        Parameter::Array(values) => format!(
            "[{}]",
            values
                .iter()
                .map(parameter_json)
                .collect::<Vec<_>>()
                .join(",")
        ),
        Parameter::Object(values) => format!(
            "{{{}}}",
            values
                .iter()
                .map(|(k, v)| format!(
                    "{}:{}",
                    serde_json::to_string(k).expect("a string serializes"),
                    parameter_json(v)
                ))
                .collect::<Vec<_>>()
                .join(",")
        ),
    }
}

pub fn parameters(key: &str, value: &Parameter, width: usize) -> Component {
    let mut values = Vec::new();
    parameter_values(value, "", 0, &mut values);
    let omitted = values
        .len()
        .saturating_sub(if values.len() > 5 { 4 } else { 5 });
    let take = if omitted > 0 { 4 } else { 5 };
    let mut rows = values
        .into_iter()
        .take(take)
        .map(|(key, value)| {
            let key = c::truncate(&key, width.saturating_sub(8));
            let available = width.saturating_sub(7 + key.width());
            vec![
                c::run("   │ ", t::GRAY_DIM),
                c::run(format!("{key}: "), t::TEXT_SECONDARY),
                c::run(c::truncate(&value, available), t::GRAY_BRIGHT),
            ]
        })
        .collect::<Vec<_>>();
    if omitted > 0 {
        rows.push(vec![
            c::run("   │ ", t::GRAY_DIM),
            c::run(
                c::truncate(&format!("… {omitted} more fields"), width.saturating_sub(5)),
                t::GRAY,
            ),
        ]);
    }
    c::column(
        key,
        rows.into_iter()
            .enumerate()
            .map(|(i, mut runs)| {
                let used = runs.iter().map(|r| r.text.width()).sum::<usize>();
                runs.push(c::run(
                    " ".repeat(width.saturating_sub(used)),
                    t::TEXT_SECONDARY,
                ));
                let mut node = c::rich(format!("{key}-{i}"), runs);
                node.style.background = Some(t::BG_DARK);
                node
            })
            .collect(),
    )
}

pub fn live_tool(
    key: &str,
    name: &str,
    input: &Parameter,
    output: &Parameter,
    status: Status,
    phase: u8,
    width: usize,
    brainstorm: Option<&str>,
) -> Component {
    let native = matches!(name, "Run" | "Read" | "Edit" | "Search");
    let glyph = match status {
        Status::Running => c::spinner(phase),
        Status::Done => "◆",
        Status::Failed => "×",
    };
    let header = vec![
        c::run(format!(" {glyph} "), t::ACCENT_SKILL),
        c::bold(c::run(
            if native { name } else { "Plugin" },
            t::ACCENT_SKILL,
        )),
        c::run(
            c::truncate(
                &if native {
                    String::new()
                } else {
                    format!(" {name}")
                },
                width.saturating_sub(9),
            ),
            t::TEXT_PRIMARY,
        ),
    ];
    let mut children = vec![
        c::rows(&format!("{key}-head"), c::wrap(&header, width)),
        parameters(&format!("{key}-args"), input, width),
    ];
    if status == Status::Running {
        children.push(c::rich(
            format!("{key}-running"),
            vec![
                c::run("   ╰ ", t::GRAY_DIM),
                c::run("Running", t::GRAY_BRIGHT),
            ],
        ));
    } else if let Some(summary) = brainstorm {
        children.push(c::rows(
            &format!("{key}-summary"),
            c::markdown::lines(summary, width),
        ));
    } else {
        children.push(parameters(&format!("{key}-result"), output, width));
    }
    c::column(key, children)
}

#[derive(Clone, Debug)]
pub struct Agent {
    pub name: String,
    pub task: String,
    pub tokens: u64,
    pub elapsed: u64,
    pub status: Status,
}
pub fn token_count(tokens: u64) -> String {
    if tokens >= 1000 {
        format!("{:.1}k", tokens as f64 / 1000.0)
    } else {
        tokens.to_string()
    }
}

pub fn delegation(key: &str, agent: &Agent, demo: bool, phase: u8, width: usize) -> Component {
    let narrow = width < 32;
    let glyph = if demo {
        "◆"
    } else {
        match agent.status {
            Status::Running => c::spinner(phase),
            Status::Done => "◆",
            Status::Failed => "×",
        }
    };
    let mut header = vec![c::run(
        format!(" {glyph} "),
        if demo {
            c::pulse(phase)
        } else {
            t::ACCENT_DELEGATE
        },
    )];
    if !demo || !narrow {
        header.extend([
            c::bold(c::run("Delegate", t::ACCENT_MODEL)),
            c::run(" ", t::TEXT_SECONDARY),
        ]);
    }
    header.push(c::run(&agent.name, t::TEXT_PRIMARY));
    let detail = if demo {
        let tokens = format!(
            " · {}{}",
            token_count(agent.tokens),
            if narrow { "" } else { " tokens" }
        );
        let room = width.saturating_sub(15 + tokens.width());
        let mut runs = vec![c::run("   ╰ ", t::GRAY_DIM)];
        if room > 0 {
            runs.extend([
                c::run(c::truncate(&agent.task, room), t::TEXT_SECONDARY),
                c::run(" · ", t::GRAY_DIM),
            ]);
        }
        runs.extend([c::run("Running", t::ACCENT_SKILL), c::run(tokens, t::GRAY)]);
        runs
    } else {
        vec![
            c::run("   ╰ ", t::GRAY_DIM),
            c::run(
                format!(
                    "{} · {}",
                    c::truncate(&agent.task, width.saturating_sub(17)),
                    match agent.status {
                        Status::Running => "Running",
                        Status::Done => "Done",
                        Status::Failed => "Failed",
                    }
                ),
                t::GRAY_BRIGHT,
            ),
        ]
    };
    let mut rows = c::wrap(&header, width);
    rows.extend(c::wrap(&detail, width));
    c::rows(key, rows)
}

pub fn agent_rail(
    key: &str,
    agents: &[Agent],
    selected: usize,
    width: usize,
    height: usize,
) -> Component {
    if agents.is_empty() || height == 0 {
        return c::column(key, Vec::new());
    }
    let narrow = width < 32;
    let name_width = 2 + agents.iter().map(|a| a.name.width()).max().unwrap_or(0);
    let count_width = agents
        .iter()
        .map(|a| token_count(a.tokens).width())
        .max()
        .unwrap_or(0);
    let tokens = agents
        .iter()
        .map(|a| {
            format!(
                "{:>count_width$}{}",
                token_count(a.tokens),
                if narrow { "↓" } else { " tokens ↓" }
            )
        })
        .collect::<Vec<_>>();
    let timed = agents
        .iter()
        .zip(&tokens)
        .map(|(a, t)| format!("{} · {t}", c::elapsed(a.elapsed)))
        .collect::<Vec<_>>();
    let show_time = width >= name_width + 1 + timed.iter().map(|s| s.width()).max().unwrap_or(0);
    let first = if selected == 0 {
        agents.len() - 1
    } else {
        selected.saturating_sub(1).min(agents.len() - 1)
    }
    .saturating_sub(height.saturating_sub(1));
    let children = agents
        .iter()
        .enumerate()
        .skip(first)
        .take(height)
        .map(|(i, agent)| {
            let active = selected == i + 1;
            let suffix = if show_time { &timed[i] } else { &tokens[i] };
            let activity = width.saturating_sub(suffix.width() + 1);
            let prefix = if active {
                "❯ "
            } else if narrow {
                "  "
            } else {
                "○ "
            };
            let color = if active {
                t::ACCENT_MODEL
            } else {
                t::TEXT_SECONDARY
            };
            let name = c::truncate(&format!("{prefix}{}", agent.name), name_width.min(activity));
            let task_width = activity.saturating_sub(name_width + 2);
            let mut runs = if name.width() >= 2 {
                vec![
                    c::run(prefix, if active { t::ACCENT_MODEL } else { t::GRAY }),
                    c::bold(c::run(name.strip_prefix(prefix).unwrap_or(&name), color)),
                ]
            } else {
                vec![c::run(&name, color)]
            };
            let task = c::truncate(&agent.task, task_width);
            let used = if task_width > 0 {
                name_width + 2 + task.width()
            } else {
                name.width()
            };
            if task_width > 0 {
                runs.extend([
                    c::run(
                        " ".repeat(name_width.saturating_sub(name.width()) + 2),
                        t::GRAY,
                    ),
                    c::run(task, t::GRAY),
                ]);
            }
            runs.extend([
                c::run(" ".repeat(activity.saturating_sub(used) + 1), t::GRAY),
                c::run(suffix, t::GRAY),
            ]);
            let row = c::rich(format!("{key}-{i}-text"), runs);
            let node = c::choice(
                format!("{key}-{i}"),
                format!("Select {}", agent.name),
                active,
                CatalogIntent::Select { index: i + 1 },
                vec![row],
            );
            if active { c::selected(node) } else { node }
        })
        .collect();
    c::column(key, children)
}

pub fn context(key: &str, directory: &str, branch: &str, width: usize) -> Component {
    let suffix = format!(" / {branch}");
    let runs = if suffix.width() < width {
        vec![
            c::run(
                c::truncate(directory, width - suffix.width()),
                t::TEXT_PRIMARY,
            ),
            c::run(suffix, t::GRAY),
        ]
    } else {
        vec![c::run(
            c::truncate(&format!("{directory}{suffix}"), width),
            t::TEXT_PRIMARY,
        )]
    };
    c::rich(key, runs)
}

pub fn composer_rail(value: &str, width: usize) -> String {
    if value.width() <= width {
        return value.into();
    }
    if let Some((model, options)) = value.split_once(':') {
        let suffix = format!(":{options}");
        if suffix.width() < width {
            return format!("{}{suffix}", c::truncate(model, width - suffix.width()));
        }
    }
    c::truncate(value, width)
}

pub fn composer(
    key: &str,
    draft: &str,
    main: bool,
    model: Option<&str>,
    bottom: Option<&str>,
    width: usize,
    available_rows: usize,
) -> Component {
    let rule = |key: &str, contribution: Option<&str>| {
        let text = contribution.map(|s| composer_rail(s, width.saturating_sub(6)));
        let runs = if let Some(text) = text {
            let reserve = text.width() + 4;
            vec![
                c::run(
                    "─".repeat(width.saturating_sub(reserve)),
                    t::PROMPT_BORDER_ACTIVE,
                ),
                c::run(format!(" {text} "), t::GRAY),
                c::run("──", t::PROMPT_BORDER_ACTIVE),
            ]
        } else {
            vec![c::run("─".repeat(width), t::PROMPT_BORDER_ACTIVE)]
        };
        c::rich(key, runs)
    };
    let rows = c::wrap_ranges(draft, width.saturating_sub(3))
        .len()
        .clamp(1, 6)
        .min(available_rows.max(1));
    let mut edit = c::field(
        format!("{key}-draft"),
        "Message",
        draft,
        false,
        true,
        CatalogIntent::Input {
            field: "draft".into(),
        },
    );
    edit.style.foreground = Some(t::TEXT_PRIMARY);
    edit.style.min_height = Some((rows * 20) as u16);
    edit.style.fill_height = Some(false);
    let mut arrow = c::text(
        format!("{key}-arrow"),
        " ❯ ",
        if main { t::TEXT_SECONDARY } else { t::GRAY_DIM },
    );
    arrow.style.intrinsic_width = Some(true);
    let mut body = c::row(format!("{key}-body"), vec![arrow, edit]);
    body.style.min_height = Some((rows * 20) as u16);
    c::column(
        key,
        vec![
            rule(&format!("{key}-top"), model),
            body,
            rule(&format!("{key}-bottom"), bottom),
        ],
    )
}

pub const COMMANDS: &[(&str, &str)] = &[
    ("demo", "Turn demo off"),
    ("plugins", "Manage plugins"),
    ("models", "Choose model and reasoning level"),
    ("export", "Export this conversation as ATIF"),
    ("resume", "Resume a saved conversation"),
    ("brainstorm", "Explicit public profile or reputation lookup"),
    ("help", "Show commands and keys"),
];
pub fn slash(
    key: &str,
    draft: &str,
    selected: usize,
    demo: bool,
    width: usize,
    height: usize,
) -> Component {
    let Some(prefix) = draft
        .strip_prefix('/')
        .filter(|s| s.bytes().all(|b| b.is_ascii_lowercase()))
    else {
        return c::column(key, Vec::new());
    };
    let commands = COMMANDS
        .iter()
        .filter(|(word, _)| (*word != "demo" || demo) && word.starts_with(prefix))
        .collect::<Vec<_>>();
    let usage_width = commands
        .iter()
        .map(|(word, _)| word.len() + 1)
        .max()
        .unwrap_or(0);
    let shown = commands.len().min(height);
    let selected = selected.min(commands.len().saturating_sub(1));
    let first = selected.saturating_sub(shown.saturating_sub(1));
    c::column(
        key,
        commands
            .iter()
            .enumerate()
            .skip(first)
            .take(shown)
            .map(|(i, (word, about))| {
                let active = i == selected;
                let usage = format!("/{word}");
                let mut label = c::run(
                    format!("{usage:<usage_width$}  "),
                    if active {
                        t::TEXT_PRIMARY
                    } else {
                        t::TEXT_SECONDARY
                    },
                );
                label.bold = active;
                let rows = c::rows(
                    &format!("{key}-{i}-text"),
                    vec![vec![
                        c::run(if active { " ❯ " } else { "   " }, t::ACCENT_MODEL),
                        label,
                        c::run(
                            c::truncate(about, width.saturating_sub(5 + usage_width)),
                            t::GRAY,
                        ),
                    ]],
                );
                let mut node = c::choice(
                    format!("{key}-{i}"),
                    usage.clone(),
                    active,
                    CatalogIntent::Pick {
                        field: "slash".into(),
                        value: usage,
                    },
                    vec![rows],
                );
                node.style.background = Some(if active { t::BG_LIGHT } else { t::BG_BASE });
                node
            })
            .collect(),
    )
}

pub fn notice(key: &str, value: &str, error: bool, width: usize) -> Component {
    c::rows(
        key,
        c::wrap(
            &[c::run(
                value,
                if error { t::DIFF_DELETE_FG } else { t::GRAY },
            )],
            width,
        ),
    )
}

pub fn heading(key: &str, title: &str) -> Component {
    let mut node = c::text(key, title, t::ACCENT_MODEL);
    node.style.align = Some(TextAlign::Start);
    node
}
pub fn tint(mut runs: Vec<RichRun>, color: Color) -> Vec<RichRun> {
    for run in &mut runs {
        run.foreground = Some(color);
    }
    runs
}
