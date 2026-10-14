//! The explicitly configured host's private file store and typed tools.

use robot_lab::document::{Camera, Geometry, Object, Pose, RobotInstance, Scene, Shape};
use robot_lab::store::{Note, Selection};
use robot_lab::{Document, Operation, Reference, Reply, Store};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::view::Bundle;

pub use crate::tools::{Tool, definitions};

#[derive(Clone)]
pub struct Host {
    pub store: Store,
    pub catalog: PathBuf,
    pub base: String,
    pub author: String,
    pub runs: Option<std::sync::Arc<crate::runs::Runs>>,
}

impl Host {
    pub fn definitions(&self) -> Value {
        let mut value = definitions();
        if self.runs.is_none() {
            value["tools"].as_array_mut().unwrap().retain(|tool| {
                !matches!(
                    tool["name"].as_str(),
                    Some("robot_experiment" | "robot_run" | "robot_run_control")
                )
            });
        }
        value
    }
    pub fn open(store: Store, catalog: &Path, base: &str) -> Result<Self, String> {
        if !(base.starts_with("http://127.0.0.1:") || base.starts_with("http://localhost:"))
            || base.trim_end_matches('/').contains(['?', '#', '\n'])
        {
            return Err("Choose a local Robot Lab address".into());
        }
        robot_description::pin::verify(catalog).map_err(|e| e.to_string())?;
        Ok(Self {
            store,
            catalog: catalog.to_path_buf(),
            base: base.trim_end_matches('/').into(),
            author: "Local user".into(),
            runs: None,
        })
    }

    pub fn documents(&self) -> Result<Value, String> {
        let mut docs = Vec::new();
        for reference in self.store.list().map_err(|e| e.to_string())? {
            let version = self.store.get(&reference).map_err(|e| e.to_string())?;
            let title = match &version.content {
                Document::Scene(s) => s.name.clone(),
                Document::Episode(_) => "Robot attempt".into(),
                _ => continue,
            };
            docs.push(json!({"id":reference.document,"title":title,"kind":version.content.kind(),"versions":version.number,"reference":reference}));
        }
        Ok(json!({"project":self.store.project(),"documents":docs}))
    }

    pub fn bundle(&self, reference: &Reference) -> Result<Bundle, String> {
        self.store.view(reference)
    }

    pub fn card(
        &self,
        reference: &Reference,
        comparison: Option<&Reference>,
    ) -> Result<Value, String> {
        let bundle = self.bundle(reference)?;
        if let Some(comparison) = comparison {
            self.bundle(comparison)?;
        }
        let player = format!(
            "{}/play?document={}&version={}{}",
            self.base,
            reference.document,
            reference.version,
            comparison.map_or(String::new(), |b| format!(
                "&compare_document={}&compare_version={}",
                b.document, b.version
            ))
        );
        Ok(
            json!({"card":"robot_lab","doc":reference.document,"reference":reference,"version":bundle.number,
            "title":bundle.scene.name,"mode":bundle.mode(),"lab":self.base,"player":player,"comparison":comparison}),
        )
    }

    pub fn execute(&self, name: &str, arguments: Value, now: u64) -> Result<Value, String> {
        let mut input = arguments
            .as_object()
            .cloned()
            .ok_or("Use named Robot Lab arguments")?;
        if input.contains_key("tool") {
            return Err("Choose the tool by its declared name".into());
        }
        input.insert("tool".into(), json!(name));
        let tool: Tool = serde_json::from_value(Value::Object(input))
            .map_err(|e| format!("Check this tool's arguments: {e}"))?;
        let reference = |document: String, version: String| Reference { document, version };
        let result = match tool {
            Tool::Run {
                experiment,
                version,
                seed,
                job,
                batch,
            } => {
                let runs = self
                    .runs
                    .as_ref()
                    .ok_or("Configure a simulator before starting a run")?;
                return runs.start(
                    self.clone(),
                    reference(experiment, version),
                    seed,
                    job,
                    batch,
                );
            }
            Tool::RunStatus { job } => return crate::runs::Runs::inspect(self, &job),
            Tool::RunControl { job, command_json } => {
                let runs = self
                    .runs
                    .as_ref()
                    .ok_or("Configure a simulator before controlling a run")?;
                let action = serde_json::from_str(&command_json)
                    .map_err(|_| "Choose pause, resume, step, stop, or reset for this run")?;
                return runs.control(&job, action);
            }
            Tool::Experiment {
                scene,
                version,
                document,
                seeds,
                max_steps,
                policy,
                frames_json,
                hold,
            } => {
                let config = &self
                    .runs
                    .as_ref()
                    .ok_or("Configure a simulator before creating an experiment")?
                    .config;
                let scene = reference(scene, version);
                let saved = if policy {
                    crate::experiments::policy_experiment(
                        self, &scene, seeds, max_steps, config, &document,
                    )?
                } else {
                    let frames = serde_json::from_str(&frames_json).map_err(
                        |_| "Use a list of named joint poses for the scripted experiment",
                    )?;
                    crate::experiments::scripted_experiment(
                        self, &scene, seeds, max_steps, frames, hold, config, &document,
                    )?
                };
                return Ok(json!({"experiment":saved,"summary":"Experiment saved"}));
            }
            Tool::List => return self.documents(),
            Tool::Import {
                document,
                source,
                revision,
            } => {
                let pin =
                    robot_description::pin::verify(&self.catalog).map_err(|e| e.to_string())?;
                if source != "so101" || revision != pin.revision {
                    return Err(format!("Choose SO-101 at {}", pin.revision));
                }
                let robot = robot_sim::import::pinned_urdf(&self.store, &self.catalog)
                    .map_err(|e| e.to_string())?;
                let initial = robot
                    .joints
                    .iter()
                    .filter(|j| j.kind != robot_lab::document::JointKind::Fixed)
                    .map(|j| {
                        (
                            j.name.clone(),
                            j.limits.map_or(0.0, |l| 0.0_f64.clamp(l.lower, l.upper)),
                        )
                    })
                    .collect();
                let robot_reference = self
                    .store
                    .save(
                        &format!("{document}-robot"),
                        None,
                        Document::Robot(robot),
                        &self.author,
                        "Added SO-101",
                        now,
                    )
                    .map_err(|e| e.to_string())?;
                let scene = table_scene(robot_reference, initial);
                let reference = self
                    .store
                    .save(
                        &document,
                        None,
                        Document::Scene(scene),
                        &self.author,
                        "Added arm, table, and cube",
                        now,
                    )
                    .map_err(|e| e.to_string())?;
                return Ok(
                    json!({"summary":"SO-101 is on the table","card":self.card(&reference,None)?,"source":pin.revision}),
                );
            }
            Tool::Open { document, version } => Operation::Open { document, version },
            Tool::Pose {
                document,
                base,
                joint,
                position,
            } => {
                let reference = reference(document, base);
                let bundle = self.bundle(&reference)?;
                Operation::Pose {
                    base: reference,
                    robot: bundle.scene.robots[0].id.clone(),
                    joints: BTreeMap::from([(joint.clone(), position)]),
                    author: self.author.clone(),
                    summary: format!("Moved {joint}"),
                }
            }
            Tool::MoveObject {
                document,
                base,
                object,
                x,
                y,
                z,
            } => Operation::MoveObject {
                base: reference(document, base),
                object: object.clone(),
                position: [x, y, z],
                author: self.author.clone(),
                summary: format!("Moved {object}"),
            },
            Tool::Edit {
                document,
                base,
                scene_json,
                summary,
            } => {
                if scene_json.len() > 1 << 20 {
                    return Err("Use a smaller scene edit".into());
                }
                let scene: Scene = serde_json::from_str(&scene_json)
                    .map_err(|e| format!("Check the scene: {e}"))?;
                let mut preview = self.bundle(&reference(document.clone(), base.clone()))?;
                preview.scene = scene.clone();
                preview.episode = None;
                preview.subject = preview.scene_reference.clone();
                preview.validate()?;
                Operation::Edit {
                    base: reference(document, base),
                    content: Document::Scene(scene),
                    author: self.author.clone(),
                    summary,
                }
            }
            Tool::Compare {
                document,
                a,
                b,
                other_document,
            } => Operation::Compare {
                a: reference(document.clone(), a),
                b: reference(other_document.unwrap_or(document), b),
            },
            Tool::Versions { document } => Operation::Versions { document },
            Tool::Notes { document, version } => Operation::Notes {
                subject: reference(document, version),
            },
            Tool::AddNote {
                document,
                version,
                frame,
                text,
            } => Operation::AddNote {
                note: Note {
                    subject: reference(document, version),
                    episode_step: frame,
                    author: self.author.clone(),
                    text,
                    created_unix_ms: now,
                },
            },
            Tool::Inspect {
                document,
                version,
                frame,
            } => Operation::Inspect {
                episode: reference(document, version),
                step: frame,
            },
            Tool::Select {
                chat,
                document,
                version,
                frame,
                camera,
                selection_base,
                comparison,
            } => {
                let reference = reference(document, version);
                let bundle = self.bundle(&reference)?;
                let selection = Selection {
                    scene: bundle.scene_reference,
                    episode: bundle.episode.map(|_| reference),
                    step: frame,
                    comparison,
                    camera,
                };
                if let Some(base) = selection_base {
                    self.store
                        .select_if_unchanged(
                            &chat,
                            &selection,
                            if base == "none" { None } else { Some(&base) },
                        )
                        .map_err(|e| e.to_string())?;
                    return Ok(
                        json!({"summary":"Robot Lab selected","result":{"selection":selection}}),
                    );
                }
                Operation::Select { chat, selection }
            }
            Tool::Restore { chat } => Operation::Restore { chat },
        };
        let reply = self.store.execute(result, now).map_err(|e| e.to_string())?;
        let card = match &reply {
            Reply::Opened { reference, .. } | Reply::Saved { reference, .. } => {
                Some(self.card(reference, None)?)
            }
            Reply::Compared { a, b, .. } => Some(self.card(a, Some(b))?),
            Reply::Selection { selection: Some(s) } => Some(self.card(
                s.episode.as_ref().unwrap_or(&s.scene),
                s.comparison.as_ref(),
            )?),
            _ => None,
        };
        let summary = match &reply {
            Reply::Saved { summary, .. } => summary.clone(),
            Reply::Noted => "Note saved".into(),
            _ => "Robot Lab opened".into(),
        };
        let items = if let Reply::Versions { versions } = &reply {
            versions.iter().map(|v|Ok(json!({"reference":v.reference().map_err(|e|e.to_string())?,"number":v.number,"summary":v.summary})))
                .collect::<Result<Vec<_>,String>>()?
        } else {
            Vec::new()
        };
        Ok(json!({"summary":summary,"result":reply,"card":card,"versions":items}))
    }
}

pub fn table_scene(reference: Reference, joints: BTreeMap<String, f64>) -> Scene {
    let object = |id: &str, position, size, mass, rgba| Object {
        id: id.into(),
        geometry: Geometry {
            pose: Pose {
                position,
                ..Pose::default()
            },
            shape: Shape::Box { size },
            rgba,
        },
        mesh: None,
        collider: Some(Shape::Box { size }),
        mass,
        friction: 0.7,
    };
    let eye = glam::DVec3::new(0.65, 1.25, 1.0);
    let forward = (glam::DVec3::new(0.1, 0.9, 0.0) - eye).normalize();
    let right = forward.cross(glam::DVec3::Y).normalize();
    let down = forward.cross(right);
    let rotation = glam::DQuat::from_mat3(&glam::DMat3::from_cols(right, down, forward));
    Scene {
        name: "SO-101 table".into(),
        robots: vec![RobotInstance {
            id: "arm".into(),
            robot: reference,
            pose: Pose {
                position: [0.0, 0.75, 0.0],
                ..Pose::default()
            },
            joints,
        }],
        objects: vec![
            object(
                "table",
                [0.0, 0.73, 0.0],
                [1.2, 0.04, 0.75],
                0.0,
                [0.42, 0.26, 0.14, 1.0],
            ),
            object(
                "cube",
                [0.22, 0.775, -0.1],
                [0.05; 3],
                0.05,
                [0.85, 0.08, 0.06, 1.0],
            ),
        ],
        lights: vec![],
        cameras: vec![Camera {
            id: "front".into(),
            pose: Pose {
                position: eye.to_array(),
                rotation: rotation.to_array(),
            },
            width: 640,
            height: 400,
            intrinsics: [812.0, 812.0, 320.0, 200.0],
            robot_link: None,
        }],
    }
}
