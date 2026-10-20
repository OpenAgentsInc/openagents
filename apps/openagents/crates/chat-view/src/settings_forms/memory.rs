//! Account memory, shared by GPUI shells. Uses the account's existing sync records.
use super::*;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Note {
    pub id: String,
    pub scope: String,
    pub project_name: Option<String>,
    pub kind: Option<String>,
    pub name: Option<String>,
    pub description: Option<String>,
    pub body: Option<String>,
    pub deleted: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Draft {
    #[serde(skip)]
    pub id: Option<String>,
    pub kind: String,
    pub name: String,
    pub description: String,
    pub body: String,
}

impl Default for Draft {
    fn default() -> Self {
        Self {
            id: None,
            kind: "user".into(),
            name: String::new(),
            description: String::new(),
            body: String::new(),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct State {
    pub notes: Load<Vec<Note>>,
    pub draft: Option<Draft>,
    pub busy: bool,
    pub page: usize,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Action {
    Reload,
    Add,
    Edit(String),
    Delete(String),
    Kind(String),
    Field(String),
    Save,
    Cancel,
    Page(usize),
}

impl State {
    pub fn edit(&mut self, id: Option<&str>) {
        self.draft = match id {
            None => Some(Draft::default()),
            Some(id) => self
                .notes
                .ready()
                .and_then(|notes| notes.iter().find(|n| n.id == id && !n.deleted))
                .map(|n| Draft {
                    id: Some(n.id.clone()),
                    kind: n.kind.clone().unwrap_or_else(|| "user".into()),
                    name: n.name.clone().unwrap_or_default(),
                    description: n.description.clone().unwrap_or_default(),
                    body: n.body.clone().unwrap_or_default(),
                }),
        };
    }

    pub fn field(&mut self, key: &str, value: String) {
        if let Some(draft) = &mut self.draft {
            match key {
                "memory-name" => draft.name = value,
                "memory-description" => draft.description = value,
                "memory-body" => draft.body = value,
                _ => {}
            }
        }
    }
}

/// An editable field's definition stays in the shared view; the shell supplies
/// its persistent native text control through the GPUI adapter's field hook.
pub(super) fn field(
    key: &str,
    label: &str,
    value: &str,
    multiline: bool,
    max_bytes: usize,
    enabled: bool,
    intent: Intent,
) -> Node<Intent> {
    node(
        key,
        Style::default(),
        Element::Field {
            label: label.into(),
            value: value.into(),
            placeholder: String::new(),
            secret: false,
            multiline,
            enabled,
            max_bytes,
            on_change: intent,
        },
    )
}

pub fn page(state: &State, paints: &Paints) -> Vec<Node<Intent>> {
    let action = |a| Intent::Memory(a);
    let mut out = vec![header(
        "memory-title",
        "Memory",
        Some("What we remember about you and your projects."),
        paints,
    )];
    if state.draft.is_none() && matches!(state.notes, Load::Ready(_)) {
        out.push(enabled_button(
            "memory-add",
            "Add memory",
            !state.busy,
            action(Action::Add),
        ));
    }
    match &state.notes {
        Load::Idle | Load::Loading => {
            out.push(text("memory-loading", "Loading memory…", paints.muted))
        }
        Load::Failed(message) => {
            out.push(text("memory-error", message, paints.danger));
            out.push(button("memory-retry", "Try again", action(Action::Reload)));
        }
        Load::Ready(notes) => {
            let live: Vec<_> = notes.iter().filter(|n| !n.deleted).collect();
            if live.is_empty() {
                out.push(text("memory-empty", "No memories yet.", paints.muted));
            }
            let start = state.page.min(live.len().saturating_sub(1) / 20) * 20;
            for note in live.iter().skip(start).take(20) {
                let key = format!("memory-{}", note.id);
                let detail = format!(
                    "{}{}",
                    note.project_name
                        .as_deref()
                        .map(|p| format!("{p} · "))
                        .unwrap_or_default(),
                    note.description.as_deref().unwrap_or_default()
                );
                out.push(row(
                    &key,
                    note.name.as_deref().unwrap_or("Memory"),
                    Some(&detail),
                    vec![
                        enabled_button(
                            &format!("{key}-edit"),
                            "Edit",
                            !state.busy,
                            action(Action::Edit(note.id.clone())),
                        ),
                        enabled_button(
                            &format!("{key}-delete"),
                            "Delete",
                            !state.busy,
                            action(Action::Delete(note.id.clone())),
                        ),
                    ],
                    paints,
                ));
            }
            paging(&mut out, "memory", start, live.len(), |p| {
                action(Action::Page(p))
            });
        }
    }
    if let Some(draft) = &state.draft {
        let kinds = [
            ("About you", "user"),
            ("How you like things done", "feedback"),
            ("About a project", "project"),
            ("Where to look", "reference"),
        ];
        let mut fields = vec![header(
            "memory-edit-title",
            if draft.id.is_some() {
                "Edit memory"
            } else {
                "Add memory"
            },
            None,
            paints,
        )];
        for (key, label, value, multi, max) in [
            ("memory-name", "Name", &draft.name, false, 320),
            (
                "memory-description",
                "Summary (optional)",
                &draft.description,
                false,
                1200,
            ),
            ("memory-body", "What to remember", &draft.body, true, 8192),
        ] {
            fields.push(field(
                key,
                label,
                value,
                multi,
                max,
                !state.busy,
                action(Action::Field(key.into())),
            ));
        }
        fields.push(wide_row(
            "memory-kind",
            "Kind",
            None,
            kinds
                .iter()
                .map(|(label, kind)| {
                    enabled_button(
                        &format!("memory-kind-{kind}"),
                        &format!("{}{}", if draft.kind == *kind { "✓ " } else { "" }, label),
                        !state.busy,
                        action(Action::Kind((*kind).into())),
                    )
                })
                .collect(),
            paints,
        ));
        fields.push(enabled_button(
            "memory-save",
            if state.busy {
                "Saving…"
            } else {
                "Save memory"
            },
            !state.busy && !draft.name.trim().is_empty() && !draft.body.trim().is_empty(),
            action(Action::Save),
        ));
        fields.push(enabled_button(
            "memory-cancel",
            "Cancel",
            !state.busy,
            action(Action::Cancel),
        ));
        out.push(card("memory-form", fields, paints));
    }
    out
}

pub(super) fn paging(
    out: &mut Vec<Node<Intent>>,
    key: &str,
    start: usize,
    count: usize,
    intent: impl Fn(usize) -> Intent,
) {
    if start > 0 {
        out.push(button(
            &format!("{key}-previous"),
            "Previous",
            intent(start / 20 - 1),
        ));
    }
    if start + 20 < count {
        out.push(button(
            &format!("{key}-next"),
            "Next",
            intent(start / 20 + 1),
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_native::View;

    #[test]
    fn edits_keep_identity_and_failed_loads_do_not_discard_drafts() {
        let mut state = State {
            notes: Load::Ready(vec![Note {
                id: "mem-one".into(),
                scope: "project".into(),
                name: Some("Build".into()),
                body: Some("Use cargo.".into()),
                kind: Some("reference".into()),
                ..Note::default()
            }]),
            ..State::default()
        };
        state.edit(Some("mem-one"));
        state.field("memory-body", "Use the pinned compiler.".into());
        state.notes = Load::Failed("Try again.".into());
        let draft = state.draft.as_ref().unwrap();
        assert_eq!(draft.id.as_deref(), Some("mem-one"));
        let body = serde_json::to_value(draft).unwrap();
        assert_eq!(body["body"], "Use the pinned compiler.");
        assert_eq!(body["kind"], "reference");
        assert!(body.get("id").is_none());
        View::new_v3(
            "settings",
            1,
            column("memory", page(&state, &Paints::default())),
        )
        .validate()
        .unwrap();
    }

    #[test]
    fn a_full_account_stays_bounded_and_deletions_are_hidden() {
        let notes = (0..1000)
            .map(|n| Note {
                id: format!("mem-{n}"),
                name: Some("A memory".into()),
                body: Some("x".repeat(8192)),
                deleted: n == 0,
                ..Note::default()
            })
            .collect();
        let mut state = State {
            notes: Load::Ready(notes),
            page: usize::MAX,
            ..State::default()
        };
        state.edit(None);
        let root = column("memory", page(&state, &Paints::default()));
        View::new_v3("settings", 1, root).validate().unwrap();
        state.edit(Some("mem-0"));
        assert!(state.draft.is_none());
    }
}
