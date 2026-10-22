//! Kiln's transcript: the Kiln page's agent run drawn with the desktop's
//! transcript in the page's `#gpui-mount` box.
//!
//! Kiln (`crates/kiln`, a separate workspace) hands the run over as JSON
//! through [`show`] each time it changes: entries of text, thinking,
//! status, and tool parts (`docs/kiln/transcript.md` has the shape).
//! [`entries`] maps them onto the desktop's [`SessionMessageEntry`] parts,
//! so a step's reasoning draws as a thinking block, each tool as the
//! desktop's tool card (a file edit with its diff), and Pylon routing as a
//! quiet notice line.

use std::cell::{Cell, RefCell};

use openagents_doc::{MessagePart, MessageRole, MessageStatus, SessionMessageEntry};
use openagents_proto::{ToolCall, ToolDiff};
use serde::Deserialize;

#[cfg(target_family = "wasm")]
mod app;

/// A tool's output kept in the card; the rest is behind Kiln's raw log.
const OUTPUT_MAX: usize = 4_000;

/// The run as Kiln sends it.
#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
pub struct Run {
    #[serde(default)]
    pub entries: Vec<Entry>,
    /// What the agent is doing right now, while it works.
    #[serde(default)]
    pub working: Option<String>,
    /// When the working turn started (ms since the epoch).
    #[serde(default)]
    pub started_ms: Option<i64>,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct Entry {
    pub id: String,
    pub role: String,
    #[serde(default)]
    pub parts: Vec<Part>,
    #[serde(default)]
    pub streaming: bool,
    #[serde(default)]
    pub created_ms: i64,
    #[serde(default)]
    pub duration_ms: Option<i64>,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Part {
    Text {
        text: String,
    },
    Reasoning {
        text: String,
    },
    /// A quiet line: Pylon routing, timings.
    Status {
        text: String,
    },
    Tool {
        id: String,
        /// `read`, `search`, `list`, `exec`, `write`, or `edit`.
        tool: String,
        #[serde(default)]
        path: Option<String>,
        #[serde(default)]
        pattern: Option<String>,
        #[serde(default)]
        command: Option<String>,
        #[serde(default)]
        output: Option<String>,
        #[serde(default)]
        failed: bool,
        #[serde(default)]
        done: bool,
        /// A file's text before and after an edit or write, for the diff.
        #[serde(default)]
        old: Option<String>,
        #[serde(default)]
        new: Option<String>,
    },
}

fn clip(text: &str) -> String {
    if text.len() <= OUTPUT_MAX {
        return text.to_owned();
    }
    let mut end = OUTPUT_MAX;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}\n…", &text[..end])
}

/// The desktop's parts for one Kiln part.
fn part(index: usize, part: &Part) -> MessagePart {
    match part {
        Part::Text { text } => MessagePart::Text {
            id: format!("t{index}"),
            text: text.clone(),
        },
        Part::Reasoning { text } => MessagePart::Reasoning {
            id: format!("r{index}"),
            text: text.clone(),
        },
        Part::Status { text } => MessagePart::Text {
            id: format!(
                "{}-{index}",
                openagents_web_chat_view::labels::NOTICE_PREFIX
            ),
            // Quieter than the answer: emphasis, with markdown's own
            // marks escaped.
            text: format!("*{}*", text.replace('*', "\\*").replace('_', "\\_")),
        },
        Part::Tool {
            id,
            tool,
            path,
            pattern,
            command,
            output,
            failed,
            done,
            old,
            new,
        } => {
            let path = path.clone().unwrap_or_default();
            let call = match tool.as_str() {
                "read" => ToolCall::ReadFile { path: path.clone() },
                "search" => ToolCall::Search {
                    pattern: pattern.clone().unwrap_or_default(),
                    path: (!path.is_empty()).then(|| path.clone()),
                },
                "write" => ToolCall::WriteFile {
                    path: path.clone(),
                    content: None,
                },
                "edit" => ToolCall::EditFile {
                    path: path.clone(),
                    old_string: None,
                    new_string: None,
                },
                _ => ToolCall::Exec {
                    command: command.clone().unwrap_or_else(|| tool.clone()),
                },
            };
            let diff = new.as_ref().map(|new| ToolDiff {
                path: path.clone(),
                old_text: old.clone(),
                new_text: new.clone(),
            });
            let output = output.as_deref().map(clip).filter(|o| !o.trim().is_empty());
            MessagePart::Tool {
                id: id.clone(),
                call,
                is_error: *failed,
                resolved: *done,
                output_bytes: output.as_ref().map(|o| o.len() as u64),
                output,
                diff,
                output_ref: None,
                diff_ref: None,
                diff_stats: None,
                subagent_ref: None,
                subagent_status: None,
                subagent_tail: None,
            }
        }
    }
}

/// The desktop's entries for a Kiln run.
#[must_use]
pub fn entries(run: &Run) -> Vec<SessionMessageEntry> {
    run.entries
        .iter()
        .map(|entry| SessionMessageEntry {
            id: entry.id.clone(),
            role: if entry.role == "user" {
                MessageRole::User
            } else {
                MessageRole::Assistant
            },
            parts: entry
                .parts
                .iter()
                .enumerate()
                .map(|(i, p)| part(i, p))
                .collect(),
            created_at: entry.created_ms,
            device_id: String::new(),
            status: Some(if entry.streaming {
                MessageStatus::Streaming
            } else {
                MessageStatus::Complete
            }),
            continuation_of: None,
            duration_ms: entry.duration_ms,
        })
        .collect()
}

thread_local! {
    /// The newest run Kiln handed over, and a count of handovers.
    static LATEST: RefCell<Run> = RefCell::new(Run::default());
    static VERSION: Cell<u64> = const { Cell::new(0) };
}

/// Takes Kiln's JSON. Anything that doesn't parse is ignored.
pub fn take(json: &str) {
    let Ok(run) = serde_json::from_str::<Run>(json) else {
        return;
    };
    LATEST.with(|latest| *latest.borrow_mut() = run);
    VERSION.with(|version| version.set(version.get() + 1));
}

/// The newest run and its handover count.
#[cfg_attr(not(target_family = "wasm"), allow(dead_code))]
pub(crate) fn latest() -> (u64, Run) {
    (
        VERSION.with(Cell::get),
        LATEST.with(|latest| latest.borrow().clone()),
    )
}

#[cfg(target_family = "wasm")]
use wasm_bindgen::prelude::*;

/// Draws the transcript in `#gpui-mount`.
#[cfg(target_family = "wasm")]
#[wasm_bindgen]
pub fn mount() {
    use std::rc::Rc;

    use gpui::{
        AppContext, Application, WindowBackgroundAppearance, WindowBounds, WindowOptions, px, size,
    };
    use openagents_gpui_web::{WebFonts, WebPlatform};
    use openagents_web_chat_view::{assets, theme};

    console_error_panic_hook::set_once();
    openagents_gpui_web::init_logging();
    let faces: [&'static [u8]; 4] = [
        include_bytes!("../../../crates/ui/assets/fonts/Geist.ttf"),
        include_bytes!("../../../crates/ui/assets/fonts/Geist-Medium.ttf"),
        include_bytes!("../../../crates/ui/assets/fonts/Geist-SemiBold.ttf"),
        include_bytes!("../../../crates/ui/assets/fonts/GeistMono.ttf"),
    ];
    let platform = WebPlatform::new(
        false,
        WebFonts {
            fallback_family: theme::SANS,
            faces: faces.into_iter().map(std::borrow::Cow::Borrowed).collect(),
        },
    );
    let handle = Application::with_platform(Rc::new(platform))
        .with_assets(assets::Assets)
        .run_embedded(move |cx| {
            openagents_web_chat_view::install(cx);
            let bounds = gpui::Bounds::centered(None, size(px(720.), px(640.)), cx);
            let opened = cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    window_background: WindowBackgroundAppearance::Transparent,
                    ..Default::default()
                },
                move |window, cx| cx.new(|cx| app::KilnTranscript::new(window, cx)),
            );
            if opened.is_err() {
                app::signal("gpui-failed");
            }
            cx.activate(true);
        });
    std::mem::forget(handle);
}

/// Hands the transcript Kiln's run ([`take`]).
#[cfg(target_family = "wasm")]
#[wasm_bindgen]
pub fn show(json: &str) {
    take(json);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kiln_parts_map_onto_the_desktops() {
        let json = r#"{"entries":[
            {"id":"u","role":"user","parts":[{"kind":"text","text":"make fib"}]},
            {"id":"a1","role":"assistant","streaming":true,"parts":[
                {"kind":"status","text":"DeepSeek V4.1 Flash · 24 s"},
                {"kind":"reasoning","text":"I should read the file."},
                {"kind":"text","text":"Read it first."},
                {"kind":"tool","id":"c1","tool":"read","path":"src/lib.rs","output":"fn a() {}","done":true},
                {"kind":"tool","id":"c2","tool":"edit","path":"src/lib.rs","old":"a\n","new":"b\n","done":true},
                {"kind":"tool","id":"c3","tool":"exec","command":"cargo test -p x","output":"tests failed: 1 passed, 1 failed","failed":true,"done":true}
            ]}],"working":"Asking DeepSeek"}"#;
        take("not json");
        assert_eq!(latest().0, 0);
        take(json);
        let (version, run) = latest();
        assert_eq!(version, 1);
        assert_eq!(run.working.as_deref(), Some("Asking DeepSeek"));
        let entries = entries(&run);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].role, MessageRole::User);
        assert_eq!(entries[1].status, Some(MessageStatus::Streaming));
        let parts = &entries[1].parts;
        assert!(matches!(&parts[0], MessagePart::Text { id, .. } if id.starts_with("notice")));
        assert!(matches!(&parts[1], MessagePart::Reasoning { text, .. } if text.contains("read")));
        assert!(
            matches!(&parts[3], MessagePart::Tool { call: ToolCall::ReadFile { path }, resolved: true, .. } if path == "src/lib.rs")
        );
        assert!(
            matches!(&parts[4], MessagePart::Tool { diff: Some(diff), .. } if diff.new_text == "b\n")
        );
        assert!(matches!(
            &parts[5],
            MessagePart::Tool {
                is_error: true,
                call: ToolCall::Exec { .. },
                ..
            }
        ));
        let long = "x".repeat(OUTPUT_MAX + 10);
        assert!(clip(&long).len() < long.len());
    }
}
