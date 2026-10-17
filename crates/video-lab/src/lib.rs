//! Video Lab as a kind on the Labs platform ([plan](../../docs/labs/README.md),
//! [Video specifics](../../docs/video-lab/shared-primitive.md)).
//!
//! The rules that decide whether a film edit is accepted live here, once:
//! - [`schema`]: the `cut.v1`, `shot.v1` and `scene.v1` schemas;
//! - [`check`]: every rule of `cut.py check` and `lab.py check`, over the
//!   document and the [`measure::Measured`] facts about its files;
//! - [`patch`]: JSON Patch with `@name` picking shots;
//! - [`edit`]: the edit pipeline (captions follow their shots, removed shots
//!   take their captions and music hits, an edit that adds a problem is
//!   refused), times the chat passes, and voiceover caption pacing;
//! - [`timeline`]: the magnetic timeline's intents.
//!
//! Refusals keep the Lab's exact words. `scripts/film/lab.py` and `cut.py`
//! call this crate through the `video-lab` program (`src/bin/video-lab.rs`).

pub mod check;
pub mod edit;
pub mod fmt;
pub mod measure;
pub mod patch;
pub mod schema;
pub mod timeline;

use lab_kit::{
    Artifact, ArtifactRole, Card, Change, Draft, JobSpec, JobState, JobStatus, KindManifest,
    LabKind, Limits, Measurements, Refusal, Report, Requirements, ToolDecl, Version, View, Want,
};
use serde_json::{Value, json};

pub use measure::Measured;

/// The Lab's tool declarations, generated from `lab.py tool_specs()`
/// (a test in `scripts/film/test_lab.py` keeps them equal).
pub const TOOLS: &str = include_str!("../tools.json");

pub struct VideoKind;

/// The document kind: `measurements.schema` when given, else from its shape.
fn kind_of(content: &Value, m: &Value) -> &'static str {
    match m.get("schema").and_then(Value::as_str) {
        Some("shot.v1") => "shot.v1",
        Some("scene.v1") => "scene.v1",
        Some(_) => "cut.v1",
        None if content.get("shots").is_some() => "cut.v1",
        None if content.get("renderer").is_some() => "shot.v1",
        None if content.get("shot").is_some() => "scene.v1",
        None => "cut.v1",
    }
}

fn measured(m: &Measurements) -> Measured {
    serde_json::from_value(m.clone()).unwrap_or_default()
}

/// Measurements for an edit: `{"old": .., "new": ..}`, or one set for both.
fn measured_pair(m: &Measurements) -> (Measured, Measured) {
    match (m.get("old"), m.get("new")) {
        (Some(old), Some(new)) => (measured(old), measured(new)),
        _ => (measured(m), measured(m)),
    }
}

impl LabKind for VideoKind {
    const KIND: &'static str = "video";

    fn manifest() -> KindManifest {
        let tools: Vec<ToolDecl> = serde_json::from_str(TOOLS).unwrap_or_default();
        KindManifest {
            kind: Self::KIND.into(),
            name: "Video Lab".into(),
            schemas: schema::KINDS.iter().map(|k| (*k).to_owned()).collect(),
            tool_prefix: "lab_".into(),
            doc_arg: "document".into(),
            tools,
            job_types: [
                "render",
                "frames",
                "measure",
                "transcribe",
                "score",
                "import",
            ]
            .map(String::from)
            .to_vec(),
            job_requirements: Default::default(),
            views: vec![View::Video, View::Compare, View::Image],
            limits: Limits {
                max_doc_bytes: 2 * 1024 * 1024,
                ..Limits::default()
            },
        }
    }

    fn check(content: &Value, m: &Measurements) -> Result<Report, Refusal> {
        let kind = kind_of(content, m);
        let found = check::check(kind, content, &measured(m));
        if !found.problems.is_empty() {
            let text = check::plain(&found.problems).join("; ");
            return Err(Refusal::new("check", text));
        }
        Ok(Report {
            warnings: found.notes,
            facts: json!({"seconds": edit::length(kind, content)}),
        })
    }

    fn apply(
        base: &Version,
        change: &Change,
        summary: &str,
        m: &Measurements,
    ) -> Result<Draft, Refusal> {
        let kind = base.schema.as_str();
        let (m_old, m_new) = measured_pair(m);
        let edited = match change {
            Change::Patch { ops } => {
                let ops =
                    serde_json::to_value(ops).map_err(|e| Refusal::new("patch", e.to_string()))?;
                {
                    let (new, said) = edit::prepare(kind, &base.content, &ops, summary, true)?;
                    edit::commit(kind, &base.content, new, said, &m_old, &m_new)?
                }
            }
            Change::Intent { intent } => match intent.get("intent").and_then(Value::as_str) {
                Some("timeline") => {
                    if summary.trim().is_empty() {
                        return Err(Refusal::new(
                            "video",
                            "an edit needs a one-line summary of what it changes, in plain words",
                        ));
                    }
                    let input = json!({"action": "edit", "document": base.content,
                        "command": intent.get("command").cloned().unwrap_or(Value::Null),
                        "media": intent.get("media").cloned().unwrap_or(json!({}))});
                    let new = timeline::request(&input).map_err(|e| Refusal::new("timeline", e))?;
                    let new = new.get("document").cloned().unwrap_or(new);
                    edit::commit(kind, &base.content, new, Vec::new(), &m_old, &m_new)?
                }
                _ => {
                    return Err(Refusal::new(
                        "unknown_intent",
                        "Video Lab changes a document with a patch or a timeline edit.",
                    ));
                }
            },
        };
        Ok(Draft {
            report: Report {
                warnings: edited.notes,
                facts: json!({"said": edited.said, "seconds": edit::length(kind, &edited.content)}),
            },
            content: edited.content,
            summary: summary.trim().to_owned(),
            assets: base.assets.clone(),
        })
    }

    fn jobs_for(version: &Version, want: &Want) -> Vec<JobSpec> {
        let spec = |job_type: &str, params: Value| JobSpec {
            kind: Self::KIND.into(),
            job_type: job_type.into(),
            version: version.reference(),
            requirements: Requirements {
                gpu: job_type == "render" && version.schema != "cut.v1",
                tools: match job_type {
                    "render" if version.schema == "shot.v1" => {
                        vec!["blender".into(), "ffmpeg".into()]
                    }
                    "render" if version.schema == "scene.v1" => {
                        vec!["verse".into(), "ffmpeg".into()]
                    }
                    "transcribe" => vec!["whisper-cli".into(), "ffmpeg".into()],
                    _ => vec!["ffmpeg".into()],
                },
                max_seconds: if job_type == "render" { 3600 } else { 600 },
                private_local: version.has_private_local(),
                ..Default::default()
            },
            params,
        };
        match want {
            Want::Card => vec![spec("render", json!({"quality": "preview"}))],
            Want::Job { job_type, params } if Self::manifest().job_types.contains(job_type) => {
                vec![spec(job_type, params.clone())]
            }
            Want::Job { .. } => Vec::new(),
        }
    }

    fn card(version: &Version, jobs: &[JobState], artifacts: &[Artifact]) -> Card {
        let mine: Vec<&Artifact> = artifacts
            .iter()
            .filter(|a| a.version == version.reference())
            .collect();
        let pick = |role: ArtifactRole| mine.iter().find(|a| a.role == role).map(|a| a.id.clone());
        let job = jobs
            .iter()
            .filter(|j| j.spec.version == version.reference() && j.spec.job_type == "render")
            .last();
        let primary = pick(ArtifactRole::Render);
        let state = if primary.is_some() {
            Some(JobStatus::Done)
        } else {
            job.map(|j| j.status)
        };
        let state_text = match (primary.is_some(), job) {
            (false, Some(j)) if matches!(j.status, JobStatus::Queued | JobStatus::Running) => {
                let quality = j.spec.params["quality"].as_str().unwrap_or("preview");
                let what = if j.status == JobStatus::Queued {
                    "Waiting to render"
                } else {
                    "Rendering"
                };
                Some(match j.estimate_seconds.filter(|e| *e > 0.0) {
                    Some(e) => format!(
                        "{what} the {quality}, about {} min…",
                        (e / 60.0).ceil() as u64
                    ),
                    None => format!("{what} the {quality}…"),
                })
            }
            (false, Some(j)) if j.status == JobStatus::Failed => j.message.clone(),
            _ => None,
        };
        Card {
            schema: lab_kit::card::SCHEMA.into(),
            kind: Self::KIND.into(),
            document: version.document.clone(),
            version: version.number,
            title: version
                .content
                .get("title")
                .and_then(Value::as_str)
                .unwrap_or(&version.document)
                .to_owned(),
            summary: version.summary.clone(),
            state,
            state_text,
            refusal: None,
            view: View::Video,
            primary,
            poster: pick(ArtifactRole::Poster),
            strip: mine
                .iter()
                .filter(|a| a.role == ArtifactRole::Strip || a.role == ArtifactRole::Frame)
                .map(|a| a.id.clone())
                .collect(),
            compare: None,
            notes: Vec::new(),
            page: Some(format!(
                "/labs/video/{}?v={}",
                version.document, version.number
            )),
            private_local: version.has_private_local(),
            detail: json!({"seconds": edit::length(&version.schema, &version.content),
                "shots": edit::timeline(&version.content).into_iter().map(|(n, s, e)| json!({"name": n, "start": s, "end": e})).collect::<Vec<_>>()}),
        }
    }
}
