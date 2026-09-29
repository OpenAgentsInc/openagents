//! The screens as Rust Native views: `DSK-01` Connect a phone, `DSK-02`
//! Connected, `DSK-03` Home, `DSK-04` a phone nearby ([`nearby`]), and the
//! adoption question.
//!
//! Words follow the wireframe's "Words on screen": plain words, and none of
//! the banned ones ([`crate::words`] checks every screen). "Project" names
//! a workspace and "phone" names a device.

use crate::codes::Held;
use crate::model::{Agent, Intent, Model, Screen};
use rust_native::style::{Color, Space, Style, TextAlign, TextWeight};
use rust_native::{Axis, Element, Glyph, Icon, Node, TextRole, ValidatedView, View};

mod nearby;

/// The drawing surface the code is painted on.
pub const CODE_SURFACE: &str = "pairing-code";

/// A card's fill.
const CARD: Color = Color::rgb(24, 24, 24);
/// A quiet button's fill.
const QUIET: Color = Color::rgb(44, 44, 44);

fn node(key: &str, style: Style, element: Element<Intent>) -> Node<Intent> {
    Node {
        key: key.into(),
        style,
        element,
    }
}

fn text(key: &str, value: impl Into<String>, role: TextRole) -> Node<Intent> {
    node(
        key,
        Style::default(),
        Element::Text {
            value: value.into(),
            role,
        },
    )
}

fn centered(mut node: Node<Intent>) -> Node<Intent> {
    node.style.align = Some(TextAlign::Center);
    node
}

fn bold(key: &str, value: impl Into<String>) -> Node<Intent> {
    let mut node = text(key, value, TextRole::Body);
    node.style.weight = Some(TextWeight::Bold);
    node
}

fn button(key: &str, label: &str, intent: Intent, enabled: bool) -> Node<Intent> {
    node(
        key,
        Style::default(),
        Element::Button {
            label: label.into(),
            enabled,
            icon: None,
            intent,
        },
    )
}

fn quiet(key: &str, label: &str, intent: Intent) -> Node<Intent> {
    let mut node = button(key, label, intent, true);
    node.style.background = Some(QUIET);
    node.style.foreground = Some(Color::rgb(245, 245, 245));
    node
}

fn link(key: &str, label: &str, intent: Intent) -> Node<Intent> {
    let mut node = button(key, label, intent, true);
    node.style.background = Some(Color {
        red: 0,
        green: 0,
        blue: 0,
        alpha: 0,
    });
    node
}

fn checkbox(key: &str, label: &str, on: bool, intent: Intent, enabled: bool) -> Node<Intent> {
    node(
        key,
        Style::default(),
        Element::Button {
            label: label.into(),
            enabled,
            icon: Some(Icon {
                glyph: if on { Glyph::Checked } else { Glyph::Unchecked },
                circular: false,
                pill: false,
            }),
            intent,
        },
    )
}

fn stack(key: &str, axis: Axis, gap: Space, children: Vec<Node<Intent>>) -> Node<Intent> {
    node(
        key,
        Style {
            gap: Some(gap),
            ..Style::default()
        },
        Element::Stack { axis, children },
    )
}

fn card(key: &str, children: Vec<Node<Intent>>) -> Node<Intent> {
    let mut node = stack(key, Axis::Vertical, Space::Sm, children);
    node.style.background = Some(CARD);
    node.style.padding_top = Some(Space::Md);
    node.style.padding_bottom = Some(Space::Md);
    node.style.padding_start = Some(Space::Md);
    node.style.padding_end = Some(Space::Md);
    node
}

/// "3 minutes ago", from Unix seconds.
pub fn ago(then: u64, now: u64) -> String {
    let seconds = now.saturating_sub(then);
    let (count, unit) = match seconds {
        0..60 => return "just now".into(),
        60..3_600 => (seconds / 60, "min"),
        3_600..86_400 => (seconds / 3_600, "hour"),
        _ => (seconds / 86_400, "day"),
    };
    let plural = if count == 1 || unit == "min" { "" } else { "s" };
    format!("{count} {unit}{plural} ago")
}

/// The task store's status, in words.
fn task_status(status: &str) -> &'static str {
    match status {
        "running" | "started" | "starting" => "Working",
        "queued" | "submitted" | "accepted" | "pending" => "Waiting to start",
        "finished" | "succeeded" | "completed" => "Done",
        "cancelled" | "canceled" => "Stopped",
        "failed" => "Didn't finish",
        _ => "Waiting",
    }
}

/// The root of the current screen. `now` is Unix seconds, for "last seen".
pub fn root(model: &Model, now: u64) -> Node<Intent> {
    if let Some(prompt) = model.nearby() {
        return nearby::prompt(prompt, model.nearby_terminal());
    }
    match &model.screen {
        Screen::Connect => connect(model),
        Screen::Connected { device } => connected(model, device),
        Screen::Home => home(model, now),
        Screen::Adopt => adopt(model),
    }
}

/// `DSK-01`: the code, the sentence, the terminal checkbox, and the copy.
fn connect(model: &Model) -> Node<Intent> {
    let mut middle = Vec::new();
    let shown = model.codes.shown();
    if shown.is_some() {
        middle.push(node(
            "code",
            Style::default(),
            Element::Surface {
                resource: CODE_SURFACE.into(),
                label: "A code for your phone to scan".into(),
            },
        ));
    } else {
        middle.extend(waiting(model));
    }
    middle.push(centered(text(
        "scan",
        "Scan with the OpenAgents app on your phone.",
        TextRole::Heading,
    )));
    middle.push(checkbox(
        "terminal",
        "Let this phone open a terminal on this Mac",
        model.codes.terminal(),
        Intent::ToggleTerminal,
        true,
    ));
    if shown.is_some() {
        middle.push(link(
            "copy",
            "Can't scan? Copy a code instead",
            Intent::CopyCode,
        ));
        if model.copied_at.is_some() {
            middle.push(centered(text(
                "copied",
                "Copied. It works for two minutes.",
                TextRole::Status,
            )));
        }
    }
    let mut middle = stack("middle", Axis::Vertical, Space::Md, middle);
    middle.style.align = Some(TextAlign::Center);
    let mut children = Vec::new();
    if !model.phones().is_empty() {
        children.push(link("back", "Back", Intent::Back));
    }
    children.push(middle);
    stack("connect", Axis::Vertical, Space::Md, children)
}

/// What shows where the code goes when there is none.
fn waiting(model: &Model) -> Vec<Node<Intent>> {
    let line = |value: &str| centered(text("waiting", value, TextRole::Body));
    if model.codes.held() == Some(Held::Idle) {
        return vec![
            line("The code is hidden because nobody used this window for 10 minutes."),
            button("show", "Show the code", Intent::ShowCode, true),
        ];
    }
    if model.host.is_none() {
        return match &model.agent {
            Agent::NeedsApproval => vec![
                line("OpenAgents needs your OK to run in the background."),
                centered(text(
                    "approve",
                    "Turn on OpenAgents under Allow in the Background, then come back here.",
                    TextRole::Status,
                )),
                button(
                    "login-items",
                    "Open Login Items",
                    Intent::OpenLoginItems,
                    true,
                ),
            ],
            Agent::Failed(_) => vec![line(
                "Coder couldn't start on this Mac. Quit OpenAgents and open it again.",
            )],
            Agent::NotRegistered => vec![line("Waiting for Coder on this Mac…")],
            Agent::Enabled if model.reached => {
                vec![line("Coder on this Mac stopped answering. Trying again…")]
            }
            Agent::Enabled => vec![line("Starting Coder on this Mac…")],
        };
    }
    vec![line("Making a code…")]
}

fn device_label<'a>(model: &'a Model, device: &str) -> &'a str {
    model
        .phones()
        .into_iter()
        .find(|d| d.device == device)
        .map_or("Your phone", |d| d.label.as_str())
}

/// The project row: the folder and **Choose folder…**, with the reason a
/// folder was refused under it.
fn project_row(model: &Model) -> Vec<Node<Intent>> {
    let mut rows = Vec::new();
    let (path, label) = match model.project() {
        Some(project) => (
            text("project-path", &project.path, TextRole::Body),
            "Choose another folder…",
        ),
        None => (
            text("project-path", "No project yet.", TextRole::Status),
            "Choose folder…",
        ),
    };
    rows.push(stack(
        "project",
        Axis::Horizontal,
        Space::Sm,
        vec![path, quiet("choose", label, Intent::ChooseFolder)],
    ));
    if let Some(problem) = &model.problem {
        rows.push(text("problem", problem, TextRole::Status));
    }
    rows
}

fn autostart(model: &Model) -> Node<Intent> {
    checkbox(
        "autostart",
        "Let my phone start Coder here",
        model.autostart(),
        Intent::ToggleAutostart,
        model.project().is_some(),
    )
}

/// One agent's line: a check when it is signed in.
fn signed_in(key: &str, name: &str, on: bool) -> Node<Intent> {
    let mark = if on { "✓" } else { "–" };
    let state = if on { "signed in" } else { "not signed in" };
    let mut line = text(key, format!("{mark}  {name} · {state}"), TextRole::Body);
    if !on {
        line.style.foreground = Some(Color::rgb(150, 150, 150));
    }
    line
}

/// `DSK-02`: the phone is connected; pick a project and say whether Coder
/// can run here.
fn connected(model: &Model, device: &str) -> Node<Intent> {
    let mut project = vec![bold("project-title", "Pick a project for Coder")];
    project.extend(project_row(model));
    let mut agents = vec![
        bold(
            "agents-title",
            "Coder uses Codex or Claude Code on this Mac",
        ),
        signed_in("codex", "Codex", model.agents.codex),
        signed_in("claude", "Claude Code", model.agents.claude),
    ];
    if !model.agents.codex && !model.agents.claude {
        agents.push(text(
            "agents-help",
            "Sign in to Codex or Claude Code on this Mac so Coder can work here.",
            TextRole::Status,
        ));
    }
    stack(
        "connected",
        Axis::Vertical,
        Space::Lg,
        vec![
            text(
                "connected-title",
                format!("✓ {} is connected.", device_label(model, device)),
                TextRole::Heading,
            ),
            card("project-card", project),
            card("agents-card", agents),
            autostart(model),
            button("done", "Done", Intent::Done, true),
        ],
    )
}

/// `DSK-03`: status, phones with Remove, Coder's tasks, and Connect another
/// phone.
fn home(model: &Model, now: u64) -> Node<Intent> {
    let online = model.host.as_ref().is_some_and(|host| host.status.online);
    let status = if online && model.phones().is_empty() {
        "Online."
    } else if online {
        "Online. Your phone can reach this Mac."
    } else if model.host.is_none() && model.old.is_some() {
        "Coder runs here from an earlier setup."
    } else {
        "Offline."
    };
    let mut phones = vec![bold("phones-title", "Phones")];
    let list = model.phones();
    if model.host.is_none() {
        let line = match &model.old {
            Some(old) if old.phones == 1 => "1 phone can reach this Mac.".to_string(),
            Some(old) => format!("{} phones can reach this Mac.", old.phones),
            None => "Waiting for Coder on this Mac…".to_string(),
        };
        phones.push(text("no-phones", line, TextRole::Status));
    } else if list.is_empty() {
        phones.push(text("no-phones", "No phones yet.", TextRole::Status));
    } else {
        let mut rows = Vec::new();
        for (index, device) in list.iter().enumerate() {
            let key = format!("phone-{index}");
            if model.confirming.as_deref() == Some(device.device.as_str()) {
                rows.push(stack(
                    &key,
                    Axis::Vertical,
                    Space::Sm,
                    vec![
                        text(
                            &format!("{key}-ask"),
                            format!(
                                "Remove {}? It can't reach this Mac until it connects again.",
                                device.label
                            ),
                            TextRole::Body,
                        ),
                        stack(
                            &format!("{key}-choice"),
                            Axis::Horizontal,
                            Space::Sm,
                            vec![
                                button(
                                    &format!("{key}-remove"),
                                    "Remove",
                                    Intent::Remove {
                                        device: device.device.clone(),
                                    },
                                    true,
                                ),
                                quiet(&format!("{key}-keep"), "Keep", Intent::Keep),
                            ],
                        ),
                    ],
                ));
                continue;
            }
            let mut detail = match device.last_seen {
                Some(seen) => format!("seen {}", ago(seen, now)),
                None => "not seen yet".to_string(),
            };
            if crate::control::terminal(device) {
                detail.push_str(" · terminal");
            }
            rows.push(stack(
                &key,
                Axis::Horizontal,
                Space::Sm,
                vec![
                    stack(
                        &format!("{key}-text"),
                        Axis::Vertical,
                        Space::None,
                        vec![
                            text(&format!("{key}-name"), &device.label, TextRole::Body),
                            text(&format!("{key}-seen"), detail, TextRole::Status),
                        ],
                    ),
                    quiet(
                        &format!("{key}-ask"),
                        "Remove",
                        Intent::AskRemove {
                            device: device.device.clone(),
                        },
                    ),
                ],
            ));
        }
        phones.push(node(
            "phones",
            Style {
                gap: Some(Space::Md),
                ..Style::default()
            },
            Element::List {
                label: "Phones".into(),
                children: rows,
            },
        ));
    }
    let mut coder = vec![bold("coder-title", "Coder")];
    if model.tasks.is_empty() {
        coder.push(text(
            "no-tasks",
            "Nothing yet. On your phone, ask for work and tap Run Coder.",
            TextRole::Status,
        ));
    } else {
        let rows = model
            .tasks
            .iter()
            .enumerate()
            .map(|(index, task)| {
                text(
                    &format!("task-{index}"),
                    format!("{} · {}", task.title, task_status(&task.status)),
                    TextRole::Body,
                )
            })
            .collect();
        coder.push(node(
            "tasks",
            Style {
                gap: Some(Space::Sm),
                ..Style::default()
            },
            Element::List {
                label: "Coder's tasks".into(),
                children: rows,
            },
        ));
    }
    if model.host.is_some() {
        coder.extend(project_row(model));
        coder.push(autostart(model));
    }
    stack(
        "home",
        Axis::Vertical,
        Space::Lg,
        vec![
            text("home-status", status, TextRole::Heading),
            card("phones-card", phones),
            card("coder-card", coder),
            button(
                "another",
                "Connect another phone",
                Intent::ConnectAnother,
                true,
            ),
        ],
    )
}

/// A Mac set up the old way: use that setup?
fn adopt(model: &Model) -> Node<Intent> {
    let Some(old) = &model.old else {
        return home(model, 0);
    };
    let phones = match old.phones {
        0 => "No phones are connected to it yet.".to_string(),
        1 => "1 phone can reach it now, and it keeps working.".to_string(),
        n => format!("{n} phones can reach it now, and they keep working."),
    };
    let mut children = if old.ready {
        vec![
            text(
                "adopt-title",
                "Use this Mac's existing Coder setup?",
                TextRole::Heading,
            ),
            text(
                "adopt-line",
                format!("This Mac already runs Coder. {phones} Your projects stay as they are."),
                TextRole::Body,
            ),
        ]
    } else {
        vec![
            text(
                "adopt-title",
                "This Mac already runs Coder.",
                TextRole::Heading,
            ),
            text(
                "adopt-line",
                format!("{phones} Your projects stay as they are."),
                TextRole::Body,
            ),
        ]
    };
    if !old.ready {
        children.push(text(
            "adopt-later",
            "This version of OpenAgents can't take it over yet, so Coder keeps running as it is.",
            TextRole::Status,
        ));
        children.push(button("ok", "OK", Intent::NotNow, true));
    } else if model.adopting() {
        children.push(text("adopting", "Moving your setup…", TextRole::Status));
    } else {
        children.push(stack(
            "choice",
            Axis::Horizontal,
            Space::Sm,
            vec![
                button("use", "Use it", Intent::Adopt, true),
                quiet("not-now", "Not now", Intent::NotNow),
            ],
        ));
    }
    if let Some(problem) = &model.problem {
        children.push(text("problem", problem, TextRole::Status));
    }
    stack("adopt", Axis::Vertical, Space::Md, children)
}

/// Keeps one view lifetime: a new revision only when the tree changed.
#[derive(Debug)]
pub struct Presenter {
    instance: String,
    revision: u64,
    current: Option<ValidatedView<Intent>>,
}

impl Presenter {
    pub fn new(instance: &str) -> Presenter {
        Presenter {
            instance: instance.into(),
            revision: 0,
            current: None,
        }
    }

    /// Shows `root`; returns whether the view changed.
    pub fn present(&mut self, root: Node<Intent>) -> bool {
        if self
            .current
            .as_ref()
            .is_some_and(|current| current.view().root == root)
        {
            return false;
        }
        self.revision += 1;
        match View::new(self.instance.clone(), self.revision, root).validate() {
            Ok(view) => {
                self.current = Some(view);
                true
            }
            Err(_) => false,
        }
    }

    /// The current view.
    pub fn view(&self) -> &ValidatedView<Intent> {
        self.current.as_ref().expect("a view is presented first")
    }
}

/// A plain outline of a view, one node a line: what the screen says and
/// what each control does. The golden snapshots under `snapshots/` hold it.
pub fn outline<I: serde::Serialize>(node: &Node<I>) -> String {
    let mut out = String::new();
    write_outline(node, 0, &mut out);
    out
}

fn write_outline<I: serde::Serialize>(node: &Node<I>, depth: usize, out: &mut String) {
    let pad = "  ".repeat(depth);
    let style = &node.style;
    let mut notes = Vec::new();
    if let Some(align) = style.align {
        notes.push(format!("align={align:?}").to_lowercase());
    }
    if style.background.is_some_and(|color| color.alpha > 0)
        && matches!(node.element, Element::Stack { .. })
    {
        notes.push("card".into());
    }
    if style.weight == Some(TextWeight::Bold) {
        notes.push("bold".into());
    }
    let notes = if notes.is_empty() {
        String::new()
    } else {
        format!(" [{}]", notes.join(" "))
    };
    let intent = |intent: &I| {
        serde_json::to_value(intent)
            .ok()
            .and_then(|value| {
                value
                    .get("kind")
                    .and_then(|k| k.as_str())
                    .map(str::to_string)
            })
            .unwrap_or_default()
    };
    let line = match &node.element {
        Element::Stack { axis, .. } => format!("{:?} stack", axis).to_lowercase(),
        Element::List { label, .. } => format!("list {label:?}"),
        Element::Text { value, role } => {
            format!("{} {value:?}", format!("{role:?}").to_lowercase())
        }
        Element::Button {
            label,
            enabled,
            icon,
            intent: action,
        } => {
            let kind = match icon.map(|icon| icon.glyph) {
                Some(Glyph::Checked) => "checkbox [x]",
                Some(Glyph::Unchecked) => "checkbox [ ]",
                _ if style.background.is_some_and(|c| c.alpha == 0) => "link",
                _ => "button",
            };
            let off = if *enabled { "" } else { " (disabled)" };
            format!("{kind} {label:?} -> {}{off}", intent(action))
        }
        Element::Surface { resource, label } => format!("surface {resource} {label:?}"),
        Element::Transcript { .. } => "transcript".into(),
        Element::Message { .. } => "message".into(),
        Element::Markdown { .. } => "markdown".into(),
        Element::Tool { name, .. } => format!("tool {name:?}"),
        Element::Working { label } => format!("working {label:?}"),
        Element::Composer { .. } => "composer".into(),
    };
    out.push_str(&format!("{pad}{line}{notes}\n"));
    if let Element::Stack { children, .. } | Element::List { children, .. } = &node.element {
        for child in children {
            write_outline(child, depth + 1, out);
        }
    }
}

/// Every visible word on a screen: text values and control labels.
pub fn words<I>(node: &Node<I>) -> Vec<String> {
    let mut out = Vec::new();
    let mut pending = vec![node];
    while let Some(node) = pending.pop() {
        match &node.element {
            Element::Text { value, .. } => out.push(value.clone()),
            Element::Button { label, .. } => out.push(label.clone()),
            Element::Surface { label, .. } | Element::List { label, .. } => out.push(label.clone()),
            _ => {}
        }
        if let Element::Stack { children, .. } | Element::List { children, .. } = &node.element {
            pending.extend(children);
        }
    }
    out
}
