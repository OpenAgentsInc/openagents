//! The two halves of an adoption over the in-memory relay: a validation
//! on a second suite by another author, published with the `validates`
//! marker and read as externally validating by the same reader the
//! candidate queue uses; and a marginal run once an operator's
//! `coder-defaults` release admits the tool, where both arms admit it,
//! the report names the release, and the delta vanishes.

mod support;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use eval_runner::config::Limits;
use eval_runner::runner::Runner;
use eval_runner::wire::{Blobs, memory::Memory};
use ext_eval::case::LoadOptions;
use ext_eval::discover::Suite;
use nostr::cj_conversation::{SubjectSource, SuiteSource};
use nostr::domain::{Event, RelaySigner};
use nostr::eval_ext::{self, Cites, EventPointer, Independence, Validation, hosted};
use serde_json::{Value, json};
use support::{FIXTURE, Phone, config_with, door, hex, result_of, secret};

async fn until<T>(within: Duration, mut find: impl FnMut() -> Option<T>) -> Option<T> {
    let started = std::time::Instant::now();
    while started.elapsed() < within {
        if let Some(found) = find() {
            return Some(found);
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    None
}

fn pointer(value: &Value) -> EventPointer {
    EventPointer {
        id: value["id"].as_str().unwrap().into(),
        pubkey: value["pubkey"].as_str().unwrap().into(),
        kind: u16::try_from(value["kind"].as_u64().unwrap()).unwrap(),
    }
}

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap().flatten() {
        let target = to.join(entry.file_name());
        if entry.path().is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), target).unwrap();
        }
    }
}

/// The fixture extension as a catalog tool the runner may release: its
/// package record names the runner as publisher.
fn catalog_copy(dir: &Path, publisher: &str) -> PathBuf {
    let root = dir.join("catalog").join("repo-map-brief");
    copy_dir(Path::new(FIXTURE), &root);
    let record = root.join("package.json");
    let mut package: Value = serde_json::from_slice(&std::fs::read(&record).unwrap()).unwrap();
    package["publisher"] = json!(publisher);
    std::fs::write(&record, serde_json::to_vec_pretty(&package).unwrap()).unwrap();
    root
}

struct World {
    runner: Arc<Runner>,
    memory: Arc<Memory>,
    runner_key: String,
    operator: RelaySigner,
    catalog: PathBuf,
    documents: PathBuf,
}

impl World {
    fn new(dir: &Path, door: &support::FakeDoor) -> Self {
        let key = secret();
        let identity = coder::relay::Identity::from_text(&hex(&key), "key").unwrap();
        let runner_key = identity.pubkey().to_string();
        let catalog = catalog_copy(dir, &runner_key);
        let operator = RelaySigner::from_secret_hex(&hex(&secret())).unwrap();
        let memory = Arc::new(Memory::default());
        let config = config_with(
            dir,
            door,
            Limits {
                runs_per_trainer: 9,
                turns_per_day: 500,
                jobs: 2,
                concurrency: 2,
            },
            vec![catalog.clone()],
            Some(operator.pubkey()),
        );
        let documents = config.defaults_documents.clone().unwrap();
        let runner = Runner::new(config, identity, memory.clone(), memory.clone()).unwrap();
        World {
            runner,
            memory,
            runner_key,
            operator,
            catalog,
            documents,
        }
    }

    fn events(&self) -> Vec<Event> {
        self.memory.events.lock().unwrap().clone()
    }

    fn event(&self, id: &str) -> Event {
        self.events()
            .into_iter()
            .find(|e| e.id == id)
            .expect("the event is on the relay")
    }

    /// Sends `input` as `phone` and waits for the result.
    async fn ask(&self, phone: &Phone, input: &Value) -> Value {
        let (request, body) = phone.request(&self.runner_key, input);
        self.runner.handle(request.clone()).await;
        until(Duration::from_secs(180), || {
            result_of(&phone.answers(&self.runner_key, &request, &body, &self.events()))
        })
        .await
        .expect("the runner answered")
    }

    /// Runs `input` as `phone`, publishes, and returns the run output and
    /// the published `3189`.
    async fn run_and_publish(&self, phone: &Phone, input: &Value) -> (hosted::RunOutput, Event) {
        let result = self.ask(phone, input).await;
        assert_eq!(result["outcome"], "completed", "{result:#}");
        let output = hosted::parse_run_output(&result["output"]).unwrap();
        let published = self
            .ask(phone, &hosted::publish_input(&output.report))
            .await;
        assert_eq!(published["outcome"], "completed", "{published:#}");
        let out = hosted::parse_publish_output(&published["output"]).unwrap();
        (output, self.event(&out.result.id))
    }

    /// A second suite of the same cases under `author`, released at `at`
    /// with its files in the blob store.
    fn second_suite(&self, author: &RelaySigner, at: u64) -> Event {
        let suite = Suite::load(&self.catalog.join("evals"), LoadOptions::default()).unwrap();
        let (suite_bytes, cases_bytes) = ext_eval::evaluate::suite_documents(
            &suite,
            author.pubkey(),
            "repo-map-brief-more-tests",
            ext_eval::publish::SUITE_COMPONENT,
        )
        .unwrap();
        let results = ext_eval::publish::Results::of_suite(suite, suite_bytes, cases_bytes);
        let release = ext_eval::publish::suite_release(&results).unwrap();
        for (bytes, media) in release.blobs() {
            self.memory.upload(author, &bytes, &media).unwrap();
        }
        let unsigned = release.event();
        let event = author.sign(at, unsigned.kind, unsigned.tags, unsigned.content);
        self.memory.events.lock().unwrap().push(event.clone());
        event
    }

    /// The operator's `coder-defaults` release adopting `subject` on
    /// `result` and `validation`, with its documents where the runner
    /// reads them.
    fn adopt(&self, subject: &str, result: &Event, validation: &Event, expires_at: u64) -> Event {
        let admission = xp_ledger::adopt::admission(
            self.operator.pubkey(),
            &[result],
            &[validation],
            expires_at,
        )
        .unwrap();
        let manifest = xp_ledger::adopt::manifest(
            &xp_ledger::adopt::package_of(self.operator.pubkey()),
            "1",
            &[subject.to_string()],
            &[xp_ledger::adopt::receipt(&admission)],
        )
        .unwrap();
        std::fs::create_dir_all(&self.documents).unwrap();
        for bytes in [&manifest, &admission] {
            let digest = nostr::contracts::digest_bytes(bytes);
            std::fs::write(
                self.documents
                    .join(format!("{}.json", digest.trim_start_matches("sha256:"))),
                bytes,
            )
            .unwrap();
        }
        let parts = xp_ledger::adopt::release(&manifest).unwrap();
        let event = self.operator.sign(
            eval_runner::unix_now(),
            parts.kind,
            parts.tags,
            parts.content,
        );
        self.memory.events.lock().unwrap().push(event.clone());
        event
    }
}

fn run_input(suite: &Value, tool: &eval_runner::catalog::Tool, cites: Option<Cites<'_>>) -> Value {
    hosted::run_input_citing(
        &SuiteSource::Published(pointer(suite)),
        &SubjectSource::Definition(Box::new(tool.definition.clone())),
        None,
        2,
        cites,
    )
    .unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_second_suite_validates_a_result_and_an_adoption_makes_the_next_run_marginal() {
    let dir = tempfile::tempdir().unwrap();
    let door = door();
    let w = World::new(dir.path(), &door);
    let tool = w.runner.catalog().tools[0].clone();

    // The runner releases the tool and its own suite.
    let released = w.runner.release_tools().await.unwrap();
    assert_eq!(released.len(), 1);
    let tool_release = w.event(released[0].1["id"].as_str().unwrap());
    let suite_a = w
        .runner
        .release_extension_suite(&w.catalog, "repo-map-brief-tests")
        .await
        .unwrap();

    // Before any adoption, the runner reads no defaults and a run's
    // report names none.
    let read = w.runner.defaults().await;
    assert!(read.defaults.is_none() && read.arms.is_none());
    let alice = Phone::new();
    let (first, result) = w
        .run_and_publish(&alice, &run_input(&suite_a, &tool, None))
        .await;
    assert_eq!(first.verdict, eval_ext::Verdict::Pass);
    let original = eval_ext::parse_publication(&result).unwrap();
    assert!(original.report.profile.defaults.is_none());
    assert!(original.validates.is_none());

    // Someone else releases a second suite after the tool's release, and
    // a trainer runs it citing the result with `validates`.
    let validator = RelaySigner::from_secret_hex(&hex(&secret())).unwrap();
    let suite_b = w.second_suite(&validator, tool_release.created_at + 1);
    let suite_b_value = json!({"id": suite_b.id, "pubkey": suite_b.pubkey, "kind": suite_b.kind});
    let bob = Phone::new();
    let (second, validation) = w
        .run_and_publish(
            &bob,
            &run_input(&suite_b_value, &tool, Some(Cites::Validates(&result.id))),
        )
        .await;
    assert_eq!(second.verdict, eval_ext::Verdict::Pass);
    let second = eval_ext::parse_publication(&validation).unwrap();
    assert_eq!(second.validates.as_deref(), Some(result.id.as_str()));
    assert!(second.checks.is_none());
    assert_eq!(second.suite_author(), validator.pubkey());
    assert_eq!(
        eval_ext::validation(&original, &second, &suite_b, &tool_release).unwrap(),
        Validation::Validates(Independence::Independent)
    );
    // The same reader the candidate queue uses counts it.
    let publications = xp_ledger::eval::publications(&w.events());
    let validated = xp_ledger::eval::validations(&w.events(), &publications);
    assert_eq!(
        validated.get(&result.id),
        Some(&vec![validation.id.clone()])
    );

    // A validation on the result's own suite, or of a result that isn't
    // there, is refused before anything runs.
    let carol = Phone::new();
    let refused = w
        .ask(
            &carol,
            &run_input(&suite_a, &tool, Some(Cites::Validates(&result.id))),
        )
        .await;
    assert_eq!(refused["code"], "not_admitted", "{refused:#}");
    assert!(
        refused["message"]
            .as_str()
            .unwrap_or_default()
            .contains("second test set")
    );
    let missing = "0a".repeat(32);
    let refused = w
        .ask(
            &carol,
            &run_input(&suite_b_value, &tool, Some(Cites::Validates(&missing))),
        )
        .await;
    assert_eq!(refused["code"], "not_admitted", "{refused:#}");

    // The operator adopts the tool; the runner now reads it as a default
    // and both arms admit it.
    let defaults_release = w.adopt(
        &tool_release.id,
        &result,
        &validation,
        eval_runner::unix_now() + 86_400,
    );
    let read = w.runner.defaults().await;
    let defaults = read.defaults.expect("the defaults are read");
    assert_eq!(defaults.release.id, defaults_release.id);
    assert_eq!(defaults.subjects(), vec![tool_release.id.clone()]);
    assert!(defaults.lapsed.is_empty());
    assert!(read.missing.is_empty(), "{:?}", read.missing);
    let arms = read.arms.expect("the arms admit the defaults");
    assert_eq!(arms.subjects.len(), 1);
    assert_eq!(arms.subjects[0].slug, tool.subject.slug);

    let dave = Phone::new();
    let (marginal, published) = w
        .run_and_publish(&dave, &run_input(&suite_a, &tool, None))
        .await;
    // Both arms reached the tool, so the delta is gone: not Better.
    assert_eq!(
        marginal.headline.baseline_passed,
        Some(marginal.headline.subject_passed),
        "{:?}",
        marginal.headline
    );
    assert_ne!(marginal.verdict, eval_ext::Verdict::Pass);
    let report = eval_ext::parse_publication(&published).unwrap().report;
    assert_eq!(
        report.profile.defaults.as_ref().map(|d| d.id.as_str()),
        Some(defaults_release.id.as_str())
    );
    // The locks changed with the defaults: a check of the earlier result
    // can't be run on these arms.
    assert_ne!(
        report.subject.lock.digest,
        original.report.subject.lock.digest
    );
    assert_ne!(
        report.baseline.as_ref().map(|b| b.lock.digest.clone()),
        original
            .report
            .baseline
            .as_ref()
            .map(|b| b.lock.digest.clone())
    );

    // Nothing here priced anything or promised money.
    for event in w.events() {
        assert!(!event.content.contains(&door.key));
    }
}
