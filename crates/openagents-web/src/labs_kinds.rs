//! The hosted Game, Ship, and Video kinds: each kind's chat tools mapped
//! onto the common Labs operations, and Game's playtest wait.
//!
//! Only the tools that map onto an operation are offered; the others stay
//! with the local services.
use lab_api::{ApiError, Kind, KindAdapter, Tool, offered};
use lab_kit::{
    Artifact, ArtifactRole, Card, Change, JobSpec, JobState, JobStatus, KindManifest, LabKind,
    PatchOp, Refusal, Version, Want,
};
use lab_store::{CheckedDraft, LabStore, Store};
use serde_json::{Value, json};

pub(crate) const GAME_TOOLS: &[&str] = &["game_open", "game_play", "game_edit"];
pub(crate) const SHIP_TOOLS: &[&str] = &["ship_open", "ship_edit"];
pub(crate) const VIDEO_TOOLS: &[&str] = &["lab_open", "lab_edit", "lab_render"];
pub(crate) const ROBOT_TOOLS: &[&str] =
    &["robot_open", "robot_pose", "robot_move_object", "robot_run"];

fn text<'a>(args: &'a Value, name: &str, missing: &str) -> Result<&'a str, ApiError> {
    args[name]
        .as_str()
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| ApiError::invalid(missing))
}

fn summary(args: &Value, otherwise: &str) -> String {
    args["summary"]
        .as_str()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or(otherwise)
        .trim()
        .to_owned()
}

/// A version number given as a number or as text like "3".
fn version(args: &Value) -> Option<u64> {
    args["version"]
        .as_u64()
        .or_else(|| args["version"].as_str().and_then(|s| s.trim().parse().ok()))
        .filter(|n| *n > 0)
}

pub(crate) fn game_tool(name: &str, args: &Value) -> Result<Tool, ApiError> {
    let document = text(args, "game", "Choose a game.")?.to_owned();
    match name {
        "game_open" | "game_play" => Ok(Tool::Read { document }),
        "game_edit" => Ok(Tool::Change {
            document,
            change: Change::Intent {
                intent: json!({"intent": "edit", "patch": args["patch"]}),
            },
            summary: summary(args, "Edited the game"),
        }),
        _ => Err(ApiError::missing()),
    }
}

pub(crate) fn ship_tool(name: &str, args: &Value) -> Result<Tool, ApiError> {
    let document = text(args, "doc", "Choose a ship.")?.to_owned();
    match name {
        "ship_open" => Ok(Tool::Read { document }),
        "ship_edit" => {
            let mut intent = args.clone();
            if let Some(fields) = intent.as_object_mut() {
                fields.remove("doc");
                fields.remove("summary");
                fields.insert("intent".into(), json!("edit"));
            }
            Ok(Tool::Change {
                document,
                change: Change::Intent { intent },
                summary: summary(args, "Edited the ship"),
            })
        }
        _ => Err(ApiError::missing()),
    }
}

pub(crate) fn video_tool(name: &str, args: &Value) -> Result<Tool, ApiError> {
    let document = text(args, "document", "Choose a document.")?.to_owned();
    match name {
        "lab_open" => Ok(Tool::Read { document }),
        "lab_edit" => {
            let patch = text(args, "patch", "Give the edit as a JSON Patch.")?;
            let ops: Vec<PatchOp> = serde_json::from_str(patch)
                .map_err(|_| ApiError::invalid("Give the edit as a JSON Patch."))?;
            Ok(Tool::Change {
                document,
                change: Change::Patch { ops },
                summary: summary(args, ""),
            })
        }
        "lab_render" => {
            let quality = args["quality"].as_str().unwrap_or("preview");
            if !["preview", "review", "master"].contains(&quality) {
                return Err(ApiError::invalid("Choose preview, review, or master."));
            }
            Ok(Tool::Job {
                document,
                version: version(args)
                    .ok_or_else(|| ApiError::invalid("Choose the version to render."))?,
                want: Want::Job {
                    job_type: "render".into(),
                    params: json!({"quality": quality}),
                },
            })
        }
        _ => Err(ApiError::missing()),
    }
}

pub(crate) fn ship() -> KindAdapter<ship_lab::kind::Ship> {
    KindAdapter::new(ship_tool).with_tools(SHIP_TOOLS)
}

/// The server has none of a cut's clips, so a hosted check treats a clip it
/// can't measure as a placeholder, as a local preview does. The worker's
/// `render` and `measure` jobs check the real files.
pub(crate) fn video() -> KindAdapter<video_lab::VideoKind> {
    KindAdapter::new(video_tool)
        .with_tools(VIDEO_TOOLS)
        .with_measurements(json!({"allow_missing": true}))
}

/// The sentence a hosted Game edit answers with while the bot plays it.
pub(crate) const PLAYTEST_WAITING: &str = "The bot needs to play this version before it's saved. We've queued the playtest, and it starts when a computer is free. Make the edit again after it finishes.";

/// Game Lab on the platform. Its check needs the bot's playtest of the
/// exact content: an edit queues a `playtest` job on the base version with
/// the patch, and saves once that job has measured the result.
pub(crate) struct GameApi;

impl GameApi {
    async fn measured(
        store: &Store,
        job: &JobState,
        content: &Value,
    ) -> lab_store::Result<Option<Value>> {
        for id in &job.artifacts {
            let artifact = store.artifact(id).await?;
            if artifact.role != ArtifactRole::Export || artifact.media_type != "application/json" {
                continue;
            }
            let bytes = store.artifact_bytes(id, None).await?;
            if let Ok(m) = serde_json::from_slice::<Value>(&bytes)
                && m["content_sha256"].as_str() == Some(lab_kit::digest(content).as_str())
            {
                return Ok(Some(m));
            }
        }
        Ok(None)
    }
}

#[async_trait::async_trait]
impl Kind for GameApi {
    fn manifest(&self) -> KindManifest {
        offered(game_lab::kind::Game::manifest(), GAME_TOOLS)
    }
    async fn initial(
        &self,
        _store: &Store,
        _document: &str,
        schema: &str,
        content: Value,
        summary: &str,
    ) -> lab_store::Result<CheckedDraft> {
        // A new game has no version to queue a playtest on, so the bot
        // plays it here, bounded, before the first version is saved.
        let played = content.clone();
        let measured = tokio::time::timeout(
            std::time::Duration::from_secs(30),
            tokio::task::spawn_blocking(move || game_lab::kind::measurements(&played)),
        )
        .await
        .map_err(|_| {
            lab_store::Error::Refused(Refusal::new(
                "playtest_timeout",
                "The bot couldn't finish playing this game. Try again.",
            ))
        })?
        .map_err(|_| lab_store::Error::Corrupt)??;
        CheckedDraft::initial::<game_lab::kind::Game>(schema, content, summary, &measured)
    }
    async fn change(
        &self,
        store: &Store,
        base: &Version,
        change: &Change,
        summary: &str,
    ) -> lab_store::Result<CheckedDraft> {
        use game_lab::kind::{Game, PLAYTEST, ops_of, propose};
        let ops = ops_of(change)?;
        let (after, _, _) = propose(&base.content, &ops)
            .map_err(|(code, why)| lab_store::Error::Refused(Refusal::new(code, why)))?;
        let mut spec = Game::jobs_for(
            base,
            &Want::Job {
                job_type: PLAYTEST.into(),
                params: json!({"patch": ops}),
            },
        );
        let spec = spec.pop().ok_or(lab_store::Error::Corrupt)?;
        let job = store.create_job(spec, &Game::manifest(), now()).await?;
        match job.status {
            JobStatus::Done => match Self::measured(store, &job, &after).await? {
                Some(m) => CheckedDraft::change::<Game>(base, change, summary, &m),
                None => Err(lab_store::Error::Refused(Refusal::new(
                    "playtest_failed",
                    "The bot's playtest didn't match this edit. Try a different edit.",
                ))),
            },
            JobStatus::Failed | JobStatus::Uncertain | JobStatus::Cancelled => {
                Err(lab_store::Error::Refused(Refusal::new(
                    "playtest_failed",
                    "The bot couldn't finish playing this version. Try a different edit.",
                )))
            }
            JobStatus::Queued | JobStatus::Running => {
                if job.status == JobStatus::Queued {
                    store
                        .wait_for_worker(
                            &job.id,
                            "Waiting for a computer to play this version.",
                            now(),
                        )
                        .await?;
                }
                Err(lab_store::Error::Refused(Refusal::new(
                    "playtest_waiting",
                    PLAYTEST_WAITING,
                )))
            }
        }
    }
    fn jobs(&self, version: &Version, want: &Want) -> Vec<JobSpec> {
        game_lab::kind::Game::jobs_for(version, want)
    }
    fn card(&self, version: &Version, jobs: &[JobState], artifacts: &[Artifact]) -> Card {
        game_lab::kind::Game::card(version, jobs, artifacts)
    }
    fn tool(&self, name: &str, args: &Value) -> Result<Tool, ApiError> {
        game_tool(name, args)
    }
}

pub(crate) fn robot_tool(name: &str, args: &Value) -> Result<Tool, ApiError> {
    let document = text(args, "document", "Choose a robot item.")?.to_owned();
    match name {
        "robot_open" => Ok(Tool::Read { document }),
        "robot_pose" => {
            let robot = text(args, "robot", "Choose a robot in this scene.")?;
            Ok(Tool::Change {
                document,
                change: Change::Intent {
                    intent: json!({"intent": "pose", "robot": robot, "joints": args["joints"]}),
                },
                summary: format!("Posed {robot}"),
            })
        }
        "robot_move_object" => {
            let object = text(args, "object", "Choose an object in this scene.")?;
            Ok(Tool::Change {
                document,
                change: Change::Intent {
                    intent: json!({"intent": "move_object", "object": object,
                        "position": args["position"]}),
                },
                summary: format!("Moved {object}"),
            })
        }
        "robot_run" => Ok(Tool::Job {
            document,
            version: version(args)
                .ok_or_else(|| ApiError::invalid("Choose the experiment version to run."))?,
            want: Want::Job {
                job_type: robot_lab::kind::SIMULATE.into(),
                params: Value::Null,
            },
        }),
        _ => Err(ApiError::missing()),
    }
}

/// Robot Lab on the platform. A reference names a version in the same
/// space by its content digest; the checks that need those items (a pose
/// against its robot's joints, an experiment against its scene) run here,
/// on exactly the content being saved, with the local lab's rules.
pub(crate) struct RobotApi;

/// At most this many referenced items are read to check one save.
const MAX_ROBOT_REFERENCES: usize = 64;

fn robot_refusal(error: robot_lab::Error) -> lab_store::Error {
    lab_store::Error::Refused(Refusal::new("robot_check", error.to_string()))
}

impl RobotApi {
    async fn pinned(store: &Store, document: &str, sha256: &str) -> lab_store::Result<Version> {
        let mut after = 0;
        for _ in 0..10 {
            let page = store.versions(document, after, 100).await?;
            if let Some(v) = page.items.into_iter().find(|v| v.content_sha256 == sha256) {
                return Ok(v);
            }
            match page.next {
                Some(next) => after = next,
                None => break,
            }
        }
        Err(lab_store::Error::Refused(Refusal::new(
            "robot_check",
            "This item refers to a version that isn't in the lab.",
        )))
    }

    /// Measurements for `content` once every reference and stored file
    /// checks out. A first version's files are uploaded after it's saved,
    /// so only an edit checks them, against the files of the version it edits.
    async fn checked(
        store: &Store,
        base: Option<&Version>,
        content: &Value,
    ) -> lab_store::Result<Value> {
        use robot_lab::{Document, Reference};
        use std::collections::BTreeMap;
        let document = robot_lab::kind::document(content)?;
        let mut found: BTreeMap<Reference, Document> = BTreeMap::new();
        let mut queue: Vec<Reference> = document.references().into_iter().cloned().collect();
        while let Some(reference) = queue.pop() {
            if found.contains_key(&reference) {
                continue;
            }
            if found.len() >= MAX_ROBOT_REFERENCES {
                return Err(lab_store::Error::Invalid(
                    "This item refers to too many others.".into(),
                ));
            }
            let v = Self::pinned(store, &reference.document, &reference.version).await?;
            let item = robot_lab::kind::document(&v.content)?;
            queue.extend(item.references().into_iter().cloned());
            found.insert(reference, item);
        }
        let mut files = std::collections::BTreeSet::new();
        if let Some(base) = base {
            for a in store.version_artifacts(&base.reference()).await? {
                files.insert((a.sha256, a.size));
            }
        }
        robot_lab::store::check_dependencies(
            &document,
            &|r| {
                found.get(r).cloned().ok_or_else(|| {
                    robot_lab::Error::Invalid(
                        "This item refers to a version that isn't in the lab.".into(),
                    )
                })
            },
            &|asset| {
                if base.is_none() || files.contains(&(asset.sha256.clone(), asset.bytes)) {
                    Ok(())
                } else {
                    Err(robot_lab::Error::Invalid(
                        "Upload this item's files before changing it.".into(),
                    ))
                }
            },
        )
        .map_err(robot_refusal)?;
        Ok(robot_lab::kind::dependencies_checked(content))
    }
}

#[async_trait::async_trait]
impl Kind for RobotApi {
    fn manifest(&self) -> KindManifest {
        offered(robot_lab::kind::Robot::manifest(), ROBOT_TOOLS)
    }
    async fn initial(
        &self,
        store: &Store,
        _document: &str,
        schema: &str,
        content: Value,
        summary: &str,
    ) -> lab_store::Result<CheckedDraft> {
        let m = Self::checked(store, None, &content).await?;
        CheckedDraft::initial::<robot_lab::kind::Robot>(schema, content, summary, &m)
    }
    async fn change(
        &self,
        store: &Store,
        base: &Version,
        change: &Change,
        summary: &str,
    ) -> lab_store::Result<CheckedDraft> {
        let next = robot_lab::kind::next_content(base, change)?;
        let m = Self::checked(store, Some(base), &next).await?;
        CheckedDraft::change::<robot_lab::kind::Robot>(base, change, summary, &m)
    }
    fn jobs(&self, version: &Version, want: &Want) -> Vec<JobSpec> {
        robot_lab::kind::Robot::jobs_for(version, want)
    }
    fn card(&self, version: &Version, jobs: &[JobState], artifacts: &[Artifact]) -> Card {
        robot_lab::kind::Robot::card(version, jobs, artifacts)
    }
    fn tool(&self, name: &str, args: &Value) -> Result<Tool, ApiError> {
        robot_tool(name, args)
    }
    fn pins(&self, content: &Value) -> Result<Vec<lab_api::Pin>, Refusal> {
        Ok(robot_lab::kind::pins(content)?
            .into_iter()
            .map(|(document, content_sha256)| lab_api::Pin {
                document,
                content_sha256,
            })
            .collect())
    }
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis().try_into().unwrap_or(u64::MAX))
}

#[cfg(test)]
mod tests;
