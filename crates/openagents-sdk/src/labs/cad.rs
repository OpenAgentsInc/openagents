//! Local CAD previews use the same portable rules as the server. Measurements
//! must describe the proposed content; the server rebuilds it independently.
pub use ::cad::doc::{DEFAULT_MATERIAL, new_part};
pub use ::cad::lab::{Cad, Intent, changed_content, measurements};
use lab_kit::{Change, Draft, LabKind, Measurements, Refusal, Report, Version};
use serde_json::Value;

/// Checks CAD content against its measurements with the server's rules.
pub fn check(content: &Value, measurements: &Measurements) -> Result<Report, Refusal> {
    Cad::check(content, measurements)
}
/// Applies a change to a CAD version and returns the draft the server would accept.
pub fn apply(
    base: &Version,
    change: &Change,
    summary: &str,
    measurements: &Measurements,
) -> Result<Draft, Refusal> {
    Cad::apply(base, change, summary, measurements)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn previews_use_shared_checks_and_require_matching_rebuild_measurements() {
        let mut content = new_part("preview", "Preview", DEFAULT_MATERIAL, None);
        content["features"] =
            json!([{"id":"body","op":"solid.box","params":{"length":40,"width":40,"height":3}}]);
        let checked = measurements(&content, json!({"checks":{"rebuilt":true}}));
        assert_eq!(
            check(&content, &checked).unwrap(),
            Cad::check(&content, &checked).unwrap()
        );
        let base = Version {
            document: "preview".into(),
            number: 1,
            parent: None,
            author: "synthetic".into(),
            summary: "Created".into(),
            created_unix_ms: 1,
            schema: "part.v1".into(),
            content_sha256: lab_kit::digest(&content),
            content,
            report: Report::default(),
            assets: vec![],
        };
        let mut proposed = base.content.clone();
        proposed["features"][0]["params"]["length"] = json!(42);
        let change = Change::Intent {
            intent: serde_json::to_value(Intent::Replace {
                content: proposed.clone(),
            })
            .unwrap(),
        };
        assert_eq!(changed_content(&base.content, &change).unwrap(), proposed);
        assert_eq!(
            apply(&base, &change, "Longer", &checked).unwrap_err().code,
            "rebuild_required"
        );
        let rebuilt = measurements(&proposed, json!({"checks":{"rebuilt":true}}));
        assert_eq!(
            apply(&base, &change, "Longer", &rebuilt).unwrap(),
            Cad::apply(&base, &change, "Longer", &rebuilt).unwrap()
        );
        proposed["material"] = json!("Unsupported test material");
        assert_eq!(
            check(
                &proposed,
                &measurements(&proposed, json!({"checks":{"rebuilt":true}}))
            )
            .unwrap_err()
            .code,
            "cad_check"
        );
        assert_eq!(base.content["features"][0]["params"]["length"], 40);
    }
}
