use super::*;
use crate::harvest::{Cost, Proposal, Proposals, Propose};
use codex_transport::price::Basis;

fn selection() -> Selection {
    Selection {
        sources: vec![Source {
            task: "scratch-source".into(),
            run: "scratch-source-123".into(),
            group: "source-family".into(),
            artifact: digest(b"retained artifact"),
            citation: "POSIX shell manual, quoting".into(),
            disclosed: "Shell arguments require quoting.".into(),
        }],
        forbidden: vec!["private-token".into()],
        costs: BTreeMap::from([
            ("acquisition_usd".into(), None),
            ("setup_usd".into(), Some(0.01)),
            ("checks_usd".into(), None),
        ]),
    }
}
fn document() -> String {
    "---\nid: shell.quoting\nversion: 1\nkind: method\ntitle: Shell quoting\nsummary: Quote shell arguments.\ntags: [shell]\napplies_when: Passing arguments to a shell.\nstatus: candidate\nauthor: scratch\nprovenance:\n  written_from: [reference]\n  cites: [\"POSIX shell manual, quoting\"]\nevidence: []\n---\n\n## Details\n\nQuote each argument.\n".into()
}
#[test]
fn edits_keep_exact_revisions_and_cannot_activate_or_disclose() {
    let mut session = Session::new(selection()).unwrap();
    let first = session
        .edit(&document(), &lint::Corpus::default())
        .unwrap()
        .clone();
    assert!(first.problems.is_empty());
    let entry = Entry::parse(&first.bytes).unwrap();
    assert!(entry.written_from.contains(&"scratch-source".into()));
    assert!(entry.written_from.contains(&"source-family".into()));
    assert_eq!(entry.status, Status::Candidate);
    assert!(
        crate::product::Corpus::of(vec![entry])
            .base
            .entries
            .is_empty()
    );
    session
        .edit(
            &document().replace("Quote each argument.", "Quote every argument separately."),
            &lint::Corpus::default(),
        )
        .unwrap();
    assert_eq!(session.candidates[0].digest, first.digest);
    assert_ne!(session.candidates[1].digest, first.digest);
    assert!(
        session
            .edit(
                &document().replace("candidate", "admitted"),
                &lint::Corpus::default()
            )
            .is_err()
    );
    assert!(
        session
            .edit(
                &document().replace("Quote each argument.", "private-token"),
                &lint::Corpus::default()
            )
            .is_err()
    );
    assert!(
        session
            .edit(
                &document().replace("POSIX shell manual, quoting", "invented source"),
                &lint::Corpus::default()
            )
            .is_err()
    );
}
struct Fake {
    invalid: bool,
}
impl Propose for Fake {
    fn model(&self) -> &str {
        "fake"
    }
    fn provider(&self) -> &str {
        "fake"
    }
    async fn propose(&self, _: &str, prompt: &str) -> Result<(Proposals, Cost), String> {
        assert!(prompt.contains("Shell arguments require quoting."));
        let entry = Entry::parse(&document()).unwrap();
        Ok((
            Proposals {
                entries: vec![Proposal {
                    id: entry.id,
                    kind: "method".into(),
                    title: entry.title,
                    summary: entry.summary,
                    tags: entry.tags,
                    applies_when: entry.applies_when,
                    body: entry.body,
                    cites: if self.invalid {
                        vec!["invented".into()]
                    } else {
                        entry.cites
                    },
                    updates: String::new(),
                }],
            },
            Cost::known(0.02, Basis::ListPrice),
        ))
    }
}
struct NoEmbed;
impl crate::search::Embed for NoEmbed {
    fn model(&self) -> &str {
        "none"
    }
    async fn embed(
        &self,
        _: Vec<String>,
    ) -> Result<(Vec<Vec<f32>>, Option<f64>), crate::search::EmbedError> {
        panic!("lexical harvest does not embed")
    }
}
#[tokio::test]
async fn scratch_harvest_retains_failed_trials_and_cost_unknowns() {
    let dir = std::env::temp_dir().join(format!("knowledge-workbench-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut session = Session::new(selection()).unwrap();
    assert!(
        session
            .harvest::<_, NoEmbed>(&dir, &Fake { invalid: true }, &lint::Corpus::default())
            .await
            .is_err()
    );
    assert_eq!(session.attempts.len(), 1);
    assert_eq!(session.attempts[0].model_usd, Some(0.02));
    assert!(dir.read_dir().unwrap().next().is_none());
    session
        .harvest::<_, NoEmbed>(&dir, &Fake { invalid: false }, &lint::Corpus::default())
        .await
        .unwrap();
    assert_eq!(session.attempts.len(), 2);
    assert_eq!(session.candidates.len(), 1);
    assert_eq!(session.attempts[1].model_usd, Some(0.02));
    assert_eq!(session.selection.costs["acquisition_usd"], None);
    assert_eq!(
        session.candidates[0].digest,
        digest(session.candidates[0].bytes.as_bytes())
    );
    std::fs::remove_dir_all(dir).unwrap();
}
#[test]
fn selection_refuses_private_sources_before_any_model_call() {
    let mut s = selection();
    s.sources[0].disclosed = "private-token".into();
    assert!(Session::new(s).is_err());
}
#[test]
fn pane_reads_only_exact_candidate_and_history_round_trips() {
    use ::workbench::pane::{PaneAdapter, PaneState, Subject};
    let mut session = Session::new(selection()).unwrap();
    session.edit(&document(), &lint::Corpus::default()).unwrap();
    let path = std::env::temp_dir().join(format!("knowledge-retained-{}.json", std::process::id()));
    let _ = std::fs::remove_file(&path);
    session.save(&path).unwrap();
    assert!(session.save(&path).is_err());
    let restored = Session::read(&path).unwrap();
    assert_eq!(restored.candidates[0].bytes, session.candidates[0].bytes);
    let host = ::workbench::Host::Local {
        instance: "11".repeat(32),
    };
    let adapter = Adapter {
        id: "lesson".into(),
        host: host.clone(),
        session: restored,
    };
    let subject = Subject::Record {
        host,
        id: "lesson".into(),
        revision: None,
    };
    let description = adapter.describe(&subject);
    assert_eq!(description.state, PaneState::Ready);
    assert!(
        !description
            .actions
            .iter()
            .any(|a| a == "publish" || a == "activate")
    );
    let mut bytes: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    bytes["candidates"][0]["bytes"] = serde_json::json!("changed");
    std::fs::write(&path, serde_json::to_vec(&bytes).unwrap()).unwrap();
    assert!(Session::read(&path).is_err());
    std::fs::remove_file(path).unwrap();
}
fn plan(candidate_digest: String) -> crate::study::Plan {
    serde_json::from_value(serde_json::json!({
        "schema":crate::study::SCHEMA,"study":"draft","owner":"owner","binary":"/tmp/binary","binary_digest":digest(b"binary"),"tasks_root":"/tmp/tasks","candidate_digest":candidate_digest,
        "configuration":{"provider":"codex","cost_basis":"list_price","retrieval_mode":"lexical","decision_base_url":"https://api.typesafe.ai","decision_model":"jev-version","model":"fake","effort":"medium","strong_model":"fake","max_steps":10,"max_seconds":60,"max_usd":1.0,"command_seconds":5,"test_seconds":5,"prompt":"Solve task","network":"none","acceptance":true},
        "cases":[{"task":"new-task","group":"new-family","partition":"confirmation","workload_digest":digest(b"task"),"environment_digest":digest(b"environment")}],"repetitions":1,"first_subject":true,"source_tasks":[],"max_total_usd":2.0
    })).unwrap()
}
#[test]
fn study_draft_pins_candidate_and_excludes_source_tasks_and_groups() {
    let mut session = Session::new(selection()).unwrap();
    let candidate = session
        .edit(&document(), &lint::Corpus::default())
        .unwrap()
        .digest
        .clone();
    let p = session.draft_study(plan(candidate.clone())).unwrap();
    assert!(p.source_tasks.contains(&"scratch-source".into()));
    assert!(session.draft_study(plan(digest(b"changed"))).is_err());
    let mut p = plan(candidate.clone());
    p.cases[0].task = "scratch-source".into();
    assert!(session.draft_study(p).unwrap_err().contains("contributed"));
    let mut p = plan(candidate);
    p.cases[0].group = "source-family".into();
    assert!(session.draft_study(p).unwrap_err().contains("Source group"));
}
#[test]
fn explicitly_selected_scratch_run_binds_retained_artifacts() {
    let dir = std::env::temp_dir().join(format!("knowledge-selected-run-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("summary.json"),
        r#"{"task":"scratch-source","reward":0,"outcome":{"steps":1,"ending":"failed"}}"#,
    )
    .unwrap();
    std::fs::write(dir.join("events.jsonl"), "").unwrap();
    let source = Source::retained_run(&dir, "source-family".into(), "Shell manual".into()).unwrap();
    assert_eq!(source.task, "scratch-source");
    assert!(source.disclosed.contains("failed"));
    let mut selected = selection();
    selected.sources[0] = source.clone();
    let mut session = Session::new(selected).unwrap();
    let candidate = session
        .edit(
            &document().replace("POSIX shell manual, quoting", "Shell manual"),
            &lint::Corpus::default(),
        )
        .unwrap();
    assert!(
        Entry::parse(&candidate.bytes)
            .unwrap()
            .written_from
            .contains(&source.run)
    );
    std::fs::write(dir.join("events.jsonl"), "{}\n").unwrap();
    let changed =
        Source::retained_run(&dir, "source-family".into(), "Shell manual".into()).unwrap();
    assert_ne!(changed.artifact, source.artifact);
    std::fs::remove_dir_all(dir).unwrap();
}
