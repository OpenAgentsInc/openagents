use std::collections::BTreeMap;
use std::ffi::OsString;
use std::process::Command;

use coder_host::Tasks as _;

use super::super::super::{Store, remote};
use super::super::{
    NewGoal, PLAN_SCHEMA, PlanOutcome, Repository, Role, Seat, Studio, parse_route,
};
use super::*;

/// A private scratch directory: the task store, the host root, the
/// person's checkout, and its `origin` all live under it, never under the
/// real home.
fn private_dir() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    }
    dir
}

fn git(dir: &Path, args: &[&str]) -> String {
    let output = local::git().arg("-C").arg(dir).args(args).output().unwrap();
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

struct Scratch {
    dir: tempfile::TempDir,
    store: PathBuf,
    root: PathBuf,
    repo: PathBuf,
    origin: PathBuf,
}

/// A checkout on `main` with one commit, pushed to a bare `origin`.
fn scratch() -> Scratch {
    let dir = private_dir();
    let repo = dir.path().join("repo");
    let origin = dir.path().join("origin.git");
    git(
        dir.path(),
        &["init", "-q", "--bare", &origin.to_string_lossy()],
    );
    git(
        dir.path(),
        &["init", "-q", "-b", "main", &repo.to_string_lossy()],
    );
    git(&repo, &["config", "user.name", "Owner Person"]);
    git(&repo, &["config", "user.email", "owner@example.invalid"]);
    git(&repo, &["config", "commit.gpgsign", "false"]);
    std::fs::write(repo.join("README.md"), "hello\n").unwrap();
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "First"]);
    git(
        &repo,
        &["remote", "add", "origin", &origin.to_string_lossy()],
    );
    git(&repo, &["push", "-q", "origin", "main"]);
    Scratch {
        store: dir.path().join("tasks"),
        root: dir.path().join("host"),
        repo,
        origin,
        dir,
    }
}

fn seat(name: &str, role: Role, route: &str, desk: u32) -> Seat {
    Seat {
        name: name.into(),
        role,
        route: parse_route(route).unwrap(),
        look: "default".into(),
        desk,
    }
}

/// Git in `worktree` as a studio task's process runs it: a cleared
/// environment confined to seat `seat`.
fn as_seat(seat: &str, worktree: &Path, home: &Path) -> Command {
    let mut variables: Vec<(OsString, OsString)> = vec![
        ("PATH".into(), "/usr/bin:/bin".into()),
        ("HOME".into(), home.into()),
    ];
    confine(&mut variables, seat, worktree);
    let mut command = local::git();
    command.env_clear().envs(variables).arg("-C").arg(worktree);
    command
}

fn succeeds(command: &mut Command) {
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "{command:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn names_follow_the_seat_and_the_task() {
    assert_eq!(slug("Write the --verbose flag!"), "write-the-verbose-flag");
    assert_eq!(slug("¿?"), "task");
    assert!(slug(&"long words ".repeat(10)).len() <= SLUG_MAX);
    assert_eq!(
        branch("ada", &"ab".repeat(32), "Parse the flag"),
        "studio/ada/abababab-parse-the-flag"
    );
    assert_eq!(
        identity("ada"),
        ("Studio Ada".to_owned(), "ada@studio.invalid".to_owned())
    );
}

#[test]
fn confinement_appends_after_existing_configuration_and_drops_redirects() {
    let mut variables: Vec<(OsString, OsString)> = vec![
        ("GIT_CONFIG_COUNT".into(), "1".into()),
        ("GIT_CONFIG_KEY_0".into(), "remote.origin.url".into()),
        ("GIT_CONFIG_VALUE_0".into(), "/elsewhere".into()),
        ("GIT_DIR".into(), "/somewhere/.git".into()),
        ("GIT_CEILING_DIRECTORIES".into(), "/outer".into()),
        ("GIT_AUTHOR_NAME".into(), "Someone".into()),
    ];
    confine(&mut variables, "ada", Path::new("/work/trees/task"));
    let value = |name: &str| {
        let found: Vec<&OsString> = variables
            .iter()
            .filter(|(key, _)| key == name)
            .map(|(_, value)| value)
            .collect();
        assert!(found.len() <= 1, "{name} is set twice");
        found
            .first()
            .map(|value| value.to_string_lossy().into_owned())
    };
    assert_eq!(value("GIT_DIR"), None);
    assert_eq!(value("GIT_CONFIG_COUNT").as_deref(), Some("8"));
    assert_eq!(
        value("GIT_CONFIG_KEY_0").as_deref(),
        Some("remote.origin.url")
    );
    assert_eq!(value("GIT_CONFIG_KEY_1").as_deref(), Some("protocol.allow"));
    assert_eq!(value("GIT_CONFIG_VALUE_1").as_deref(), Some("never"));
    assert_eq!(value("GIT_ALLOW_PROTOCOL").as_deref(), Some(NO_PROTOCOL));
    assert_eq!(value("GIT_AUTHOR_NAME").as_deref(), Some("Studio Ada"));
    assert_eq!(
        value("GIT_CEILING_DIRECTORIES").as_deref(),
        Some("/work/trees:/outer")
    );
}

/// The acceptance flow (#10542): two seats' tasks in separate worktrees,
/// a review of each diff, a merge that fast-forwards a clean checkout and
/// is refused on a dirty one, and a push from inside a task that fails.
#[test]
fn two_seats_work_apart_review_and_merge_locally_without_pushing() {
    let s = scratch();
    let first = git(&s.repo, &["rev-parse", "HEAD"]);
    let mut tasks = Store::open(&s.store).unwrap();
    let mut studio = Studio::open(&s.store)
        .unwrap()
        .with_host_root(&s.root)
        .with_worktrees(worktrees_dir(&s.root));
    studio
        .set_seat(seat("lead", Role::Lead, "codex:gpt-6-luna", 0))
        .unwrap();
    studio
        .set_seat(seat("ada", Role::Worker, "claude:claude-opus-5-5", 1))
        .unwrap();
    studio
        .set_seat(seat("grace", Role::Worker, "codex:gpt-6-luna", 2))
        .unwrap();
    let (goal, _) = studio
        .submit_goal(
            &mut tasks,
            NewGoal {
                text: "Add two notes.".into(),
                repository: Repository {
                    label: "demo".into(),
                    path: s.repo.to_string_lossy().into_owned(),
                },
                lead: None,
            },
            1_000,
        )
        .unwrap();
    let plan = serde_json::json!({
        "schema": PLAN_SCHEMA,
        "tasks": [
            {"id": "left", "title": "Write the left note", "seat": "ada"},
            {"id": "right", "title": "Write the right note", "seat": "grace"},
        ],
    })
    .to_string();
    let PlanOutcome::Accepted { released } = studio
        .accept_plan(&mut tasks, &goal, plan.as_bytes(), 1_001)
        .unwrap()
    else {
        panic!("the plan is valid");
    };
    assert_eq!(released.len(), 2);

    // Each task has its own worktree and branch under the host's state,
    // from the checkout's commit, and works there.
    let home = s.dir.path().join("home");
    std::fs::create_dir_all(&home).unwrap();
    let mut worktrees = Vec::new();
    for (item, file) in released.iter().zip(["left.txt", "right.txt"]) {
        let record = local::record(&s.store, &item.task_id).expect("a run record");
        let worktree = PathBuf::from(&record.worktree);
        assert!(worktree.starts_with(worktrees_dir(&s.root)));
        assert_eq!(record.base, first);
        assert_eq!(
            Path::new(&record.checkout).canonicalize().unwrap(),
            s.repo.canonicalize().unwrap()
        );
        assert_eq!(
            tasks.show(&item.task_id).unwrap().intent.workspace.path,
            record.worktree
        );
        assert_eq!(
            seat_of(&s.store, &item.task_id).as_deref(),
            Some(item.seat.as_str())
        );
        let on = git(&worktree, &["symbolic-ref", "--short", "HEAD"]);
        assert!(on.starts_with(&format!("studio/{}/", item.seat)), "{on}");

        std::fs::write(worktree.join(file), format!("{}\n", item.seat)).unwrap();
        succeeds(as_seat(&item.seat, &worktree, &home).args(["add", "-A"]));
        succeeds(as_seat(&item.seat, &worktree, &home).args([
            "commit",
            "-q",
            "-m",
            "Write a note",
        ]));
        let (name, email) = identity(&item.seat);
        assert_eq!(
            git(&worktree, &["log", "-1", "--format=%an <%ae>"]),
            format!("{name} <{email}>")
        );

        // A push from inside the task fails, however it names the remote.
        let origin = s.origin.to_string_lossy().into_owned();
        let url = format!("file://{origin}");
        for args in [
            vec!["push", "origin", "HEAD:refs/heads/leak"],
            vec!["push", origin.as_str(), "HEAD:refs/heads/leak"],
            vec!["push", url.as_str(), "HEAD:refs/heads/leak"],
            vec![
                "-c",
                "protocol.allow=always",
                "push",
                origin.as_str(),
                "HEAD:refs/heads/leak",
            ],
        ] {
            let pushed = as_seat(&item.seat, &worktree, &home)
                .args(&args)
                .output()
                .unwrap();
            assert!(!pushed.status.success(), "{args:?} pushed");
        }
        assert!(git(&s.origin, &["for-each-ref", "refs/heads/leak"]).is_empty());
        worktrees.push((item.task_id.clone(), worktree, file));
    }
    assert_ne!(worktrees[0].1, worktrees[1].1);
    drop(studio);
    drop(tasks);

    // The review shows each task's own diff.
    let inbox = remote::Inbox::new(
        &s.store,
        BTreeMap::from([("demo".to_owned(), s.repo.clone())]),
    );
    let mut reviewed = Vec::new();
    for (task, _, file) in &worktrees {
        let review = inbox.review(task).unwrap();
        assert_eq!(review.base, first);
        assert_eq!(review.files.len(), 1, "{:?}", review.files);
        assert_eq!(review.files[0].path, *file);
        assert!(review.diff.contains(file));
        reviewed.push(coder_host::Reviewed {
            base: review.base,
            head_commit: review.head_commit,
            head: review.head,
        });
    }
    let principal = coder_host::Principal {
        device: "d".repeat(64),
        grant: None,
        epoch: None,
    };

    // Merge fast-forwards the clean checkout's branch to a merge commit
    // the person authored; nothing is pushed.
    let merged = inbox
        .publish(&principal, &worktrees[0].0, &reviewed[0])
        .unwrap();
    assert_eq!(merged.state, PublishState::Published, "{}", merged.note);
    assert_eq!(merged.landing, Landing::Branch);
    assert_eq!(merged.branch.as_deref(), Some("main"));
    let commit = merged.commit.clone().unwrap();
    assert_eq!(git(&s.repo, &["rev-parse", "HEAD"]), commit);
    assert!(s.repo.join("left.txt").is_file());
    assert_eq!(
        git(&s.repo, &["log", "-1", "--format=%an <%ae>"]),
        "Owner Person <owner@example.invalid>"
    );
    assert_eq!(
        git(&s.repo, &["log", "-1", "--format=%P"])
            .split_whitespace()
            .count(),
        2
    );
    assert_eq!(git(&s.origin, &["rev-parse", "refs/heads/main"]), first);
    // A retry answers with the same merge.
    let again = inbox
        .publish(&principal, &worktrees[0].0, &reviewed[0])
        .unwrap();
    assert_eq!(again, merged);

    // A dirty checkout refuses the merge with the reason, and nothing moves.
    std::fs::write(s.repo.join("README.md"), "edited\n").unwrap();
    let refused = inbox
        .publish(&principal, &worktrees[1].0, &reviewed[1])
        .unwrap();
    assert_eq!(refused.state, PublishState::Refused);
    assert!(
        refused.note.contains("uncommitted changes"),
        "{}",
        refused.note
    );
    assert_eq!(git(&s.repo, &["rev-parse", "HEAD"]), commit);
    assert!(!s.repo.join("right.txt").exists());
    let shown = inbox.review(&worktrees[1].0).unwrap();
    assert_eq!(shown.publication.unwrap().state, PublishState::Refused);

    // Once the checkout is clean, the same decision merges.
    git(&s.repo, &["checkout", "--", "README.md"]);
    let second = inbox
        .publish(&principal, &worktrees[1].0, &reviewed[1])
        .unwrap();
    assert_eq!(second.state, PublishState::Published, "{}", second.note);
    assert!(s.repo.join("left.txt").is_file());
    assert!(s.repo.join("right.txt").is_file());
    assert_eq!(git(&s.origin, &["rev-parse", "refs/heads/main"]), first);
}

#[test]
fn a_conflicting_merge_is_refused_and_leaves_the_checkout() {
    let s = scratch();
    let worktrees = worktrees_dir(&s.root);
    let task = "c".repeat(64);
    let worktree = prepare(
        &worktrees,
        &s.store,
        &s.repo,
        "ada",
        &task,
        "Edit the readme",
        None,
        None,
    )
    .unwrap();
    // Preparing again keeps the same worktree.
    assert_eq!(
        prepare(&worktrees, &s.store, &s.repo, "ada", &task, "Edit", None, None).unwrap(),
        worktree
    );
    let home = s.dir.path().join("home");
    std::fs::create_dir_all(&home).unwrap();
    std::fs::write(worktree.join("README.md"), "from the task\n").unwrap();
    succeeds(as_seat("ada", &worktree, &home).args(["commit", "-q", "-am", "Edit"]));
    std::fs::write(s.repo.join("README.md"), "from the person\n").unwrap();
    git(&s.repo, &["commit", "-q", "-am", "Edit too"]);
    let before = git(&s.repo, &["rev-parse", "HEAD"]);
    let head = review::head(&worktree).unwrap();
    let record = local::record(&s.store, &task).unwrap();
    let reviewed = Reviewed {
        base: record.base,
        head_commit: head.commit,
        head: head.tree,
    };
    let refused = merge(&s.store, &task, &reviewed).unwrap();
    assert_eq!(refused.state, PublishState::Refused);
    assert!(refused.note.contains("README.md"), "{}", refused.note);
    assert_eq!(git(&s.repo, &["rev-parse", "HEAD"]), before);
    assert!(git(&s.repo, &["status", "--porcelain"]).is_empty());
}

/// The workshop agent's task mode: a direct request is a one-task goal for
/// her seat, released at once into a worktree of her own, whose change
/// the person reviews and merges; nothing is pushed.
#[test]
fn a_direct_request_works_in_its_own_worktree_and_merges_locally() {
    use super::super::direct::Direct;
    let s = scratch();
    let first = git(&s.repo, &["rev-parse", "HEAD"]);
    let mut tasks = Store::open(&s.store).unwrap();
    let mut studio = Studio::open(&s.store)
        .unwrap()
        .with_host_root(&s.root)
        .with_worktrees(worktrees_dir(&s.root));
    studio
        .set_seat(seat("alice", Role::Worker, "codex:gpt-6-luna", 3))
        .unwrap();
    let (goal, task, released) = studio
        .submit_direct(
            &mut tasks,
            Direct {
                text: "Add a notes file.".into(),
                title: "Add a notes file".into(),
                repository: Repository {
                    label: "demo".into(),
                    path: s.repo.to_string_lossy().into_owned(),
                },
                seat: "alice".into(),
            },
            2_000,
        )
        .unwrap();
    assert_eq!(released.len(), 1);
    assert_eq!(released[0].task_id, task);
    assert_eq!(released[0].seat, "alice");
    let state = studio.state().goal(&goal).unwrap().clone();
    assert!(state.planned);
    assert_eq!(
        state.lead.task_id, task,
        "the goal's progress is her task's"
    );
    assert_eq!(studio.direct_task(&goal).unwrap().0, task);
    let record = local::record(&s.store, &task).expect("a run record");
    let worktree = PathBuf::from(&record.worktree);
    assert!(worktree.starts_with(worktrees_dir(&s.root)));
    let on = git(&worktree, &["symbolic-ref", "--short", "HEAD"]);
    assert!(on.starts_with("studio/alice/"), "{on}");
    // A paused seat takes no direct request.
    studio.pause_seat("alice").unwrap();
    assert!(
        studio
            .submit_direct(
                &mut tasks,
                Direct {
                    text: "Another.".into(),
                    title: "Another".into(),
                    repository: state.repository.clone(),
                    seat: "alice".into(),
                },
                2_001,
            )
            .is_err()
    );
    drop(studio);
    drop(tasks);
    let home = s.dir.path().join("home");
    std::fs::create_dir_all(&home).unwrap();
    std::fs::write(worktree.join("notes.txt"), "alice\n").unwrap();
    succeeds(as_seat("alice", &worktree, &home).args(["add", "-A"]));
    succeeds(as_seat("alice", &worktree, &home).args(["commit", "-q", "-m", "Add notes"]));
    let inbox = remote::Inbox::new(
        &s.store,
        BTreeMap::from([("demo".to_owned(), s.repo.clone())]),
    );
    let binding = inbox.terminal_binding(&task).unwrap();
    assert!(binding.interactive);
    assert_eq!(binding.directory, worktree.canonicalize().unwrap());
    assert!(inbox.terminal_binding("missing-task").is_err());
    let record_path = s.store.join("local").join(format!("{task}.json"));
    let record_bytes = std::fs::read(&record_path).unwrap();
    let mut record: serde_json::Value = serde_json::from_slice(&record_bytes).unwrap();
    record["shape"]["read_only"] = serde_json::json!(true);
    std::fs::write(&record_path, serde_json::to_vec(&record).unwrap()).unwrap();
    assert!(!inbox.terminal_binding(&task).unwrap().interactive);
    std::fs::write(&record_path, record_bytes).unwrap();
    let review = inbox.review(&task).unwrap();
    assert_eq!(review.base, first);
    let principal = coder_host::Principal {
        device: "d".repeat(64),
        grant: None,
        epoch: None,
    };
    // A concurrent shell edit changes the review identity and cannot land under the old review.
    let frozen = coder_host::Reviewed {
        base: review.base.clone(),
        head_commit: review.head_commit.clone(),
        head: review.head.clone(),
    };
    std::fs::write(worktree.join("notes.txt"), "edited by the person\n").unwrap();
    assert_ne!(inbox.review(&task).unwrap().head, frozen.head);
    let refused = inbox.publish(&principal, &task, &frozen).unwrap();
    assert_eq!(refused.state, PublishState::Refused);
    assert!(!s.repo.join("notes.txt").exists());
    std::fs::write(worktree.join("notes.txt"), "alice\n").unwrap();
    // Her change's tip is signed as her when it merges.
    let key = secp256k1::SecretKey::from_byte_array([5; 32]).unwrap();
    set_seat_signer(
        &s.store,
        Arc::new(move |seat: &str, repo: &Path, commit: &str| {
            (seat == "alice")
                .then(|| super::super::super::agent_git_sign::sign_commit(repo, commit, &key, None))
        }),
    );
    let merged = inbox
        .publish(
            &principal,
            &task,
            &coder_host::Reviewed {
                base: review.base,
                head_commit: review.head_commit,
                head: review.head,
            },
        )
        .unwrap();
    assert_eq!(merged.state, PublishState::Published, "{}", merged.note);
    assert!(s.repo.join("notes.txt").is_file());
    let tip = git(&s.repo, &["rev-parse", "HEAD^2"]);
    let signed = super::super::super::agent_git_sign::verify_commit(&s.repo, &tip).unwrap();
    assert_eq!(signed.pubkey, crate::task::agent::public_hex(&key));
    assert_eq!(git(&s.origin, &["rev-parse", "refs/heads/main"]), first);
}

/// A remote task's change lands at the merge decision, and a failed one
/// closes with its reason (#10930).
#[test]
fn a_remote_tasks_change_lands_and_a_failed_one_closes() {
    use super::super::direct::Direct;
    use super::super::flow::Stage;
    let s = scratch();
    let mut tasks = Store::open(&s.store).unwrap();
    let mut studio = Studio::open(&s.store)
        .unwrap()
        .with_host_root(&s.root)
        .with_worktrees(worktrees_dir(&s.root));
    studio
        .set_seat(seat("alice", Role::Worker, "devin:default", 0))
        .unwrap();
    let direct = |text: &str| Direct {
        text: text.into(),
        title: "The work".into(),
        repository: Repository {
            label: "demo".into(),
            path: s.repo.to_string_lossy().into_owned(),
        },
        seat: "alice".into(),
    };
    // One placed task lands its patch at the merge decision.
    let (_goal, task) = studio
        .submit_remote(direct("Add notes.txt"), "coderos-4080", "HEAD", 1_000)
        .unwrap();
    studio.attach_remote_task(&task, "rt-1").unwrap();
    let works = studio.remote_tasks("alice");
    assert_eq!(works.len(), 1);
    assert_eq!(works[0].computer, "coderos-4080");
    assert_eq!(works[0].remote_task.as_deref(), Some("rt-1"));
    assert_eq!(works[0].stage, Stage::Work);
    let patch = "diff --git a/notes.txt b/notes.txt\nnew file mode 100644\nindex \
                 0000000..ce01362\n--- /dev/null\n+++ b/notes.txt\n@@ -0,0 +1 @@\n+hello\n";
    let worktree = studio.land_remote(&task, patch).unwrap();
    assert!(worktree.join("notes.txt").exists());
    assert_eq!(git(&worktree, &["show", "HEAD:notes.txt"]), "hello");
    let works = studio.remote_tasks("alice");
    assert_eq!(works[0].stage, Stage::Merge);
    // A second remote task that fails is closed with its reason, and its
    // note waits for the person.
    let (_goal, task) = studio
        .submit_remote(direct("Add more"), "coderos-4080", "HEAD", 1_001)
        .unwrap();
    studio.attach_remote_task(&task, "rt-2").unwrap();
    studio
        .fail_remote(&task, "the task on coderos-4080 failed")
        .unwrap();
    let works = studio.remote_tasks("alice");
    assert_eq!(works[1].stage, Stage::Rejected);
    // A task that is not a remote one is refused.
    assert!(studio.land_remote("d9-unknown.work", patch).is_err());
    assert!(studio.fail_remote("d9-unknown.work", "x").is_err());
    let _ = tasks;
}
