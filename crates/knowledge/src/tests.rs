//! Entries, the lint, and search, with a fake embedder and no network.

use std::cell::Cell;
use std::path::PathBuf;

use crate::lint::{Corpus, lint};
use crate::search::{Embed, Retriever, bm25, cosine};
use crate::{Base, Entry, Kind, Status};

fn entry_text(id: &str, kind: &str, title: &str, summary: &str, tags: &str) -> String {
    format!(
        "---\nid: {id}\nversion: 1\nkind: {kind}\ntitle: {title}\nsummary: >-\n  {summary}\ntags: [{tags}]\napplies_when: >-\n  Code does this.\nstatus: admitted\nauthor: openagents\nprovenance:\n  written_from: [reference]\n  cites: [\"A Book, 2001\"]\nevidence: []\n---\n\n## Details\n\nThe body of {id}.\n"
    )
}

fn entry(id: &str, title: &str, summary: &str, tags: &str) -> Entry {
    Entry::parse(&entry_text(id, "method", title, summary, tags)).unwrap()
}

fn base() -> Base {
    Base {
        entries: vec![
            entry(
                "stats.mmd",
                "MMD estimators",
                "The biased and unbiased maximum mean discrepancy estimators.",
                "kernel, two-sample",
            ),
            entry(
                "stats.psi",
                "Population stability index",
                "PSI compares binned distributions; empty bins need an epsilon.",
                "drift, bins",
            ),
            entry(
                "shell.heredoc",
                "Heredoc quoting",
                "A quoted heredoc delimiter keeps dollar signs literal.",
                "bash, quoting",
            ),
        ],
    }
}

/// A fresh directory under the system's temporary directory.
fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("knowledge-test-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn an_entry_parses_with_its_digest() {
    let text = entry_text("a.b", "slip", "T", "S.", "x, y");
    let e = Entry::parse(&text).unwrap();
    assert_eq!(e.kind, Kind::Slip);
    assert_eq!(e.status, Status::Admitted);
    assert_eq!(e.tags, ["x", "y"]);
    assert_eq!(e.cites, ["A Book, 2001"]);
    assert!(e.body.starts_with("## Details"));
    assert!(e.digest.starts_with("sha256:"));
    let changed = Entry::parse(&text.replace("The body", "A body")).unwrap();
    assert_ne!(e.digest, changed.digest);
}

#[test]
fn a_bad_entry_says_what_is_wrong() {
    let good = entry_text("a.b", "method", "T", "S.", "x");
    let cases = [
        (good.replace("title: T\n", ""), "`title` is missing"),
        (good.replace("kind: method", "kind: fact"), "unknown kind"),
        (
            good.replace("status: admitted", "status: ok"),
            "unknown status",
        ),
        (good.replace("id: a.b", "id: A B"), "must be lowercase"),
        (
            good.replace("evidence: []", "extra: 1"),
            "unknown key `extra`",
        ),
        (
            good.split("## Details").next().unwrap().to_string(),
            "no body",
        ),
    ];
    for (text, expected) in cases {
        let error = Entry::parse(&text).unwrap_err();
        assert!(error.contains(expected), "{error}");
    }
}

#[test]
fn a_directory_reports_misnamed_and_repeated_entries() {
    let dir = scratch("read");
    std::fs::write(
        dir.join("a.b.md"),
        entry_text("a.b", "method", "T", "S.", "x"),
    )
    .unwrap();
    std::fs::write(
        dir.join("other.md"),
        entry_text("a.b", "method", "T", "S.", "x"),
    )
    .unwrap();
    std::fs::write(dir.join("broken.md"), "no front matter").unwrap();
    let (entries, problems) = Base::read(&dir);
    assert_eq!(entries.len(), 1);
    let text: Vec<String> = problems.iter().map(ToString::to_string).collect();
    assert!(
        text.iter()
            .any(|p| p.contains("broken.md: the file doesn't start"))
    );
    assert!(text.iter().any(|p| p.contains("must be named a.b.md")));
    assert!(text.iter().any(|p| p.contains("used by another file")));
    assert!(Base::load(&dir, false).is_err());
}

#[test]
fn candidates_are_hidden_unless_asked_for() {
    let dir = scratch("status");
    let text = entry_text("a.b", "method", "T", "S.", "x").replace("admitted", "candidate");
    std::fs::write(dir.join("a.b.md"), text).unwrap();
    assert!(Base::load(&dir, false).unwrap().entries.is_empty());
    assert_eq!(Base::load(&dir, true).unwrap().entries.len(), 1);
}

#[test]
fn bm25_ranks_the_entry_that_names_the_term() {
    let scores = bm25(&base().entries, "the mmd statistic of two samples");
    assert!(scores[0] > scores[1] && scores[0] > scores[2], "{scores:?}");
    assert_eq!(bm25(&base().entries, "zzz")[0], 0.0);
}

#[test]
fn cosine_of_a_zero_vector_is_zero() {
    assert_eq!(cosine(&[0.0, 0.0], &[1.0, 0.0]), 0.0);
    assert!((cosine(&[1.0, 1.0], &[2.0, 2.0]) - 1.0).abs() < 1e-9);
}

/// Embeds text by whether it holds each of three words, and counts calls
/// and inputs.
struct Fake {
    calls: Cell<usize>,
    inputs: Cell<usize>,
    fail: bool,
}

impl Fake {
    fn new(fail: bool) -> Self {
        Fake {
            calls: Cell::new(0),
            inputs: Cell::new(0),
            fail,
        }
    }
}

impl Embed for Fake {
    fn model(&self) -> &str {
        "fake"
    }

    async fn embed(&self, inputs: Vec<String>) -> Result<(Vec<Vec<f32>>, f64), String> {
        self.calls.set(self.calls.get() + 1);
        self.inputs.set(self.inputs.get() + inputs.len());
        if self.fail {
            return Err("no network".to_string());
        }
        let vectors = inputs
            .iter()
            .map(|text| {
                let text = text.to_lowercase();
                ["kernel", "bins", "bash"]
                    .iter()
                    .map(|w| if text.contains(w) { 1.0 } else { 0.1 })
                    .collect()
            })
            .collect();
        Ok((vectors, 0.001))
    }
}

#[tokio::test]
async fn embeddings_lift_an_entry_that_shares_no_word_with_the_query() {
    // "shell script" names no word of the heredoc entry's search text, but
    // the fake embeds "bash" close to it.
    let retriever = Retriever::new(base(), Fake::new(false), None);
    let lexical = Retriever::<Fake>::lexical(base(), "test");
    let with = retriever.search("a bash shell script", 3).await;
    assert_eq!(with.hits[0].id, "shell.heredoc");
    assert!(with.lexical_only.is_none());
    assert!(with.hits[0].semantic.is_some());
    assert!((with.usd - 0.001).abs() < 1e-12);
    let without = lexical.search("a bash shell script", 3).await;
    assert_eq!(without.lexical_only.as_deref(), Some("test"));
    assert!(without.hits.iter().all(|h| h.semantic.is_none()));
}

#[tokio::test]
async fn entry_vectors_are_cached_on_disk_by_digest() {
    let path = scratch("cache").join("embeddings.json");
    let first = Retriever::new(base(), Fake::new(false), Some(path.clone()));
    first.search("kernel", 3).await;
    // Three entries and the query, in one call.
    let fake = second_embedder(&first);
    assert_eq!((fake.calls.get(), fake.inputs.get()), (1, 4));
    assert!(path.exists());
    // A new retriever reads the cache and embeds only the query.
    let second = Retriever::new(base(), Fake::new(false), Some(path));
    second.search("bins", 3).await;
    let fake = second_embedder(&second);
    assert_eq!((fake.calls.get(), fake.inputs.get()), (1, 1));
    // The same query again costs nothing.
    let again = second.search("bins", 3).await;
    assert_eq!(fake.calls.get(), 1);
    assert_eq!(again.usd, 0.0);
}

fn second_embedder(retriever: &Retriever<Fake>) -> &Fake {
    retriever.embedder().unwrap()
}

#[tokio::test]
async fn a_failed_embeddings_call_falls_back_to_words() {
    let retriever = Retriever::new(base(), Fake::new(true), None);
    let search = retriever.search("mmd", 2).await;
    assert_eq!(search.hits.len(), 2);
    assert_eq!(search.hits[0].id, "stats.mmd");
    assert!(
        search
            .lexical_only
            .unwrap()
            .contains("the embeddings call failed: no network")
    );
}

#[test]
fn the_lint_refuses_a_task_name_a_quote_and_a_missing_citation() {
    let tasks = scratch("corpus");
    std::fs::create_dir_all(tasks.join("drift-watch/tests")).unwrap();
    std::fs::create_dir_all(tasks.join("unrelated/tests")).unwrap();
    std::fs::write(
        tasks.join("drift-watch/tests/test_it.py"),
        "def test_x():\n    assert value   ==   compute_the_statistic(sample_one, sample_two)\n",
    )
    .unwrap();
    let corpus = Corpus::read(&[tasks.clone(), tasks.join("absent")]);
    assert_eq!(corpus.names, ["drift-watch", "unrelated"]);
    assert_eq!(corpus.absent.len(), 1);
    let clean = entry(
        "a.clean",
        "Clean",
        "Says nothing about drift-watchers.",
        "x",
    );
    let named = entry("a.named", "Named", "Seen in drift-watch.", "x");
    let mut quoted = entry("a.quoted", "Quoted", "S.", "x");
    quoted.body = "assert value == compute_the_statistic(sample_one, sample_two)".to_string();
    let mut uncited = entry("a.uncited", "Uncited", "S.", "x");
    uncited.cites.clear();
    let problems = lint(&[clean, named, quoted, uncited], &corpus);
    let text: Vec<String> = problems.iter().map(ToString::to_string).collect();
    assert_eq!(problems.len(), 3, "{text:?}");
    assert!(text[0].starts_with("a.named: it names the benchmark task drift-watch"));
    assert!(text[1].starts_with("a.uncited: it cites no source"));
    assert!(text[2].starts_with("a.quoted: it shares"));
}

#[test]
fn the_seed_entries_parse_and_pass_the_lint_without_a_corpus() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../knowledge");
    let (entries, problems) = Base::read(&dir);
    assert!(problems.is_empty(), "{problems:?}");
    assert!(entries.len() >= 8, "{} entries", entries.len());
    for id in ["statistics.mmd-estimators", "slip.comments-in-broken-code"] {
        assert!(entries.iter().any(|e| e.id == id), "{id} is missing");
    }
    assert!(lint(&entries, &Corpus::default()).is_empty());
}

#[tokio::test]
async fn the_mmd_entry_ranks_first_for_an_mmd_query_by_words_alone() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../knowledge");
    let retriever = Retriever::<Fake>::lexical(Base::load(&dir, false).unwrap(), "test");
    let search = retriever
        .search("mmd kernel two-sample estimator rbf", 3)
        .await;
    assert_eq!(search.hits[0].id, "statistics.mmd-estimators");
}

fn args(words: &[&str]) -> Vec<String> {
    words.iter().map(|w| (*w).to_string()).collect()
}

async fn kb(words: &[String]) -> u8 {
    crate::cli::main(words).await
}

#[tokio::test]
async fn the_cli_adds_admits_measures_and_withdraws_an_entry() {
    let dir = scratch("cli");
    let d = dir.to_str().unwrap();
    assert_eq!(
        kb(&args(&[
            "add",
            "numerics.kahan",
            "--kind",
            "method",
            "--title",
            "Kahan summation",
            "--dir",
            d
        ]))
        .await,
        0
    );
    let path = dir.join("numerics.kahan.md");
    assert!(Entry::parse(&std::fs::read_to_string(&path).unwrap()).is_ok());
    // A second add of the same ID refuses; a bad kind is bad usage.
    assert_eq!(
        kb(&args(&[
            "add",
            "numerics.kahan",
            "--kind",
            "method",
            "--title",
            "T",
            "--dir",
            d
        ]))
        .await,
        1
    );
    assert_eq!(
        kb(&args(&[
            "add",
            "numerics.x",
            "--kind",
            "fact",
            "--title",
            "T",
            "--dir",
            d
        ]))
        .await,
        2
    );
    // The template's placeholders fail the lint.
    assert_eq!(kb(&args(&["lint", "--dir", d, "--corpus", d])).await, 1);

    // Measured with no runs: inconclusive, so --evidence refuses.
    let runs = scratch("cli-runs");
    let evidence = scratch("cli-evidence");
    let (r, e) = (runs.to_str().unwrap(), evidence.to_str().unwrap());
    assert_eq!(
        kb(&args(&[
            "evidence",
            "--dir",
            d,
            "--runs",
            r,
            "--evidence-dir",
            e,
            "--attach"
        ]))
        .await,
        0
    );
    assert!(evidence.join("numerics.kahan.v1.json").exists());
    let entry = Entry::parse(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert!(
        entry.evidence[0].starts_with("measured "),
        "{:?}",
        entry.evidence
    );
    assert_eq!(
        kb(&args(&[
            "admit",
            "numerics.kahan",
            "--evidence",
            "--dir",
            d,
            "--evidence-dir",
            e
        ]))
        .await,
        1
    );
    assert_eq!(kb(&args(&["admit", "numerics.kahan", "--dir", d])).await, 2);

    // Admitted by review, then withdrawn; the evidence keeps both lines.
    assert_eq!(
        kb(&args(&[
            "admit",
            "numerics.kahan",
            "--reviewer",
            "A. Person",
            "--dir",
            d
        ]))
        .await,
        0
    );
    let entry = Entry::parse(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(entry.status, Status::Admitted);
    assert!(entry.evidence[1].ends_with("by review: A. Person"));
    assert_eq!(
        kb(&args(&[
            "withdraw",
            "numerics.kahan",
            "--reason",
            "wrong sign",
            "--dir",
            d
        ]))
        .await,
        0
    );
    let entry = Entry::parse(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(entry.status, Status::Withdrawn);
    assert!(entry.evidence[2].ends_with(": wrong sign"));
    assert_eq!(kb(&args(&["review", "--dir", d, "--runs", r])).await, 0);
    assert_eq!(kb(&args(&["publish", "--dir", d])).await, 2);
}
