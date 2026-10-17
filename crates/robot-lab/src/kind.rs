//! Robot Lab as a Labs platform kind (#11882): robot definitions, scenes,
//! experiments and episodes as platform documents, checked by the same
//! rules as the local [`crate::Store`].
//!
//! A platform version's content is the tagged [`Document`]
//! (`{"schema": "robot.v1", "content": {...}}`), so the content names its
//! own schema. A reference inside it names the referenced platform
//! version by that version's `content_sha256`.
//!
//! The document checks are pure. Checks that need other documents or
//! files (a pose against its robot's joints, a file's checksum) run on the
//! host, which passes [`dependencies_checked`] for exactly the content it
//! checked. Physical-robot controls never pass through this kind: they
//! stay on [`crate::remote`] through the paired host.

use std::collections::BTreeMap;

use lab_kit::{
    Artifact, ArtifactRole, Card, Change, Draft, JobSpec, JobState, JobStatus, KindManifest,
    LabKind, Limits, Locator, Measurements, Refusal, Report, Requirements, ToolDecl, Version, View,
    Want,
    change::{Plain, apply_patch},
};
use serde_json::{Value, json};

use crate::document::{Document, Scene};
use crate::{Error, Result};

/// Runs an experiment and saves the episode it records.
pub const SIMULATE: &str = "simulate";
/// Renders a robot or scene as a 3D model, or an episode as a replay video.
pub const PREVIEW: &str = "preview";
/// A simulation's replay: the recorded `robot-episode.v1` document as JSON,
/// naming its experiment by platform content digest.
pub const REPLAY_MEDIA: &str = "application/vnd.openagents.robot-episode+json";

pub const SCHEMAS: [&str; 4] = [
    "robot.v1",
    "robot-scene.v1",
    "robot-experiment.v1",
    "robot-episode.v1",
];

pub struct Robot;

/// Sets joints on a robot in a scene, as the local `pose` operation does.
///
/// # Errors
/// The robot isn't in the scene.
pub fn pose(scene: &mut Scene, robot: &str, joints: BTreeMap<String, f64>) -> Result<()> {
    let instance = scene
        .robots
        .iter_mut()
        .find(|r| r.id == robot)
        .ok_or_else(|| Error::Invalid("Choose a robot in this scene.".into()))?;
    instance.joints.extend(joints);
    Ok(())
}

/// Moves an object in a scene, as the local `move_object` operation does.
///
/// # Errors
/// The object isn't in the scene.
pub fn move_object(scene: &mut Scene, object: &str, position: [f64; 3]) -> Result<()> {
    let object = scene
        .objects
        .iter_mut()
        .find(|o| o.id == object)
        .ok_or_else(|| Error::Invalid("Choose an object in this scene.".into()))?;
    object.geometry.pose.position = position;
    Ok(())
}

/// What a host passes once it has checked every reference and file in
/// `content` (robot poses, scene and experiment links, file checksums).
#[must_use]
pub fn dependencies_checked(content: &Value) -> Measurements {
    json!({"content_sha256": lab_kit::digest(content), "dependencies": "checked"})
}

/// Where a note points: a step of an episode, or the whole version.
///
/// # Errors
/// The step isn't in this episode, or the version isn't an episode.
pub fn note_locator(content: &Value, step: Option<u32>) -> std::result::Result<Locator, Refusal> {
    let Some(step) = step else {
        return Ok(Locator::None);
    };
    match document(content)? {
        Document::Episode(e) if (step as usize) < e.steps.len() => Ok(Locator::Step {
            step: u64::from(step),
        }),
        _ => Err(Refusal::new(
            "note_step",
            "Choose a step from this saved episode.",
        )),
    }
}

/// The content `change` makes from `base`, with the document checks but
/// before the checks that need other items, so a host can check those
/// for exactly this content.
///
/// # Errors
/// The change doesn't apply, or changes the item's type.
pub fn next_content(base: &Version, change: &Change) -> std::result::Result<Value, Refusal> {
    let old = document(&base.content)?;
    let next = match change {
        Change::Patch { ops } => apply_patch(&base.content, ops, &Plain)?,
        Change::Intent { intent } => match intent["intent"].as_str() {
            Some("replace") => intent["content"].clone(),
            Some("pose") => {
                let Document::Scene(mut scene) = old.clone() else {
                    return Err(refusal(Error::Invalid(
                        "Open a scene before moving a joint.".into(),
                    )));
                };
                let joints = serde_json::from_value(intent["joints"].clone())
                    .map_err(|_| Refusal::new("invalid_intent", "Give joint values."))?;
                pose(
                    &mut scene,
                    intent["robot"].as_str().unwrap_or_default(),
                    joints,
                )
                .map_err(refusal)?;
                content(&Document::Scene(scene))?
            }
            Some("move_object") => {
                let Document::Scene(mut scene) = old.clone() else {
                    return Err(refusal(Error::Invalid(
                        "Open a scene before moving an object.".into(),
                    )));
                };
                let position = serde_json::from_value(intent["position"].clone())
                    .map_err(|_| Refusal::new("invalid_intent", "Give a position as [x, y, z]."))?;
                move_object(
                    &mut scene,
                    intent["object"].as_str().unwrap_or_default(),
                    position,
                )
                .map_err(refusal)?;
                content(&Document::Scene(scene))?
            }
            _ => {
                return Err(Refusal::new(
                    "unknown_intent",
                    "Choose replace, pose, or move_object.",
                ));
            }
        },
    };
    if document(&next)?.kind() != old.kind() {
        return Err(refusal(Error::Invalid(
            "Keep the same item type when editing.".into(),
        )));
    }
    Ok(next)
}

/// The versions `content` names, as `(document, content_sha256)`.
///
/// # Errors
/// The content isn't a robot document.
pub fn pins(content: &Value) -> std::result::Result<Vec<(String, String)>, Refusal> {
    Ok(document(content)?
        .references()
        .into_iter()
        .map(|r| (r.document.clone(), r.version.clone()))
        .collect())
}

fn refusal(error: Error) -> Refusal {
    match error {
        Error::Invalid(message) => Refusal::new("robot_check", message),
        Error::Json(_) => format(),
        other => Refusal::new("robot_check", other.to_string()),
    }
}

fn format() -> Refusal {
    Refusal::new(
        "document_format",
        "Choose a robot, scene, experiment, or episode.",
    )
}

/// The checked document in a version's content.
///
/// # Errors
/// The content isn't a robot document, or fails its checks.
pub fn document(content: &Value) -> std::result::Result<Document, Refusal> {
    let document: Document = serde_json::from_value(content.clone()).map_err(|_| format())?;
    document.validate().map_err(refusal)?;
    Ok(document)
}

fn content(document: &Document) -> std::result::Result<Value, Refusal> {
    serde_json::to_value(document).map_err(|_| format())
}

fn facts(document: &Document) -> Value {
    match document {
        Document::Robot(r) => json!({"item": "robot", "name": r.name,
            "links": r.links.len(), "joints": r.joints.len()}),
        Document::Scene(s) => json!({"item": "scene", "name": s.name,
            "robots": s.robots.len(), "objects": s.objects.len(), "cameras": s.cameras.len()}),
        Document::Experiment(e) => json!({"item": "experiment", "seeds": e.seeds.len(),
            "max_steps": e.limits.max_steps}),
        Document::Episode(e) => json!({"item": "episode", "seed": e.seed,
            "steps": e.steps.len(), "outcome": e.outcome, "wall_seconds": e.wall_seconds}),
    }
}

fn tool(name: &str, description: &str, properties: Value, required: &[&str]) -> ToolDecl {
    ToolDecl {
        name: name.into(),
        description: description.into(),
        parameters: json!({"type": "object", "properties": properties,
            "required": required, "additionalProperties": false}),
    }
}

impl LabKind for Robot {
    const KIND: &'static str = "robot";

    fn manifest() -> KindManifest {
        let doc = json!({"type": "string"});
        let version = json!({"type": "integer", "minimum": 1});
        KindManifest {
            kind: Self::KIND.into(),
            name: "Robot Lab".into(),
            schemas: SCHEMAS.map(Into::into).to_vec(),
            tool_prefix: "robot_".into(),
            doc_arg: "document".into(),
            tools: vec![
                tool(
                    "robot_open",
                    "Open a saved robot, scene, experiment, or episode.",
                    json!({"document": doc, "version": version}),
                    &["document"],
                ),
                tool(
                    "robot_versions",
                    "List an item's saved versions.",
                    json!({"document": doc}),
                    &["document"],
                ),
                tool(
                    "robot_pose",
                    "Set joint values for a robot in a scene, making a new version.",
                    json!({"document": doc, "robot": {"type": "string"},
                        "joints": {"type": "object", "additionalProperties": {"type": "number"}}}),
                    &["document", "robot", "joints"],
                ),
                tool(
                    "robot_move_object",
                    "Move an object in a scene, making a new version.",
                    json!({"document": doc, "object": {"type": "string"},
                        "position": {"type": "array", "items": {"type": "number"}, "minItems": 3, "maxItems": 3}}),
                    &["document", "object", "position"],
                ),
                tool(
                    "robot_compare",
                    "Compare two saved versions of the same item.",
                    json!({"document": doc, "a": version, "b": version}),
                    &["document", "a", "b"],
                ),
                tool(
                    "robot_add_note",
                    "Add a note to a version, at an episode step when one is given.",
                    json!({"document": doc, "version": version, "step": {"type": "integer", "minimum": 0},
                        "text": {"type": "string"}}),
                    &["document", "version", "text"],
                ),
                tool(
                    "robot_run",
                    "Run a saved experiment in simulation and record its episode.",
                    json!({"document": doc, "version": version}),
                    &["document", "version"],
                ),
            ],
            job_types: vec![SIMULATE.into(), PREVIEW.into()],
            job_requirements: [
                (
                    SIMULATE.into(),
                    Requirements {
                        cpu_millis: 2000,
                        memory_bytes: 2 * 1024 * 1024 * 1024,
                        tools: vec!["robot-sim".into()],
                        max_seconds: 900,
                        ..Requirements::default()
                    },
                ),
                (
                    PREVIEW.into(),
                    Requirements {
                        tools: vec!["robot-lab".into()],
                        max_seconds: 120,
                        ..Requirements::default()
                    },
                ),
            ]
            .into(),
            views: vec![View::Model3d, View::Video, View::Report],
            limits: Limits {
                max_doc_bytes: crate::document::MAX_DOCUMENT_BYTES as u64,
                ..Limits::default()
            },
        }
    }

    fn check(content: &Value, m: &Measurements) -> std::result::Result<Report, Refusal> {
        let document = document(content)?;
        if (!document.references().is_empty() || !document.assets().is_empty())
            && (m["content_sha256"].as_str() != Some(lab_kit::digest(content).as_str())
                || m["dependencies"] != "checked")
        {
            return Err(Refusal::new(
                "dependencies_required",
                "Check this item's robot, scene, and files before saving it.",
            ));
        }
        Ok(Report {
            warnings: Vec::new(),
            facts: facts(&document),
        })
    }

    fn apply(
        base: &Version,
        change: &Change,
        summary: &str,
        m: &Measurements,
    ) -> std::result::Result<Draft, Refusal> {
        let next = next_content(base, change)?;
        let report = Self::check(&next, m)?;
        Ok(Draft {
            content: next,
            summary: summary.into(),
            report,
            assets: base.assets.clone(),
        })
    }

    fn jobs_for(version: &Version, want: &Want) -> Vec<JobSpec> {
        let item = version.content["schema"].as_str().unwrap_or_default();
        let (job_type, params) = match want {
            Want::Card if item != "robot-experiment.v1" => (PREVIEW, Value::Null),
            Want::Job { job_type, params }
                if job_type == SIMULATE && item == "robot-experiment.v1" =>
            {
                (SIMULATE, params.clone())
            }
            Want::Job { job_type, params }
                if job_type == PREVIEW && item != "robot-experiment.v1" =>
            {
                (PREVIEW, params.clone())
            }
            _ => return Vec::new(),
        };
        let mut requirements = Self::manifest()
            .job_requirements
            .remove(job_type)
            .unwrap_or_default();
        requirements.private_local = version.has_private_local();
        vec![JobSpec {
            kind: Self::KIND.into(),
            job_type: job_type.into(),
            version: version.reference(),
            params,
            requirements,
        }]
    }

    fn card(version: &Version, jobs: &[JobState], artifacts: &[Artifact]) -> Card {
        let mine: Vec<_> = artifacts
            .iter()
            .filter(|a| a.version == version.reference())
            .collect();
        let episode = version.content["schema"] == "robot-episode.v1";
        let experiment = version.content["schema"] == "robot-experiment.v1";
        let (view, primary) = if experiment {
            // The replay card: the latest simulation's recorded episode.
            let replay = mine
                .iter()
                .rev()
                .find(|a| a.role == ArtifactRole::Replay && a.media_type == REPLAY_MEDIA);
            (View::Report, replay.map(|a| a.id.clone()))
        } else if episode {
            let replay = mine
                .iter()
                .find(|a| a.role == ArtifactRole::Replay && a.media_type.starts_with("video/"));
            (
                replay.map_or(View::Report, |_| View::Video),
                replay.map(|a| a.id.clone()),
            )
        } else {
            let model = mine.iter().find(|a| a.media_type == "model/gltf-binary");
            (
                model.map_or(View::Report, |_| View::Model3d),
                model.map(|a| a.id.clone()),
            )
        };
        let job = jobs
            .iter()
            .rev()
            .find(|j| j.spec.version == version.reference());
        let state_text = match (experiment, job.map(|j| j.status), &primary) {
            (true, Some(JobStatus::Done), Some(_)) => {
                Some("The simulation finished. Its replay is saved.".to_owned())
            }
            (true, Some(JobStatus::Queued), _) => Some(
                job.and_then(|j| j.message.clone())
                    .unwrap_or_else(|| "Waiting for a computer to run the simulation.".into()),
            ),
            (true, Some(JobStatus::Running), _) => Some("Running the simulation…".into()),
            (true, Some(JobStatus::Failed | JobStatus::Uncertain), _) => {
                Some("The simulation didn't finish. Run it again.".into())
            }
            _ => None,
        };
        let mut detail = version.report.facts.clone();
        if let (Some(facts), Some(replay)) = (detail.as_object_mut(), &primary)
            && experiment
        {
            facts.insert("replay".into(), json!(replay));
        }
        Card {
            schema: lab_kit::card::SCHEMA.into(),
            kind: Self::KIND.into(),
            document: version.document.clone(),
            version: version.number,
            title: version.content["content"]["name"]
                .as_str()
                .unwrap_or(&version.document)
                .into(),
            summary: version.summary.clone(),
            state: job.map(|j| j.status),
            state_text,
            refusal: None,
            view,
            primary,
            poster: mine
                .iter()
                .find(|a| a.role == ArtifactRole::Poster)
                .map(|a| a.id.clone()),
            strip: Vec::new(),
            compare: None,
            notes: Vec::new(),
            page: Some(format!(
                "/labs/robot/{}?v={}",
                version.document, version.number
            )),
            private_local: version.has_private_local(),
            detail,
        }
    }
}
