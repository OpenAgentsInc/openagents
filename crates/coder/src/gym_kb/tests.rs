use super::*;
use knowledge::search::EmbedError;
use sha2::{Digest, Sha256};

/// Bag-of-words vectors: deterministic and offline, for ranking fixtures.
struct Words;

impl Embed for Words {
    fn model(&self) -> &str {
        "words-64"
    }

    async fn embed(&self, inputs: Vec<String>) -> Result<(Vec<Vec<f32>>, Option<f64>), EmbedError> {
        let vectors = inputs
            .iter()
            .map(|text| {
                let mut vector = vec![0.0_f32; 64];
                for word in knowledge::search::words(text) {
                    let bucket = word
                        .bytes()
                        .fold(7_usize, |h, b| h.wrapping_mul(31).wrapping_add(b.into()));
                    vector[bucket % 64] += 1.0;
                }
                vector
            })
            .collect();
        Ok((vectors, Some(0.0)))
    }
}

/// A judge that finds every record of `kind` relevant at `relevance` and
/// every other at 0.1.
struct Kinds {
    kind: &'static str,
    relevance: f64,
}

impl Judge for Kinds {
    fn judge(
        &self,
        request: jev::SystemOneRequest,
    ) -> BoxFuture<'_, Result<jev::SystemOneResponse, String>> {
        let state = request.state.to_value();
        let records = state["records"].as_object().cloned().unwrap_or_default();
        let mut answers = serde_json::Map::new();
        for (key, record) in &records {
            let n = key.trim_start_matches("item_");
            let p = if record["kind"] == self.kind {
                self.relevance
            } else {
                0.1
            };
            answers.insert(format!("relevant_{n}"), json!({"type": "noul", "noul": p}));
        }
        let bytes = json!({"model": "jev-test", "answers": answers})
            .to_string()
            .into_bytes();
        Box::pin(async move {
            jev::SystemOneResponse::decode(jev::RawResponse {
                status: 200,
                headers: Default::default(),
                bytes,
            })
            .map_err(|e| e.to_string())
        })
    }
}

struct Down;

impl Judge for Down {
    fn judge(
        &self,
        _: jev::SystemOneRequest,
    ) -> BoxFuture<'_, Result<jev::SystemOneResponse, String>> {
        Box::pin(async { Err("down".to_string()) })
    }
}

fn corpus() -> Corpus {
    let root = knowledge::product::repository();
    Corpus::load(&knowledge::product::default_dir(), Some(&root)).expect("the corpus loads")
}

/// The compiled-in changelog reads as builds, newest first, each with its
/// title and items, and a release whose strings hold escapes reads whole.
#[test]
fn the_changelog_reads_as_builds() {
    let builds = changelog();
    assert!(builds.len() >= 3, "{builds:?}");
    let newest = &builds[0];
    assert!(newest.build.parse::<u32>().is_ok(), "{newest:?}");
    assert!(!newest.title.is_empty() && !newest.items.is_empty());
    assert_eq!(newest.source, CHANGELOG_PATH);
    let numbers: Vec<u32> = builds
        .iter()
        .filter_map(|release| release.build.parse().ok())
        .collect();
    assert!(
        numbers.windows(2).all(|pair| pair[0] > pair[1]),
        "{numbers:?}"
    );

    let source = r#"
pub const CHANGELOG: &[Release] = &[
    Release {
        version: "1.0.0",
        build: "21",
        title: "Tests in \"chat\"",
        what_to_test: "Ask \"What's new?\"",
        items: &[
            Item { title: "Cards", detail: "A \"card\" per reply." },
            Item {
                title: "Checks",
                detail: "x",
            },
        ],
    },
];
"#;
    let read = changelog_of(source);
    assert_eq!(read.len(), 1);
    assert_eq!(read[0].title, "Tests in \"chat\"");
    assert_eq!(read[0].items, ["Cards", "Checks"]);
    assert!(changelog_of("fn main() {}").is_empty());
}

/// The catalog is the notes tagged `tool`, Project map first (in the
/// fixture notes; the product corpus has none, since the sample plugins are
/// never shown); the Gym notes are the ones tagged `gym` and cite their
/// note's path.
#[test]
fn the_catalog_and_notes_come_from_tagged_product_notes() {
    let corpus = corpus();
    assert!(
        tools(&corpus).is_empty(),
        "no product note is a sample plugin's"
    );
    let tools = super::fixture_tools();
    assert_eq!(tools.len(), 6, "{tools:?}");
    assert_eq!(tools[0].id, DEFAULT_TOOL);
    assert_eq!(tools[0].name, "Project map");
    let root = knowledge::product::repository();
    let notes = notes(&corpus);
    assert!(
        notes
            .iter()
            .any(|note| note.id.starts_with("openagents.gym-news@")),
        "{notes:?}"
    );
    assert!(notes.iter().all(|note| !note.id.contains("tool-")));
    for note in &notes {
        assert!(root.join(&note.source).exists(), "{}", note.source);
    }
}

// Publications built as the runner will build them: a report under the
// ext-eval profile, published with `nostr::eval_ext::publication`, and
// signed with throwaway keys derived from a label (as the nostr crate's
// own tests do); no key is stored.

use nostr::contracts::{digest_bytes, jcs};
use nostr::domain::RelaySigner;
use nostr::eval_ext::{CASE_SCHEMA, PROFILE_SCHEMA, SUITE_SCHEMA};

const AT: u64 = 1_790_000_000;

fn signer(label: &str) -> RelaySigner {
    let hex: String = Sha256::digest(label.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    RelaySigner::from_secret_hex(&hex).expect("throwaway key")
}

fn pubkey(label: &str) -> String {
    signer(label).pubkey().to_string()
}

fn art(bytes: &[u8], media: &str, schema: Option<&str>) -> Value {
    let mut value =
        json!({ "digest": digest_bytes(bytes), "size": bytes.len(), "media_type": media });
    if let Some(schema) = schema {
        value["schema"] = json!(schema);
    }
    value
}

const PROMPT: &[u8] =
    b"+++\nv = \"openagents.eval-case.v1\"\nkind = \"should-fire\"\n+++\n\nMap this repository.\n";
const QUIET: &[u8] =
    b"+++\nv = \"openagents.eval-case.v1\"\nkind = \"should-not-fire\"\n+++\n\nSay hello.\n";
const GRADER: &[u8] = b"+++\ntype = \"decision\"\nquestion = \"Did it map the repository?\"\nthreshold = 0.7\n+++\n\nA map.\n";

fn case(id: &str, kind: &str, prompt: &[u8]) -> Value {
    json!({
        "id": id, "kind": kind, "runs": 3,
        "prompt": art(prompt, "text/markdown", Some(CASE_SCHEMA)),
        "config": null,
        "graders": [{"name": "criteria", "artifact": art(GRADER, "text/markdown", Some(CASE_SCHEMA))}],
        "fixtures": [],
    })
}

fn suite_bytes() -> Vec<u8> {
    let cases = eval_ext::case_manifest(&[
        case("map-repo", "should-fire", PROMPT),
        case("say-hello", "should-not-fire", QUIET),
    ])
    .expect("a case manifest");
    let small = |label: &str| art(label.as_bytes(), "application/json", None);
    jcs(&json!({
        "v": SUITE_SCHEMA, "requires": [],
        "id": format!("{}:project-map-tests/suite", pubkey("suite-author")),
        "purpose": "operation",
        "workload": small("workload"),
        "cases": art(&cases, "application/json", Some(CASE_SCHEMA)),
        "partition": small("partition"), "labels": small("labels"), "metrics": small("metrics"),
        "acceptance": {
            "id": format!("{}:gym/ext-eval-v1", pubkey("operator")),
            "artifact": art(b"{\"gate\":\"ext-eval-v1\"}", "application/json", None),
        },
        "environment": small("environment"),
    }))
    .unwrap()
}

/// A report by `evaluator` with `verdict`, for the Project map subject.
fn report(evaluator: &str, verdict: &str) -> String {
    report_locked(evaluator, verdict, "lock-a")
}

/// [`report`] with the subject arm run under `lock`.
fn report_locked(evaluator: &str, verdict: &str, lock: &str) -> String {
    let mut suite = art(&suite_bytes(), "application/json", Some(SUITE_SCHEMA));
    suite["event"] = json!({"id": "11".repeat(32), "pubkey": pubkey("suite-author"), "kind": 3184});
    let arm = |definition: Value, lock: &str| {
        json!({
            "definition": definition,
            "lock": art(lock.as_bytes(), "application/json", None),
            "configuration": art(b"config", "application/json", None),
        })
    };
    let subject = json!({
        "id": format!("{}:project-map/map", pubkey("ext-author")),
        "artifact": art(b"project map program", "application/json", None),
        "event": {"id": "22".repeat(32), "pubkey": pubkey("ext-author"), "kind": 3184},
    });
    let baseline = json!({
        "id": format!("{}:coder-defaults/coder", pubkey("operator")),
        "artifact": art(b"coder", "application/json", None),
    });
    let counts = |completed: u64| {
        json!({"planned": 2, "attempted": 6, "completed": completed,
        "refused": 0, "failed": 6 - completed, "cancelled": 0, "unknown": 0, "excluded": 0})
    };
    json!({
        "v": nostr::kb::REPORT_SCHEMA, "requires": [],
        "suite": suite,
        "partition": art(b"partition", "application/json", None),
        "subject": arm(subject, lock),
        "baseline": arm(baseline, "lock-base"),
        "evaluator": pubkey(evaluator),
        "started_at": AT - 600, "ended_at": AT - 60,
        "runs": art(b"runs", "application/json", None),
        "coverage": {"subject": counts(5), "baseline": counts(4)},
        "measurements": [{"arm": "subject", "metric": "cases_passed", "value": 2, "denominator": 2,
            "unknown_count": 0, "uncertainty": null, "evidence": []}],
        "verdict": verdict,
        "limitations": art(b"limitations", "text/plain", None),
        "meta": {"ext_eval": {
            "v": PROFILE_SCHEMA,
            "gate": digest_bytes(b"{\"gate\":\"ext-eval-v1\"}"),
            "cases": [{"id": "map-repo", "kind": "should-fire"}, {"id": "say-hello", "kind": "should-not-fire"}],
            "headline": {"subject_passed": 2, "baseline_passed": 1, "total": 2},
            "requester": null,
        }},
    })
    .to_string()
}

/// A signed publication of `evaluator`'s report, checking `checks`.
fn published(evaluator: &str, verdict: &str, checks: Option<&str>, at: u64) -> Event {
    let parts = eval_ext::publication(&report(evaluator, verdict), checks).expect("a publication");
    signer(evaluator).sign(at, parts.kind, parts.tags, parts.content)
}

/// [`published`] with the subject arm run under `lock`.
fn published_locked(evaluator: &str, lock: &str, at: u64) -> Event {
    let parts = eval_ext::publication(&report_locked(evaluator, "pass", lock), None)
        .expect("a publication");
    signer(evaluator).sign(at, parts.kind, parts.tags, parts.content)
}

fn catalog() -> Vec<Tool> {
    vec![Tool {
        id: DEFAULT_TOOL.into(),
        name: "Project map".into(),
        line: "Shows Coder how a project is laid out.".into(),
        source: "knowledge/openagents/openagents.tool-project-map.md".into(),
        slugs: vec!["project-map".into(), "repo-map".into()],
    }]
}

/// Only a publication `nostr::eval_ext` verifies reaches the corpus: a
/// forged signature, a report whose bytes do not match its digest, or a
/// missing marker is refused, and nothing it says is read.
#[test]
fn an_unverified_or_digest_mismatched_publication_never_reaches_the_corpus() {
    let good = published("evaluator", "pass", None, AT);
    // Signed content changed after signing: the id and signature no longer
    // hold.
    let mut forged = good.clone();
    forged.content.push(' ');
    forged.id = signer("forger").sign(AT, 1, Vec::new(), String::new()).id;
    let mut mismatched = good.clone();
    {
        let mut content: Value = serde_json::from_str(&mismatched.content).unwrap();
        let bytes = content["meta"]["ext_eval_report"]
            .as_str()
            .unwrap()
            .replace("\"pass\"", "\"fail\"");
        content["meta"]["ext_eval_report"] = json!(bytes);
        mismatched = signer("evaluator").sign(
            AT,
            mismatched.kind,
            mismatched.tags.clone(),
            content.to_string(),
        );
    }
    let mut unmarked = good.clone();
    unmarked
        .tags
        .retain(|tag| tag.value() != Some(eval_ext::PROFILE_MARKER));
    let unmarked = signer("evaluator").sign(AT, unmarked.kind, unmarked.tags, unmarked.content);

    let admitted = admit(
        &catalog(),
        &[
            forged.clone(),
            mismatched.clone(),
            unmarked.clone(),
            good.clone(),
        ],
        &PendingReleases,
        &[],
        &[],
    );
    assert_eq!(admitted.results.len(), 1, "{:?}", admitted.refused);
    let refused: Vec<&str> = admitted.refused.iter().map(|(id, _)| id.as_str()).collect();
    assert_eq!(
        refused,
        [
            forged.id.as_str(),
            mismatched.id.as_str(),
            unmarked.id.as_str()
        ]
    );
    let record = &admitted.results[0];
    assert_eq!(record.publication.id, good.id);
    assert_eq!(
        record.tool.as_deref(),
        Some(DEFAULT_TOOL),
        "matched by its package slug"
    );
    assert_eq!(record.tool_name, "Project map");
    assert_eq!(record.headline.subject_passed, 2);
    assert_eq!(record.headline.baseline_passed, Some(1));
    assert_eq!(record.cases, 2);
    assert_eq!(record.trainer, pubkey("evaluator"));
}

/// Checks count as `nostr::eval_ext::linkage` reads them: another
/// trainer's matching verdict confirms, a different one disputes, and a
/// trainer's check of their own result is not a check.
#[test]
fn checks_are_counted_as_the_profile_links_them() {
    let original = published("evaluator", "pass", None, AT);
    let confirm = published("checker-a", "pass", Some(&original.id), AT + 10);
    let dispute = published("checker-b", "fail", Some(&original.id), AT + 20);
    let own = published("evaluator", "pass", Some(&original.id), AT + 30);
    let admitted = admit(
        &catalog(),
        &[original.clone(), confirm, dispute, own],
        &PendingReleases,
        &[],
        &[],
    );
    assert!(admitted.refused.is_empty(), "{:?}", admitted.refused);
    let first = admitted
        .results
        .iter()
        .find(|record| record.publication.id == original.id)
        .unwrap();
    assert_eq!(
        first.checked,
        Checks {
            confirmed: 1,
            disputed: 1
        }
    );
    assert!(
        admitted
            .results
            .windows(2)
            .all(|pair| pair[0].at >= pair[1].at)
    );
    assert_eq!(
        admitted.results[0].checks.as_deref(),
        Some(original.id.as_str())
    );
    // One lock throughout: every result is current.
    assert!(admitted.results.iter().all(|record| record.current));
    // Releases are not read until an artifact fetcher is wired.
    let pending = admit(
        &catalog(),
        &[],
        &PendingReleases,
        std::slice::from_ref(&original),
        std::slice::from_ref(&original),
    );
    assert_eq!(pending.refused.len(), 2);
}

/// A result is current when it ran under the newest subject lock read
/// for its test set and subject: after the runner's redeploy, an older
/// result's check would run under another lock and earn nothing.
#[test]
fn a_result_from_before_the_newest_lock_is_not_current() {
    let old = published_locked("evaluator", "lock-a", AT);
    let new = published_locked("checker-a", "lock-b", AT + 100);
    let admitted = admit(
        &catalog(),
        &[old.clone(), new.clone()],
        &PendingReleases,
        &[],
        &[],
    );
    assert!(admitted.refused.is_empty(), "{:?}", admitted.refused);
    let current = |id: &str| {
        admitted
            .results
            .iter()
            .find(|record| record.publication.id == id)
            .unwrap()
            .current
    };
    assert!(!current(&old.id));
    assert!(current(&new.id));
}

/// The relay filters ask for the profile's publications and the starter
/// publishers' releases only.
#[test]
fn the_relay_filter_names_the_profile() {
    assert_eq!(
        filter(),
        json!({ "kinds": [3189], "#t": ["oa:ext-eval:v1"], "limit": MAX_RESULTS })
    );
    assert_eq!(
        suite_filter(),
        json!({
            "kinds": [3184],
            "authors": [nostr::eval_ext::hosted::RUNNER],
            "#t": ["oa:ext:release:v1"],
            "limit": MAX_SUITES,
        })
    );
}

// The starter release as the hosted runner published it on 2026-09-29:
// Project map's test set, its `3184` from the relay and its three files
// from the runner's bucket, byte for byte.
const STARTER_RELEASE: &str = include_str!("../../fixtures/gym/starter-release/release.json");
const STARTER_MANIFEST: &[u8] = include_bytes!("../../fixtures/gym/starter-release/manifest.json");
const STARTER_SUITE: &[u8] = include_bytes!("../../fixtures/gym/starter-release/suite.json");
const STARTER_CASES: &[u8] = include_bytes!("../../fixtures/gym/starter-release/cases.json");

fn starter_files() -> Fetched {
    let files = [STARTER_MANIFEST, STARTER_SUITE, STARTER_CASES]
        .into_iter()
        .map(|bytes| (digest_bytes(bytes), Arc::new(bytes.to_vec())))
        .collect();
    Fetched {
        files,
        ..Fetched::default()
    }
}

/// A signed `coder-defaults` release by `root` with one manifest whose
/// provenance cites `admissions`, and the documents by digest.
fn defaults_release(root: &str, admissions: &[Vec<u8>]) -> (Event, HashMap<String, Arc<Vec<u8>>>) {
    let package = defaults_package(&pubkey(root));
    let receipts: Vec<Value> = admissions
        .iter()
        .map(|bytes| {
            art(
                bytes,
                "application/json",
                Some(nostr::eval_ext::ADMISSION_SCHEMA),
            )
        })
        .collect();
    let manifest = jcs(&json!({
        "v": "openagents.package.v1",
        "requires": [],
        "package": package,
        "version": "1",
        "license": "CC0-1.0",
        "provenance": {
            "source": "local",
            "receipts": receipts,
            "unknowns": ["An admission records an operator's decision; it is not a security review."],
        },
        "components": [],
        "files": [],
        "dependencies": [],
    }))
    .unwrap();
    let content = json!({
        "v": 1, "requires": [], "type": "release",
        "package": package,
        "version": "1",
        "manifest": art(&manifest, "application/json", Some("openagents.package.v1")),
    });
    let release = signer(root).sign(
        AT + 10,
        nostr::ext::RELEASE_KIND,
        vec![nostr::domain::Tag::new(vec![
            "t".into(),
            "oa:ext:release:v1".into(),
        ])],
        content.to_string(),
    );
    let mut files = HashMap::new();
    files.insert(digest_bytes(&manifest), Arc::new(manifest));
    for bytes in admissions {
        files.insert(digest_bytes(bytes), Arc::new(bytes.clone()));
    }
    (release, files)
}

/// An `openagents.eval-admission.v1` document by `root` deciding
/// `decision` on the subject `<key>:project-map/repo-map`.
fn admission(root: &str, decision: &str) -> Vec<u8> {
    let issuer = pubkey(root);
    let report = art(
        b"report",
        "application/json",
        Some(nostr::kb::REPORT_SCHEMA),
    );
    jcs(&json!({
        "v": nostr::eval_ext::ADMISSION_SCHEMA,
        "requires": [],
        "subject": {
            "id": format!("{}:project-map/repo-map", pubkey("author")),
            "artifact": art(b"definition", "application/json", None),
        },
        "reports": [report.clone(), report.clone()],
        "validation": [report],
        "policy": {
            "id": format!("{issuer}:coder-defaults/policy"),
            "artifact": art(b"policy", "text/markdown", None),
        },
        "scope": art(b"scope", "application/json", None),
        "decision": decision,
        "issuer": issuer,
        "expires_at": AT + 1_000_000,
    }))
    .unwrap()
}

/// The newest coder-defaults release reads as one adoption per admit
/// decision, its subject matched to the catalog tool by slug; a release
/// another key signed, a rejecting admission, or a document not at hand is
/// refused, skipped, or asked for.
#[test]
fn an_adoption_is_read_from_the_defaults_release_and_its_admissions() {
    let tools = super::fixture_tools();
    let root = pubkey("defaults-root");
    let (release, files) = defaults_release(
        "defaults-root",
        &[
            admission("defaults-root", "admit"),
            admission("defaults-root", "reject"),
        ],
    );
    let fetched = Fetched {
        files: files.clone(),
        root: root.clone(),
    };
    let records = Fetched::adoptions(&fetched, &release, &tools).expect("the release reads");
    assert_eq!(records.len(), 1, "{records:?}");
    assert_eq!(records[0].tool.as_deref(), Some(DEFAULT_TOOL));
    assert_eq!(records[0].tool_name, "Project map");
    assert_eq!(records[0].release.id, release.id);
    assert_eq!(records[0].release.kind, 3184);
    assert_eq!(records[0].at, release.created_at);
    assert!(is_defaults_release(&release, &root));
    assert!(!is_defaults_release(&release, &pubkey("someone")));

    // Through `admit`, the adoptions join the records.
    let admitted = admit(&tools, &[], &fetched, &[], std::slice::from_ref(&release));
    assert!(admitted.refused.is_empty(), "{:?}", admitted.refused);
    assert_eq!(admitted.adoptions, records);

    // The admitted set names it as the tool's own entry.
    let set = crate::router::capability::Admitted::of(&tools, &admitted.adoptions);
    assert_eq!(
        set.entries
            .iter()
            .filter(|entry| entry.id == DEFAULT_TOOL)
            .count(),
        1
    );

    // Another root: refused, and nothing it says is read.
    let other = Fetched {
        files: files.clone(),
        root: pubkey("someone"),
    };
    assert!(Fetched::adoptions(&other, &release, &tools).is_err());
    let (forged, forged_files) = defaults_release("someone", &[admission("someone", "admit")]);
    let forged_fetched = Fetched {
        files: forged_files,
        root: root.clone(),
    };
    assert!(Fetched::adoptions(&forged_fetched, &forged, &tools).is_err());

    // A document not at hand is asked for by digest, the manifest first.
    let asked = std::cell::RefCell::new(Vec::new());
    let result = adoption_records(&release, &tools, &root, &|digest| {
        asked.borrow_mut().push(digest.to_string());
        None
    });
    assert!(
        matches!(result, Err(ReleaseError::Missing(_))),
        "{result:?}"
    );
    assert_eq!(asked.borrow().len(), 1);

    // The checked-in package record names a root the reader uses.
    assert_eq!(defaults_root().len(), 64);
    assert_eq!(Fetched::default().root, defaults_root());
}

fn starter_release() -> Event {
    serde_json::from_str(STARTER_RELEASE).expect("the release parses")
}

/// The published starter test set reads as Project map's, with its six
/// tests, and the offer names the starter catalog's reference the hosted
/// runner admits; a missing file is asked for by digest, one at a time.
#[test]
fn a_starter_release_reads_as_its_tools_test_set() {
    let release = starter_release();
    let tools = super::fixture_tools();
    let suite = Fetched::suite(&starter_files(), &release, &tools).expect("the release reads");
    assert_eq!(suite.tool.as_deref(), Some(DEFAULT_TOOL));
    assert_eq!(suite.tool_name, "Project map");
    assert_eq!(suite.cases, 6);
    assert_eq!(suite.release.id, release.id);
    assert_eq!(suite.release.kind, 3184);
    assert_eq!(suite.author, nostr::eval_ext::hosted::RUNNER);
    assert_eq!(suite.at, release.created_at);
    assert_eq!(
        suite.subject.id,
        format!(
            "{}:openagents/repo-map",
            ext_eval::author::catalog::STARTER_KEY
        )
    );
    // Every catalog tool has a reference the runner admits.
    for tool in &tools {
        assert!(catalog_definition(tool).is_some(), "{}", tool.id);
    }

    let asked = std::cell::RefCell::new(Vec::new());
    let have = [STARTER_MANIFEST];
    let result = suite_record(&release, &tools, &|digest| {
        asked.borrow_mut().push(digest.to_string());
        have.iter()
            .find(|bytes| digest_bytes(bytes) == digest)
            .map(|bytes| bytes.to_vec())
    });
    assert_eq!(
        result,
        Err(ReleaseError::Missing(digest_bytes(STARTER_SUITE)))
    );

    assert!(is_test_set_release(&release));
    let mut tool_release = release.clone();
    tool_release.content = tool_release
        .content
        .replace("project-map-tests", "project-map");
    assert!(!is_test_set_release(&tool_release));

    // Admitted through `admit`, the test set is the one `eval.run` offers.
    let admitted = admit(&tools, &[], &starter_files(), &[release], &[]);
    assert!(admitted.refused.is_empty(), "{:?}", admitted.refused);
    assert_eq!(admitted.suites, vec![suite]);
}

/// A release another key signed, a forged one, or one whose files don't
/// match its digests is refused, and nothing it says is read.
#[test]
fn a_release_that_does_not_check_is_refused() {
    let release = starter_release();
    let tools = super::fixture_tools();
    let files = starter_files();

    let other = signer("someone").sign(
        release.created_at,
        release.kind,
        release.tags.clone(),
        release.content.clone(),
    );
    let mut forged = release.clone();
    forged.content = forged
        .content
        .replace("project-map-tests", "code-finder-tests");
    let mut wrong = starter_files();
    let manifest = digest_bytes(STARTER_MANIFEST);
    wrong
        .files
        .insert(manifest, Arc::new(STARTER_SUITE.to_vec()));

    for (event, reader) in [(&other, &files), (&forged, &files), (&release, &wrong)] {
        assert!(
            matches!(
                suite_record(event, &tools, &|digest| {
                    reader.files.get(digest).map(|bytes| bytes.to_vec())
                }),
                Err(ReleaseError::Refused(_))
            ),
            "{}",
            event.id
        );
    }
    // No tool in the catalog: refused, not guessed.
    assert!(matches!(
        Fetched::suite(&files, &release, &[]),
        Err(why) if why.contains("no tool")
    ));
}

/// The starter quests name the same publishers the router reads.
#[test]
fn the_starter_publishers_are_the_quests() {
    let dir = knowledge::product::repository().join("knowledge/quests");
    let mut seen = 0;
    for name in ["project-map", "code-finder", "test-reader"] {
        let text = std::fs::read_to_string(dir.join(format!("ext-eval.{name}.json"))).unwrap();
        let quest: Value = serde_json::from_str(&text).unwrap();
        let publishers: Vec<&str> = quest["suite"]["publishers"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(Value::as_str)
            .collect();
        assert_eq!(publishers, STARTER_PUBLISHERS, "{name}");
        assert_eq!(quest["suite"]["package"], format!("{name}-tests"));
        seen += 1;
    }
    assert_eq!(seen, 3);
}

fn knowledge(judge: Arc<dyn Judge>) -> GymKnowledge<Words> {
    GymKnowledge::new(&corpus(), Words, "words", judge, None)
}

fn lookup(message: &str) -> GymLookup {
    GymLookup {
        route: crate::router::RouteId::GymNews,
        message: message.to_string(),
        transcript: vec![Message {
            role: Role::User,
            text: message.to_string(),
        }],
    }
}

/// News keeps what Jev found relevant, at most five, and the newest
/// dated records are always among the candidates.
#[tokio::test]
async fn news_is_what_jev_keeps_from_the_candidates() {
    let gym = knowledge(Arc::new(Kinds {
        kind: "build",
        relevance: 0.9,
    }));
    let news = gym.news(&lookup("what's new in the app?")).await.unwrap();
    assert!(!news.is_empty() && news.len() <= MAX_NEWS);
    assert!(
        news.iter()
            .all(|(item, p)| item.kind() == "build" && *p >= RELEVANCE_FLOOR)
    );

    let mut admitted = Admitted::default();
    for n in 0..6u8 {
        let mut result = crate::router::gym::fixtures::result(n, "project-map", 0);
        result.at = 1_790_000_000 + u64::from(n);
        admitted.results.push(result);
    }
    admitted
        .results
        .sort_by_key(|result| std::cmp::Reverse(result.at));
    gym.publish(&admitted);
    let items = gym.records().items();
    let candidates = gym.candidates("zzz unrelated words", &items).await.unwrap();
    let dated: Vec<u64> = candidates.iter().filter_map(Item::at).collect();
    assert!(
        dated.contains(&1_790_000_005),
        "the newest is a candidate: {dated:?}"
    );

    let nothing = knowledge(Arc::new(Kinds {
        kind: "build",
        relevance: 0.3,
    }));
    assert!(
        nothing
            .news(&lookup("what's new?"))
            .await
            .unwrap()
            .is_empty()
    );
    let down = knowledge(Arc::new(Down));
    assert!(matches!(
        down.news(&lookup("what's new?")).await,
        Err(SeamError::Failed(_))
    ));
}

/// The judgment reads each candidate's record text and asks one Noul per
/// candidate, over options code lists.
#[test]
fn the_questions_ask_relevance_for_each_candidate() {
    let items = crate::router::gym::fixtures::records().items();
    let questions = questions(&items);
    questions.validate().expect("valid");
    assert_eq!(questions.iter().count(), items.len());
    let state = state(&lookup("what's new?"), &items);
    assert_eq!(state["records"]["item_1"]["record"], items[0].text());
}

/// Live: the deployed relay and the runner's bucket hold the three starter
/// test sets, and a refresh reads each as its tool's, so `eval.run` offers
/// every catalog tool a test to start.
///
/// ```sh
/// cargo test -p coder --lib live_starter_test_sets -- --ignored --nocapture
/// ```
#[tokio::test]
#[ignore = "reads the production relay and the runner's bucket"]
async fn live_starter_test_sets() {
    let secret: String = Sha256::digest(format!("gym-kb-live-{:?}", std::time::SystemTime::now()))
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    let identity = crate::relay::Identity::from_text(&secret, "a throwaway key").unwrap();
    let gym = GymKnowledge::new(
        &corpus(),
        Words,
        "words",
        Arc::new(Down),
        Some(SUITE_BLOBS.to_string()),
    );
    let started = std::time::Instant::now();
    let admitted = gym
        .refresh("wss://relay.openagents.com", &identity)
        .await
        .expect("the relay reads");
    println!(
        "{} results, {} test sets, {} refused in {} ms",
        admitted.results.len(),
        admitted.suites.len(),
        admitted.refused.len(),
        started.elapsed().as_millis()
    );
    for (id, why) in &admitted.refused {
        println!("refused {}: {why}", &id[..12]);
    }
    let records = gym.records();
    for tool in &records.tools {
        let suite = records
            .suite(&tool.id)
            .expect("a test set per catalog tool");
        println!(
            "{}: {} tests, release {}",
            tool.name, suite.cases, suite.release.id
        );
        assert!(suite.cases > 0 && suite.cases <= nostr::eval_ext::HOSTED_MAX_CASES);
    }
}
