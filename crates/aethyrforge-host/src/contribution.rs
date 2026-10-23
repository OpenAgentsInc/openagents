//! Saved contributions and separately recorded credits; saving never creates money.
use crate::{ProjectDocument, SavedRevision, WorldNamespace, project_hash};
use serde::{Deserialize, Serialize};

pub fn contribution_id(
    project: &str,
    namespace: WorldNamespace,
    branch: &str,
    revision: &SavedRevision,
) -> String {
    project_hash(
        &serde_json::to_vec(&(
            "openagents.verse.contribution.v1",
            project,
            namespace,
            branch,
            revision.version,
            &revision.document_sha256,
            &revision.actor,
        ))
        .expect("fixed contribution tuple"),
    )
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Contributor {
    pub name: String,
    pub profile: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CreditState {
    Pending,
    Confirmed,
    Paid,
    Reversed,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecordedCredit {
    pub receipt: String,
    pub source: String,
    pub amount: String,
    pub unit: String,
    pub state: CreditState,
    pub recorded_unix: u64,
    pub test_ledger: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum ContributionEarnings {
    NotRecorded,
    Lookup,
    Unavailable,
    Recorded { credits: Vec<RecordedCredit> },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Contribution {
    pub id: String,
    pub version: u64,
    pub actor: Contributor,
    pub created_unix: u64,
    pub summary: String,
    pub saved_summary: String,
    pub source_state: String,
    pub application_state: String,
    pub document_sha256: String,
    pub undo_of: Option<u64>,
    pub earnings: ContributionEarnings,
}
pub fn source_state(document: &ProjectDocument) -> &'static str {
    if document.source_draft != document.active_source {
        "draft"
    } else {
        "saved"
    }
}
#[derive(Default)]
pub struct VersionInspection {
    serial: u64,
    wanted: u64,
}
impl VersionInspection {
    pub fn request(&mut self, version: u64) -> u64 {
        self.serial = self.serial.wrapping_add(1);
        self.wanted = version;
        self.serial
    }
    pub fn accepts(&self, serial: u64, version: u64) -> bool {
        self.serial == serial && self.wanted == version
    }
}
pub struct ReviewChange {
    pub label: String,
    pub before: String,
    pub after: String,
}
fn words(value: &str) -> String {
    let value = value.rsplit([':', '/']).next().unwrap_or(value);
    if value.len() > 48 {
        return "Object".into();
    }
    let mut text = String::new();
    for c in value.chars() {
        if c == '_' || c == '-' {
            text.push(' ');
        } else {
            if c.is_uppercase() && !text.is_empty() {
                text.push(' ');
            }
            text.push(c);
        }
    }
    let mut chars = text.chars();
    chars.next().map_or_else(
        || "Object".into(),
        |c| c.to_uppercase().chain(chars).collect(),
    )
}
fn field(value: &str) -> String {
    match value {
        "position" | "center" => "Position".into(),
        "yaw" => "Rotation".into(),
        "scale" | "size" => "Size".into(),
        "floor_y" => "Floor height".into(),
        "red" => "Red".into(),
        "green" => "Green".into(),
        "blue" => "Blue".into(),
        "normal" => "Surface detail".into(),
        "fogDensity" => "Fog".into(),
        "sunIntensity" => "Sunlight".into(),
        "ambientIntensity" => "Fill light".into(),
        "exposure" => "Brightness".into(),
        "bloomStrength" => "Glow".into(),
        other => words(other),
    }
}
fn short(value: &serde_json::Value) -> String {
    let text = match value {
        serde_json::Value::Null => "None".into(),
        serde_json::Value::String(s) => words(s),
        serde_json::Value::Array(a) => a
            .iter()
            .take(3)
            .map(|v| {
                v.as_f64()
                    .map_or_else(|| "Changed".into(), |n| n.to_string())
            })
            .collect::<Vec<_>>()
            .join(", "),
        serde_json::Value::Object(_) => "Present".into(),
        other => other.to_string(),
    };
    text.chars().take(80).collect()
}
pub fn review_changes(document: &ProjectDocument, raw: &serde_json::Value) -> Vec<ReviewChange> {
    let mut rows = Vec::new();
    for change in raw.as_array().into_iter().flatten().take(48) {
        let Some(path) = change["path"].as_str() else {
            continue;
        };
        let parts = path.trim_start_matches('/').split('/').collect::<Vec<_>>();
        let value = |key: &str| {
            change[key]
                .as_str()
                .and_then(|s| serde_json::from_str::<serde_json::Value>(s).ok())
                .unwrap_or(serde_json::Value::Null)
        };
        let before = value("before");
        let after = value("after");
        let label = match parts.as_slice() {
            ["source_revision"] => continue,
            ["source_draft"] => "Draft source".into(),
            ["active_source"] => "World effect source".into(),
            ["entities", id, rest @ ..] => {
                let asset = document
                    .entities
                    .get(*id)
                    .map(|e| e.asset.as_str())
                    .or_else(|| before.get("asset").and_then(|a| a.as_str()));
                format!(
                    "{} · {}",
                    asset.map(words).unwrap_or_else(|| "Object".into()),
                    rest.first().map_or_else(|| "Object".into(), |f| field(f))
                )
            }
            ["materials", id, rest @ ..] => format!(
                "{} material · {}",
                words(id),
                rest.first().map_or_else(|| "Settings".into(), |f| field(f))
            ),
            ["environment", parameter] => format!("World · {}", field(parameter)),
            ["rooms", _, rest @ ..] => format!(
                "Room · {}",
                rest.first().map_or_else(|| "Room".into(), |f| field(f))
            ),
            ["layers"] => "Material layers".into(),
            ["originals"] => "Imported files".into(),
            ["removed_walls"] => "Wall openings".into(),
            [""] => "First saved version".into(),
            ["template", ..] => "World version".into(),
            ["schema"] => "Project format".into(),
            ["namespace"] => "World".into(),
            _ => "Saved content".into(),
        };
        let (old, new) = match parts.as_slice() {
            ["source_draft" | "active_source"] => {
                let lines = |v: &serde_json::Value| {
                    format!("{} lines", v.as_str().map_or(0, |s| s.lines().count()))
                };
                (lines(&before), lines(&after))
            }
            ["layers" | "originals" | "removed_walls"] => {
                let count = |v: &serde_json::Value| v.as_array().map_or(0, Vec::len).to_string();
                (count(&before), count(&after))
            }
            [""] => (
                "None".into(),
                format!(
                    "{} objects, {} rooms",
                    document.entities.len(),
                    document.rooms.len()
                ),
            ),
            ["template", ..] | ["schema"] => ("Earlier version".into(), "Selected version".into()),
            _ => (short(&before), short(&after)),
        };
        rows.push(ReviewChange {
            label,
            before: old,
            after: new,
        });
        if rows.len() == 12 {
            break;
        }
    }
    rows
}
/// Bounded accepted field changes shared by the history list and version review.
pub fn project_changes(
    before: Option<&ProjectDocument>,
    after: &ProjectDocument,
) -> serde_json::Value {
    use serde_json::{Value, json};
    fn diff(path: &str, before: &Value, after: &Value, changes: &mut Vec<Value>) {
        if before == after || changes.len() >= 48 {
            return;
        }
        if let (Some(a), Some(b)) = (before.as_object(), after.as_object()) {
            let keys: std::collections::BTreeSet<_> = a.keys().chain(b.keys()).collect();
            for k in keys {
                diff(
                    &format!("{path}/{k}"),
                    a.get(k).unwrap_or(&Value::Null),
                    b.get(k).unwrap_or(&Value::Null),
                    changes,
                );
            }
            return;
        }
        // Both documents are bounded to 48 KiB. Preserve source for accurate
        // line counts; the ordinary UI displays labels instead of raw fields.
        changes.push(json!({"path":path,"before":before.to_string(),"after":after.to_string()}));
    }
    let before = before
        .map(|d| serde_json::to_value(d).expect("project document"))
        .unwrap_or(Value::Null);
    let after = serde_json::to_value(after).expect("project document");
    let mut changes = Vec::new();
    diff("", &before, &after, &mut changes);
    json!(changes)
}
pub fn contribution_summary(document: &ProjectDocument, changes: &serde_json::Value) -> String {
    let rows = review_changes(document, changes);
    let mut labels = Vec::new();
    for row in rows {
        let label = if row.label == "First saved version" {
            format!("First save · {}", row.after)
        } else {
            row.label
        };
        if !labels.contains(&label) {
            labels.push(label);
        }
    }
    if labels.is_empty() {
        return "No content changes".into();
    }
    let extra = labels.len() > 3 || changes.as_array().is_some_and(|a| a.len() >= 48);
    let mut text = labels.into_iter().take(3).collect::<Vec<_>>().join(" + ");
    if text.chars().count() > 116 {
        text = text.chars().take(113).collect::<String>() + "…";
    }
    if extra {
        text.push_str(" + more");
    }
    text
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn accepted_row_summaries_distinguish_movement_from_source_only_drafts() {
        let original = crate::project::tests::document();
        let mut moved = original.clone();
        moved.entities.get_mut("prop-1").unwrap().position[0] += 1.;
        let movement = contribution_summary(&moved, &project_changes(Some(&original), &moved));
        assert_eq!(movement, "Pumpkin · Position");
        let mut draft = original.clone();
        draft.source_draft.push_str("\n// unfinished edit");
        assert_eq!(
            contribution_summary(&draft, &project_changes(Some(&original), &draft)),
            "Draft source"
        );
        moved.source_draft = draft.source_draft;
        assert_eq!(
            contribution_summary(&moved, &project_changes(Some(&original), &moved)),
            "Pumpkin · Position + Draft source"
        );
        assert_eq!(
            contribution_summary(&original, &project_changes(Some(&original), &original)),
            "No content changes"
        );
        assert_eq!(original, crate::project::tests::document());
    }
    #[test]
    fn readable_review_is_bounded_and_late_inspection_cannot_replace_the_latest_choice() {
        let d = crate::project::tests::document();
        let raw = serde_json::json!([
            {"path":"/entities/prop-1/position","before":"[0,0,-8]","after":"[1,0,-8]"},
            {"path":"/source_draft","before":"\"one line\"","after":serde_json::to_string(&"long shader line\n".repeat(100)).unwrap()},
            {"path":"/materials/fur/roughness","before":"0.8","after":"0.9"}
        ]);
        let review = review_changes(&d, &raw);
        assert_eq!(review[0].label, "Pumpkin · Position");
        assert_eq!(review[0].after, "1, 0, -8");
        assert_eq!(review[1].label, "Draft source");
        assert_eq!(review[1].after, "100 lines");
        assert_eq!(review[2].label, "Fur material · Roughness");
        assert!(
            review
                .iter()
                .all(|r| r.after.len() < 81 && !r.after.contains('{'))
        );
        let mut inspector = VersionInspection::default();
        let old = inspector.request(1);
        let latest = inspector.request(2);
        let mut shown = 0;
        if inspector.accepts(latest, 2) {
            shown = 2;
        }
        if inspector.accepts(old, 1) {
            shown = 1;
        }
        assert_eq!(shown, 2);
    }
    #[test]
    fn contribution_identity_binds_author_version_project_and_content_not_a_claimed_payment() {
        let mut h = crate::ProjectHistory::new(WorldNamespace::HauntedHouse);
        let mut d = crate::project::tests::document();
        d.source_draft = "failed shader draft".into();
        let r = h.save("alice", 1, "saved", 0, d, "Saved draft").unwrap();
        assert_eq!(source_state(&r.document), "draft");
        let key = contribution_id("project-a", WorldNamespace::HauntedHouse, "main", &r);
        assert_eq!(
            key,
            contribution_id("project-a", WorldNamespace::HauntedHouse, "main", &r)
        );
        assert_ne!(
            key,
            contribution_id("project-b", WorldNamespace::HauntedHouse, "main", &r)
        );
        let mut other = r.clone();
        other.actor = "bob".into();
        assert_ne!(
            key,
            contribution_id("project-a", WorldNamespace::HauntedHouse, "main", &other)
        );
        assert_eq!(
            serde_json::to_string(&ContributionEarnings::NotRecorded).unwrap(),
            r#"{"state":"not_recorded"}"#
        );
    }
}
