//! Design Lab as a Labs platform kind: `board.v1` documents whose screens
//! are renamed or moved between areas only through checked changes. The
//! screenshots themselves come from git history, so no change edits an
//! image. The card is the newest version of each screen.

use lab_kit::{
    Artifact, Card, Change, Draft, JobSpec, JobState, KindManifest, LabKind, Limits, Measurements,
    Refusal, Report, ToolDecl, Version, View, Want,
    change::{Plain, apply_patch},
};
use serde_json::{Value, json};
use std::collections::BTreeSet;

use crate::{Board, SCHEMA};

pub struct Design;

fn board_of(content: &Value) -> Result<Board, Refusal> {
    serde_json::from_value(content.clone())
        .map_err(|_| Refusal::new("document_format", "Choose a board.v1 board."))
}

fn content_of(board: &Board) -> Result<Value, Refusal> {
    serde_json::to_value(board)
        .map_err(|_| Refusal::new("document_format", "Choose a board.v1 board."))
}

fn facts(board: &Board) -> Value {
    let areas: BTreeSet<&str> = board.screens.iter().map(|s| s.area.as_str()).collect();
    json!({
        "screens": board.screens.len(),
        "versions": board.versions(),
        "areas": areas.len(),
    })
}

fn tool(name: &str, description: &str, extra: Value) -> ToolDecl {
    let mut properties = json!({
        "board": {"type": "string", "description": "The board's document id."},
        "screen": {"type": "string", "description": "The screen's id."},
        "summary": {"type": "string", "description": "One line saying what changed."}
    });
    let mut required = vec![json!("board"), json!("screen")];
    for (key, schema) in extra.as_object().into_iter().flatten() {
        properties[key] = schema.clone();
        required.push(json!(key));
    }
    ToolDecl {
        name: name.into(),
        description: description.into(),
        parameters: json!({"type": "object", "properties": properties, "required": required}),
    }
}

impl LabKind for Design {
    const KIND: &'static str = "design";

    fn manifest() -> KindManifest {
        KindManifest {
            kind: Self::KIND.into(),
            name: "Design Lab".into(),
            schemas: vec![SCHEMA.into()],
            tool_prefix: "design_".into(),
            doc_arg: "board".into(),
            tools: vec![
                tool(
                    "design_rename",
                    "Give a screen on the board a new title.",
                    json!({"title": {"type": "string"}}),
                ),
                tool(
                    "design_move",
                    "Move a screen to another area of the board.",
                    json!({"area": {"type": "string"}}),
                ),
            ],
            job_types: Vec::new(),
            job_requirements: Default::default(),
            views: vec![View::Image, View::Report],
            limits: Limits {
                max_doc_bytes: 4 * 1024 * 1024,
                ..Limits::default()
            },
        }
    }

    fn check(content: &Value, _: &Measurements) -> Result<Report, Refusal> {
        let board = board_of(content)?;
        if board.schema != SCHEMA {
            return Err(Refusal::new("board_check", "Choose a board.v1 board."));
        }
        let mut ids = BTreeSet::new();
        for screen in &board.screens {
            if screen.id.is_empty() || screen.title.trim().is_empty() {
                return Err(Refusal::new(
                    "board_check",
                    "Every screen needs an id and a title.",
                ));
            }
            if !ids.insert(screen.id.as_str()) {
                return Err(Refusal::new(
                    "board_check",
                    format!("Two screens are both called {}.", screen.id),
                ));
            }
            if screen.versions.is_empty() {
                return Err(Refusal::new(
                    "board_check",
                    format!("The screen {} has no versions.", screen.id),
                ));
            }
            let blob =
                |b: &str| (40..=64).contains(&b.len()) && b.bytes().all(|c| c.is_ascii_hexdigit());
            if screen.versions.iter().any(|v| !blob(&v.blob)) {
                return Err(Refusal::new(
                    "board_check",
                    format!("A version of {} doesn't name its picture.", screen.id),
                ));
            }
        }
        Ok(Report {
            warnings: Vec::new(),
            facts: facts(&board),
        })
    }

    fn apply(
        base: &Version,
        change: &Change,
        summary: &str,
        m: &Measurements,
    ) -> Result<Draft, Refusal> {
        let content = match change {
            Change::Intent { intent } => {
                let mut board = board_of(&base.content)?;
                let id = intent["screen"].as_str().unwrap_or_default();
                let screen = board
                    .screens
                    .iter_mut()
                    .find(|s| s.id == id)
                    .ok_or_else(|| {
                        Refusal::new("unknown_screen", format!("This board has no screen {id}."))
                    })?;
                let text = |key: &str| {
                    intent[key]
                        .as_str()
                        .map(str::trim)
                        .filter(|t| !t.is_empty())
                        .map(str::to_string)
                        .ok_or_else(|| Refusal::new("invalid_edit", format!("Say the new {key}.")))
                };
                match intent["intent"].as_str() {
                    Some("rename") => screen.title = text("title")?,
                    Some("move") => screen.area = text("area")?,
                    _ => {
                        return Err(Refusal::new(
                            "unknown_intent",
                            "Change a board with rename or move.",
                        ));
                    }
                }
                content_of(&board)?
            }
            Change::Patch { ops } => apply_patch(&base.content, ops, &Plain)?,
        };
        let report = Self::check(&content, m)?;
        Ok(Draft {
            content,
            summary: summary.into(),
            report,
            assets: base.assets.clone(),
        })
    }

    fn jobs_for(_: &Version, _: &Want) -> Vec<JobSpec> {
        // Every picture is already in git; nothing is made.
        Vec::new()
    }

    fn card(version: &Version, _: &[JobState], _: &[Artifact]) -> Card {
        let board = board_of(&version.content).ok();
        let latest: Vec<String> = board
            .iter()
            .flat_map(|b| &b.screens)
            .filter_map(|s| s.versions.last().map(|v| v.blob.clone()))
            .collect();
        Card {
            schema: lab_kit::card::SCHEMA.into(),
            kind: Self::KIND.into(),
            document: version.document.clone(),
            version: version.number,
            title: version.content["name"]
                .as_str()
                .unwrap_or(&version.document)
                .into(),
            summary: version.summary.clone(),
            state: None,
            state_text: None,
            refusal: None,
            view: if latest.is_empty() {
                View::Report
            } else {
                View::Image
            },
            primary: latest.first().cloned(),
            poster: None,
            strip: latest.into_iter().take(12).collect(),
            compare: None,
            notes: Vec::new(),
            page: Some("/".into()),
            private_local: false,
            detail: version.report.facts.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Screen, ScreenVersion};

    fn first() -> Version {
        let board = Board {
            schema: SCHEMA.into(),
            name: "Phone".into(),
            screens: vec![Screen {
                id: "home".into(),
                title: "Home".into(),
                area: "mobile-polish".into(),
                versions: vec![ScreenVersion {
                    blob: "a".repeat(40),
                    path: "x/mobile-polish/home.png".into(),
                    commit: "c".repeat(40),
                    unix: 1,
                    subject: "Add home".into(),
                    width: 1179,
                    height: 2556,
                    app: true,
                }],
            }],
        };
        let content = serde_json::to_value(board).unwrap();
        Version {
            document: "mobile".into(),
            number: 1,
            parent: None,
            author: "test".into(),
            summary: "Scanned".into(),
            created_unix_ms: 0,
            schema: SCHEMA.into(),
            content_sha256: lab_kit::digest(&content),
            report: Design::check(&content, &Value::Null).unwrap(),
            content,
            assets: Vec::new(),
        }
    }

    fn intent(v: Value) -> Change {
        Change::Intent { intent: v }
    }

    #[test]
    fn renames_and_moves_are_checked_changes() {
        let base = first();
        let draft = Design::apply(
            &base,
            &intent(json!({"intent": "rename", "screen": "home", "title": "Home, glass"})),
            "Rename",
            &Value::Null,
        )
        .unwrap();
        assert_eq!(draft.content["screens"][0]["title"], "Home, glass");
        let draft = Design::apply(
            &base,
            &intent(json!({"intent": "move", "screen": "home", "area": "chat"})),
            "Move",
            &Value::Null,
        )
        .unwrap();
        assert_eq!(draft.content["screens"][0]["area"], "chat");
        assert_eq!(draft.report.facts["versions"], 1);

        let refuse = |v: Value| {
            Design::apply(&base, &intent(v), "x", &Value::Null)
                .unwrap_err()
                .code
        };
        assert_eq!(
            refuse(json!({"intent": "rename", "screen": "nope", "title": "x"})),
            "unknown_screen"
        );
        assert_eq!(
            refuse(json!({"intent": "rename", "screen": "home", "title": " "})),
            "invalid_edit"
        );
        assert_eq!(
            refuse(json!({"intent": "paint", "screen": "home"})),
            "unknown_intent"
        );
    }

    #[test]
    fn broken_boards_are_refused_and_the_card_shows_pictures() {
        let base = first();
        let mut twice = base.content.clone();
        let copy = twice["screens"][0].clone();
        twice["screens"].as_array_mut().unwrap().push(copy);
        assert_eq!(
            Design::check(&twice, &Value::Null).unwrap_err().code,
            "board_check"
        );
        let mut empty = base.content.clone();
        empty["screens"][0]["versions"] = json!([]);
        assert_eq!(
            Design::check(&empty, &Value::Null).unwrap_err().code,
            "board_check"
        );
        assert_eq!(
            Design::check(&json!({"name": 1}), &Value::Null)
                .unwrap_err()
                .code,
            "document_format"
        );

        let card = Design::card(&base, &[], &[]);
        assert_eq!(card.view, View::Image);
        assert_eq!(card.strip.len(), 1);
        assert!(Design::jobs_for(&base, &Want::Card).is_empty());
        assert!(
            Design::manifest()
                .tools
                .iter()
                .all(|t| t.name.starts_with("design_"))
        );
    }
}
