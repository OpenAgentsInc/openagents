//! Site administration (#12335), shared by GPUI hosts. Hosts must check
//! site-admin authority before mounting; the server checks every read/write.
use rust_native::Node;
use serde::{Deserialize, Serialize};

use super::{Intent, Load, Paints, button, column, danger_button, header, key, row, section, text};

#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
pub struct Report {
    #[serde(default)]
    pub description: String,
    pub sections: Vec<Section>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
pub struct Section {
    pub title: String,
    pub rows: Vec<Row>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
pub struct Row {
    pub label: String,
    pub detail: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct State {
    pub report: Load<Report>,
    pub busy: bool,
    pub confirm_remove: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    Refresh,
    Open(String),
    Add,
    ConfirmRemove(Option<String>),
    Remove(String),
}

fn act(action: Action) -> Intent {
    Intent::Admin(action)
}

/// Adapt the existing Drive-access API without changing its contract.
pub fn drive_report(answer: &serde_json::Value) -> Report {
    Report {
        description: "Until Google approves the app, only the people here and site admins see Connections and Drive sources. Add each person to Google's test users too.".into(),
        sections: vec![Section {
            title: "Allowed".into(),
            rows: answer["data"].as_array().into_iter().flatten().filter_map(|entry| {
                Some(Row {
                    label: entry["email"].as_str().or(entry["account"].as_str())?.into(),
                    detail: entry["added_at"].as_str().unwrap_or("").into(),
                })
            }).collect(),
        }],
    }
}

pub fn view(title: &str, drive: bool, state: &State, paints: &Paints) -> Node<Intent> {
    let mut out = vec![header(
        "admin-title",
        title,
        state.report.ready().map(|r| r.description.as_str()),
        paints,
    )];
    out.push(section(
        "admin-pages",
        "Admin",
        [
            ("Analytics", "admin-analytics"),
            ("Links", "admin-attribution"),
            ("Google Drive access", "admin-drive-access"),
        ]
        .into_iter()
        .map(|(label, page)| button(page, label, act(Action::Open(page.into()))))
        .collect(),
        paints,
    ));
    match &state.report {
        Load::Ready(report) => {
            for (index, group) in report.sections.iter().enumerate() {
                let rows = if group.rows.is_empty() {
                    vec![text(
                        &format!("admin-empty-{index}"),
                        "Nothing yet.",
                        paints.muted,
                    )]
                } else {
                    group
                        .rows
                        .iter()
                        .enumerate()
                        .map(|(i, item)| {
                            let id = format!("admin-{index}-{i}");
                            let mut controls = Vec::new();
                            if drive && !state.busy {
                                if state.confirm_remove.as_deref() == Some(&item.label) {
                                    controls.push(danger_button(
                                        &key(&id, "confirm"),
                                        "Remove access",
                                        act(Action::Remove(item.label.clone())),
                                        paints,
                                    ));
                                    controls.push(button(
                                        &key(&id, "cancel"),
                                        "Cancel",
                                        act(Action::ConfirmRemove(None)),
                                    ));
                                } else {
                                    controls.push(button(
                                        &key(&id, "remove"),
                                        "Remove",
                                        act(Action::ConfirmRemove(Some(item.label.clone()))),
                                    ));
                                }
                            }
                            row(&id, &item.label, Some(&item.detail), controls, paints)
                        })
                        .collect()
                };
                out.push(section(
                    &format!("admin-section-{index}"),
                    &group.title,
                    rows,
                    paints,
                ));
            }
        }
        Load::Failed(error) => out.push(text("admin-error", error, paints.muted)),
        _ => out.push(text("admin-loading", "Loading…", paints.muted)),
    }
    if !state.busy {
        out.push(button("admin-refresh", "Refresh", act(Action::Refresh)));
        if drive && state.report.ready().is_some() {
            out.push(button("admin-add", "Add person", act(Action::Add)));
            out.push(button(
                "admin-google",
                "Google's test users",
                act(Action::Open(
                    "https://console.cloud.google.com/auth/audience?project=openagentsgemini"
                        .into(),
                )),
            ));
        }
    }
    column("admin", out)
}
