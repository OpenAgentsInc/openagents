//! Ship Lab as a Labs platform kind (#11881): `spacecraft.v1` documents
//! whose every change passes [`Spacecraft::edit`]'s checks, with the same
//! refusal sentences as the local service's `ship_edit`.
//!
//! The checks are pure arithmetic on the document, so `check` and `apply`
//! need no measurements. The card is a report of the ship's numbers, or a
//! 3D model when a version has a glTF artifact. The Lagrange 2 zone's file
//! is the `zone_file` job's artifact.

use lab_kit::{
    Artifact, Card, Change, Draft, JobSpec, JobState, KindManifest, LabKind, Limits, Measurements,
    Refusal, Report, Requirements, ToolDecl, Version, View, Want,
    change::{Plain, apply_patch},
};
use serde_json::{Value, json};
use verse_lagrange2::ship::{SCHEMA, Spacecraft};

/// The job type that writes the zone's ship file for a version.
pub const ZONE_FILE: &str = "zone_file";

pub struct Ship;

fn requirements() -> Requirements {
    Requirements {
        tools: vec!["ship-lab".into()],
        max_seconds: 30,
        ..Requirements::default()
    }
}

fn ship_of(content: &Value) -> Result<Spacecraft, Refusal> {
    serde_json::from_value(content.clone())
        .map_err(|_| Refusal::new("document_format", "Choose a spacecraft.v1 ship."))
}

fn content_of(ship: &Spacecraft) -> Result<Value, Refusal> {
    serde_json::to_value(ship)
        .map_err(|_| Refusal::new("document_format", "Choose a spacecraft.v1 ship."))
}

fn facts(ship: &Spacecraft) -> Value {
    json!({"numbers": super::numbers(ship), "parts": super::parts(ship)})
}

impl LabKind for Ship {
    const KIND: &'static str = "ship";

    fn manifest() -> KindManifest {
        let tools = super::tools()["tools"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|t| ToolDecl {
                name: t["name"].as_str().unwrap_or_default().into(),
                description: t["description"].as_str().unwrap_or_default().into(),
                parameters: t["parameters"].clone(),
            })
            .collect();
        KindManifest {
            kind: Self::KIND.into(),
            name: "Ship Lab".into(),
            schemas: vec![SCHEMA.into()],
            tool_prefix: "ship_".into(),
            doc_arg: "doc".into(),
            tools,
            job_types: vec![ZONE_FILE.into()],
            job_requirements: [(ZONE_FILE.into(), requirements())].into(),
            views: vec![View::Report, View::Model3d],
            limits: Limits::default(),
        }
    }

    fn check(content: &Value, _: &Measurements) -> Result<Report, Refusal> {
        let ship = ship_of(content)?;
        if let Some(finding) = ship.check().into_iter().next() {
            return Err(Refusal::new(
                "ship_check",
                format!(
                    "Refused by the {} check: {}.",
                    finding.requirement, finding.detail
                ),
            ));
        }
        Ok(Report {
            warnings: Vec::new(),
            facts: facts(&ship),
        })
    }

    fn apply(
        base: &Version,
        change: &Change,
        summary: &str,
        m: &Measurements,
    ) -> Result<Draft, Refusal> {
        let ship = ship_of(&base.content)?;
        let (content, warnings) = match change {
            Change::Intent { intent } if intent["intent"] == "edit" => {
                let edit =
                    super::parse_edit(intent).map_err(|why| Refusal::new("invalid_edit", why))?;
                let next = ship.edit(&edit).map_err(|finding| {
                    Refusal::new("ship_check", super::refused(&finding, base.number))
                })?;
                (content_of(&next)?, super::left_cad(&ship, &next))
            }
            Change::Intent { .. } => {
                return Err(Refusal::new(
                    "unknown_intent",
                    "Change a ship with an edit: scale, set_dry_mass, set_propellant, move or remove.",
                ));
            }
            Change::Patch { ops } => (apply_patch(&base.content, ops, &Plain)?, Vec::new()),
        };
        let mut report = Self::check(&content, m)?;
        report.warnings = warnings;
        if let Some(facts) = report.facts.as_object_mut() {
            facts.insert("before".into(), super::numbers(&ship));
        }
        Ok(Draft {
            content,
            summary: summary.into(),
            report,
            assets: base.assets.clone(),
        })
    }

    fn jobs_for(version: &Version, want: &Want) -> Vec<JobSpec> {
        match want {
            Want::Job { job_type, params } if job_type == ZONE_FILE => vec![JobSpec {
                kind: Self::KIND.into(),
                job_type: ZONE_FILE.into(),
                version: version.reference(),
                params: params.clone(),
                requirements: Requirements {
                    private_local: version.has_private_local(),
                    ..requirements()
                },
            }],
            // The report needs nothing made.
            Want::Card | Want::Job { .. } => Vec::new(),
        }
    }

    fn card(version: &Version, jobs: &[JobState], artifacts: &[Artifact]) -> Card {
        let model = artifacts
            .iter()
            .find(|a| a.version == version.reference() && a.media_type == "model/gltf-binary");
        let job = jobs
            .iter()
            .rev()
            .find(|j| j.spec.version == version.reference());
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
            state: job.map(|j| j.status),
            state_text: None,
            refusal: None,
            view: if model.is_some() {
                View::Model3d
            } else {
                View::Report
            },
            primary: model.map(|a| a.id.clone()),
            poster: None,
            strip: Vec::new(),
            compare: None,
            notes: Vec::new(),
            page: None,
            private_local: version.has_private_local(),
            detail: version.report.facts.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Lab, SAMPLE};

    fn first() -> Version {
        let content = serde_json::to_value(Spacecraft::sample()).unwrap();
        Version {
            document: SAMPLE.into(),
            number: 1,
            parent: None,
            author: "test".into(),
            summary: "The sample tug".into(),
            created_unix_ms: 0,
            schema: SCHEMA.into(),
            content_sha256: lab_kit::digest(&content),
            report: Ship::check(&content, &Value::Null).unwrap(),
            content,
            assets: Vec::new(),
        }
    }

    fn next(base: &Version, draft: Draft) -> Version {
        Version {
            number: base.number + 1,
            parent: Some(base.number),
            summary: draft.summary,
            content_sha256: lab_kit::digest(&draft.content),
            content: draft.content,
            report: draft.report,
            ..base.clone()
        }
    }

    fn edit(args: Value) -> Change {
        let mut intent = args;
        intent["intent"] = json!("edit");
        Change::Intent { intent }
    }

    #[test]
    fn tool_edits_make_the_same_versions_and_refusals_as_the_local_lab() {
        let dir = tempfile::tempdir().unwrap();
        let lab = Lab::open(dir.path().to_path_buf()).unwrap();
        let mut platform = first();
        let (ship, _) = lab.load(SAMPLE, 1).unwrap();
        assert_eq!(serde_json::to_value(&ship).unwrap(), platform.content);

        let steps = [
            json!({"doc": "tug", "edit": "scale", "parts": ["tank-port", "tank-starboard"], "factor": 1.05, "summary": "Both tanks 5% bigger"}),
            json!({"doc": "tug", "edit": "set_dry_mass", "parts": ["habitat"], "kg": 60000.0, "summary": "Heavy"}),
            json!({"doc": "tug", "edit": "scale", "parts": ["nope"], "factor": 2.0}),
            json!({"doc": "tug", "edit": "paint", "parts": ["engine"]}),
            json!({"doc": "tug", "edit": "scale", "factor": 2.0}),
            json!({"doc": "tug", "edit": "move", "parts": ["engine"], "position": [1.0]}),
        ];
        for args in steps {
            let local = lab.run("ship_edit", &args);
            let summary = args["summary"].as_str().unwrap_or("An edit");
            match Ship::apply(&platform, &edit(args.clone()), summary, &Value::Null) {
                Ok(draft) => {
                    assert_eq!(local["accepted"], true, "{local}");
                    assert_eq!(local["notes"], json!(draft.report.warnings));
                    assert_eq!(local["after"], draft.report.facts["numbers"]);
                    assert_eq!(local["before"], draft.report.facts["before"]);
                    platform = next(&platform, draft);
                    assert_eq!(local["version"], platform.number);
                    let (saved, _) = lab.load(SAMPLE, platform.number).unwrap();
                    assert_eq!(serde_json::to_value(&saved).unwrap(), platform.content);
                }
                Err(refusal) => {
                    assert_ne!(local["accepted"], true, "{local}");
                    assert_eq!(local["error"], refusal.message.as_str(), "{args}");
                }
            }
        }
        assert_eq!(lab.versions(SAMPLE).len() as u64, platform.number);
        assert_eq!(platform.number, 2);
    }

    #[test]
    fn checks_refuse_a_broken_document_and_the_card_reports_numbers() {
        let base = first();
        let mut broken = base.content.clone();
        broken["schema"] = json!("spacecraft.v0");
        let refusal = Ship::check(&broken, &Value::Null).unwrap_err();
        assert_eq!(refusal.code, "ship_check");
        assert!(refusal.message.starts_with("Refused by the schema check"));
        assert_eq!(
            Ship::check(&json!({"name": 1}), &Value::Null)
                .unwrap_err()
                .code,
            "document_format"
        );
        let patch = Change::Patch {
            ops: vec![lab_kit::PatchOp::Replace {
                path: "/schema".into(),
                value: json!("spacecraft.v0"),
            }],
        };
        assert_eq!(
            Ship::apply(&base, &patch, "x", &Value::Null)
                .unwrap_err()
                .code,
            "ship_check"
        );

        let card = Ship::card(&base, &[], &[]);
        assert_eq!(card.view, View::Report);
        assert!(card.detail["numbers"]["delta_v_m_s"].as_f64().is_some());
        assert!(Ship::jobs_for(&base, &Want::Card).is_empty());
        let glb = Artifact {
            id: "m1".into(),
            version: base.reference(),
            role: lab_kit::ArtifactRole::Render,
            media_type: "model/gltf-binary".into(),
            sha256: String::new(),
            size: 0,
            class: lab_kit::AssetClass::Public,
            no_training: false,
        };
        let card = Ship::card(&base, &[], &[glb]);
        assert_eq!(card.view, View::Model3d);
        assert!(card.state().playable);
        let zone = Ship::jobs_for(
            &base,
            &Want::Job {
                job_type: ZONE_FILE.into(),
                params: Value::Null,
            },
        );
        assert_eq!(zone[0].version, base.reference());
        assert!(Ship::manifest().tools.iter().any(|t| t.name == "ship_edit"));
    }
}
