//! The Runs screen's learning judgments, applied to both replay collections.
//! Ranking runs in small background batches; the replay clock never orders text
//! by a judgment. Public bodies must pass their manifest's integrity check.

use std::collections::HashMap;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc,
};

use crate::runs::{Agent, Files, Outcome, Run};
use crate::runs_learning::{self as learning, Answer, Context, Judge, Rarity, Report, Store};
use crate::runs_replay::Source;
use crate::terminal_bench::timestamp_ms;

/// Adapts public ATIF files to the same evidence reader as local runs.
fn run(source: &Source) -> Run {
    let Source::Public { trial, cache } = source else {
        let Source::Local(run) = source else {
            unreachable!()
        };
        return *run.clone();
    };
    let started_ms = source.started_ms();
    let ended_ms = trial.finished_at.as_deref().and_then(timestamp_ms);
    let identity = atif::digest(&serde_json::to_value(trial).expect("serializable manifest entry"));
    Run {
        // Include manifest metadata in the cache identity as well as file stamps.
        job: format!("harbor-fable-{}-{}", trial.effort, &identity[..12]),
        trial: trial.id.clone(),
        batch: "public-tb4".to_owned(),
        retained: true,
        files: Files {
            dir: cache.clone(),
            trajectory: Some(cache.join(&trial.file)),
            ..Files::default()
        },
        task: trial.task.clone(),
        task_path: None,
        ask: None,
        category: None,
        expert_hours: None,
        time_limit_sec: None,
        agent: Agent::ClaudeCode,
        variant: Some(format!("{} · {}", trial.model, trial.effort)),
        model: Some(trial.model.clone()),
        started_ms,
        ended_ms,
        // Manifest timestamps bound the trial, not necessarily agent working time.
        agent_ms: None,
        active_ms: ended_ms,
        outcome: match trial.reward {
            Some(reward) if reward >= 1.0 => Outcome::Passed,
            Some(_) => Outcome::Failed,
            None => Outcome::NotGraded("Public manifest has no reward".to_owned()),
        },
        reward: trial.reward,
        tests: None,
        cost_usd: trial.cost_usd,
        cost_estimated: false,
        microcoder: None,
        notes: vec![format!(
            "Public Harbor trajectory: {}. Individual verifier results and agent-only duration are unavailable.",
            trial.source_url
        )],
    }
}

struct Update {
    answers: Vec<(String, Answer)>,
    failures: Vec<(String, String)>,
    report: Report,
    done: bool,
}

pub(crate) struct Learning {
    pub enabled: bool,
    context: Context,
    judge: Judge,
    store: Store,
    inherited: Store,
    entries: Vec<(Source, Run)>,
    answers: HashMap<String, Answer>,
    failures: HashMap<String, String>,
    rarity: Rarity,
    job: Option<mpsc::Receiver<Update>>,
    cancel: Arc<AtomicBool>,
    attempted: bool,
    asked: usize,
    spent: f64,
    error: Option<String>,
}

impl Default for Learning {
    fn default() -> Self {
        Self {
            enabled: false,
            context: Context::default(),
            judge: Judge::Off("Jev isn't configured".to_owned()),
            store: Store::default(),
            inherited: Store::default(),
            entries: Vec::new(),
            answers: HashMap::new(),
            failures: HashMap::new(),
            rarity: Rarity::none(),
            job: None,
            cancel: Arc::new(AtomicBool::new(false)),
            attempted: false,
            asked: 0,
            spent: 0.0,
            error: None,
        }
    }
}

impl Drop for Learning {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}

impl Learning {
    pub fn new(
        sources: impl Iterator<Item = Source>,
        inherited: Store,
        judge: Judge,
        context: Context,
    ) -> Self {
        // A separate index prevents a simultaneous main-screen pass from replacing
        // this pass's index. Matching main-screen answers are still reused.
        let store = Store::open(inherited.dir.as_ref().map(|dir| dir.join("head-to-head")));
        let mut learning = Self::default();
        learning.store = store;
        learning.inherited = inherited;
        learning.judge = judge;
        learning.context = context;
        for source in sources {
            if let Source::Public { trial, cache } = &source
                && !cache.join(&trial.file).is_file()
            {
                learning.failures.insert(
                    source.id(),
                    "Transcript is not on this computer; download it before asking Jev.".to_owned(),
                );
                continue;
            }
            let run = run(&source);
            if !learning::rankable(&run) {
                learning.failures.insert(
                    source.id(),
                    "This run is still running; Jev ranks completed runs.".to_owned(),
                );
                continue;
            }
            let fingerprint = learning::fingerprint(&run, &learning.context);
            if let Some(answer) = learning
                .store
                .lookup(&fingerprint)
                .or_else(|| learning.inherited.lookup(&fingerprint))
            {
                learning.answers.insert(source.id(), answer.clone());
            }
            learning.entries.push((source, run));
        }
        learning.rarity = Rarity::of(learning.answers.values());
        learning
    }

    pub fn start(&mut self, task: Option<&str>) {
        if self.attempted || self.job.is_some() || self.judge.unavailable().is_some() {
            return;
        }
        let mut pending: Vec<_> = self
            .entries
            .iter()
            .filter(|(source, _)| !self.answers.contains_key(&source.id()))
            .cloned()
            .collect();
        if pending.is_empty() {
            return;
        }
        // Show useful answers for the current comparison before the full catalog.
        pending.sort_by_key(|(source, _)| Some(source.task()) != task);
        self.attempted = true;
        let context = self.context.clone();
        let judge = self.judge.clone();
        let mut store = self.store.clone();
        let cancel = self.cancel.clone();
        let (send, receive) = mpsc::channel();
        self.job = Some(receive);
        std::thread::spawn(move || {
            // A closing pane lets its current batch settle. A reopened pane waits
            // for that batch and rereads its cache before making another request.
            static WORKER: std::sync::Mutex<()> = std::sync::Mutex::new(());
            let _guard = WORKER.lock().unwrap_or_else(|poison| poison.into_inner());
            if cancel.load(Ordering::Relaxed) {
                return;
            }
            if store.dir.is_some() {
                store = Store::open(store.dir.clone());
            }
            let runtime = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(runtime) => runtime,
                Err(error) => {
                    let _ = send.send(Update {
                        answers: vec![],
                        failures: vec![],
                        report: Report {
                            errors: vec![format!("Cannot start Jev analysis: {error}")],
                            ..Report::default()
                        },
                        done: true,
                    });
                    return;
                }
            };
            for batch in pending.chunks(8) {
                if cancel.load(Ordering::Relaxed) {
                    break;
                }
                let mut failures = Vec::new();
                let runs: Vec<_> = batch
                    .iter()
                    .filter_map(|(source, run)| match source.verify() {
                        Ok(()) => Some(run.clone()),
                        Err(error) => {
                            failures.push((source.id(), error));
                            None
                        }
                    })
                    .collect();
                let report = runtime.block_on(learning::rank(
                    &runs, &context, &mut store, &judge, None, None,
                ));
                let answers = batch
                    .iter()
                    .filter_map(|(source, run)| {
                        store
                            .lookup(&learning::fingerprint(run, &context))
                            .map(|answer| (source.id(), answer.clone()))
                    })
                    .collect();
                for (source, run) in batch {
                    if store
                        .lookup(&learning::fingerprint(run, &context))
                        .is_none()
                        && !failures.iter().any(|(id, _)| id == &source.id())
                    {
                        failures.push((
                            source.id(),
                            report.errors.first().cloned().unwrap_or_else(|| {
                                "Jev returned no complete assessment.".to_owned()
                            }),
                        ));
                    }
                }
                if send
                    .send(Update {
                        answers,
                        failures,
                        report,
                        done: false,
                    })
                    .is_err()
                {
                    return;
                }
            }
            let _ = send.send(Update {
                answers: vec![],
                failures: vec![],
                report: Report::default(),
                done: true,
            });
        });
    }

    pub fn poll(&mut self) -> bool {
        let mut changed = false;
        while let Some(job) = &self.job {
            match job.try_recv() {
                Ok(update) => {
                    self.answers.extend(update.answers);
                    self.failures.extend(update.failures);
                    self.asked += update.report.asked;
                    self.spent += update.report.cost_usd;
                    if let Some(error) = update.report.errors.first() {
                        self.error = Some(error.clone());
                    }
                    changed = true;
                    if update.done {
                        self.job = None;
                        break;
                    }
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.job = None;
                    self.error = Some("The Jev analysis worker stopped.".to_owned());
                    changed = true;
                    break;
                }
            }
        }
        if changed {
            self.rarity = Rarity::of(self.answers.values());
        }
        changed
    }

    pub fn ranking(&self) -> bool {
        self.job.is_some()
    }

    pub fn score(&self, source: &Source) -> Option<f64> {
        self.answers
            .get(&source.id())
            .map(|answer| answer.learning(&self.rarity))
    }

    pub fn label(&self, source: &Source) -> String {
        if !self.enabled {
            return source.label();
        }
        match self.answers.get(&source.id()) {
            Some(answer) => format!("{:.2} · {}", answer.learning(&self.rarity), source.label()),
            None => format!("— · {}", source.label()),
        }
    }

    pub fn assessment(&self, source: &Source) -> Vec<String> {
        if let Some(answer) = self.answers.get(&source.id()) {
            let mut lines = vec!["Jev's whole-run assessment (includes events after the replay clock). Judgments and hypotheses, not verifier results.".to_owned()];
            lines.extend(learning::summary_lines(answer, &self.rarity));
            lines.push(format!(
                "Model: {} · questions: {} · evidence: {}",
                answer.model, answer.questions, answer.key
            ));
            return lines;
        }
        vec![self.failures.get(&source.id()).cloned().unwrap_or_else(|| {
            if let Some(why) = self.judge.unavailable() {
                format!("No cached assessment. {why}")
            } else if self.job.is_some() {
                "Waiting for Jev's assessment…".to_owned()
            } else {
                "No assessment. Press l to start Jev analysis.".to_owned()
            }
        })]
    }

    pub fn status(&self) -> String {
        let state = if self.job.is_some() {
            "ranking…"
        } else {
            "ready"
        };
        let why = self
            .error
            .as_deref()
            .or_else(|| self.judge.unavailable())
            .unwrap_or("");
        format!(
            "Jev {state} · {} of {} ranked · {} unavailable · {} calls · ${:.4} estimated · {}",
            self.answers.len(),
            self.entries.len(),
            self.failures.len(),
            self.asked,
            self.spent,
            crate::runs::clip_words(why, 100)
        )
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::runs_learning::{JUDGMENTS, Recorded};
    use crate::runs_replay::PublicTrial;
    use crate::runs_story::Detail;
    use serde_json::{Value, json};
    use sha2::{Digest, Sha256};
    use std::{
        path::Path,
        time::{Duration, Instant},
    };

    pub fn public(dir: &Path, id: &str) -> Source {
        let bytes = serde_json::to_vec(&json!({
            "schema_version":"ATIF-v1.7", "session_id":id,
            "agent":{"name":"Claude Code","version":"test","model_name":"Fable 5.1"},
            "steps":[
                {"step_id":1,"timestamp":"2026-09-02T00:00:00Z","source":"user","message":"Repair the parser and run its tests."},
                {"step_id":2,"timestamp":"2026-09-02T00:00:01Z","source":"agent","message":"I will inspect the parser.","tool_calls":[{"tool_call_id":"one","function_name":"bash","arguments":{"command":"cat parser.rs"}}],"observation":{"results":[{"source_call_id":"one","content":"syntax error"}]}},
                {"step_id":3,"timestamp":"2026-09-02T00:00:02Z","source":"agent","message":"The parser needs a fix."}
            ]
        })).unwrap();
        std::fs::write(dir.join(format!("{id}.json")), &bytes).unwrap();
        Source::Public {
            cache: dir.to_owned(),
            trial: Box::new(PublicTrial {
                id: id.to_owned(),
                task: "parser".to_owned(),
                model: "Fable 5.1".to_owned(),
                agent: "Claude Code".to_owned(),
                agent_version: Some("test".to_owned()),
                effort: "max".to_owned(),
                source_url: format!("https://example.com/{id}"),
                file: format!("{id}.json"),
                sha256: Some(format!("{:x}", Sha256::digest(&bytes))),
                available: Some(true),
                started_at: Some("2026-09-02T00:00:00Z".to_owned()),
                finished_at: Some("2026-09-02T00:01:00Z".to_owned()),
                reward: Some(0.0),
                cost_usd: Some(1.25),
            }),
        }
    }

    fn judgments(value: f64) -> Value {
        let mut answers = json!({"value":{"score":value,"confidence":0.9}});
        for judgment in JUDGMENTS {
            answers[judgment.id] = json!({"noul":value / 4.0});
        }
        answers
    }

    pub fn seed(store: &mut Store, source: &Source, context: &Context, value: f64) {
        let run = run(source);
        let state = learning::evidence(&Detail::load(&run), context);
        let key = learning::key(&state);
        store
            .insert(Answer::from_answers(
                &run.id(),
                key.clone(),
                state,
                &judgments(value),
            ))
            .unwrap();
        store.point(learning::fingerprint(&run, context), key);
    }

    pub fn recorded(sources: &[Source], context: &Context) -> Recorded {
        let mut recorded = Recorded::empty();
        for source in sources {
            let state = learning::evidence(&Detail::load(&run(source)), context);
            recorded
                .entries
                .insert(learning::key(&state), judgments(3.0));
        }
        recorded
    }

    fn finish(learning: &mut Learning) {
        let deadline = Instant::now() + Duration::from_secs(15);
        while learning.ranking() {
            assert!(Instant::now() < deadline, "{}", learning.status());
            learning.poll();
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    #[test]
    fn public_analysis_reads_the_transcript_and_reuses_cached_answers_offline() {
        let dir = tempfile::tempdir().unwrap();
        let source = public(dir.path(), "aa");
        let run = run(&source);
        let context = Context::default();
        let detail = Detail::load(&run);
        let state = learning::evidence(&detail, &context);
        assert!(state.to_string().contains("Repair the parser"), "{state}");
        assert!(
            state["activity"]["commands"].as_u64().unwrap() > 0,
            "{state}"
        );
        assert_eq!(run.outcome, Outcome::Failed);
        assert_eq!(run.cost_usd, Some(1.25));
        assert_eq!(run.agent_ms, None);
        assert!(run.agent_label().contains("Fable 5.1 · max"));
        let inherited = Store::open(Some(dir.path().join("learning")));
        let judge = Judge::Recorded(recorded(std::slice::from_ref(&source), &context));
        let mut learning = Learning::new(
            [source.clone()].into_iter(),
            inherited.clone(),
            judge,
            context.clone(),
        );
        learning.start(Some("parser"));
        finish(&mut learning);
        assert_eq!(learning.asked, 1, "{}", learning.status());
        assert!(learning.score(&source).unwrap() > 0.0);
        assert_eq!(learning.answers[&source.id()].nouls.len(), JUDGMENTS.len());
        learning.start(Some("parser"));
        assert!(!learning.ranking());
        let cached = Learning::new(
            [source.clone()].into_iter(),
            inherited,
            Judge::Off("offline".to_owned()),
            context,
        );
        assert_eq!(cached.score(&source), learning.score(&source));
        assert_eq!(cached.asked, 0);
        assert!(
            cached
                .assessment(&source)
                .join("\n")
                .contains("whole-run assessment")
        );
    }

    #[test]
    fn missing_and_corrupt_public_bodies_never_receive_a_judgment() {
        let dir = tempfile::tempdir().unwrap();
        let missing = public(dir.path(), "aa");
        let corrupt = public(dir.path(), "bb");
        std::fs::remove_file(dir.path().join("aa.json")).unwrap();
        std::fs::write(dir.path().join("bb.json"), "{}").unwrap();
        let mut learning = Learning::new(
            [missing.clone(), corrupt.clone()].into_iter(),
            Store::default(),
            Judge::Recorded(Recorded::empty()),
            Context::default(),
        );
        learning.start(None);
        finish(&mut learning);
        assert_eq!(learning.asked, 0);
        assert!(learning.score(&missing).is_none());
        assert!(learning.score(&corrupt).is_none());
        assert!(learning.assessment(&missing)[0].contains("not on this computer"));
        assert!(learning.assessment(&corrupt)[0].contains("Integrity check failed"));
    }

    #[test]
    fn reuses_main_screen_judgments_and_invalidates_changed_evidence() {
        let (_dir, sources) = crate::runs::fixture_sources();
        let catalog = crate::runs::Catalog::load(sources);
        let context = Context::new(&catalog, None);
        let source = Source::Local(Box::new(
            catalog
                .runs
                .iter()
                .find(|run| learning::rankable(run))
                .unwrap()
                .clone(),
        ));
        let mut store = Store::default();
        seed(&mut store, &source, &context, 3.0);
        let cached = Learning::new(
            [source.clone()].into_iter(),
            store.clone(),
            Judge::Off("offline".to_owned()),
            context.clone(),
        );
        assert!(cached.score(&source).is_some());
        let mut changed = source.clone();
        if let Source::Local(run) = &mut changed {
            run.reward = Some(0.321);
        }
        let mut missing = Learning::new(
            [changed.clone()].into_iter(),
            store,
            Judge::Off("offline".to_owned()),
            context,
        );
        missing.start(None);
        assert!(missing.score(&changed).is_none());
        assert!(!missing.ranking());
        assert!(missing.assessment(&changed)[0].contains("offline"));
    }

    #[test]
    fn public_metadata_changes_invalidate_the_identity() {
        let dir = tempfile::tempdir().unwrap();
        let source = public(dir.path(), "aa");
        let mut changed = source.clone();
        if let Source::Public { trial, .. } = &mut changed {
            trial.cost_usd = Some(20.0);
        }
        assert_ne!(run(&source).id(), run(&changed).id());
    }
}
