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
//! - [`timeline`]: the magnetic timeline's intents;
//! - [`private`]: which files stay on their computer (`lab_private.json`);
//! - [`runner`]: the Labs worker's jobs: `render`, `frames`, `measure`,
//!   `transcribe`, `import` and `score`;
//! - [`text`]: title cards and captions, drawn as `cut.py` draws them.
//!
//! Refusals keep the Lab's exact words. `scripts/film/lab.py` and `cut.py`
//! call this crate through the `video-lab` program (`src/bin/video-lab.rs`).

pub mod check;
pub mod edit;
pub mod fmt;
pub mod measure;
pub mod patch;
pub mod private;
pub mod runner;
pub mod schema;
pub mod text;
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

/// The job types, all run by [`runner`].
pub const JOB_TYPES: [&str; 6] = [
    "render",
    "frames",
    "measure",
    "transcribe",
    "score",
    "import",
];

/// The least a worker needs for each job: renders get an hour, the rest ten minutes.
fn requirements(job: &str) -> Requirements {
    Requirements {
        cpu_millis: 2000,
        memory_bytes: 2 * 1024 * 1024 * 1024,
        tools: match job {
            "transcribe" => vec!["whisper-cli".into(), "ffmpeg".into()],
            "score" => vec!["python3".into(), "ffmpeg".into()],
            _ => vec!["ffmpeg".into()],
        },
        max_seconds: if job == "render" { 3600 } else { 600 },
        ..Requirements::default()
    }
}

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
            job_types: JOB_TYPES.map(String::from).to_vec(),
            job_requirements: JOB_TYPES
                .iter()
                .map(|job| ((*job).to_owned(), requirements(job)))
                .collect(),
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
                    "score" => vec!["python3".into(), "ffmpeg".into()],
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

#[cfg(test)]
mod tests {
    use super::*;
    use lab_kit::{AssetClass, AssetEntry, Reference};

    fn cut() -> Value {
        json!({
            "fps": 30, "size": [320, 180], "out": "~/out.mp4",
            "timeline": {},
            "shots": [
                {"name": "open", "src": "~/clips/a.mp4", "in": 0, "dur": 1.0},
                {"name": "end", "src": "~/clips/b.mp4", "in": 0, "dur": 1.0}
            ]
        })
    }

    fn version(content: Value, assets: Vec<AssetEntry>) -> Version {
        Version {
            document: "film".into(),
            number: 2,
            parent: Some(1),
            author: "test".into(),
            summary: "First cut".into(),
            created_unix_ms: 0,
            schema: "cut.v1".into(),
            content_sha256: lab_kit::digest(&content),
            content,
            report: Report::default(),
            assets,
        }
    }

    /// Placeholders for the clips, as a preview with missing files measures.
    fn placeholders() -> Value {
        json!({"allow_missing": true})
    }

    #[test]
    fn the_manifest_declares_every_job_with_its_needs() {
        let m = VideoKind::manifest();
        assert_eq!(m.kind, "video");
        assert!(!m.tools.is_empty());
        assert!(m.tools.iter().all(|t| t.name.starts_with("lab_")));
        assert_eq!(m.job_types.len(), JOB_TYPES.len());
        for job in JOB_TYPES {
            let r = &m.job_requirements[job];
            assert!(r.tools.contains(&"ffmpeg".to_owned()), "{job}");
            assert!(r.max_seconds > 0);
        }
        assert_eq!(m.job_requirements["render"].max_seconds, 3600);
    }

    #[test]
    fn check_refuses_a_broken_document_and_reports_length() {
        let report = VideoKind::check(&cut(), &placeholders()).unwrap();
        assert_eq!(report.facts["seconds"], 2.0);
        let mut broken = cut();
        broken["shots"][0]["duration"] = json!(4);
        let refusal = VideoKind::check(&broken, &placeholders()).unwrap_err();
        assert_eq!(refusal.code, "check");
    }

    #[test]
    fn a_timeline_edit_makes_a_checked_draft() {
        let base = version(cut(), Vec::new());
        let change = Change::Intent {
            intent: json!({"intent": "timeline", "command": {"id": "video:open", "op": "speed", "value": 2.0}}),
        };
        let draft = VideoKind::apply(&base, &change, " Faster open ", &placeholders()).unwrap();
        assert_eq!(draft.summary, "Faster open");
        assert_eq!(draft.content["shots"][0]["speed"], 2.0);
        assert_eq!(draft.report.facts["seconds"], 1.5);

        let refusal = VideoKind::apply(&base, &change, " ", &placeholders()).unwrap_err();
        assert_eq!(refusal.code, "video");
        let bad = Change::Intent {
            intent: json!({"intent": "timeline", "command": {"id": "video:open", "op": "speed", "value": 9.0}}),
        };
        let refusal = VideoKind::apply(&base, &bad, "x", &placeholders()).unwrap_err();
        assert_eq!(refusal.code, "timeline");
        assert_eq!(refusal.message, "Choose a speed from 0.25× to 4×.");
        let other = Change::Intent {
            intent: json!({"intent": "paint"}),
        };
        assert_eq!(
            VideoKind::apply(&base, &other, "x", &placeholders())
                .unwrap_err()
                .code,
            "unknown_intent"
        );
    }

    #[test]
    fn private_files_keep_jobs_on_their_computer() {
        let assets = vec![private::entry("~/work/metahuman/a.mp4", &"a".repeat(64))];
        assert_eq!(assets[0].class, AssetClass::PrivateLocal);
        let v = version(cut(), assets);
        let jobs = VideoKind::jobs_for(&v, &Want::Card);
        assert_eq!(jobs[0].job_type, "render");
        assert!(jobs[0].requirements.private_local);
        let public = version(cut(), Vec::new());
        let jobs = VideoKind::jobs_for(
            &public,
            &Want::Job {
                job_type: "frames".into(),
                params: Value::Null,
            },
        );
        assert!(!jobs[0].requirements.private_local);
        assert!(
            VideoKind::jobs_for(
                &public,
                &Want::Job {
                    job_type: "paint".into(),
                    params: Value::Null
                }
            )
            .is_empty()
        );
    }

    #[test]
    fn the_card_plays_the_render_and_says_what_is_happening() {
        let v = version(cut(), Vec::new());
        let spec = VideoKind::jobs_for(&v, &Want::Card).remove(0);
        let running = JobState {
            id: "j1".into(),
            spec: spec.clone(),
            status: JobStatus::Running,
            message: None,
            estimate_seconds: Some(90.0),
            artifacts: Vec::new(),
        };
        let card = VideoKind::card(&v, std::slice::from_ref(&running), &[]);
        assert_eq!(card.title, "film");
        assert_eq!(card.view, View::Video);
        assert_eq!(
            card.state_text.as_deref(),
            Some("Rendering the preview, about 2 min…")
        );
        assert_eq!(card.page.as_deref(), Some("/labs/video/film?v=2"));
        let artifact = |id: &str, role| Artifact {
            id: id.into(),
            version: Reference {
                document: "film".into(),
                version: 2,
            },
            role,
            media_type: String::new(),
            sha256: String::new(),
            size: 0,
            class: AssetClass::Public,
            no_training: false,
        };
        let made = [
            artifact("r", ArtifactRole::Render),
            artifact("p", ArtifactRole::Poster),
            artifact("s", ArtifactRole::Strip),
        ];
        let card = VideoKind::card(&v, &[running], &made);
        assert_eq!(card.primary.as_deref(), Some("r"));
        assert_eq!(card.poster.as_deref(), Some("p"));
        assert_eq!(card.strip, ["s"]);
        assert_eq!(card.state, Some(JobStatus::Done));
        assert_eq!(card.state_text, None);
        assert_eq!(card.detail["shots"][1]["start"], 1.0);
    }
}
