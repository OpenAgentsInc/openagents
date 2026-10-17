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

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis().try_into().unwrap_or(u64::MAX))
}

#[cfg(test)]
mod tests;
