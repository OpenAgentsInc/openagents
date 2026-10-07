//! The artifact queue over a scratch repository and a bare remote, with
//! stand-in regenerate and check commands.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use super::*;
use crate::{Broker, Holder, Limits, Request, Resource, Wait};

const LIMITS: Limits = Limits {
    build: 2,
    memory_gib: 96,
    disk_floor_gb: 10,
    build_disk_gb: 0,
};

/// The stand-in regenerate command: the digest is the checksum of the
/// sources, the "pack" is `out/<digest>.bin`, and it prints the pin line.
const REGENERATE: &str = "d=$(cat src/*.txt | cksum | cut -d' ' -f1); rm -f out/*.bin; \
    echo \"$d\" > \"out/$d.bin\"; echo \"pub const DIGEST: &str = \\\"$d\\\";\"";
/// The stand-in check: the pin matches the sources, the pack is there,
/// and no source says BROKEN.
const CHECK: &str = "d=$(cat src/*.txt | cksum | cut -d' ' -f1); \
    grep -q \"DIGEST: &str = \\\"$d\\\"\" pin.rs && test -f \"out/$d.bin\" && \
    ! grep -q BROKEN src/*.txt";

struct Scratch {
    _dir: tempfile::TempDir,
    remote: PathBuf,
    work: PathBuf,
    root: PathBuf,
    env: Vec<(OsString, OsString)>,
}

impl Scratch {
    fn new() -> Scratch {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().join("home");
        std::fs::create_dir_all(&home).unwrap();
        let config = home.join(".gitconfig");
        std::fs::write(
            &config,
            "[user]\n\tname = Queue Test\n\temail = queue@example.invalid\n[init]\n\tdefaultBranch = main\n[advice]\n\tdetachedHead = false\n",
        )
        .unwrap();
        let env = vec![
            ("HOME".into(), home.clone().into_os_string()),
            ("GIT_CONFIG_GLOBAL".into(), config.into_os_string()),
            ("GIT_CONFIG_NOSYSTEM".into(), "1".into()),
            ("GIT_TERMINAL_PROMPT".into(), "0".into()),
        ];
        let scratch = Scratch {
            remote: dir.path().join("remote.git"),
            work: dir.path().join("work"),
            root: dir.path().join("leases"),
            env,
            _dir: dir,
        };
        scratch.git(
            Path::new("."),
            &[
                "init",
                "--quiet",
                "--bare",
                "-b",
                "main",
                scratch.remote.to_str().unwrap(),
            ],
        );
        scratch.git(
            Path::new("."),
            &[
                "clone",
                "--quiet",
                scratch.remote.to_str().unwrap(),
                scratch.work.to_str().unwrap(),
            ],
        );
        let registry = serde_json::json!({
            "schema": ARTIFACT_SCHEMA,
            "name": "demo-pack",
            "description": "A stand-in pack.",
            "regenerate": REGENERATE,
            "check": CHECK,
            "pinned": ["out/*.bin"],
            "pin": {
                "file": "pin.rs",
                "lines": ["pub const DIGEST: &str = "],
                "history": {"from": "pub const DIGEST: &str = ", "after": "    DIGEST,", "end": "];"}
            },
            "message": "Repin the demo pack with {changes}"
        });
        scratch.write(
            "artifacts/demo-pack.json",
            &serde_json::to_string_pretty(&registry).unwrap(),
        );
        scratch.write("src/a.txt", "alpha\n");
        scratch.write("src/b.txt", "beta\n");
        scratch.write(
            "pin.rs",
            "pub const DIGEST: &str = \"0\";\nconst HISTORY: &[&str] = &[\n    DIGEST,\n];\n",
        );
        std::fs::create_dir_all(scratch.work.join("out")).unwrap();
        scratch.sh(REGENERATE_AND_PIN);
        scratch.commit_all("Start the demo pack");
        scratch.git(&scratch.work, &["push", "--quiet", "origin", "main"]);
        scratch
    }

    fn git(&self, dir: &Path, args: &[&str]) -> String {
        let output = Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(args)
            .envs(self.env.iter().cloned())
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8_lossy(&output.stdout).into_owned()
    }

    fn sh(&self, script: &str) {
        let status = Command::new("sh")
            .arg("-c")
            .arg(script)
            .current_dir(&self.work)
            .envs(self.env.iter().cloned())
            .status()
            .unwrap();
        assert!(status.success(), "{script}");
    }

    fn write(&self, path: &str, text: &str) {
        let path = self.work.join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    fn commit_all(&self, message: &str) {
        self.git(&self.work, &["add", "--all"]);
        self.git(&self.work, &["commit", "--quiet", "-m", message]);
    }

    /// A branch off `main` that writes `text` to `path` and repins on its
    /// own, as agents do today.
    fn branch(&self, name: &str, path: &str, text: &str) {
        self.git(
            &self.work,
            &["checkout", "--quiet", "-b", name, "origin/main"],
        );
        self.write(path, text);
        self.sh(REGENERATE_AND_PIN);
        self.commit_all(&format!("Change {path} on {name}"));
        self.git(
            &self.work,
            &["checkout", "--quiet", "--detach", "origin/main"],
        );
    }

    fn options(&self) -> Options {
        let mut options = Options::new(self.work.clone());
        options.env.clone_from(&self.env);
        options
    }

    fn broker(&self) -> Broker {
        Broker::new(self.root.clone(), LIMITS).with_poll(Duration::from_millis(10))
    }

    fn submit(&self, branch: &str, summary: &str) -> Submission {
        submit(
            &self.root,
            "demo-pack",
            Some(branch),
            Some(summary),
            "test:1",
            &self.options(),
        )
        .unwrap()
    }

    /// `main` on the remote: its subjects, newest first, and a file.
    fn remote_log(&self) -> Vec<String> {
        self.git(&self.remote, &["log", "--format=%s", "main"])
            .lines()
            .map(str::to_owned)
            .collect()
    }

    fn remote_file(&self, path: &str) -> String {
        self.git(&self.remote, &["show", &format!("main:{path}")])
    }
}

/// Regenerates and writes the pin by hand, as the agents' own repin does.
const REGENERATE_AND_PIN: &str = "d=$(cat src/*.txt | cksum | cut -d' ' -f1); rm -f out/*.bin; \
    echo \"$d\" > \"out/$d.bin\"; \
    sed \"s/^pub const DIGEST: &str = .*/pub const DIGEST: \\&str = \\\"$d\\\";/\" pin.rs > pin.tmp && mv pin.tmp pin.rs";

fn digest_of(sources: &[&str]) -> String {
    let output = Command::new("sh")
        .arg("-c")
        .arg("printf '%s' \"$1\" | cksum | cut -d' ' -f1")
        .arg("sh")
        .arg(sources.concat())
        .output()
        .unwrap();
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

#[test]
fn two_submissions_that_both_repin_land_as_one_repin() {
    let scratch = Scratch::new();
    let original = scratch.remote_file("pin.rs");
    let original_digest = digest_of(&["alpha\n", "beta\n"]);
    assert!(original.contains(&original_digest));
    scratch.branch("add-a", "src/a.txt", "alpha\nand more alpha\n");
    scratch.branch("add-b", "src/b.txt", "beta\nand more beta\n");
    let first = scratch.submit("add-a", "the longer alpha");
    let second = scratch.submit("add-b", "the longer beta");

    let outcome = run(&scratch.broker(), "demo-pack", &scratch.options()).unwrap();
    let Outcome::Ran { landed, rejected } = outcome else {
        panic!("the queue was busy");
    };
    assert!(rejected.is_empty(), "{rejected:?}");
    assert_eq!(
        landed.iter().map(|s| s.id.clone()).collect::<Vec<_>>(),
        [first.id.clone(), second.id.clone()]
    );

    let log = scratch.remote_log();
    assert_eq!(
        log[..3],
        [
            "Repin the demo pack with the longer alpha and the longer beta".to_owned(),
            "Change src/b.txt on add-b".to_owned(),
            "Change src/a.txt on add-a".to_owned(),
        ]
    );
    assert_eq!(
        log.iter()
            .filter(|subject| subject.starts_with("Repin"))
            .count(),
        1
    );
    // The change commits carry no pin of their own; the repin carries it.
    let change_files = scratch.git(
        &scratch.remote,
        &["show", "--name-only", "--format=", "main~1"],
    );
    assert_eq!(change_files.trim(), "src/b.txt");
    let digest = digest_of(&["alpha\nand more alpha\n", "beta\nand more beta\n"]);
    let pin = scratch.remote_file("pin.rs");
    assert!(
        pin.contains(&format!("DIGEST: &str = \"{digest}\";")),
        "{pin}"
    );
    assert!(
        pin.contains(&format!("    \"{original_digest}\",")),
        "{pin}"
    );
    let packs = scratch.git(&scratch.remote, &["ls-tree", "--name-only", "main", "out/"]);
    assert_eq!(packs.trim(), format!("out/{digest}.bin"));

    let all = Queue::new(&scratch.root, "demo-pack").list().unwrap();
    assert!(all.iter().all(|s| s.status == Status::Landed));
    let head = scratch.git(&scratch.remote, &["rev-parse", "main"]);
    assert_eq!(all[0].landed.as_deref(), Some(head.trim()));
    // The refs that kept the commits alive are gone.
    let refs = scratch.git(&scratch.work, &["for-each-ref", REF_PREFIX]);
    assert!(refs.trim().is_empty(), "{refs}");
}

#[test]
fn a_conflicting_submission_comes_back_with_its_reason() {
    let scratch = Scratch::new();
    scratch.branch("first", "src/a.txt", "alpha, first\n");
    scratch.branch("second", "src/a.txt", "alpha, second\n");
    scratch.branch("third", "src/b.txt", "beta, third\n");
    scratch.submit("first", "the first alpha");
    let second = scratch.submit("second", "the second alpha");
    scratch.submit("third", "the third beta");

    let Outcome::Ran { landed, rejected } =
        run(&scratch.broker(), "demo-pack", &scratch.options()).unwrap()
    else {
        panic!("the queue was busy");
    };
    assert_eq!(landed.len(), 2);
    assert_eq!(rejected.len(), 1);
    assert_eq!(rejected[0].id, second.id);
    let reason = rejected[0].reason.as_deref().unwrap();
    assert!(reason.contains("conflicts"), "{reason}");
    assert!(reason.contains("src/a.txt"), "{reason}");
    let stored = Queue::new(&scratch.root, "demo-pack")
        .get(&second.id)
        .unwrap()
        .unwrap();
    assert_eq!(stored.status, Status::Rejected);
    assert_eq!(stored.reason.as_deref(), Some(reason));
    assert_eq!(
        scratch.remote_log()[0],
        "Repin the demo pack with the first alpha and the third beta"
    );
    assert_eq!(scratch.remote_file("src/a.txt"), "alpha, first\n");
}

#[test]
fn a_change_that_fails_the_check_is_rejected_and_the_rest_land() {
    let scratch = Scratch::new();
    scratch.branch("good", "src/a.txt", "alpha, good\n");
    scratch.branch("bad", "src/b.txt", "beta BROKEN\n");
    scratch.submit("good", "the good alpha");
    let bad = scratch.submit("bad", "the broken beta");

    let Outcome::Ran { landed, rejected } =
        run(&scratch.broker(), "demo-pack", &scratch.options()).unwrap()
    else {
        panic!("the queue was busy");
    };
    assert_eq!(landed.len(), 1);
    assert_eq!(rejected.len(), 1);
    assert_eq!(rejected[0].id, bad.id);
    assert!(
        rejected[0]
            .reason
            .as_deref()
            .unwrap()
            .contains("the check failed"),
        "{:?}",
        rejected[0].reason
    );
    assert_eq!(
        scratch.remote_log()[0],
        "Repin the demo pack with the good alpha"
    );
    assert_eq!(scratch.remote_file("src/b.txt"), "beta\n");
}

#[test]
fn the_queue_lists_pending_changes_in_order() {
    let scratch = Scratch::new();
    scratch.branch("one", "src/a.txt", "alpha one\n");
    scratch.branch("two", "src/b.txt", "beta two\n");
    let one = scratch.submit("one", "the first");
    let two = scratch.submit("two", "the second");
    let queues = queues(&scratch.root).unwrap();
    assert_eq!(queues.len(), 1);
    assert_eq!(queues[0].name(), "demo-pack");
    let pending = queues[0].pending().unwrap();
    assert_eq!(
        pending
            .iter()
            .map(|s| (s.id.as_str(), s.branch.as_str(), s.summary.as_str()))
            .collect::<Vec<_>>(),
        [
            (one.id.as_str(), "one", "the first"),
            (two.id.as_str(), "two", "the second")
        ]
    );
    assert!(pending.iter().all(|s| s.status == Status::Pending));
    // Each submission keeps its commit alive under a ref.
    let refs = scratch.git(
        &scratch.work,
        &["for-each-ref", "--format=%(refname)", REF_PREFIX],
    );
    assert_eq!(refs.lines().count(), 2);
}

#[test]
fn a_held_lease_means_another_runner_applies_the_change() {
    let scratch = Scratch::new();
    scratch.branch("one", "src/a.txt", "alpha one\n");
    scratch.submit("one", "the first");
    let broker = scratch.broker();
    let holder = Holder {
        session: "other:1".to_owned(),
        agent: "none".to_owned(),
        pid: std::process::id(),
        command: "artifact".to_owned(),
    };
    let lease = broker
        .acquire(Request::new(Resource::Artifact("demo-pack".to_owned()), holder).wait(Wait::No))
        .unwrap();
    let outcome = run(&broker, "demo-pack", &scratch.options()).unwrap();
    assert!(
        matches!(&outcome, Outcome::Busy { session, .. } if session == "other:1"),
        "{outcome:?}"
    );
    assert_eq!(
        Queue::new(&scratch.root, "demo-pack")
            .pending()
            .unwrap()
            .len(),
        1
    );
    drop(lease);
}

#[test]
fn submitting_refuses_unknown_artifacts_and_empty_branches() {
    let scratch = Scratch::new();
    let options = scratch.options();
    let error = submit(
        &scratch.root,
        "nope",
        Some("main"),
        None,
        "test:1",
        &options,
    )
    .unwrap_err();
    assert!(error.contains("not a queue-managed artifact"), "{error}");
    scratch.git(&scratch.work, &["branch", "--quiet", "same", "origin/main"]);
    let error = submit(
        &scratch.root,
        "demo-pack",
        Some("same"),
        None,
        "test:1",
        &options,
    )
    .unwrap_err();
    assert!(error.contains("no commits"), "{error}");
}

fn pin() -> Pin {
    Pin {
        file: "pack.rs".to_owned(),
        lines: vec![
            "pub const PACK_SHA256: &str = ".to_owned(),
            "pub const PACK_BYTES: u64 = ".to_owned(),
        ],
        history: Some(History {
            from: "pub const PACK_SHA256: &str = ".to_owned(),
            after: "    PACK_SHA256,".to_owned(),
            end: "];".to_owned(),
        }),
    }
}

const PACK_RS: &str = "pub const PACK_SHA256: &str = \"aaa\";\npub const PACK_BYTES: u64 = 1;\nconst EVERGLADE_PACK_HISTORY: &[&str] = &[\n    PACK_SHA256,\n    \"old\",\n];\n";

#[test]
fn a_pin_takes_the_printed_lines_and_keeps_the_earlier_digest() {
    let output =
        "wrote x\npub const PACK_SHA256: &str = \"bbb\";\npub const PACK_BYTES: u64 = 2;\n";
    let next = apply_pin(&pin(), PACK_RS, output).unwrap();
    assert_eq!(
        next,
        "pub const PACK_SHA256: &str = \"bbb\";\npub const PACK_BYTES: u64 = 2;\nconst EVERGLADE_PACK_HISTORY: &[&str] = &[\n    PACK_SHA256,\n    \"aaa\",\n    \"old\",\n];\n"
    );
    // The same output again changes nothing.
    assert_eq!(apply_pin(&pin(), &next, output).unwrap(), next);
    assert!(apply_pin(&pin(), PACK_RS, "nothing printed").is_err());
}

#[test]
fn a_reset_pin_puts_back_the_base_lines_and_history() {
    let change = "pub const PACK_SHA256: &str = \"ccc\";\npub const PACK_BYTES: u64 = 3;\nconst EVERGLADE_PACK_HISTORY: &[&str] = &[\n    PACK_SHA256,\n    \"aaa\",\n    \"old\",\n];\nfn added() {}\n";
    let reset = reset_pin(&pin(), change, PACK_RS);
    assert_eq!(reset, format!("{PACK_RS}fn added() {{}}\n"));
}

#[test]
fn pinned_patterns_match_within_one_segment() {
    assert!(matches(
        "assets/verse/everglade/*.vtp",
        "assets/verse/everglade/abc.vtp"
    ));
    assert!(!matches(
        "assets/verse/everglade/*.vtp",
        "assets/verse/everglade/x/abc.vtp"
    ));
    assert!(!matches(
        "assets/verse/everglade/*.vtp",
        "assets/verse/everglade/abc.glb"
    ));
    assert!(matches(
        "assets/verse/grid/pack.json",
        "assets/verse/grid/pack.json"
    ));
    assert_eq!(
        join_summaries(&["a".into(), "b".into(), "c".into()]),
        "a, b, and c"
    );
}

#[test]
fn the_repository_registry_admits_its_entries() {
    let top = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for name in ["everglade-pack", "grid-pack"] {
        let artifact = Artifact::load(&top, name).unwrap();
        assert!(!artifact.pinned.is_empty(), "{name}");
        if let Some(pin) = &artifact.pin {
            let source = std::fs::read_to_string(top.join(&pin.file)).unwrap();
            for prefix in &pin.lines {
                assert!(
                    source.lines().any(|line| line.starts_with(prefix.as_str())),
                    "{name}: {} has no `{prefix}`",
                    pin.file
                );
            }
            if let Some(history) = &pin.history {
                assert!(source.lines().any(|line| line == history.after), "{name}");
            }
        }
    }
}
