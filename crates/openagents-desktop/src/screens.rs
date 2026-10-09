//! The screens as Rust Native views: `DSK-01` Connect a phone, `DSK-02`
//! Connected, `DSK-03` Home, and `DSK-04` a phone nearby ([`nearby`]).
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
fn card_fill() -> Color {
    openagents_chat_app::visual::pick(
        Color::rgb(24, 24, 24),
        openagents_chat_app::visual::current().selected,
    )
}
/// A quiet button's fill.
fn quiet_fill() -> Color {
    openagents_chat_app::visual::pick(
        Color::rgb(44, 44, 44),
        openagents_chat_app::visual::current().selected,
    )
}

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
            shortcut: None,
            label: label.into(),
            enabled,
            icon: None,
            intent,
        },
    )
}

fn quiet(key: &str, label: &str, intent: Intent) -> Node<Intent> {
    let mut node = button(key, label, intent, true);
    node.style.background = Some(quiet_fill());
    node.style.foreground = Some(openagents_chat_app::visual::pick(
        Color::rgb(245, 245, 245),
        openagents_chat_app::visual::current().text,
    ));
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
            shortcut: None,
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
    node.style.background = Some(card_fill());
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
        return nearby::prompt(prompt);
    }
    match &model.screen {
        Screen::Connect => connect(model),
        Screen::Connected { device } => connected(model, device),
        Screen::Home => home(model, now),
    }
}

/// `DSK-01`: the code, the sentence, and the copy.
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
    let line = |value: String| centered(text("waiting", value, TextRole::Body));
    if model.codes.held() == Some(Held::Idle) {
        return vec![
            line("The code is hidden because nobody used this window for 10 minutes.".into()),
            button("show", "Show the code", Intent::ShowCode, true),
        ];
    }
    if model.host.is_none() {
        return unanswered(model)
            .into_iter()
            .map(|node| match node.element {
                Element::Text { .. } => centered(node),
                _ => node,
            })
            .collect();
    }
    vec![line("Making a code…".into())]
}

/// What shows while Coder does not answer, on the code screen and on
/// home: what is happening, and after [`crate::model::STALL`] a plain
/// line and **Try again**. It never waits without saying so.
fn unanswered(model: &Model) -> Vec<Node<Intent>> {
    let line = |value: String| text("waiting", value, TextRole::Body);
    let computer = model.computer;
    match &model.agent {
        Agent::NeedsApproval => {
            return vec![
                line("OpenAgents needs your OK to run in the background.".into()),
                text(
                    "approve",
                    "Turn on OpenAgents under Allow in the Background, then come back here.",
                    TextRole::Status,
                ),
                button(
                    "login-items",
                    "Open Login Items",
                    Intent::OpenLoginItems,
                    true,
                ),
            ];
        }
        Agent::Failed(_) => {
            return vec![
                line(format!(
                    "Coder couldn't start on this {computer}. Quit OpenAgents and open it again."
                )),
                button("retry", "Try again", Intent::Retry, true),
            ];
        }
        _ => {}
    }
    if model.stalled {
        return vec![
            line(format!(
                "Coder isn't answering on this {computer}. OpenAgents keeps trying."
            )),
            text(
                "waiting-help",
                "If this lasts, quit OpenAgents and open it again.",
                TextRole::Status,
            ),
            button("retry", "Try again", Intent::Retry, true),
        ];
    }
    if let (Agent::NotRegistered, Some(note)) = (&model.agent, &model.note) {
        return vec![line(note.clone())];
    }
    if model.reached {
        return vec![line(format!(
            "Coder on this {computer} stopped answering. Trying again…"
        ))];
    }
    vec![line(format!("Starting Coder on this {computer}…"))]
}

fn device_label<'a>(model: &'a Model, device: &str) -> &'a str {
    model
        .phones()
        .into_iter()
        .find(|d| d.device == device)
        .map_or("Your phone", |d| phone_name(&d.label))
}

/// A phone's name as the host lists it, or **Your phone** when it has none:
/// a phone that paired by scanning a code gives the host no name.
fn phone_name(label: &str) -> &str {
    if label.trim().is_empty() {
        "Your phone"
    } else {
        label
    }
}

/// The project row: the folder the person picked (never the host's
/// worktree of it) and **Choose folder…**, with the reason a folder was
/// refused under it.
fn project_row(model: &Model) -> Vec<Node<Intent>> {
    let mut rows = Vec::new();
    // A folder just chosen shows at once, with Saving… until the host has
    // taken it on (it starts again to serve it).
    if let Some(saving) = &model.saving {
        let mut row = vec![text(
            "project-path",
            saving.path.display().to_string(),
            TextRole::Body,
        )];
        row.push(if saving.running {
            text("project-saving", "Saving…", TextRole::Status)
        } else {
            quiet("choose", "Choose another folder…", Intent::ChooseFolder)
        });
        rows.push(stack("project", Axis::Horizontal, Space::Sm, row));
        if let Some(problem) = &model.problem {
            rows.push(text("problem", problem, TextRole::Status));
        }
        return rows;
    }
    let (path, label) = match model.project() {
        Some(project) => (
            text("project-path", project.shown(), TextRole::Body),
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
        line.style.foreground = Some(openagents_chat_app::visual::pick(
            Color::rgb(150, 150, 150),
            openagents_chat_app::visual::current().muted,
        ));
    }
    line
}

/// `DSK-02`: the phone is connected; pick a project and say whether Coder
/// can run here.
fn connected(model: &Model, device: &str) -> Node<Intent> {
    let mut project = vec![bold("project-title", "Pick a project for Coder")];
    project.extend(project_row(model));
    // Grok Build, allowed by default (#10091), shows when it is installed.
    let engines = if model.agents.grok.is_some() {
        "Codex, Claude Code, or Grok Build"
    } else {
        "Codex or Claude Code"
    };
    let mut agents = vec![
        bold(
            "agents-title",
            format!("Coder uses {engines} on this {}", model.computer),
        ),
        signed_in("codex", "Codex", model.agents.codex),
        signed_in("claude", "Claude Code", model.agents.claude),
    ];
    if let Some(grok) = model.agents.grok {
        agents.push(signed_in("grok", "Grok Build", grok));
    }
    if !model.agents.codex && !model.agents.claude && model.agents.grok != Some(true) {
        agents.push(text(
            "agents-help",
            format!(
                "Sign in to {engines} on this {} so Coder can work here.",
                model.computer
            ),
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

/// `DSK-03`: status, phones with Remove and Connect another phone, and
/// Coder's tasks.
fn home(model: &Model, now: u64) -> Node<Intent> {
    let online = model.host.as_ref().is_some_and(|host| host.status.online);
    let computer = model.computer;
    let status = if online && model.phones().is_empty() {
        "Online.".to_string()
    } else if online {
        format!("Online. Your phone can reach this {computer}.")
    } else {
        "Offline.".to_string()
    };
    // The way to a QR code sits in the Phones header, near the top, so it
    // shows without scrolling however many phones are listed.
    let mut phones = vec![stack(
        "phones-header",
        Axis::Horizontal,
        Space::Md,
        vec![
            bold("phones-title", "Phones"),
            button(
                "another",
                "Connect another phone",
                Intent::ConnectAnother,
                true,
            ),
        ],
    )];
    let list = model.phones();
    // Every pairing grants the same rights, so a right every row shares
    // says nothing; it shows only where phones differ.
    let terminal_differs = list
        .windows(2)
        .any(|pair| crate::control::terminal(pair[0]) != crate::control::terminal(pair[1]));
    if model.host.is_none() {
        phones.extend(unanswered(model));
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
                            if device.label.trim().is_empty() {
                                format!(
                                    "Remove this phone? It can't reach this {computer} until it connects again."
                                )
                            } else {
                                format!(
                                    "Remove {}? It can't reach this {computer} until it connects again.",
                                    device.label
                                )
                            },
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
            if terminal_differs && crate::control::terminal(device) {
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
                            text(
                                &format!("{key}-name"),
                                phone_name(&device.label),
                                TextRole::Body,
                            ),
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
                let line = match &task.reason {
                    Some(reason) => {
                        format!("{} · {} · {reason}", task.title, task_status(&task.status))
                    }
                    None => format!("{} · {}", task.title, task_status(&task.status)),
                };
                text(&format!("task-{index}"), line, TextRole::Body)
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
        ],
    )
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
            shortcut: _,
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
        Element::RichText { .. }
        | Element::Field { .. }
        | Element::Choice { .. }
        | Element::Dialog { .. } => "unsupported v3 component".into(),
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
