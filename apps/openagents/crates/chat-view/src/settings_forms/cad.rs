//! Shared CAD document and configurator views (#12334).
use super::{Intent, Load, Paints, button, card, column, text};
use rust_native::Node;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct State {
    pub document: String,
    pub project: String,
    pub configurator: bool,
    pub world: bool,
    pub files_ready: bool,
    pub data: Load<Value>,
    pub controls: Vec<Value>,
    pub face: Option<(String, String)>,
    pub busy: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    List,
    Open(String, u64),
    Configurator,
    MoveFace,
    Toggle(usize),
    Edit(usize, String),
    Publish,
    Revoke(String),
    Visit(String),
    Reload,
    Download(String),
}

fn action(key: &str, label: impl Into<String>, intent: Action) -> Node<Intent> {
    button(key, &label.into(), Intent::Cad(intent))
}
fn line(key: &str, value: impl Into<String>, paints: &Paints) -> Node<Intent> {
    text(key, &value.into(), paints.muted)
}

pub fn view(state: &State, paints: &Paints) -> Node<Intent> {
    let mut rows = vec![];
    if !state.document.is_empty() {
        rows.push(action("cad-list", "Your designs", Action::List));
    }
    match &state.data {
        Load::Ready(data) if state.world => {
            rows.push(action(
                "cad-world-open",
                "Open your world",
                Action::Visit("/everglade".into()),
            ));
            if let Some(entries) = data.as_array() {
                if entries.is_empty() {
                    rows.push(line(
                        "cad-world-empty",
                        "Your placed designs appear here.",
                        paints,
                    ));
                }
                for (i, item) in entries.iter().enumerate() {
                    let doc = item["doc"].as_str().unwrap_or("Design");
                    rows.push(line(
                        &format!("cad-placement-{i}"),
                        format!(
                            "{doc} · version {} · {}",
                            item["version"],
                            item["zone"].as_str().unwrap_or("Your world")
                        ),
                        paints,
                    ));
                    if let Some(glb) = item["glb"].as_str() {
                        rows.push(action(
                            &format!("cad-placement-file-{i}"),
                            "Download GLB",
                            Action::Visit(glb.into()),
                        ));
                    }
                }
            }
        }
        Load::Ready(data) if state.document.is_empty() => {
            rows.push(action(
                "cad-world",
                "Designs in your world",
                Action::Visit("/cad-world?ui=gpui".into()),
            ));
            if let Some(docs) = data["docs"].as_object() {
                if docs.is_empty() {
                    rows.push(line(
                        "cad-empty",
                        "Your designs appear here when you make one in a chat.",
                        paints,
                    ));
                }
                for (id, doc) in docs {
                    rows.push(action(
                        &format!("cad-doc-{id}"),
                        doc["title"].as_str().unwrap_or(id),
                        Action::Open(id.clone(), 0),
                    ));
                    rows.push(line(
                        &format!("cad-versions-{id}"),
                        format!("{} versions", doc["versions"]),
                        paints,
                    ));
                }
            }
        }
        Load::Ready(data) if state.configurator => {
            rows.push(action(
                "cad-document",
                "Back to design",
                Action::Open(
                    state.document.clone(),
                    data["version"].as_u64().unwrap_or(0),
                ),
            ));
            if data["pro"] == true && !state.busy {
                rows.push(line(
                    "cad-sliders-help",
                    "Choose up to eight sliders and their limits.",
                    paints,
                ));
                if let Some(options) = data["options"].as_array() {
                    for (i, option) in options.iter().enumerate() {
                        let chosen = state
                            .controls
                            .iter()
                            .position(|c| c["path"] == option["path"]);
                        let label = option["label"].as_str().unwrap_or("Dimension");
                        rows.push(action(
                            &format!("cad-option-{i}"),
                            format!(
                                "{} {label}",
                                if chosen.is_some() { "Remove" } else { "Add" }
                            ),
                            Action::Toggle(i),
                        ));
                        if let Some(index) = chosen {
                            for (field, label) in [
                                ("label", "Name"),
                                ("min", "Minimum"),
                                ("max", "Maximum"),
                                ("step", "Step"),
                            ] {
                                let value = &state.controls[index][field];
                                let value = value
                                    .as_str()
                                    .map(str::to_owned)
                                    .unwrap_or_else(|| value.to_string());
                                rows.push(action(
                                    &format!("cad-control-{i}-{field}"),
                                    format!("{label}: {value}"),
                                    Action::Edit(index, field.into()),
                                ));
                            }
                        }
                    }
                }
                if !state.controls.is_empty() {
                    rows.push(action("cad-publish", "Create link", Action::Publish));
                }
            } else if data["pro"] != true {
                rows.push(line(
                    "cad-pro",
                    "Sharing designs with sliders is part of Pro.",
                    paints,
                ));
            }
            if let Some(links) = data["links"].as_array() {
                for (i, token) in links.iter().filter_map(Value::as_str).enumerate() {
                    rows.push(action(
                        &format!("cad-link-{i}"),
                        "Open shared design",
                        Action::Visit(format!("/cad-config/{token}")),
                    ));
                    if !state.busy {
                        rows.push(action(
                            &format!("cad-revoke-{i}"),
                            "Stop sharing",
                            Action::Revoke(token.into()),
                        ));
                    }
                }
            }
        }
        Load::Ready(data) => {
            rows.push(line(
                "cad-version",
                format!("Version {}", data["version"]),
                paints,
            ));
            if let Some(summary) = data["record"]["summary"].as_str() {
                rows.push(line("cad-summary", summary, paints));
            }
            if let Some(kg) = data["record"]["report"]["mass"]["mass_kg"].as_f64() {
                rows.push(line("cad-mass", format!("{:.1} g", kg * 1000.), paints));
            }
            if data["edits"] == true && !state.busy {
                if let Some((part, face)) = &state.face {
                    rows.push(line("cad-face", format!("Selected {part}: {face}"), paints));
                    rows.push(action("cad-move", "Move face and save", Action::MoveFace));
                } else {
                    rows.push(line(
                        "cad-pick",
                        "Select a face in the viewer to change its dimensions.",
                        paints,
                    ));
                }
            }
            if let Some(glb) = data["glb"].as_str() {
                rows.push(action("cad-glb", "Download GLB", Action::Visit(glb.into())));
            }
            if state.files_ready {
                for (format, label) in [
                    ("step", "STEP"),
                    ("stl", "STL"),
                    ("3mf", "3MF"),
                    ("pdf", "Drawing (PDF)"),
                    ("dxf", "Drawing (DXF)"),
                ] {
                    if data["record"]["report"]["mesh_bodies"]
                        .as_u64()
                        .unwrap_or(0)
                        == 0
                        || matches!(format, "stl" | "3mf")
                    {
                        rows.push(action(
                            &format!("cad-download-{format}"),
                            label,
                            Action::Download(format.into()),
                        ));
                    }
                }
            }
            if data["share"] == true {
                rows.push(action(
                    "cad-configurator",
                    "Share with sliders",
                    Action::Configurator,
                ));
            }
            if let Some(history) = data["history"].as_array() {
                for version in history.iter().rev() {
                    let n = version["n"].as_u64().unwrap_or(0);
                    rows.push(action(
                        &format!("cad-v-{n}"),
                        format!(
                            "v{n} · {}",
                            version["summary"].as_str().unwrap_or("Saved design")
                        ),
                        Action::Open(state.document.clone(), n),
                    ));
                }
            }
        }
        Load::Failed(error) => rows.push(line("cad-error", error, paints)),
        _ => rows.push(line("cad-loading", "Loading…", paints)),
    }
    if state.busy {
        rows.push(line("cad-busy", "Saving…", paints));
    } else {
        rows.push(action("cad-reload", "Refresh", Action::Reload));
    }
    column("cad", vec![card("cad-card", rows, paints)])
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn labels(node: &Node<Intent>, out: &mut Vec<String>) {
        use rust_native::Element;
        match &node.element {
            Element::Button { label, .. } => out.push(label.clone()),
            Element::Stack { children, .. } => {
                for c in children {
                    labels(c, out)
                }
            }
            _ => {}
        }
    }
    fn buttons(state: &State) -> Vec<String> {
        let mut out = vec![];
        labels(&view(state, &Paints::default()), &mut out);
        out
    }
    #[test]
    fn edit_and_share_follow_the_servers_permissions() {
        let mut state = State {
            document: "plate".into(),
            data: Load::Ready(json!({"edits":false,"share":false})),
            face: Some(("plate".into(), "body:+z".into())),
            ..State::default()
        };
        assert!(
            !buttons(&state)
                .iter()
                .any(|l| l.contains("Move face") || l.contains("Share"))
        );
        state.data = Load::Ready(json!({"edits":true,"share":true}));
        assert!(buttons(&state).contains(&"Move face and save".into()));
        state.busy = true;
        assert!(!buttons(&state).contains(&"Move face and save".into()));
    }
    #[test]
    fn public_links_and_hidden_shop_stay_out_of_the_editor() {
        let state = State {
            document: "plate".into(),
            configurator: true,
            data: Load::Ready(
                json!({"pro":true,"options":[{"path":"/thickness","label":"Thickness"}],"links":["abc"]}),
            ),
            ..State::default()
        };
        let labels = buttons(&state);
        assert!(labels.contains(&"Add Thickness".into()));
        assert!(labels.contains(&"Open shared design".into()));
        assert!(!labels.contains(&"Create link".into()));
        assert!(
            !labels
                .iter()
                .any(|l| l.contains("Sell") || l.contains("sale"))
        );
        for label in labels {
            assert!(oa_copy::violations(&label, &[]).is_empty(), "{label}");
        }
    }
}
