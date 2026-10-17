//! The portable CAD Lab contract. A worker rebuilds the proposed content;
//! checks accept only measurements for that exact content.
use lab_kit::{
    Artifact, ArtifactRole, Card, Change, Draft, JobSpec, JobState, KindManifest, LabKind, Limits,
    Measurements, Refusal, Report, Requirements, ToolDecl, Version, View, Want,
    change::{Plain, apply_patch},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

pub struct Cad;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "intent", rename_all = "snake_case", deny_unknown_fields)]
pub enum Intent {
    Replace {
        content: Value,
    },
    PullFace {
        part: String,
        face: String,
        delta_mm: f64,
    },
}

/// Build the proposed document without running an engine or changing `base`.
pub fn changed_content(base: &Value, change: &Change) -> Result<Value, Refusal> {
    let content = match change {
        Change::Patch { ops } => apply_patch(base, ops, &Plain),
        Change::Intent { intent } => {
            let intent: Intent = serde_json::from_value(intent.clone())
                .map_err(|_| Refusal::new("invalid_intent", "Choose a supported CAD change."))?;
            match intent {
                Intent::Replace { content } => Ok(content),
                Intent::PullFace {
                    part,
                    face,
                    delta_mm,
                } => {
                    let mut content = base.clone();
                    crate::drag::apply(&mut content, &part, &face, delta_mm)
                        .map_err(|message| Refusal::new("face_pull", message))?;
                    Ok(content)
                }
            }
        }
    }?;
    if content["id"] != base["id"] {
        return Err(Refusal::new(
            "document_id",
            "Keep this design's document name.",
        ));
    }
    Ok(content)
}

/// Bind an engine's report to its input. Servers construct this after rebuilding,
/// rather than accepting measurements supplied by an API caller.
pub fn measurements(content: &Value, report: Value) -> Measurements {
    json!({"content_sha256": lab_kit::digest(content), "report": report})
}

impl LabKind for Cad {
    const KIND: &'static str = "cad";

    fn manifest() -> KindManifest {
        KindManifest {
            kind: Self::KIND.into(),
            name: "CAD Lab".into(),
            schemas: vec![crate::doc::PART.into(), crate::doc::ASSEMBLY.into()],
            tool_prefix: "cad_".into(),
            doc_arg: "doc".into(),
            tools: vec![
                ToolDecl {
                    name: "cad_from_json".into(),
                    description: "Save a checked part or assembly from JSON.".into(),
                    parameters: json!({"type":"object","required":["doc","document"],"properties":{"doc":{"type":"string"},"document":{"type":"object"},"summary":{"type":"string"}},"additionalProperties":false}),
                },
                ToolDecl {
                    name: "cad_open".into(),
                    description: "Open a saved design and its checks.".into(),
                    parameters: json!({"type":"object","required":["doc"],"properties":{"doc":{"type":"string"}},"additionalProperties":false}),
                },
                ToolDecl {
                    name: "cad_drag_face".into(),
                    description: "Pull a picked face by a distance in millimeters.".into(),
                    parameters: json!({"type":"object","required":["doc","part","face","delta_mm"],"properties":{"doc":{"type":"string"},"part":{"type":"string"},"face":{"type":"string"},"delta_mm":{"type":"number"}},"additionalProperties":false}),
                },
                ToolDecl {
                    name: "cad_export".into(),
                    description: "Export a saved design.".into(),
                    parameters: json!({"type":"object","required":["doc","version","formats"],"properties":{"doc":{"type":"string"},"version":{"type":"integer","minimum":1},"formats":{"type":"array","items":{"type":"string","enum":["step","glb","stl","3mf"]}}},"additionalProperties":false}),
                },
            ],
            job_types: vec!["rebuild".into(), "export".into(), "render".into()],
            job_requirements: ["rebuild", "export", "render"]
                .map(|job| {
                    (
                        job.into(),
                        lab_kit::Requirements {
                            max_seconds: 75,
                            memory_bytes: 640 * 1024 * 1024,
                            ..Default::default()
                        },
                    )
                })
                .into(),
            views: vec![View::Model3d],
            limits: Limits {
                runs_per_hour: 150,
                ..Limits::default()
            },
        }
    }

    fn check(content: &Value, m: &Measurements) -> Result<Report, Refusal> {
        let problems = match content["kind"].as_str() {
            Some(crate::doc::PART) => {
                crate::doc::part_problems(content["id"].as_str().unwrap_or_default(), content)
            }
            Some(crate::doc::ASSEMBLY) => crate::doc::check_assembly(content),
            _ => {
                return Err(Refusal::new(
                    "document_format",
                    "Choose a part or an assembly.",
                ));
            }
        };
        if !problems.is_empty() {
            return Err(Refusal::new("cad_check", problems.join(" ")));
        }
        if m["content_sha256"].as_str() != Some(lab_kit::digest(content).as_str())
            || m["report"]["checks"]["rebuilt"] != true
        {
            return Err(Refusal::new(
                "rebuild_required",
                "Build this design before saving it.",
            ));
        }
        Ok(Report {
            warnings: m["report"]["warnings"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect(),
            facts: m["report"].clone(),
        })
    }

    fn apply(
        base: &Version,
        change: &Change,
        summary: &str,
        m: &Measurements,
    ) -> Result<Draft, Refusal> {
        let content = changed_content(&base.content, change)?;
        if content["kind"].as_str() != Some(base.schema.as_str()) {
            return Err(Refusal::new(
                "document_format",
                "Keep this design's document format.",
            ));
        }
        let report = Self::check(&content, m)?;
        Ok(Draft {
            content,
            summary: summary.into(),
            report,
            assets: base.assets.clone(),
        })
    }

    fn jobs_for(version: &Version, want: &Want) -> Vec<JobSpec> {
        let (job_type, params) = match want {
            Want::Card => ("render", json!({"view":"hero"})),
            Want::Job { job_type, params }
                if ["rebuild", "export", "render"].contains(&job_type.as_str()) =>
            {
                (job_type.as_str(), params.clone())
            }
            _ => return vec![],
        };
        vec![JobSpec {
            kind: Self::KIND.into(),
            job_type: job_type.into(),
            version: version.reference(),
            params,
            requirements: Requirements {
                tools: vec!["cad-lab".into()],
                max_seconds: 75,
                private_local: version.has_private_local(),
                ..Requirements::default()
            },
        }]
    }

    fn card(version: &Version, jobs: &[JobState], artifacts: &[Artifact]) -> Card {
        let matching: Vec<_> = artifacts
            .iter()
            .filter(|a| a.version == version.reference())
            .collect();
        let primary = matching
            .iter()
            .find(|a| a.media_type == "model/gltf-binary");
        let poster = matching.iter().find(|a| a.role == ArtifactRole::Poster);
        let job = jobs
            .iter()
            .rev()
            .find(|j| j.spec.version == version.reference());
        Card {
            schema: lab_kit::card::SCHEMA.into(),
            kind: Self::KIND.into(),
            document: version.document.clone(),
            version: version.number,
            title: version.content["title"]
                .as_str()
                .unwrap_or(&version.document)
                .into(),
            summary: version.summary.clone(),
            state: job.map(|j| j.status),
            state_text: None,
            refusal: None,
            view: View::Model3d,
            primary: primary.map(|a| a.id.clone()),
            poster: poster.map(|a| a.id.clone()),
            strip: vec![],
            compare: None,
            notes: vec![],
            page: Some(format!("/cad/{}?v={}", version.document, version.number)),
            private_local: version.has_private_local(),
            detail: version.report.facts.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plate() -> Value {
        let mut content = crate::doc::new_part(
            "plate",
            "Mounting plate",
            crate::doc::DEFAULT_MATERIAL,
            None,
        );
        content["features"] =
            json!([{"id":"body","op":"solid.box","params":{"length":40,"width":40,"height":3}}]);
        content
    }
    fn version(content: Value, report: Report) -> Version {
        Version {
            document: "fan-bracket".into(),
            number: 1,
            parent: None,
            author: "test".into(),
            summary: "Mounting plate".into(),
            created_unix_ms: 0,
            schema: crate::doc::PART.into(),
            content_sha256: lab_kit::digest(&content),
            content,
            report,
            assets: vec![],
        }
    }

    #[test]
    fn face_edit_matches_existing_engine_and_rejects_stale_measurements() {
        let cache = tempfile::tempdir().unwrap();
        let content = plate();
        let evaluation = crate::evaluate_with(&content, cache.path()).unwrap();
        let measured = measurements(&content, evaluation.report);
        let report = Cad::check(&content, &measured).unwrap();
        let base = version(content, report);
        let mut existing = base.content.clone();
        crate::drag::apply(&mut existing, "plate", "body:+z", 5.0).unwrap();
        let change = Change::Intent {
            intent: json!({"intent":"pull_face","part":"plate","face":"body:+z","delta_mm":5}),
        };
        assert_eq!(changed_content(&base.content, &change).unwrap(), existing);
        assert_eq!(
            Cad::apply(&base, &change, "5 mm thicker", &measured)
                .unwrap_err()
                .code,
            "rebuild_required"
        );
        let rebuilt = crate::evaluate_with(&existing, cache.path()).unwrap();
        let new_measurements = measurements(&existing, rebuilt.report.clone());
        let draft = Cad::apply(&base, &change, "5 mm thicker", &new_measurements).unwrap();
        assert_eq!(draft.content, existing);
        assert_eq!(draft.report.facts, rebuilt.report);
        assert!(draft.report.facts["mass"]["mass_kg"].as_f64().is_some());
        assert_eq!(base.content["features"][0]["params"]["height"], 3);
        assert_eq!(
            Cad::check(&base.content, &Value::Null).unwrap_err().code,
            "rebuild_required"
        );
    }

    #[test]
    fn refused_geometry_does_not_produce_a_draft() {
        let cache = tempfile::tempdir().unwrap();
        let content = plate();
        let report = crate::evaluate_with(&content, cache.path()).unwrap().report;
        let base = version(
            content.clone(),
            Cad::check(&content, &measurements(&content, report)).unwrap(),
        );
        let change = Change::Intent {
            intent: json!({"intent":"pull_face","part":"plate","face":"body:+z","delta_mm":-5}),
        };
        assert_eq!(
            Cad::apply(&base, &change, "Too thin", &Value::Null)
                .unwrap_err()
                .code,
            "face_pull"
        );
        assert_eq!(
            changed_content(
                &base.content,
                &Change::Patch {
                    ops: vec![lab_kit::change::PatchOp::Replace {
                        path: "/id".into(),
                        value: json!("another-design")
                    }],
                }
            )
            .unwrap_err()
            .code,
            "document_id"
        );
        let mut invalid = content;
        invalid["features"][0]["op"] = json!("unavailable.operation");
        assert_eq!(
            Cad::check(&invalid, &Value::Null).unwrap_err().code,
            "cad_check"
        );
        assert!(crate::evaluate_with(&invalid, cache.path()).is_err());
    }

    #[test]
    fn reads_make_no_work_and_jobs_bind_the_saved_version() {
        let v = version(plate(), Report::default());
        let card = Cad::card(&v, &[], &[]);
        assert_eq!(card.view, View::Model3d);
        assert_eq!(card.state, None);
        assert_eq!(card.primary, None);
        let jobs = Cad::jobs_for(&v, &Want::Card);
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].version, v.reference());
        assert_eq!(jobs[0].requirements.tools, ["cad-lab"]);
        assert!(
            Cad::jobs_for(
                &v,
                &Want::Job {
                    job_type: "unknown".into(),
                    params: Value::Null
                }
            )
            .is_empty()
        );
    }
}
