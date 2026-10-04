//! A repository turn on Windows, with a fixture model: the task store, the
//! grant, Git for Windows, the handle-relative workspace snapshot, a bash
//! command started with its script in the environment, supervision in a
//! job object, and the retained artifact read without following a link.
//!
//! The cases need Git for Windows installed for all users; a computer
//! without it skips them. A boundary turn runs in the boundary's
//! AppContainer where Windows can make one, and otherwise must refuse
//! before any command runs (Wine makes none).

use super::*;
use crate::models::{Basis, NextAction};
use coder::task::adapter::{Access, CONFIG_SCHEMA, Configuration, NAME};
use coder::task::{Action, Command, RequestedConfiguration, Store, TaskIntent, Workspace};
use std::cell::{Cell, RefCell};
use std::collections::VecDeque;

fn installed(paths: &[&str]) -> Option<PathBuf> {
    paths
        .iter()
        .find_map(|path| Path::new(path).canonicalize().ok())
}

fn git(dir: &Path, args: &[&str]) {
    let git = installed(&coder::task::owner::GIT_PATHS).expect("Git for Windows");
    let status = std::process::Command::new(git)
        .args(args)
        .current_dir(coder_boundary::plain_path(dir))
        .status()
        .unwrap();
    assert!(status.success(), "git {args:?}");
}

/// A queued task in a fresh store over an isolated worktree, and the grant
/// for it at `access`; `None` without Git for Windows.
fn fixture(access: Access) -> Option<(tempfile::TempDir, PathBuf, Vec<u8>)> {
    let Some(shell) = installed(&coder::task::owner::SYSTEM_SHELLS) else {
        eprintln!("skipped: Git for Windows is not installed");
        return None;
    };
    installed(&coder::task::owner::GIT_PATHS)?;
    let root = tempfile::tempdir().unwrap();
    let repo = root.path().join("repo");
    let checkout = root.path().join("checkout");
    std::fs::create_dir(&repo).unwrap();
    git(&repo, &["init", "-q"]);
    git(
        &repo,
        &[
            "-c",
            "user.name=Fixture",
            "-c",
            "user.email=fixture@example.invalid",
            "commit",
            "--allow-empty",
            "-qm",
            "Fixture",
        ],
    );
    git(
        &repo,
        &[
            "worktree",
            "add",
            "--detach",
            "-q",
            &checkout.display().to_string(),
        ],
    );
    let store = root.path().join("tasks");
    let command = Command {
        schema: task::COMMAND_SCHEMA.into(),
        command_id: "submit-fixture".into(),
        task_id: "fixture".into(),
        expected_revision: None,
        action: Action::Submit {
            intent: TaskIntent {
                title: "Repository fixture".into(),
                prompt: "Write result.txt containing output.".into(),
                workspace: Workspace {
                    path: checkout.canonicalize().unwrap().display().to_string(),
                    source_revision: None,
                },
                configuration: RequestedConfiguration {
                    adapter: NAME.into(),
                    model: Some("fixture-model".into()),
                },
                images: Vec::new(),
            },
        },
    };
    let mut inbox = Store::open(&store).unwrap();
    inbox.apply(&serde_json::to_vec(&command).unwrap()).unwrap();
    let task = inbox.show("fixture").unwrap();
    let grant = task::owner::Grant {
        schema: task::owner::GRANT_SCHEMA.into(),
        task_id: task.task_id,
        intent_digest: task.intent_digest,
        expected_revision: 1,
        expected_source_snapshot: None,
        program: shell,
        arguments: Vec::new(),
        write_workspace: true,
        wall_seconds: 60,
        stream_bytes: 4096,
        memory_bytes: 256 * 1024 * 1024,
        requirements: None,
        adapter_configuration: Some(Configuration {
            schema: CONFIG_SCHEMA.into(),
            provider: "synthetic".into(),
            model: "fixture-model".into(),
            effort: Some("medium".into()),
            generation_endpoint: "in-process".into(),
            decision_endpoint: "in-process".into(),
            decision_model: "fixture-judge".into(),
            max_steps: Some(4),
            acceptance: false,
            route: "never".into(),
            knowledge: "off".into(),
            dollar_limit_micros: None,
            expected_controller_digest: None,
            container: None,
            fallbacks: Vec::new(),
            access,
            studio_seat: None,
        }),
    };
    Some((root, store, serde_json::to_vec(&grant).unwrap()))
}

struct Generator {
    actions: RefCell<VecDeque<NextAction>>,
    calls: Cell<usize>,
}

fn action(commands: &[&str], finished: bool) -> NextAction {
    NextAction {
        rationale: "Fixture step.".into(),
        commands: commands.iter().map(|command| (*command).into()).collect(),
        view: Vec::new(),
        freeze_tests: false,
        expand: Vec::new(),
        finished,
        reply: String::new(),
        ask: crate::models::Ask::None,
    }
}

fn generator(commands: &[&str]) -> Generator {
    Generator {
        actions: RefCell::new(VecDeque::from([action(commands, false), action(&[], true)])),
        calls: Cell::new(0),
    }
}

impl Generate for Generator {
    async fn generate(&self, _system: &str, _prompt: &str) -> Generated {
        self.calls.set(self.calls.get() + 1);
        Generated {
            action: self
                .actions
                .borrow_mut()
                .pop_front()
                .ok_or("no more fixture actions".into()),
            model: "fixture-model".into(),
            prompt_tokens: 10,
            completion_tokens: 5,
            usd: Some(0.0),
            known_usd: 0.0,
            cost_unknown: None,
            usd_upper: Some(0.0),
            cost_basis: Basis::ListPrice,
            milliseconds: 1,
        }
    }
}

struct JudgeFixture;

impl Judge for JudgeFixture {
    async fn judge(&self, _set: &QuestionSet, _state: &Value) -> Judgment {
        Judgment::free()
    }
}

/// The script reaches bash byte for byte: quotes, backslashes, and a
/// newline that a command line would have mangled.
const SCRIPT: &str = "printf output > result.txt\nprintf '%s' \"a\\\"b\\\\c\" > quoted.txt; printf 'full command output'";

fn assert_the_turn_ran(store: &Path, root: &Path) {
    assert_eq!(
        task::artifact::read(store, "fixture", Path::new("result.txt")).unwrap(),
        b"output"
    );
    assert_eq!(
        std::fs::read(root.join("checkout").join("quoted.txt")).unwrap(),
        br#"a"b\c"#
    );
    let view = task::view::read(store, "fixture", None, 200).unwrap();
    assert_eq!(view.evidence.state, "sealed");
    assert!(
        serde_json::to_string(&view)
            .unwrap()
            .contains("full command output")
    );
}

#[tokio::test]
async fn a_full_access_turn_runs_in_git_bash_and_retains_its_artifact() {
    let Some((root, store, grant)) = fixture(Access::Full) else {
        return;
    };
    let generator = generator(&[SCRIPT]);
    let host = Host::admit(&store, &grant).await.unwrap();
    let result = run(host, &generator, &JudgeFixture).await.unwrap();
    assert_eq!(result.execution, task::Execution::Finished, "{result:?}");
    assert_eq!(generator.calls.get(), 2);
    assert!(result.run.unwrap().result.unwrap().group_clear);
    assert_the_turn_ran(&store, root.path());
    assert!(Host::admit(&store, &grant).await.is_err());
}

#[tokio::test]
async fn a_boundary_turn_runs_in_its_appcontainer_or_refuses_before_any_command() {
    runs_in_its_appcontainer_or_refuses(Access::Boundary).await;
}

/// A person's local run (`access: toolchains`, #10045) on Windows: the same
/// AppContainer with the network, and no extra reads, so it runs there or
/// refuses before any command where no AppContainer can be made (Wine),
/// never unconfined.
#[tokio::test]
async fn a_toolchain_turn_runs_in_its_appcontainer_or_refuses_before_any_command() {
    runs_in_its_appcontainer_or_refuses(Access::Toolchains).await;
}

async fn runs_in_its_appcontainer_or_refuses(access: Access) {
    let Some((root, store, grant)) = fixture(access) else {
        return;
    };
    // Outside the workspace: the container has no entry there.
    let outside = tempfile::tempdir().unwrap();
    let marker = outside.path().canonicalize().unwrap().join("outside");
    let escape = format!(
        "printf changed > '{}'",
        coder_boundary::plain_path(&marker)
            .display()
            .to_string()
            .replace('\\', "/")
    );
    let generator = generator(&[SCRIPT, &escape]);
    match Host::admit(&store, &grant).await {
        Ok(host) => {
            let result = run(host, &generator, &JudgeFixture).await.unwrap();
            assert_eq!(result.execution, task::Execution::Finished, "{result:?}");
            assert_the_turn_ran(&store, root.path());
            assert!(!marker.exists(), "a boundary command wrote outside");
        }
        Err(error) => {
            // Only a computer that cannot make an AppContainer refuses,
            // and it refuses before the task starts or any command runs.
            assert!(
                matches!(
                    coder_boundary::Boundary::readonly().build(),
                    Err(coder_boundary::Error::Inoperable { .. }
                        | coder_boundary::Error::Unavailable(_))
                ),
                "{error}"
            );
            assert!(error.to_string().contains("boundary"), "{error}");
            assert_eq!(generator.calls.get(), 0);
            let task = Store::open(&store).unwrap().show("fixture").unwrap();
            assert_eq!(task.status, task::Status::Queued);
            assert!(!root.path().join("checkout").join("result.txt").exists());
        }
    }
}
