//! The model picker: one surface, per-lane item sources (issues #323/#324).
//!
//! Modeled on grok-build's `/model` (`xai-grok-pager/src/slash/commands/
//! model.rs`): a slash command whose argument list *is* the picker — rows of
//! display text, a match target, and a description — opened input-focused so
//! typing filters before arrows move. Our version runs in the composer
//! overlay family the command suggestions already use; the item builders here
//! are pure functions so a fixture test can hold them and the pty suite can
//! hold the surface.
//!
//! The commit is a **lane pin that already exists**: Pro commits
//! [`Lane::Named(id)`] (a direct pin the catalog checks at turn open), Local
//! commits [`Lane::Local(tag)`] (the tag pin). No new session state; the
//! picker only chooses between pins the CLI could always express.

use crate::runtime::{Lane, PRO_MODEL_IDS, ServedModel, is_pro_model_id};

/// One row of the picker, the shape a render surface consumes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelItem {
    /// The model id as the deployment names it. Also the match target.
    pub id: String,
    /// The line rendered: the id, plus `(current)` when it is the active one.
    pub display: String,
    /// The description line: what the row means on this lane.
    pub description: String,
    /// Whether this row is the session's active model.
    pub current: bool,
}

impl ModelItem {
    /// The text a filter matches against: the id, case-insensitively.
    pub fn matches(&self, query: &str) -> bool {
        query.is_empty() || self.id.to_lowercase().contains(&query.to_lowercase())
    }
}

/// The items for the Pro lane: the Pro door's ids, in [`PRO_MODEL_IDS`]
/// order, filtered to what the deployment actually serves *with credentials*.
///
/// A model the catalog lists but the account cannot use (no provider
/// credential configured) is not a choice; rendering it would be a picker
/// that promises what the turn will refuse. `resolved` is the id the
/// session's thread last opened on — the honest `(current)` marker, not the
/// lane the session was launched on.
pub fn pro_items(served: &[ServedModel], resolved: Option<&str>) -> Vec<ModelItem> {
    PRO_MODEL_IDS
        .iter()
        .filter_map(|id| {
            let entry = served
                .iter()
                .find(|model| &model.id == id && model.available)?;
            let current = resolved.is_some_and(|resolved| resolved == *id);
            Some(ModelItem {
                id: (*id).to_string(),
                display: if current {
                    format!("{id} (current)")
                } else {
                    (*id).to_string()
                },
                description: if entry.default {
                    "reasoning medium · deployment default".to_string()
                } else {
                    "reasoning medium".to_string()
                },
                current,
            })
        })
        .collect()
}

/// One model as Ollama reports it, reduced to what a picker row shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalModel {
    pub tag: String,
    pub size_bytes: Option<u64>,
    pub quantization: Option<String>,
}

/// The items for the Local lane: what Ollama has installed, most recently
/// modified first (the probe's own order — the same order the shift+tab
/// cycle gate reads).
///
/// `resolved` is the tag the session's lane is pinned to (`Lane::Local(tag)`),
/// or `None` on the bare local lane — which row is `(current)` then is the
/// probe's most-recent row, because that is what the lane would resolve to.
pub fn local_items(models: &[LocalModel], resolved: Option<&str>) -> Vec<ModelItem> {
    models
        .iter()
        .enumerate()
        .map(|(index, model)| {
            let current = match resolved {
                Some(tag) => tag == model.tag,
                // The bare local lane answers with the most recent model.
                None => index == 0,
            };
            let mut description = String::from("Coder Local");
            if let Some(size) = model.size_bytes {
                description.push_str(&format!(" · {}", format_size(size)));
            }
            if let Some(quant) = &model.quantization {
                description.push_str(&format!(" · {quant}"));
            }
            // The cycle gate (#292) still only walks Qwen 3.8. A pin on
            // any other installed tag is real; shift+tab just will not
            // offer Local unless that family is present (#327).
            let family = model.tag.split(':').next().unwrap_or(model.tag.as_str());
            if !family.eq_ignore_ascii_case("qwen3.8") {
                description.push_str(" · not on the shift+tab walk");
            }
            ModelItem {
                id: model.tag.clone(),
                display: if current {
                    format!("{} (current)", model.tag)
                } else {
                    model.tag.clone()
                },
                description,
                current,
            }
        })
        .collect()
}

/// Byte count in picker units — GB, one decimal, the way `ollama list` shows
/// it. Binary or decimal, a picker row needs a rough size, not precision.
fn format_size(bytes: u64) -> String {
    let gb = bytes as f64 / 1_000_000_000.0;
    format!("{gb:.1} GB")
}

/// The lane a committed row becomes, checked against what the surface can
/// offer. Refusals name the refusal; a picker that commits to a pin the
/// turn will reject has moved the failure past the point where the reader
/// could choose differently.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommitError {
    /// The id is not one the picker offered. The message names what was.
    UnknownModel { id: String },
    /// The lane has no per-model list (Flash and Free today).
    NoList { lane: String, id: String },
    /// No Ollama server answered, so there is nothing to list (the #291/#292
    /// gate's refusal, reused by the picker).
    NoLocalServer,
}

impl std::fmt::Display for CommitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CommitError::UnknownModel { id } => write!(
                f,
                "Unknown model: {id}. Pro offers {}.",
                PRO_MODEL_IDS.join(", ")
            ),
            CommitError::NoList { lane, id } => write!(
                f,
                "Unknown model: {id} (lane {lane} has no model list). \
                 /model works on Pro and Local."
            ),
            CommitError::NoLocalServer => {
                write!(
                    f,
                    "No Ollama server answered, so there is no local model \
                           list to pick from. Start one, or choose a hosted lane."
                )
            }
        }
    }
}

/// The lane a committed row becomes, on the lane the picker was opened from.
///
/// Pro commits a direct pin — the catalog checks it at turn open, and a pin
/// that outlives the catalog's memory of the id refuses by name rather than
/// silently substituting. Local commits the tag pin. A picker opened on any
/// other lane is a caller bug: only Pro and Local have per-model choice
/// today, and the surface says so rather than offering Flash's gateway tier
/// as if it were a choice.
pub fn commit_lane(lane: &Lane, id: &str) -> Result<Lane, CommitError> {
    match lane {
        Lane::Local(_) => Ok(Lane::Local(id.to_string())),
        lane if lane.uses_pro_origin() => {
            if is_pro_model_id(id) {
                Ok(Lane::Named(id.to_string()))
            } else {
                Err(CommitError::UnknownModel { id: id.to_string() })
            }
        }
        other => Err(CommitError::NoList {
            lane: other.label(),
            id: id.to_string(),
        }),
    }
}

/// The picker's filter applied: the rows a query leaves visible, order kept.
pub fn filtered<'a>(items: &'a [ModelItem], query: &str) -> Vec<&'a ModelItem> {
    items.iter().filter(|item| item.matches(query)).collect()
}

/// One open picker's state: the items, the filter, and the cursor.
///
/// Modeled on grok-build's `PickerState::input_active()` — the picker opens
/// already filtering, because a list of three to twenty model tags does not
/// need a navigation phase before the first keystroke does something.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PickerState {
    /// The rows offered, in source order.
    pub items: Vec<ModelItem>,
    /// The live filter text.
    pub query: String,
    /// The cursor into the *filtered* list.
    pub selected: usize,
    /// Whether the picker is still filling in (the Local probe is async).
    pub loading: bool,
    /// The loading line, when `loading` is true. Pro says it is loading
    /// models; Local names the Ollama probe so the wait is not a mystery.
    pub loading_label: String,
    /// True when this picker is the Local lane's. Empty and probe-miss
    /// states share the #326 install sign instead of the Pro refusal.
    pub local: bool,
}

impl PickerState {
    /// Open a picker over the given items, filtering from scratch.
    pub fn new(items: Vec<ModelItem>) -> Self {
        Self {
            items,
            query: String::new(),
            selected: 0,
            loading: false,
            loading_label: String::new(),
            local: false,
        }
    }

    /// Mark this picker as the Local lane's, so empty states share the
    /// #326 install sign.
    pub fn local(mut self) -> Self {
        self.local = true;
        self
    }

    /// An open picker whose items have not arrived yet.
    pub fn loading(label: impl Into<String>) -> Self {
        Self {
            loading: true,
            loading_label: label.into(),
            ..Self::default()
        }
    }

    /// The rows the current query leaves visible.
    pub fn visible(&self) -> Vec<&ModelItem> {
        filtered(&self.items, &self.query)
    }

    /// Type one character into the filter, keeping the cursor on a real row.
    pub fn push_char(&mut self, character: char) {
        self.query.push(character);
        self.clamp_selection();
    }

    /// Backspace one character out of the filter.
    pub fn pop_char(&mut self) {
        self.query.pop();
        self.clamp_selection();
    }

    /// Move the cursor, clamped at both ends. A picker that scrolls its
    /// cursor off the end of the filtered list is a picker lying about what
    /// Enter will commit.
    pub fn move_selection(&mut self, delta: isize) {
        let count = self.visible().len();
        if count == 0 {
            self.selected = 0;
            return;
        }
        let current = self.selected as isize + delta;
        self.selected = current.clamp(0, count as isize - 1) as usize;
    }

    /// The row Enter will commit, when the filtered list has one.
    pub fn selected_item(&self) -> Option<&ModelItem> {
        self.visible().get(self.selected).copied()
    }

    fn clamp_selection(&mut self) {
        let count = self.visible().len();
        if self.selected >= count {
            self.selected = count.saturating_sub(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn served(id: &str, available: bool) -> ServedModel {
        ServedModel {
            id: id.to_string(),
            available,
            default: false,
        }
    }

    #[test]
    fn pro_items_list_the_served_pro_models_in_table_order() {
        let served = vec![
            served("glm-5.3-flash", true),
            served("gpt-5.6-luna", true),
            served("gpt-5.6-sol", true),
            served("gpt-5.6-terra", false), // listed but not usable
        ];
        let items = pro_items(&served, None);
        // PRO_MODEL_IDS order (sol, terra, luna), not the wire's order — and
        // the unavailable terra is absent entirely.
        assert_eq!(
            items
                .iter()
                .map(|item| item.id.as_str())
                .collect::<Vec<_>>(),
            vec!["gpt-5.6-sol", "gpt-5.6-luna"]
        );
    }

    #[test]
    fn pro_items_mark_the_resolved_model_current() {
        let served = vec![served("gpt-5.6-sol", true), served("gpt-5.6-luna", true)];
        let items = pro_items(&served, Some("gpt-5.6-luna"));
        assert_eq!(items[0].display, "gpt-5.6-sol");
        assert_eq!(items[1].display, "gpt-5.6-luna (current)");
        assert_eq!(items[0].description, "reasoning medium");
        assert_eq!(items[1].description, "reasoning medium");
        assert!(items[1].current);
        assert!(!items[0].current);
    }

    #[test]
    fn local_items_keep_probe_order_and_mark_the_first_row_current_when_bare() {
        let models = vec![
            LocalModel {
                tag: "qwen3.8:27b-mtp-q8_0".to_string(),
                size_bytes: Some(30_000_000_000),
                quantization: Some("Q8_0".to_string()),
            },
            LocalModel {
                tag: "qwen3:8b".to_string(),
                size_bytes: Some(5_000_000_000),
                quantization: None,
            },
        ];
        let items = local_items(&models, None);
        assert_eq!(items[0].id, "qwen3.8:27b-mtp-q8_0");
        assert!(items[0].current, "the bare local lane resolves most-recent");
        assert!(!items[1].current);
        // The description carries what `ollama list` shows, picker-sized.
        assert_eq!(
            items[0].description, "Coder Local · 30.0 GB · Q8_0",
            "{}",
            items[0].description
        );
        assert_eq!(
            items[1].description,
            "Coder Local · 5.0 GB · not on the shift+tab walk"
        );
    }

    #[test]
    fn local_items_mark_the_pinned_tag_current_when_pinned() {
        let models = vec![
            LocalModel {
                tag: "qwen3.8:27b-mtp-q8_0".to_string(),
                size_bytes: None,
                quantization: None,
            },
            LocalModel {
                tag: "qwen3:8b".to_string(),
                size_bytes: None,
                quantization: None,
            },
        ];
        let items = local_items(&models, Some("qwen3:8b"));
        assert_eq!(items[1].display, "qwen3:8b (current)");
        assert_eq!(items[0].display, "qwen3.8:27b-mtp-q8_0");
    }

    #[test]
    fn commit_on_pro_pins_named_and_refuses_unknown_ids_by_name() {
        assert_eq!(
            commit_lane(&Lane::Pro, "gpt-5.6-terra"),
            Ok(Lane::Named("gpt-5.6-terra".to_string()))
        );
        // A pin already on the Pro door is still a Pro picker: committing
        // luna, then sol, must not refuse because the lane is now Named.
        assert_eq!(
            commit_lane(&Lane::Named("gpt-5.6-luna".to_string()), "gpt-5.6-sol"),
            Ok(Lane::Named("gpt-5.6-sol".to_string()))
        );
        let error = commit_lane(&Lane::Pro, "gpt-5.6-nope").unwrap_err();
        let text = error.to_string();
        assert!(text.contains("Unknown model: gpt-5.6-nope"), "{text}");
        assert!(text.contains("gpt-5.6-sol"), "{text}");
        assert!(text.contains("gpt-5.6-luna"), "{text}");
    }

    #[test]
    fn commit_on_local_pins_the_tag_and_other_lanes_refuse() {
        assert_eq!(
            commit_lane(&Lane::Local(String::new()), "qwen3:8b"),
            Ok(Lane::Local("qwen3:8b".to_string()))
        );
        let error = commit_lane(&Lane::Flash, "gpt-5.6-sol").unwrap_err();
        assert!(error.to_string().contains("no model list"));
    }

    #[test]
    fn filtering_is_case_insensitive_and_empty_query_keeps_all() {
        let served = vec![served("gpt-5.6-sol", true), served("gpt-5.6-luna", true)];
        let items = pro_items(&served, None);
        assert_eq!(filtered(&items, "").len(), 2);
        assert_eq!(filtered(&items, "LUNA")[0].id, "gpt-5.6-luna");
        assert_eq!(filtered(&items, "sol").len(), 1);
        assert_eq!(filtered(&items, "zorp").len(), 0);
    }

    #[test]
    fn picker_state_filters_moves_and_clamps() {
        let served = vec![served("gpt-5.6-sol", true), served("gpt-5.6-luna", true)];
        let mut picker = PickerState::new(pro_items(&served, None));
        assert_eq!(picker.visible().len(), 2);
        assert_eq!(
            picker.selected_item().map(|item| item.id.as_str()),
            Some("gpt-5.6-sol")
        );

        // Filtering to one row clamps the cursor onto it.
        picker.push_char('l');
        picker.push_char('u');
        picker.push_char('n');
        picker.push_char('a');
        assert_eq!(picker.visible().len(), 1);
        assert_eq!(picker.selected, 0);
        assert_eq!(
            picker.selected_item().map(|item| item.id.as_str()),
            Some("gpt-5.6-luna")
        );

        // Movement clamps at both ends — never off the list.
        picker.move_selection(5);
        assert_eq!(picker.selected, 0);
        picker.move_selection(-5);
        assert_eq!(picker.selected, 0);

        // Backspace widens the list again and the cursor stays legal.
        for _ in 0..4 {
            picker.pop_char();
        }
        assert_eq!(picker.visible().len(), 2);
        assert_eq!(picker.selected, 0);

        // An empty list after a filter selects nothing, and Enter has no row.
        picker.push_char('?');
        assert!(picker.selected_item().is_none());
    }

    #[test]
    fn loading_picker_shows_nothing_until_filled() {
        let mut picker = PickerState::loading("loading models…");
        assert!(picker.loading);
        assert_eq!(picker.loading_label, "loading models…");
        assert!(picker.visible().is_empty());
        assert!(picker.selected_item().is_none());
        picker.loading = false;
        picker.items = pro_items(&[served("gpt-5.6-sol", true)], None);
        assert_eq!(picker.visible().len(), 1);
    }
}
