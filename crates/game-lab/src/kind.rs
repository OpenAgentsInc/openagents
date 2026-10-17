//! Game Lab as a Labs platform kind (#11881): the same edits, checks and
//! refusal sentences as the local service's `game_edit`, written against
//! [`LabKind`] so the platform's store keeps the versions.
//!
//! The bot playtest is the kind's measurement. [`measurements`] runs it
//! and binds the result to the exact content it played, as CAD binds a
//! rebuild; `check` and `apply` accept only measurements for the content
//! they are given. On the platform the `playtest` job computes them.

use lab_kit::{
    Artifact, ArtifactRole, Card, Change, Draft, JobSpec, JobState, KindManifest, LabKind, Limits,
    Measurements, Refusal, Report, Requirements, ToolDecl, Version, View, Want,
};
use serde_json::{Value, json};

use crate::doc::{self, KIND};
use crate::lab::{differences, short, stats_of};
use crate::patch;
use crate::sim::{self, Stats};

/// The job type that runs the seeded bot runs.
pub const PLAYTEST: &str = "playtest";
/// The job type that makes the playable build.
pub const BUILD: &str = "build";

pub struct Game;

/// A patch applied to `before` and checked against the schema and the
/// rules. A refusal carries a stable code and the sentence `game_edit`
/// says.
///
/// # Errors
/// The patch doesn't apply, changes nothing, or breaks the schema or rules.
pub fn propose(
    before: &Value,
    ops: &[Value],
) -> Result<(Value, Vec<(String, Value, Value)>, doc::Game), (&'static str, String)> {
    let after = patch::apply(before, ops).map_err(|why| ("patch", why))?;
    let changes = doc::changes(before, &after);
    if changes.is_empty() && after == *before {
        return Err(("no_change", "That edit changes nothing.".into()));
    }
    let checked = doc::read(&after).map_err(|p| ("game_schema", p.join(" ")))?;
    let broken = doc::rule_problems(&checked);
    if !broken.is_empty() {
        return Err(("game_rules", broken.join(" ")));
    }
    Ok((after, changes, checked))
}

/// The refusal for runs nobody won, naming the wins before the edit when
/// `before` holds that version's stats.
#[must_use]
pub fn unwinnable(measured: &Stats, before: &Value) -> Option<String> {
    if measured.wins() > 0 {
        return None;
    }
    let was = stats_of(before).map_or(String::new(), |s| {
        format!(" Before the edit it won {} of {}.", s.wins(), s.runs)
    });
    Some(format!(
        "That would make the game unwinnable: in {} seeded runs of up to {} minutes the bot never escaped or melted him.{was}",
        measured.runs,
        sim::LIMIT / 60
    ))
}

/// The playtest job's work: the bot's seeded runs on `content`, bound to it.
///
/// # Errors
/// The content isn't a valid game.
pub fn measurements(content: &Value) -> Result<Measurements, Refusal> {
    let checked = doc::read(content).map_err(|p| Refusal::new("game_schema", p.join(" ")))?;
    let stats = sim::measure(checked.tuning, checked.seed);
    Ok(json!({"content_sha256": lab_kit::digest(content), "stats": stats.json()}))
}

/// What a playtest or build needs: the bot plays 24 runs side by side.
fn requirements(max_seconds: u64) -> Requirements {
    Requirements {
        cpu_millis: 2000,
        memory_bytes: 512 * 1024 * 1024,
        tools: vec!["game-lab".into()],
        max_seconds,
        ..Requirements::default()
    }
}

/// The `playtest` job handler for a Labs worker: reads the job on stdin
/// (`{"work": ..., "local_assets": ...}`) and returns the handler output,
/// one JSON artifact holding [`measurements`] for the job's version.
///
/// # Errors
/// The input isn't a job, or the version isn't a valid game.
pub fn playtest_handler(input: &[u8]) -> Result<Value, String> {
    use base64::Engine;
    let input: Value =
        serde_json::from_slice(input).map_err(|_| "Read the playtest job.".to_owned())?;
    let content = &input["work"]["version"]["content"];
    let measured = measurements(content).map_err(|r| r.message)?;
    let bytes = serde_json::to_vec(&measured).map_err(|e| e.to_string())?;
    Ok(json!({"outputs": [{
        "role": "export",
        "media_type": "application/json",
        "content_base64": base64::engine::general_purpose::STANDARD.encode(bytes),
    }]}))
}

/// The measured stats for exactly `content`, or a refusal to save unplayed content.
fn measured(content: &Value, m: &Measurements) -> Result<Stats, Refusal> {
    if m["content_sha256"].as_str() == Some(lab_kit::digest(content).as_str())
        && let Some(stats) = stats_of(&m["stats"])
    {
        return Ok(stats);
    }
    Err(Refusal::new(
        "playtest_required",
        "The bot needs to play this version before it can be saved.",
    ))
}

fn checked(content: &Value) -> Result<doc::Game, Refusal> {
    let game = doc::read(content).map_err(|p| Refusal::new("game_schema", p.join(" ")))?;
    let broken = doc::rule_problems(&game);
    if broken.is_empty() {
        Ok(game)
    } else {
        Err(Refusal::new("game_rules", broken.join(" ")))
    }
}

fn ops_of(change: &Change) -> Result<Vec<Value>, Refusal> {
    match change {
        Change::Patch { ops } => ops
            .iter()
            .map(serde_json::to_value)
            .collect::<Result<_, _>>()
            .map_err(|_| Refusal::new("patch", "The patch couldn't be read.")),
        Change::Intent { intent } if intent["intent"] == "edit" => {
            patch::parse(&intent["patch"]).map_err(|why| Refusal::new("patch", why))
        }
        Change::Intent { .. } => Err(Refusal::new(
            "unknown_intent",
            "Change a game with an edit: a JSON Patch on its document.",
        )),
    }
}

impl LabKind for Game {
    const KIND: &'static str = "game";

    fn manifest() -> KindManifest {
        let tools = crate::lab::tools()["tools"]
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
            name: "Game Lab".into(),
            schemas: vec![KIND.into()],
            tool_prefix: "game_".into(),
            doc_arg: "game".into(),
            tools,
            job_types: vec![PLAYTEST.into(), BUILD.into()],
            job_requirements: [(PLAYTEST, 120), (BUILD, 900)]
                .map(|(job, max_seconds)| (job.into(), requirements(max_seconds)))
                .into(),
            views: vec![View::Playable, View::Report],
            limits: Limits::default(),
        }
    }

    fn check(content: &Value, m: &Measurements) -> Result<Report, Refusal> {
        checked(content)?;
        let stats = measured(content, m)?;
        if let Some(why) = unwinnable(&stats, &Value::Null) {
            return Err(Refusal::new("unwinnable", why));
        }
        Ok(Report {
            warnings: Vec::new(),
            facts: json!({"stats": stats.json()}),
        })
    }

    fn apply(
        base: &Version,
        change: &Change,
        summary: &str,
        m: &Measurements,
    ) -> Result<Draft, Refusal> {
        let ops = ops_of(change)?;
        let (content, changes, _) =
            propose(&base.content, &ops).map_err(|(code, why)| Refusal::new(code, why))?;
        let stats = measured(&content, m)?;
        let before = &base.report.facts["stats"];
        if let Some(why) = unwinnable(&stats, before) {
            return Err(Refusal::new("unwinnable", why));
        }
        let after = stats.json();
        let lines: Vec<String> = changes
            .iter()
            .map(|(path, x, y)| format!("{path}: {} → {}", short(x), short(y)))
            .collect();
        Ok(Draft {
            report: Report {
                warnings: Vec::new(),
                facts: json!({
                    "stats": after,
                    "changes": lines,
                    "differences": differences(before, &after),
                }),
            },
            content,
            summary: summary.into(),
            assets: base.assets.clone(),
        })
    }

    fn jobs_for(version: &Version, want: &Want) -> Vec<JobSpec> {
        let (job_type, params, max_seconds) = match want {
            Want::Card => (BUILD, Value::Null, 900),
            Want::Job { job_type, params } if job_type == PLAYTEST => {
                (PLAYTEST, params.clone(), 120)
            }
            Want::Job { job_type, params } if job_type == BUILD => (BUILD, params.clone(), 900),
            Want::Job { .. } => return Vec::new(),
        };
        vec![JobSpec {
            kind: Self::KIND.into(),
            job_type: job_type.into(),
            version: version.reference(),
            params,
            requirements: Requirements {
                private_local: version.has_private_local(),
                ..requirements(max_seconds)
            },
        }]
    }

    fn card(version: &Version, jobs: &[JobState], artifacts: &[Artifact]) -> Card {
        let build = artifacts
            .iter()
            .find(|a| a.version == version.reference() && a.role == ArtifactRole::Build);
        let job = jobs
            .iter()
            .rev()
            .find(|j| j.spec.version == version.reference());
        let mut detail = version.report.facts.clone();
        if let (Some(facts), Ok(game)) = (detail.as_object_mut(), doc::read(&version.content)) {
            // Each play is a fresh house, as on the local play page.
            facts.insert("fragment".into(), json!(doc::play_fragment(&game)));
        }
        Card {
            schema: lab_kit::card::SCHEMA.into(),
            kind: Self::KIND.into(),
            document: version.document.clone(),
            version: version.number,
            title: version.content["copy"]["title"]
                .as_str()
                .unwrap_or(&version.document)
                .into(),
            summary: version.summary.clone(),
            state: job.map(|j| j.status),
            state_text: None,
            refusal: None,
            view: View::Playable,
            primary: build.map(|a| a.id.clone()),
            poster: None,
            strip: Vec::new(),
            compare: None,
            notes: Vec::new(),
            page: Some(format!(
                "/labs/game/{}?v={}",
                version.document, version.number
            )),
            private_local: version.has_private_local(),
            detail,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doc::ICE_CREAM_MAN;
    use lab_kit::{JobStatus, PatchOp};

    fn version(n: u64, content: Value, report: Report) -> Version {
        Version {
            document: ICE_CREAM_MAN.into(),
            number: n,
            parent: n.checked_sub(1).filter(|p| *p > 0),
            author: "test".into(),
            summary: String::new(),
            created_unix_ms: 0,
            schema: KIND.into(),
            content_sha256: lab_kit::digest(&content),
            content,
            report,
            assets: Vec::new(),
        }
    }

    fn first() -> Version {
        let content = doc::original();
        let report = Game::check(&content, &measurements(&content).unwrap()).unwrap();
        version(1, content, report)
    }

    /// Applies a tool's patch through the kind, measuring as the job would.
    fn apply(base: &Version, patch: &str) -> Result<Draft, Refusal> {
        let ops = patch::parse(&json!(patch)).unwrap();
        let measured = propose(&base.content, &ops)
            .ok()
            .map_or(Value::Null, |(after, _, _)| measurements(&after).unwrap());
        let change = Change::Intent {
            intent: json!({"intent": "edit", "patch": patch}),
        };
        Game::apply(base, &change, "x", &measured)
    }

    #[test]
    fn tool_edits_make_the_same_versions_and_refusals_as_the_local_lab() {
        let dir = std::env::temp_dir().join(format!("game-lab-kind-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let repo = std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."));
        let lab = crate::lab::Lab::open(dir.clone(), repo).unwrap();
        let base = first();
        let (saved, record) = lab.load(ICE_CREAM_MAN, 1).unwrap();
        assert_eq!(saved, base.content);
        assert_eq!(record["stats"], base.report.facts["stats"]);

        let edit = "[{\"op\":\"replace\",\"path\":\"/man/man_chase\",\"value\":6.2},{\"op\":\"replace\",\"path\":\"/layout/extra_closets\",\"value\":0}]";
        let local = lab.run(
            "game_edit",
            &json!({"game": ICE_CREAM_MAN, "patch": edit, "summary": "x"}),
        );
        assert_eq!(local["saved"], true, "{local}");
        let draft = apply(&base, edit).unwrap();
        let (saved, record) = lab.load(ICE_CREAM_MAN, 2).unwrap();
        assert_eq!(draft.content, saved);
        assert_eq!(lab_kit::digest(&draft.content), lab_kit::digest(&saved));
        assert_eq!(draft.report.facts["stats"], record["stats"]);
        assert_eq!(draft.report.facts["changes"], local["changes"]);
        assert_eq!(draft.report.facts["differences"], local["differences"]);

        // The same patch as typed operations gives the same draft.
        let ops: Vec<PatchOp> = serde_json::from_str(edit).unwrap();
        let typed = Game::apply(
            &base,
            &Change::Patch { ops },
            "x",
            &measurements(&draft.content).unwrap(),
        )
        .unwrap();
        assert_eq!(typed.content, draft.content);

        for (patch, code) in [
            (
                "[{\"op\":\"replace\",\"path\":\"/rules/front_door\",\"value\":\"locked\"}]",
                "game_rules",
            ),
            (
                "[{\"op\":\"add\",\"path\":\"/rules/door_locked\",\"value\":true}]",
                "game_schema",
            ),
            (
                "[{\"op\":\"replace\",\"path\":\"/rules/unlock\",\"value\":600},{\"op\":\"replace\",\"path\":\"/weapons/earned_weapons\",\"value\":0}]",
                "unwinnable",
            ),
            (
                "[{\"op\":\"remove\",\"path\":\"/man/sight\"}]",
                "game_schema",
            ),
            ("[{\"op\":\"remove\",\"path\":\"/nothing/here\"}]", "patch"),
            (
                "[{\"op\":\"replace\",\"path\":\"/man/man_chase\",\"value\":1000}]",
                "game_schema",
            ),
        ] {
            let local = lab.run(
                "game_edit",
                &json!({"game": ICE_CREAM_MAN, "patch": patch, "summary": "x", "version": 1}),
            );
            let platform = apply(&base, patch);
            match platform {
                Err(refusal) => {
                    assert_eq!(local["refused"], true, "{local}");
                    assert_eq!(refusal.code, code);
                    assert_eq!(local["reason"], refusal.message.as_str());
                }
                Ok(_) => assert_eq!(code, "", "{local}"),
            }
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_playtest_handler_measures_the_jobs_version() {
        use base64::Engine;
        let content = doc::original();
        let input = json!({"work": {"version": {"content": content}}, "local_assets": {}});
        let output = playtest_handler(input.to_string().as_bytes()).unwrap();
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(output["outputs"][0]["content_base64"].as_str().unwrap())
            .unwrap();
        let measured: Value = serde_json::from_slice(&bytes).unwrap();
        assert!(Game::check(&content, &measured).is_ok());
        assert!(playtest_handler(b"{}").is_err());
    }

    #[test]
    fn unplayed_or_stale_measurements_are_refused() {
        let base = first();
        let content = doc::original();
        assert_eq!(
            Game::check(&content, &Value::Null).unwrap_err().code,
            "playtest_required"
        );
        let edit = "[{\"op\":\"replace\",\"path\":\"/man/man_chase\",\"value\":6.2}]";
        let change = Change::Intent {
            intent: json!({"intent": "edit", "patch": edit}),
        };
        let stale = measurements(&content).unwrap();
        assert_eq!(
            Game::apply(&base, &change, "x", &stale).unwrap_err().code,
            "playtest_required"
        );
        assert_eq!(
            Game::apply(
                &base,
                &Change::Intent {
                    intent: json!({"intent": "paint"})
                },
                "x",
                &stale
            )
            .unwrap_err()
            .code,
            "unknown_intent"
        );
    }

    #[test]
    fn the_card_plays_the_version_and_reports_its_numbers() {
        let v = first();
        let card = Game::card(&v, &[], &[]);
        assert_eq!(card.view, View::Playable);
        assert_eq!(
            card.page.as_deref(),
            Some("/labs/game/the-ice-cream-man?v=1")
        );
        assert_eq!(card.detail["fragment"], "");
        assert!(card.detail["stats"]["runs"].as_u64().is_some());
        assert!(!card.state().playable);
        let jobs = Game::jobs_for(&v, &Want::Card);
        assert_eq!(jobs[0].job_type, BUILD);
        let build = Artifact {
            id: "a1".into(),
            version: v.reference(),
            role: ArtifactRole::Build,
            media_type: "application/wasm".into(),
            sha256: String::new(),
            size: 0,
            class: lab_kit::AssetClass::Public,
            no_training: false,
        };
        let state = JobState {
            id: "j1".into(),
            spec: jobs[0].clone(),
            status: JobStatus::Done,
            message: None,
            estimate_seconds: None,
            artifacts: vec!["a1".into()],
        };
        let card = Game::card(&v, &[state], &[build]);
        assert_eq!(card.primary.as_deref(), Some("a1"));
        assert!(card.state().playable);
        let playtest = Game::jobs_for(
            &v,
            &Want::Job {
                job_type: PLAYTEST.into(),
                params: Value::Null,
            },
        );
        assert_eq!(playtest[0].requirements.max_seconds, 120);
        let manifest = Game::manifest();
        assert!(manifest.tools.iter().any(|t| t.name == "game_edit"));
        assert!(manifest.tools.iter().all(|t| t.name.starts_with("game_")));
    }
}
