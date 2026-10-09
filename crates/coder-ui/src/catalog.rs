//! A source-bound, effect-free catalog of reusable Coder presentation.

mod conversation;
mod settings;

use rust_native::view::{Node, View};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const SOURCE_REVISION: &str = "36d59f3806";
pub const LARGE_TRANSCRIPT_TURNS: usize = 5_000;

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct TranscriptFixtureStats {
    pub turns: usize,
    pub messages: usize,
    pub original_text_bytes: usize,
}

/// Original synthetic records remain addressable without an oversized view.
pub fn retained_fixture_turn(index: usize) -> Option<(String, String)> {
    (index<LARGE_TRANSCRIPT_TURNS).then(||(
        format!("Synthetic saved turn {index:05}. Review the shared Rust Native components, keeping their presentation, stable keys, and records intact."),
        format!("Synthetic saved reply {index:05}. The shared components keep text selectable, show the model that answered, and keep whole characters in a bounded viewport. You can scroll back to this reply when it is off screen.")
    ))
}

pub fn transcript_fixture_stats() -> TranscriptFixtureStats {
    let (prompt, reply) = retained_fixture_turn(0).expect("the first synthetic turn exists");
    TranscriptFixtureStats {
        turns: LARGE_TRANSCRIPT_TURNS,
        messages: LARGE_TRANSCRIPT_TURNS * 2,
        original_text_bytes: LARGE_TRANSCRIPT_TURNS * (prompt.len() + reply.len()),
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SourceRef {
    pub path: String,
    pub symbol: String,
    pub branch: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CatalogVariant {
    pub id: String,
    pub label: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CatalogEntry {
    pub id: String,
    pub family: String,
    pub title: String,
    pub description: String,
    pub sources: Vec<SourceRef>,
    pub variants: Vec<CatalogVariant>,
}

/// Presentation inputs only. No credential, provider, host, or execution owner.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct FixtureState {
    pub component: String,
    pub variant: String,
    pub width: u16,
    pub height: u16,
    pub phase: u8,
    pub elapsed: u64,
    pub selected: usize,
    pub draft: String,
    pub expanded: bool,
    pub scroll: usize,
    pub stage: String,
    pub notice: String,
    pub fields: BTreeMap<String, String>,
    pub flags: BTreeMap<String, bool>,
    pub events: Vec<String>,
    pub revision: u64,
}

impl Default for FixtureState {
    fn default() -> Self {
        Self {
            component: "screen.main".into(),
            variant: "demo".into(),
            width: 110,
            height: 36,
            phase: 0,
            elapsed: 0,
            selected: 0,
            draft: String::new(),
            expanded: false,
            scroll: 0,
            stage: String::new(),
            notice: String::new(),
            fields: BTreeMap::new(),
            flags: BTreeMap::new(),
            events: Vec::new(),
            revision: 1,
        }
    }
}

impl FixtureState {
    pub fn default_for(id: &str, variant: &str) -> Self {
        let mut state = Self {
            component: id.into(),
            variant: variant.into(),
            ..Self::default()
        };
        match variant {
            "narrow" => state.width = 40,
            "minimum" => { state.width = 24; state.height = 12; }
            "tiny" => { state.width = 20; state.height = 8; }
            "multiline" => state.draft = "Review the shared components.\nKeep the exact source geometry.\nReturn a checked result.".into(),
            "overflow" => state.draft = (1..=9).map(|n| format!("Draft row {n}")).collect::<Vec<_>>().join("\n"),
            "selected" | "child" => state.selected = 1,
            "running" | "streaming" => { state.flags.insert("busy".into(), true); }
            _ => {}
        }
        conversation::defaults(&mut state);
        settings::defaults(&mut state);
        state
    }
}

/// Closed fixture actions. Their reducer cannot invoke an external effect.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum CatalogIntent {
    Reset,
    Tick,
    Select { index: usize },
    Input { field: String },
    Toggle { field: String },
    Pick { field: String, value: String },
    Action { name: String },
    Scroll { delta: i16 },
    Resize { width: u16, height: u16 },
}

pub fn entries() -> Vec<CatalogEntry> {
    let mut entries = conversation::entries();
    entries.extend(settings::entries());
    entries
}

pub(crate) fn entry(
    id: &str,
    family: &str,
    title: &str,
    path: &str,
    symbols: &str,
    variants: &[&str],
) -> CatalogEntry {
    CatalogEntry {
        id: id.into(),
        family: family.into(),
        title: title.into(),
        description: format!(
            "Reusable {title}; deterministic fixture controls create no domain effects."
        ),
        sources: symbols
            .split(',')
            .map(|symbol| SourceRef {
                path: path.into(),
                symbol: symbol.trim().into(),
                branch: "all visible source branches; named fixtures".into(),
            })
            .collect(),
        variants: variants
            .iter()
            .map(|variant| CatalogVariant {
                id: (*variant).into(),
                label: variant.replace(['.', '-'], " "),
            })
            .collect(),
    }
}

pub fn view(id: &str, variant: &str, state: &FixtureState) -> View<CatalogIntent> {
    let root = settings::render(id, variant, state)
        .or_else(|| conversation::render(id, variant, state))
        .unwrap_or_else(|| {
            crate::components::text(
                "unavailable",
                "This component or variant is unavailable.",
                crate::source_theme::DIFF_DELETE_FG,
            )
        });
    let instance = format!("catalog:{}:{}", id, variant);
    View::new_v3(instance, state.revision.max(1), root)
}

/// Apply one bounded fixture action. The event journal retains names, never values.
pub fn reduce(
    state: &mut FixtureState,
    intent: CatalogIntent,
    input: Option<&str>,
) -> Result<(), String> {
    if input.is_some_and(|text| text.len() > 8_192) {
        return Err("The fixture input is too long.".into());
    }
    if intent == CatalogIntent::Reset {
        let next = state.revision.saturating_add(1);
        *state = FixtureState::default_for(&state.component, &state.variant);
        state.revision = next;
        return Ok(());
    }
    let handled_overlay = if state.component == "screen.main" {
        reduce_overlay(state, &intent, input)?
    } else {
        false
    };
    if handled_overlay {
    } else if let Some(result) = settings::reduce(state, &intent, input) {
        result?;
    } else {
        match &intent {
            CatalogIntent::Reset => {
                let next = state.revision.saturating_add(1);
                *state = FixtureState::default_for(&state.component, &state.variant);
                state.revision = next;
                return Ok(());
            }
            CatalogIntent::Tick => {
                state.phase = (state.phase + 1) % 8;
                state.elapsed = state.elapsed.saturating_add(1);
            }
            CatalogIntent::Select { index } => {
                if *index > 32 {
                    return Err("The fixture selection is unavailable.".into());
                }
                state
                    .fields
                    .insert(format!("draft.{}", state.selected), state.draft.clone());
                state.fields.insert(
                    format!("scroll.{}", state.selected),
                    state.scroll.to_string(),
                );
                state.selected = *index;
                state.draft = state
                    .fields
                    .get(&format!("draft.{index}"))
                    .cloned()
                    .unwrap_or_default();
                state.scroll = state
                    .fields
                    .get(&format!("scroll.{index}"))
                    .and_then(|n| n.parse().ok())
                    .unwrap_or(0);
                if let Some(message) = state.fields.get(&format!("message.{index}")).cloned() {
                    state.fields.insert("last_message".into(), message);
                } else {
                    state.fields.remove("last_message");
                }
            }
            CatalogIntent::Input { field } => {
                if field.len() > 96 {
                    return Err("The fixture field is unavailable.".into());
                }
                let value = input.unwrap_or_default();
                if field == "draft" {
                    state.draft = value.into();
                } else {
                    state.fields.insert(field.clone(), value.into());
                }
            }
            CatalogIntent::Toggle { field } => {
                if field == "expanded" {
                    state.expanded = !state.expanded;
                } else {
                    let current = state.flags.get(field).copied().unwrap_or(false);
                    state.flags.insert(field.clone(), !current);
                }
            }
            CatalogIntent::Pick { field, value } => {
                if field.len() > 96 || value.len() > 512 {
                    return Err("The fixture choice is unavailable.".into());
                }
                if field == "slash" {
                    state.draft = value.clone();
                } else {
                    state.fields.insert(field.clone(), value.clone());
                }
            }
            CatalogIntent::Scroll { delta } => {
                let maximum = conversation::maximum_scroll(state).unwrap_or(500_000);
                state.scroll = state
                    .scroll
                    .min(maximum)
                    .saturating_add_signed(isize::from(*delta))
                    .min(maximum);
                state.flags.insert("stick-to-end".into(), state.scroll == 0);
            }
            CatalogIntent::Resize { width, height } => {
                state.width = (*width).clamp(24, 160);
                state.height = (*height).clamp(12, 80);
            }
            CatalogIntent::Action { name } => {
                match name.as_str() {
                    "send" => {
                        let text = state.draft.trim().to_owned();
                        if !text.is_empty() {
                            let command = text.split_whitespace().next().unwrap_or_default();
                            match command {
                                "/plugins" => {
                                    state.draft.clear();
                                    open_overlay(state, "plugins.manager", "default");
                                }
                                "/models" => {
                                    state.draft.clear();
                                    open_overlay(state, "models.picker", "models");
                                }
                                "/resume" => {
                                    state.draft.clear();
                                    open_overlay(state, "sessions.resume", "recent");
                                }
                                "/brainstorm" => {
                                    state.draft.clear();
                                    open_overlay(
                                        state,
                                        "approvals.disclosure",
                                        if text.contains("rank") {
                                            "rank"
                                        } else {
                                            "search"
                                        },
                                    );
                                }
                                "/export" => {
                                    state.draft.clear();
                                    state.notice="Synthetic ATIF fixture ready. No private session was read.".into();
                                }
                                "/help" => {
                                    state.draft.clear();
                                    state.notice="/plugins · /models · /resume · /brainstorm · /export · /demo · /help\nTab: complete a command · Esc: stop or close · F2: plugins".into();
                                }
                                "/demo" => {
                                    let mode =
                                        if state.fields.get("mode").is_some_and(|m| m == "live") {
                                            "demo"
                                        } else {
                                            "live"
                                        };
                                    state.fields.insert("mode".into(), mode.into());
                                    state.draft.clear();
                                    state.selected = 0;
                                    state.scroll = 0;
                                }
                                _ => {
                                    state.fields.insert(
                                        format!("message.{}", state.selected),
                                        std::mem::take(&mut state.draft),
                                    );
                                    state.fields.insert("last_message".into(), text);
                                    state.flags.insert("busy".into(), true);
                                    state.notice =
                                        "Preview message added. No agent is connected.".into();
                                }
                            }
                        }
                    }
                    "open-plugins" => open_overlay(state, "plugins.manager", "default"),
                    "open-models" => open_overlay(state, "models.picker", "models"),
                    "open-resume" => open_overlay(state, "sessions.resume", "recent"),
                    "slash.previous" | "slash.next" => {
                        let current = state
                            .fields
                            .get("slash.selected")
                            .and_then(|v| v.parse::<usize>().ok())
                            .unwrap_or(0);
                        let next = if name == "slash.previous" {
                            current.saturating_sub(1)
                        } else {
                            current.saturating_add(1).min(6)
                        };
                        state
                            .fields
                            .insert("slash.selected".into(), next.to_string());
                    }
                    "close-overlay" => close_overlay(state),
                    "stop" => {
                        state.flags.insert("busy".into(), false);
                        state.notice = "Request stopped. This is a fixture.".into();
                    }
                    "complete" => {
                        state.flags.insert("busy".into(), false);
                        state.notice = "Synthetic reply completed.".into();
                    }
                    "expand" => state.expanded = !state.expanded,
                    "latest" => {
                        state.scroll = 0;
                        state.flags.insert("stick-to-end".into(), true);
                    }
                    "export" => {
                        state.notice =
                            "Synthetic ATIF fixture ready. No private session was read.".into()
                    }
                    "back" => state.selected = 0,
                    _ => return Err("The fixture action is unavailable.".into()),
                }
            }
        }
    }
    state.events.push(match &intent {
        CatalogIntent::Input { field } => format!("input:{field}"),
        CatalogIntent::Action { name } => format!("action:{name}"),
        CatalogIntent::Toggle { field } => format!("toggle:{field}"),
        CatalogIntent::Reset => "reset".into(),
        CatalogIntent::Tick => "tick".into(),
        CatalogIntent::Select { index } => format!("select:{index}"),
        CatalogIntent::Pick { field, .. } => format!("pick:{field}"),
        CatalogIntent::Scroll { .. } => "scroll".into(),
        CatalogIntent::Resize { .. } => "resize".into(),
    });
    if state.events.len() > 32 {
        state.events.remove(0);
    }
    state.revision = state.revision.saturating_add(1);
    Ok(())
}

fn open_overlay(state: &mut FixtureState, id: &str, variant: &str) {
    if let Some(previous) = state.fields.get("overlay").cloned() {
        state.fields.insert("overlay-underlay".into(), previous);
        state
            .fields
            .insert("underlay.selected".into(), state.selected.to_string());
        state
            .fields
            .insert("underlay.stage".into(), state.stage.clone());
    } else {
        state
            .fields
            .insert("main.selected".into(), state.selected.to_string());
        state
            .fields
            .insert("main.scroll".into(), state.scroll.to_string());
        state
            .fields
            .insert("main.draft".into(), std::mem::take(&mut state.draft));
        if let Some(mode) = state.fields.get("mode").cloned() {
            state.fields.insert("main.mode".into(), mode);
        } else {
            state.fields.remove("main.mode");
        }
    }
    let mut projected = FixtureState::default_for(id, variant);
    projected.width = state.width;
    projected.height = state.height;
    state.fields.extend(projected.fields);
    state.flags.extend(projected.flags);
    state.stage = projected.stage;
    state.selected = projected.selected;
    state.scroll = projected.scroll;
    state.fields.insert("overlay".into(), id.into());
    state
        .fields
        .insert("overlay-variant".into(), variant.into());
    state.notice.clear();
}

fn close_overlay(state: &mut FixtureState) {
    if let Some(previous) = state.fields.remove("overlay-underlay") {
        state.fields.insert("overlay".into(), previous);
        state
            .fields
            .insert("overlay-variant".into(), "default".into());
        state.selected = state
            .fields
            .remove("underlay.selected")
            .and_then(|v| v.parse().ok())
            .unwrap_or(0);
        state.stage = state.fields.remove("underlay.stage").unwrap_or_default();
        return;
    }
    state.fields.remove("overlay");
    state.fields.remove("overlay-variant");
    state.selected = state
        .fields
        .remove("main.selected")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    state.scroll = state
        .fields
        .remove("main.scroll")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    state.draft = state.fields.remove("main.draft").unwrap_or_default();
    state.stage.clear();
    if let Some(mode) = state.fields.remove("main.mode") {
        state.fields.insert("mode".into(), mode);
    } else {
        state.fields.remove("mode");
    }
}

fn reduce_overlay(
    state: &mut FixtureState,
    intent: &CatalogIntent,
    input: Option<&str>,
) -> Result<bool, String> {
    let Some(overlay) = state.fields.get("overlay").cloned() else {
        return Ok(false);
    };
    if matches!(intent,CatalogIntent::Action{name} if name=="close-overlay"||name=="back") {
        close_overlay(state);
        return Ok(true);
    }
    let component = state.component.clone();
    let variant = state.variant.clone();
    let before = state.stage.clone();
    let mut projected = state.clone();
    projected.component = overlay.clone();
    projected.variant = state
        .fields
        .get("overlay-variant")
        .cloned()
        .unwrap_or_else(|| "default".into());
    let Some(result) = settings::reduce(&mut projected, intent, input) else {
        return Ok(false);
    };
    result?;
    projected.component = component;
    projected.variant = variant;
    *state = projected;
    let close = state.stage == "closed"
        || matches!(state.stage.as_str(), "confirmed" | "rejected" | "cancelled")
        || matches!(intent,CatalogIntent::Action{name} if (name=="settings.back"||name=="settings.cancel")&&(overlay!="plugins.manager"||before!="configure"))
        || matches!(intent,CatalogIntent::Action{name} if name=="resume.select"&&state.notice.starts_with("Resumed"));
    if close {
        close_overlay(state);
    }
    Ok(true)
}

/// Read the bounded, rendered text for accessibility and deterministic checks.
pub fn plain<I>(node: &Node<I>) -> String {
    use rust_native::view::Element;
    match &node.element {
        Element::Text { value, .. } => value.clone(),
        Element::RichText { runs, .. } => runs.iter().map(|run| run.text.as_str()).collect(),
        Element::Field {
            label,
            value,
            secret,
            ..
        } => format!("{label}: {}", if *secret { "••••" } else { value }),
        Element::Choice {
            label, children, ..
        } => {
            if children.is_empty() {
                label.clone()
            } else {
                children.iter().map(plain).collect::<Vec<_>>().join("\n")
            }
        }
        Element::Button { label, .. } | Element::Working { label } => label.clone(),
        Element::Stack { children, .. }
        | Element::List { children, .. }
        | Element::Dialog { children, .. }
        | Element::Transcript { children, .. }
        | Element::Message { children, .. }
        | Element::Tool { children, .. } => {
            children.iter().map(plain).collect::<Vec<_>>().join("\n")
        }
        Element::Markdown { blocks } => rust_native::markdown::plain(blocks),
        Element::Surface { label, .. } => label.clone(),
        Element::Composer { placeholder, .. } => placeholder.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn action(state: &mut FixtureState, name: &str) {
        reduce(state, CatalogIntent::Action { name: name.into() }, None).unwrap();
    }
    fn input(state: &mut FixtureState, text: &str) {
        reduce(
            state,
            CatalogIntent::Input {
                field: "draft".into(),
            },
            Some(text),
        )
        .unwrap();
    }
    #[test]
    fn main_and_child_keep_drafts_scroll_and_messages_across_overlay_navigation() {
        let mut state = FixtureState::default_for("screen.main", "demo");
        input(&mut state, "Main draft 日本語");
        reduce(&mut state, CatalogIntent::Scroll { delta: 5 }, None).unwrap();
        reduce(&mut state, CatalogIntent::Select { index: 1 }, None).unwrap();
        assert_eq!(state.draft, "");
        assert_eq!(state.scroll, 0);
        input(&mut state, "Child draft 👩🏽‍💻");
        action(&mut state, "open-plugins");
        assert_eq!(
            state.fields.get("overlay").map(String::as_str),
            Some("plugins.manager")
        );
        reduce(
            &mut state,
            CatalogIntent::Pick {
                field: "plugin-index".into(),
                value: "2".into(),
            },
            None,
        )
        .unwrap();
        action(&mut state, "plugins.configure");
        assert_eq!(state.stage, "configure");
        action(&mut state, "settings.cancel");
        assert!(state.fields.contains_key("overlay"));
        action(&mut state, "settings.back");
        assert_eq!(state.selected, 1);
        assert_eq!(state.draft, "Child draft 👩🏽‍💻");
        reduce(&mut state, CatalogIntent::Select { index: 0 }, None).unwrap();
        assert_eq!(state.draft, "Main draft 日本語");
        assert_eq!(state.scroll, 5);
        action(&mut state, "send");
        assert!(state.events.iter().all(|event| !event.contains("日本語")));
        reduce(&mut state, CatalogIntent::Select { index: 1 }, None).unwrap();
        assert!(!state.fields.contains_key("last_message"));
        reduce(&mut state, CatalogIntent::Select { index: 0 }, None).unwrap();
        assert_eq!(
            state.fields.get("last_message").map(String::as_str),
            Some("Main draft 日本語")
        );
    }
    #[test]
    fn slash_and_model_stages_are_local_and_return_to_the_source_scene() {
        let mut state = FixtureState::default_for("screen.main", "demo");
        input(&mut state, "/models");
        action(&mut state, "send");
        assert_eq!(
            state.fields.get("overlay").map(String::as_str),
            Some("models.picker")
        );
        let view = view(&state.component, &state.variant, &state)
            .validate()
            .unwrap();
        assert!(plain(&view.view().root).contains("Review the terminal with four agents."));
        action(&mut state, "models.back");
        assert!(!state.fields.contains_key("overlay"));
        assert!(state.draft.is_empty());
        reduce(&mut state, CatalogIntent::Select { index: 2 }, None).unwrap();
        input(&mut state, "/");
        action(&mut state, "slash.next");
        assert_eq!(state.selected, 2);
        assert_eq!(
            state.fields.get("slash.selected").map(String::as_str),
            Some("1")
        );
        reduce(
            &mut state,
            CatalogIntent::Pick {
                field: "slash".into(),
                value: "/plugins".into(),
            },
            None,
        )
        .unwrap();
        assert_eq!(state.draft, "/plugins");
        let revision = state.revision;
        reduce(&mut state, CatalogIntent::Reset, None).unwrap();
        assert!(state.revision > revision);
        assert_eq!(state.selected, 0);
        assert!(state.events.is_empty());
    }
    #[test]
    fn all_registered_variants_render_with_bounded_names_and_pinned_sources() {
        let mut ids = std::collections::BTreeSet::new();
        for item in entries() {
            assert!(ids.insert(item.id.clone()), "duplicate {}", item.id);
            for variant in &item.variants {
                let state = FixtureState::default_for(&item.id, &variant.id);
                view(&item.id, &variant.id, &state)
                    .validate()
                    .unwrap_or_else(|error| panic!("{}/{}: {error}", item.id, variant.id));
                assert!(
                    item.sources
                        .iter()
                        .any(|source| source.branch.contains(&format!("fixture:{};", variant.id))),
                    "missing source branch {}/{}",
                    item.id,
                    variant.id
                );
            }
        }
    }
}
