//! Knowledge drafts on scratch homes: no model, no Jev, no relay, and no
//! real key. The owner's key in the publish test is a throwaway.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use knowledge::remote::{self, Trust, TrustConfig};

use super::fake::{Fixed, Same};
use super::*;

const SRC: &str = include_str!("agent_share.rs");

fn store(dir: &tempfile::TempDir) -> Store {
    let store = Store::new(&dir.path().join("host"), "alice").unwrap();
    store.open(dir.path(), 1).unwrap();
    store
}

fn reply(title: &str, details: &str) -> String {
    serde_json::json!({
        "kind": "environment",
        "title": title,
        "summary": "A crate that is its own Cargo workspace isn't reachable with `cargo test -p` from the root; build it with `--manifest-path`.",
        "applies_when": "A Rust repository holds a crate with its own [workspace] table, excluded from the root workspace.",
        "tags": ["cargo", "workspace", "Cargo Manifest"],
        "details": details,
        "check": "Run `cargo metadata --no-deps` at the root: the crate isn't among the packages.",
    })
    .to_string()
}

const TITLE: &str = "A nested Cargo workspace needs --manifest-path";
const DETAILS: &str =
    "`cargo test -p NAME` from the root fails with `package ID specification did not match`.";

struct Run {
    judged: Arc<AtomicUsize>,
    wrote: Arc<AtomicUsize>,
}

fn fakes(general: f64, about_owner: f64, text: String) -> (Services, Run) {
    let run = Run {
        judged: Arc::new(AtomicUsize::new(0)),
        wrote: Arc::new(AtomicUsize::new(0)),
    };
    let services = Services {
        writer: Box::new(Same {
            text,
            calls: run.wrote.clone(),
        }),
        judge: Box::new(Fixed {
            general,
            about_owner,
            calls: run.judged.clone(),
        }),
        corpus: Corpus::default(),
    };
    (services, run)
}

/// Two journal rows and a stored insight that cites them.
fn lesson(store: &Store) -> (Memory, u64, Vec<usize>) {
    for (at, kind, text) in [
        (
            100,
            Kind::Ran,
            "cargo test -p openagents-mobile: package ID specification did not match",
        ),
        (
            110,
            Kind::Ran,
            "cargo test --manifest-path crates/openagents-mobile/Cargo.toml passed",
        ),
    ] {
        store.append(&Entry::new(at, kind, text)).unwrap();
    }
    let rows: Vec<usize> = store
        .journal_rows()
        .unwrap()
        .iter()
        .rev()
        .take(2)
        .map(|(p, _)| *p)
        .rev()
        .collect();
    let memory = Memory::new(store.clone(), secret_screen::Screen::shapes());
    let id = memory
        .add(
            MemoryKind::Insight,
            Author::Agent,
            "The mobile crate is its own workspace; build it with --manifest-path.",
            rows.iter().map(|p| format!("journal:{p}")).collect(),
            120,
        )
        .unwrap();
    (memory, id, rows)
}

fn journal_map(store: &Store) -> Vec<(usize, Entry)> {
    store.journal_rows().unwrap()
}

fn kept(shared: &Shared) -> &str {
    match &shared.outcomes[0] {
        Outcome::Kept { why, .. } => why,
        Outcome::Drafted(draft) => panic!("drafted {}", draft.entry.id),
    }
}

#[test]
fn a_general_lesson_becomes_a_draft_that_passes_the_lint_and_the_screen_and_cites_real_rows() {
    let dir = tempfile::tempdir().unwrap();
    let store = store(&dir);
    let (memory, id, rows) = lesson(&store);
    let (mut services, run) = fakes(0.9, 0.05, reply(TITLE, DETAILS));
    let (shared, written) = memory.share(&mut services, &[id], 200).unwrap();
    assert_eq!(run.judged.load(Ordering::SeqCst), 1);
    assert_eq!(run.wrote.load(Ordering::SeqCst), 1);
    assert_eq!(shared.usd(), Some(0.01));
    let draft = shared.drafts().next().expect("a draft");
    assert_eq!(draft.rows, rows);
    assert_eq!(
        draft.entry.id,
        "alice.a-nested-cargo-workspace-needs-manifest-path"
    );
    assert_eq!(draft.entry.kind, knowledge::Kind::Environment);
    assert_eq!(draft.entry.status, knowledge::Status::Candidate);
    assert_eq!(draft.entry.tags, ["cargo", "cargo-manifest", "workspace"]);
    assert_eq!(draft.entry.written_from, [format!("alice memory:{id}")]);

    // The file on disk is the draft, and it passes every check again.
    assert_eq!(
        written,
        [drafts_dir(&store).join(format!("{}.md", draft.entry.id))]
    );
    let text = std::fs::read_to_string(&written[0]).unwrap();
    assert_eq!(text, draft.text);
    let parsed = knowledge::Entry::parse(&text).unwrap();
    assert!(lint(std::slice::from_ref(&parsed), &Corpus::default()).is_empty());
    assert!(secret_screen::Screen::shapes().check(&text).is_ok());
    let rows_now = journal_map(&store);
    let journal: HashMap<usize, &Entry> = rows_now.iter().map(|(p, e)| (*p, e)).collect();
    check_draft(
        &text,
        "alice",
        &journal,
        &Corpus::default(),
        &secret_screen::Screen::shapes(),
    )
    .unwrap();
    // Every citation names a row that exists, and only those rows.
    let cited: Vec<usize> = parsed
        .cites
        .iter()
        .map(|c| parse_cite(c).unwrap().1)
        .collect();
    assert_eq!(cited, rows);
    assert!(cited.iter().all(|p| journal.contains_key(p)));

    // The draft and the run are journaled, and the owner sees it.
    let lines: Vec<String> = store
        .journal(20)
        .unwrap()
        .into_iter()
        .map(|e| e.text)
        .collect();
    let drafted = lines
        .iter()
        .find(|l| l.starts_with("drafted knowledge entry alice.a-nested"))
        .expect("the draft is journaled");
    assert!(
        drafted.contains(&format!("from insight entry {id}")),
        "{drafted}"
    );
    assert!(drafted.contains("nothing is published until the owner runs: microcoder kb publish"));
    assert!(
        lines
            .last()
            .unwrap()
            .starts_with("knowledge drafting: model fake-writer; cost $0.0100; drafted 1, kept 0")
    );
    let rows = draft_rows(&store).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].kind, "environment");
    assert!(rows[0].publish.starts_with("microcoder kb publish --dir "));
    assert!(
        rows[0]
            .publish
            .ends_with(&format!("--relay RELAY {}", rows[0].id))
    );

    // A second pass doesn't draft the same insight again.
    let (mut again, run) = fakes(0.9, 0.05, reply(TITLE, DETAILS));
    let (shared, written) = memory.share(&mut again, &[id], 300).unwrap();
    assert_eq!(kept(&shared), "it is drafted already");
    assert!(written.is_empty());
    assert_eq!(run.judged.load(Ordering::SeqCst), 0);
}

#[test]
fn a_fact_about_the_owner_is_never_drafted() {
    let dir = tempfile::tempdir().unwrap();
    let store = store(&dir);
    let (memory, id, rows) = lesson(&store);

    // Jev reads it as about the owner: no draft, and no writer call.
    let (mut services, run) = fakes(0.95, 0.8, reply(TITLE, DETAILS));
    let (shared, written) = memory.share(&mut services, &[id], 200).unwrap();
    assert!(
        kept(&shared).starts_with("it is about the owner (Jev 0.80)"),
        "{}",
        kept(&shared)
    );
    assert!(written.is_empty());
    assert_eq!(run.wrote.load(Ordering::SeqCst), 0);

    // An insight resting on what the owner told her stays private before
    // Jev is asked.
    let note = memory
        .add(
            MemoryKind::Note,
            Author::Owner,
            "I work late on Fridays.",
            Vec::new(),
            130,
        )
        .unwrap();
    let on_note = memory
        .add(
            MemoryKind::Insight,
            Author::Agent,
            "Builds run late on Fridays.",
            vec![format!("journal:{}", rows[0]), format!("memory:{note}")],
            140,
        )
        .unwrap();
    let (mut services, run) = fakes(0.95, 0.0, reply(TITLE, DETAILS));
    let (shared, _) = memory.share(&mut services, &[on_note], 200).unwrap();
    assert_eq!(
        kept(&shared),
        format!("it rests on memory:{note}, a note about the owner")
    );
    assert_eq!(run.judged.load(Ordering::SeqCst), 0);

    // A preference, even an accepted one, is never a draft.
    let preference = memory
        .add(
            MemoryKind::Preference,
            Author::Owner,
            "Use short imperative commit messages.",
            Vec::new(),
            150,
        )
        .unwrap();
    let (mut services, run) = fakes(0.95, 0.0, reply(TITLE, DETAILS));
    let (shared, _) = memory.share(&mut services, &[preference], 200).unwrap();
    assert!(
        kept(&shared).contains("only a stored insight is drafted"),
        "{}",
        kept(&shared)
    );
    assert_eq!(run.judged.load(Ordering::SeqCst), 0);

    // A lesson under the gate isn't drafted either.
    let (mut services, run) = fakes(0.4, 0.0, reply(TITLE, DETAILS));
    let (shared, _) = memory.share(&mut services, &[id], 200).unwrap();
    assert!(kept(&shared).starts_with("it isn't a general lesson (Jev 0.40, under 0.70)"));
    assert_eq!(run.wrote.load(Ordering::SeqCst), 0);
    assert!(read_drafts(&drafts_dir(&store)).is_empty());
    let lines: Vec<String> = store
        .journal(40)
        .unwrap()
        .into_iter()
        .map(|e| e.text)
        .collect();
    assert!(lines.iter().any(|l| l.starts_with(&format!(
        "did not draft insight entry {id} as a knowledge entry: it is about the owner"
    ))));
}

#[test]
fn a_draft_cites_only_journal_rows_that_exist_and_hold_what_was_cited() {
    let dir = tempfile::tempdir().unwrap();
    let store = store(&dir);
    let (memory, _, rows) = lesson(&store);
    // An insight citing a row that doesn't exist isn't drafted.
    let ghost = memory
        .add(
            MemoryKind::Insight,
            Author::Agent,
            "Nested workspaces need --manifest-path.",
            vec![format!("journal:{}", rows[0]), "journal:999".into()],
            130,
        )
        .unwrap();
    let (mut services, _) = fakes(0.9, 0.0, reply(TITLE, DETAILS));
    let (shared, _) = memory.share(&mut services, &[ghost], 200).unwrap();
    assert_eq!(
        kept(&shared),
        "it rests on journal:999, which doesn't exist"
    );

    // A draft file is checked against the journal as it is now.
    let journal_rows = journal_map(&store);
    let journal: HashMap<usize, &Entry> = journal_rows.iter().map(|(p, e)| (*p, e)).collect();
    let written = parse_written(&reply(TITLE, DETAILS)).unwrap();
    let behind: Vec<(usize, &Entry)> = rows.iter().map(|p| (*p, journal[p])).collect();
    let entry = build("alice", 1, &written, &behind).unwrap();
    let screen = secret_screen::Screen::shapes();
    let check = |entry: &knowledge::Entry| {
        check_draft(
            &entry.render(),
            "alice",
            &journal,
            &Corpus::default(),
            &screen,
        )
    };
    assert!(check(&entry).is_ok());
    let mut missing = entry.clone();
    missing.cites[0] = missing.cites[0].replace(&format!("journal:{} ", rows[0]), "journal:999 ");
    assert_eq!(
        check(&missing).unwrap_err(),
        "it cites journal:999, which doesn't exist"
    );
    let mut changed = entry.clone();
    let row = journal[&rows[1]];
    changed.cites[0] = cite("alice", rows[0], row);
    assert!(
        check(&changed)
            .unwrap_err()
            .ends_with("which isn't what that row holds")
    );
    let mut other = entry.clone();
    other.cites[0] = other.cites[0].replacen("alice", "bob", 1);
    assert!(
        check(&other)
            .unwrap_err()
            .contains("another agent's journal")
    );
    let mut loose = entry.clone();
    loose.cites.push("the Cargo book".into());
    assert!(
        check(&loose)
            .unwrap_err()
            .contains("isn't a journal row citation")
    );
    let mut none = entry.clone();
    none.cites.clear();
    assert!(
        check(&none)
            .unwrap_err()
            .starts_with("the lint refused it: it cites no source")
    );
    let mut admitted = entry;
    admitted.status = knowledge::Status::Admitted;
    assert_eq!(check(&admitted).unwrap_err(), "a draft is a candidate");
}

#[test]
fn the_lint_and_the_secret_screen_refuse_a_draft() {
    let dir = tempfile::tempdir().unwrap();
    let store = store(&dir);
    let (memory, id, _) = lesson(&store);
    // A draft that names a benchmark task fails the lint.
    let (mut services, _) = fakes(0.9, 0.0, reply(TITLE, "This bit fix-git once."));
    services.corpus = Corpus {
        names: vec!["fix-git".into()],
        ..Corpus::default()
    };
    let (shared, written) = memory.share(&mut services, &[id], 200).unwrap();
    assert_eq!(
        kept(&shared),
        "the lint refused it: it names the benchmark task fix-git"
    );
    assert!(written.is_empty());
    // A draft that carries a credential fails the screen.
    let token = format!("ghp_{}", "Ab1".repeat(12));
    let (mut services, _) = fakes(0.9, 0.0, reply(TITLE, &format!("Use {token}.")));
    let (shared, _) = memory.share(&mut services, &[id], 200).unwrap();
    assert!(
        kept(&shared).starts_with("the secret screen refused it"),
        "{}",
        kept(&shared)
    );
    // A reply of another kind isn't an entry.
    let method = reply(TITLE, DETAILS).replace("\"environment\"", "\"method\"");
    let (mut services, _) = fakes(0.9, 0.0, method);
    let (shared, _) = memory.share(&mut services, &[id], 200).unwrap();
    assert_eq!(
        kept(&shared),
        "`method` isn't environment, edge-case, or slip"
    );
    assert!(read_drafts(&drafts_dir(&store)).is_empty());
}

/// Every file under `dir`, relative.
fn files(dir: &Path) -> BTreeSet<PathBuf> {
    fn walk(root: &Path, dir: &Path, out: &mut BTreeSet<PathBuf>) {
        for entry in std::fs::read_dir(dir).unwrap().flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(root, &path, out);
            } else {
                out.insert(path.strip_prefix(root).unwrap().to_path_buf());
            }
        }
    }
    let mut out = BTreeSet::new();
    walk(dir, dir, &mut out);
    out
}

#[test]
fn no_draft_is_published_without_the_owners_command() {
    // Drafting has no key and no relay to publish with.
    for word in [
        "RelaySigner",
        "Identity",
        "coder::relay",
        "crate::relay",
        "kb::entry(",
        "entry_event",
        ".sign(",
        "secret_key",
        "knowledge-key",
    ] {
        assert!(
            !SRC.contains(word),
            "agent_share.rs mentions `{word}`, which publishing needs"
        );
    }
    let dir = tempfile::tempdir().unwrap();
    let store = store(&dir);
    let (memory, id, _) = lesson(&store);
    let before = files(dir.path());
    let (mut services, _) = fakes(0.9, 0.0, reply(TITLE, DETAILS));
    let (shared, written) = memory.share(&mut services, &[id], 200).unwrap();
    let draft = shared.drafts().next().unwrap();
    // The only new file is the draft, in her drafts directory.
    let added: Vec<PathBuf> = files(dir.path()).difference(&before).cloned().collect();
    assert_eq!(
        added,
        [written[0].strip_prefix(dir.path()).unwrap().to_path_buf()]
    );
    assert!(written[0].starts_with(drafts_dir(&store)));

    // The owner's command publishes it: the drafts directory is a `--dir`
    // the knowledge base reads, and the file is a NIP-KB entry once the
    // owner's key signs it. That key is a throwaway here.
    let (entries, problems) = knowledge::Base::read(&drafts_dir(&store));
    assert!(problems.is_empty(), "{problems:?}");
    assert_eq!(entries.len(), 1);
    let owner = nostr::domain::RelaySigner::from_secret_hex(&"11".repeat(32)).unwrap();
    let parts = remote::entry_event(&draft.text).unwrap();
    let event = owner.sign(1_790_000_000, parts.kind, parts.tags, parts.content);

    // Another reader syncs it. Trusting only itself, it doesn't see it;
    // trusting all, it sees a candidate, which only with-and-without
    // evidence admits.
    let sync = remote::accept(&[event], &Corpus::default());
    assert_eq!(sync.accepted.len(), 1, "{:?}", sync.refused);
    let reader = tempfile::tempdir().unwrap();
    let local = reader.path().join("entries");
    std::fs::create_dir_all(&local).unwrap();
    let cache = reader.path().join("remote");
    remote::write_cache(&cache, &sync).unwrap();
    let own = TrustConfig::default();
    let (base, _) = remote::load(&local, Some(&cache), &own, None, true).unwrap();
    assert!(base.get(&draft.entry.id).is_none());
    let all = TrustConfig {
        mode: Trust::All,
        authors: Vec::new(),
    };
    let (base, _) = remote::load(&local, Some(&cache), &all, None, false).unwrap();
    assert!(base.get(&draft.entry.id).is_none());
    let (base, _) = remote::load(&local, Some(&cache), &all, None, true).unwrap();
    let seen = base.get(&draft.entry.id).unwrap();
    assert_eq!(seen.status, knowledge::Status::Candidate);
    assert_eq!(seen.author, remote::npub(owner.pubkey()));
}

#[test]
fn the_share_set_and_its_request() {
    let set = share_set();
    assert_eq!(set.id, "openagents.insight-share.v1");
    assert_eq!(set.gate, GENERAL);
    assert!((threshold(GENERAL) - 0.7).abs() < 1e-9);
    assert!((threshold(ABOUT_OWNER) - 0.3).abs() < 1e-9);
    let record = Record {
        reference: Ref::Journal(4),
        body: Body::Journal(Entry::new(40, Kind::Ran, "cargo test exited 101")),
    };
    assert!(judge_request("alice", "Tests fail.", &[&record]).is_ok());
    assert_eq!(
        slug("  The --manifest-path flag!  "),
        "the-manifest-path-flag"
    );
    assert_eq!(slug(&"word ".repeat(40)).len(), SLUG_MAX - 1);
    assert_eq!(
        parse_cite("alice journal:12 2026-10-05 sha256:0123456789abcdef"),
        Some(("alice".into(), 12, "0123456789abcdef".into()))
    );
    assert_eq!(parse_cite("alice journal:x 2026-10-05 sha256:00"), None);
    assert!(parse_written("no json").is_err());
    assert!(
        parse_written(&reply(TITLE, " "))
            .unwrap_err()
            .contains("details")
    );
}
