//! Kiln's cards as Rust Native views (#12456): the task, the loaded crate,
//! its files, and the patch, drawn from the [`State`] Kiln's page publishes.
//!
//! Kiln's page (`crates/kiln`) runs the agent, the workspace, and the
//! runner jobs, and publishes a [`State`] as JSON (`kilnState()`). A shell
//! draws [`cards`] with its Rust Native adapter: the Grid terminal's GPUI
//! chat today, and the desktop app with the same adapter. The two text
//! fields, [`TASK_FIELD`] and [`SLICE_FIELD`], are the shell's own text
//! controls, supplied through the adapter's field hook; a pressed button
//! resolves to an [`Intent`] the shell sends back to the page.

use rust_native::style::{Color, Style, TextWeight};
use rust_native::{Axis, Element, Node, TextRole, ValidatedView, View};
use serde::{Deserialize, Serialize};

/// The task field's node key.
pub const TASK_FIELD: &str = "kiln-task";
/// The crate field's node key.
pub const SLICE_FIELD: &str = "kiln-slice";
/// The crate a new workspace offers to load.
pub const DEFAULT_SLICE: &str = "crates/memory-stream";
/// A task that fits the default crate.
pub const DEFAULT_TASK: &str = "In crates/memory-stream/src/lib.rs, add a public function `half_life_hours() -> f64` with a doc comment: the number of hours after which `recency` falls to 0.5. Add a unit test that checks `recency(half_life_hours())` is within 1e-9 of 0.5. Run `cargo test -p memory-stream` until it passes, then publish the change.";
/// The most files the files card lists.
pub const FILES_SHOWN: usize = 40;

pub const TITLE_TASK: &str = "Task";
pub const TITLE_REPO: &str = "Crate";
pub const TITLE_FILES: &str = "Files";
pub const TITLE_PATCH: &str = "Patch";
pub const TASK_LABEL: &str = "What should change?";
pub const SLICE_LABEL: &str = "Directory";
pub const TASK_PLACEHOLDER: &str = "Describe a change to the loaded crate.";
pub const RUN: &str = "Run";
pub const STOP: &str = "Stop";
pub const LOAD: &str = "Load";
pub const PUBLISH: &str = "Propose patch";
pub const REVIEW: &str = "Open review";
pub const STARTING: &str = "Starting Kiln…";
pub const NO_FILES: &str = "No files yet. Load a crate to start.";
pub const NO_RUNNER: &str = "Tests can't run here: no test runner is set up.";
pub const UNTESTED: &str = "Run the tests to propose this change.";
pub const NOTHING_CHANGED: &str = "Nothing has changed yet.";
pub const PASSED: &str = "Tests pass. Ready to propose.";
pub const FAILED: &str = "Tests fail. Fix them to propose this change.";
pub const PROPOSED: &str = "Proposed as a patch.";
pub const CHANGED: &str = "changed";

/// What Kiln's page publishes about itself.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct State {
    /// The page has its tools and workspace.
    pub ready: bool,
    /// The agent or a load is running.
    pub busy: bool,
    /// What it's doing now, while busy.
    pub working: Option<String>,
    /// The crate in the workspace, when one is loaded.
    pub repo: Option<Repo>,
    /// The workspace's files, first [`FILES_SHOWN`] of them.
    pub files: Vec<File>,
    /// How many files the workspace holds in all.
    pub file_count: usize,
    /// A test runner is set up.
    pub runner: bool,
    /// The last `cargo test`, when one ran.
    pub tests: Option<Tests>,
    /// The loaded crate differs from what was loaded.
    pub changed: bool,
    /// The crate's change as it is now passed its tests.
    pub green: bool,
    /// The change was proposed.
    pub patch: Option<Patch>,
    /// The last thing that went wrong, in a sentence.
    pub problem: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Repo {
    /// The loaded directories, comma-separated.
    pub slice: String,
    /// The branch or commit asked for.
    pub reference: String,
    /// The commit it resolved to.
    pub commit: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct File {
    pub path: String,
    pub bytes: usize,
    /// Differs from the loaded base.
    pub changed: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Tests {
    pub passed: bool,
    /// The runner's summary, such as "12 passed".
    pub summary: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Patch {
    /// The `1617` event's ID, hex.
    pub id: String,
    /// The review page for it, relative to Kiln's page.
    pub review: String,
    /// The runner's check of the published patch.
    pub check: Option<String>,
}

/// What a press asks the page to do.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Intent {
    /// The task field changed (the shell keeps the text).
    Task,
    /// The crate field changed.
    Slice,
    /// Run the agent on the task.
    Run,
    Stop,
    /// Load the crate the field names.
    Load,
    /// Propose the change as a patch.
    Publish,
    /// Open the patch's review page.
    Review(String),
}

/// Colors the cards use: the shell's palette in Rust Native's terms.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Paints {
    pub text: Color,
    pub muted: Color,
    pub card: Color,
    pub border: Color,
    pub good: Color,
    pub bad: Color,
}

impl Default for Paints {
    fn default() -> Self {
        Self {
            text: Color::rgb(236, 236, 236),
            muted: Color::rgb(160, 160, 160),
            card: Color::rgb(20, 20, 20),
            border: Color::rgb(58, 58, 58),
            good: Color::rgb(110, 200, 140),
            bad: Color::rgb(235, 110, 100),
        }
    }
}

fn node(key: &str, style: Style, element: Element<Intent>) -> Node<Intent> {
    Node {
        key: key.into(),
        style,
        element,
    }
}

fn stack(key: &str, axis: Axis, gap: u16, children: Vec<Node<Intent>>) -> Node<Intent> {
    node(
        key,
        Style {
            gap_points: Some(gap),
            ..Style::default()
        },
        Element::Stack { axis, children },
    )
}

fn text(key: &str, value: &str, color: Color) -> Node<Intent> {
    node(
        key,
        Style {
            foreground: Some(color),
            ..Style::default()
        },
        Element::Text {
            value: value.into(),
            role: TextRole::Body,
        },
    )
}

fn code(key: &str, value: &str, color: Color) -> Node<Intent> {
    node(
        key,
        Style {
            foreground: Some(color),
            ..Style::default()
        },
        Element::Text {
            value: value.into(),
            role: TextRole::Code,
        },
    )
}

fn button(key: &str, label: &str, enabled: bool, intent: Intent) -> Node<Intent> {
    styled_button(key, label, enabled, intent, Style::default())
}

/// The card's main action: light on the dark card.
fn primary(key: &str, label: &str, enabled: bool, intent: Intent, paints: &Paints) -> Node<Intent> {
    let style = if enabled {
        Style {
            background: Some(paints.text),
            foreground: Some(paints.card),
            weight: Some(TextWeight::Semibold),
            ..Style::default()
        }
    } else {
        Style::default()
    };
    styled_button(key, label, enabled, intent, style)
}

fn styled_button(
    key: &str,
    label: &str,
    enabled: bool,
    intent: Intent,
    style: Style,
) -> Node<Intent> {
    node(
        key,
        style,
        Element::Button {
            label: label.into(),
            enabled,
            icon: None,
            shortcut: None,
            intent,
        },
    )
}

fn field(key: &str, label: &str, value: &str, multiline: bool, intent: Intent) -> Node<Intent> {
    node(
        key,
        Style::default(),
        Element::Field {
            label: label.into(),
            value: value.into(),
            placeholder: if multiline {
                TASK_PLACEHOLDER.into()
            } else {
                String::new()
            },
            secret: false,
            multiline,
            enabled: true,
            max_bytes: 8 * 1024,
            on_change: intent,
        },
    )
}

/// A titled card.
fn card(key: &str, title: &str, rows: Vec<Node<Intent>>, paints: &Paints) -> Node<Intent> {
    let mut children = vec![node(
        &format!("{key}-title"),
        Style {
            foreground: Some(paints.muted),
            weight: Some(TextWeight::Semibold),
            text_size: Some(11),
            ..Style::default()
        },
        Element::Text {
            value: title.to_uppercase(),
            role: TextRole::Heading,
        },
    )];
    children.extend(rows);
    node(
        key,
        Style {
            background: Some(paints.card),
            border: Some(paints.border),
            radius: Some(12),
            padding_points: Some([12, 14, 12, 14]),
            gap_points: Some(8),
            ..Style::default()
        },
        Element::Stack {
            axis: Axis::Vertical,
            children,
        },
    )
}

/// The patch card's line for where the change stands.
#[must_use]
pub fn patch_line(state: &State) -> &'static str {
    if state.patch.is_some() {
        PROPOSED
    } else if !state.changed {
        NOTHING_CHANGED
    } else if state.green {
        PASSED
    } else if !state.runner {
        NO_RUNNER
    } else if state.tests.as_ref().is_some_and(|t| !t.passed) {
        FAILED
    } else {
        UNTESTED
    }
}

/// The cards: the task, the crate, the patch, and the files. `task` and
/// `slice` are the fields' text as the shell holds it.
///
/// # Panics
///
/// Never: the tree is fixed and its keys are checked by the tests.
#[must_use]
pub fn cards(
    state: &State,
    task: &str,
    slice: &str,
    revision: u64,
    paints: &Paints,
) -> ValidatedView<Intent> {
    let ready = state.ready;
    let mut task_rows = vec![field(TASK_FIELD, TASK_LABEL, task, true, Intent::Task)];
    let mut buttons = vec![primary(
        "kiln-run",
        RUN,
        ready && !state.busy && !task.trim().is_empty(),
        Intent::Run,
        paints,
    )];
    if state.busy {
        buttons.push(button("kiln-stop", STOP, true, Intent::Stop));
    }
    task_rows.push(stack("kiln-task-buttons", Axis::Horizontal, 8, buttons));
    let status = if !ready {
        Some(STARTING.to_owned())
    } else {
        state.working.clone().filter(|_| state.busy)
    };
    if let Some(status) = status {
        task_rows.push(text("kiln-working", &status, paints.muted));
    }
    if let Some(problem) = &state.problem {
        task_rows.push(text("kiln-problem", problem, paints.bad));
    }

    let mut repo_rows = vec![stack(
        "kiln-slice-row",
        Axis::Vertical,
        8,
        vec![
            field(SLICE_FIELD, SLICE_LABEL, slice, false, Intent::Slice),
            button(
                "kiln-load",
                LOAD,
                ready && !state.busy && !slice.trim().is_empty(),
                Intent::Load,
            ),
        ],
    )];
    if let Some(repo) = &state.repo {
        let short = repo.commit.get(..10).unwrap_or(&repo.commit);
        repo_rows.push(code(
            "kiln-commit",
            &format!("{} @ {short}", repo.slice),
            paints.muted,
        ));
    }

    let mut patch_rows = vec![text(
        "kiln-patch-line",
        patch_line(state),
        if state.green || state.patch.is_some() {
            paints.good
        } else if state.tests.as_ref().is_some_and(|t| !t.passed) {
            paints.bad
        } else {
            paints.muted
        },
    )];
    if let Some(tests) = state.tests.as_ref().filter(|t| !t.summary.is_empty()) {
        patch_rows.push(code("kiln-tests", &tests.summary, paints.muted));
    }
    match &state.patch {
        Some(patch) => {
            let short = patch.id.get(..12).unwrap_or(&patch.id);
            patch_rows.push(code("kiln-patch-id", short, paints.text));
            // The runner's check of the published patch, when it says
            // something the last test run didn't.
            if let Some(check) = patch
                .check
                .as_ref()
                .filter(|c| state.tests.as_ref().is_none_or(|t| &t.summary != *c))
            {
                patch_rows.push(code("kiln-patch-check", check, paints.muted));
            }
            patch_rows.push(button(
                "kiln-review",
                REVIEW,
                true,
                Intent::Review(patch.review.clone()),
            ));
        }
        None => patch_rows.push(primary(
            "kiln-publish",
            PUBLISH,
            ready && !state.busy && state.green && state.changed,
            Intent::Publish,
            paints,
        )),
    }

    let mut file_rows = Vec::new();
    if state.files.is_empty() {
        file_rows.push(text("kiln-no-files", NO_FILES, paints.muted));
    }
    for (i, file) in state.files.iter().take(FILES_SHOWN).enumerate() {
        let line = if file.changed {
            format!("{}  {}  {CHANGED}", file.path, file.bytes)
        } else {
            format!("{}  {}", file.path, file.bytes)
        };
        file_rows.push(code(
            &format!("kiln-file-{i}"),
            &line,
            if file.changed {
                paints.text
            } else {
                paints.muted
            },
        ));
    }
    let shown = state.files.len().min(FILES_SHOWN);
    if state.file_count > shown {
        file_rows.push(text(
            "kiln-more-files",
            &format!("and {} more", state.file_count - shown),
            paints.muted,
        ));
    }

    let root = stack(
        "kiln-cards",
        Axis::Vertical,
        12,
        vec![
            card("kiln-task-card", TITLE_TASK, task_rows, paints),
            card("kiln-repo-card", TITLE_REPO, repo_rows, paints),
            card("kiln-patch-card", TITLE_PATCH, patch_rows, paints),
            card("kiln-files-card", TITLE_FILES, file_rows, paints),
        ],
    );
    View::new_v3("kiln-cards", revision.max(1), root)
        .validate()
        .expect("Kiln's cards are a fixed, valid tree")
}

/// The pressed node's intent, or `None` for a press on an older tree.
#[must_use]
pub fn intent(view: &ValidatedView<Intent>, node: &str) -> Option<Intent> {
    view.activate(&rust_native::Activation {
        instance: view.view().instance.clone(),
        revision: view.view().revision,
        node: node.into(),
    })
    .ok()
    .cloned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn loaded() -> State {
        State {
            ready: true,
            repo: Some(Repo {
                slice: DEFAULT_SLICE.into(),
                reference: "main".into(),
                commit: "0123456789abcdef".into(),
            }),
            files: (0..50)
                .map(|i| File {
                    path: format!("crates/memory-stream/src/f{i}.rs"),
                    bytes: 10,
                    changed: i == 3,
                })
                .collect(),
            file_count: 55,
            runner: true,
            changed: true,
            ..State::default()
        }
    }

    #[test]
    fn the_cards_follow_the_change_from_untested_to_proposed() {
        let mut state = loaded();
        assert_eq!(patch_line(&state), UNTESTED);
        let view = cards(&state, "task", DEFAULT_SLICE, 1, &Paints::default());
        assert_eq!(intent(&view, "kiln-run"), Some(Intent::Run));
        assert_eq!(intent(&view, "kiln-load"), Some(Intent::Load));
        // Not green: the publish button is there but disabled.
        assert_eq!(intent(&view, "kiln-publish"), None);
        state.tests = Some(Tests {
            passed: false,
            summary: "1 passed, 1 failed".into(),
        });
        assert_eq!(patch_line(&state), FAILED);
        state.green = true;
        state.tests = Some(Tests {
            passed: true,
            summary: "2 passed".into(),
        });
        assert_eq!(patch_line(&state), PASSED);
        let view = cards(&state, "task", DEFAULT_SLICE, 2, &Paints::default());
        assert_eq!(intent(&view, "kiln-publish"), Some(Intent::Publish));
        state.patch = Some(Patch {
            id: "ab".repeat(32),
            review: "review.html#ab".into(),
            check: Some("2 passed".into()),
        });
        let view = cards(&state, "task", DEFAULT_SLICE, 3, &Paints::default());
        assert_eq!(patch_line(&state), PROPOSED);
        assert_eq!(
            intent(&view, "kiln-review"),
            Some(Intent::Review("review.html#ab".into()))
        );
    }

    #[test]
    fn busy_offers_stop_and_not_run() {
        let mut state = loaded();
        state.busy = true;
        state.working = Some("Asking DeepSeek V4.1 Flash".into());
        let view = cards(&state, "task", DEFAULT_SLICE, 1, &Paints::default());
        assert_eq!(intent(&view, "kiln-stop"), Some(Intent::Stop));
        assert_eq!(intent(&view, "kiln-run"), None);
        // A file list longer than the card says how many more there are.
        let json = String::from_utf8(view.to_json().unwrap()).unwrap();
        assert!(json.contains("and 15 more"));
        assert!(json.contains("f3.rs  10  changed"));
    }

    #[test]
    fn an_empty_or_starting_page_validates() {
        let view = cards(&State::default(), "", "", 1, &Paints::default());
        let json = String::from_utf8(view.to_json().unwrap()).unwrap();
        assert!(json.contains(STARTING) && json.contains(NO_FILES));
        assert_eq!(intent(&view, "kiln-run"), None);
    }

    #[test]
    fn the_state_reads_what_the_page_writes() {
        let state = loaded();
        let json = serde_json::to_string(&state).unwrap();
        assert_eq!(serde_json::from_str::<State>(&json).unwrap(), state);
        // An older page's partial state still reads.
        let partial: State = serde_json::from_str(r#"{"ready":true}"#).unwrap();
        assert!(partial.ready && partial.files.is_empty());
    }

    #[test]
    fn the_words_are_plain() {
        for words in [
            TITLE_TASK,
            TITLE_REPO,
            TITLE_FILES,
            TITLE_PATCH,
            TASK_PLACEHOLDER,
            TASK_LABEL,
            SLICE_LABEL,
            RUN,
            STOP,
            LOAD,
            PUBLISH,
            REVIEW,
            STARTING,
            NO_FILES,
            NO_RUNNER,
            UNTESTED,
            NOTHING_CHANGED,
            PASSED,
            FAILED,
            PROPOSED,
        ] {
            assert!(oa_copy::violations(words, &[]).is_empty(), "{words}");
        }
    }
}
