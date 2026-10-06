//! An explicitly selected, persisted, free scratch practice owner.
use super::*;
use coder::task::{
    Store,
    studio::{NewGoal, Repository, Role, Seat, Studio},
};
use openagents_chat::{
    plugin_flow::{Flow, Step},
    plugin_workbench::{Declarations, Owner, Source},
};
use serde_json::json;
struct DraftOnly;
impl openagents_chat::plugin_workbench::Engine for DraftOnly {
    fn tests(&self, _: &Path) -> Result<Vec<openagents_chat::plugin_flow::Test>, String> {
        Ok(vec![openagents_chat::plugin_flow::Test {
            name: "greeting".into(),
            kind: "should-fire".into(),
            task: "Inspect the fixed greeting in the simulated starter".into(),
        }])
    }
    fn run(
        &self,
        _: &[String],
        _: std::time::Duration,
    ) -> Result<openagents_chat::client::Ran, String> {
        Err("Practice retains authored metadata and executes no plugin command".into())
    }
}
fn mkdir(path: &Path) -> Result<(), String> {
    let mut builder = std::fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(path).map_err(|e| e.to_string())
}
pub(super) fn write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    use std::io::Write;
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path).map_err(|e| e.to_string())?;
    file.write_all(bytes)
        .and_then(|_| file.sync_all())
        .map_err(|e| e.to_string())
}
pub(super) fn replace(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if bytes.len() > 1024 * 1024 {
        return Err("Retained practice evidence exceeds its bound".into());
    }
    let temporary = path.with_extension("new");
    write(&temporary, bytes)?;
    std::fs::rename(&temporary, path).map_err(|e| e.to_string())
}
fn commit(repo: &Path, message: &str) -> Result<String, String> {
    git(repo, &["add", "."])?;
    git(
        repo,
        &[
            "-c",
            "user.name=Onboarding practice",
            "-c",
            "user.email=practice@example.invalid",
            "commit",
            "-qm",
            message,
        ],
    )?;
    git(repo, &["rev-parse", "HEAD"])
}
/// Prepare a new starter explicitly. Reading an existing starter never resets it.
pub fn starter(root: &Path, lane: Lane) -> Result<Config, String> {
    if !root.is_absolute() {
        return Err("Starter requires an explicit absolute scratch directory".into());
    }
    if root.join("config.json").is_file() {
        let config = Config::load(&root.join("config.json"))?;
        if config.scope.lane != lane {
            return Err("Starter belongs to another lane".into());
        }
        config.book()?;
        return Ok(config);
    }
    if root.exists()
        && std::fs::read_dir(root)
            .map_err(|e| e.to_string())?
            .next()
            .is_some()
    {
        return Err(
            "Starter requires a new empty scratch directory; incomplete work is never replayed"
                .into(),
        );
    }
    mkdir(root)?;
    let root = root.canonicalize().map_err(|e| e.to_string())?;
    let snapshot = Snapshot {
        stream: digest(&root.display().to_string()),
        sequence: 1,
        view: Default::default(),
    };
    let scope = Scope {
        host: instance(&snapshot),
        workspace: "starter".into(),
        lane,
    };
    let config = Config {
        scope: scope.clone(),
        starter: root.clone(),
        book: root.join("progress.json"),
        tasks: root.join("tasks"),
    };
    mkdir(&root.join("repo/src"))?;
    mkdir(&root.join("home"))?;
    write(&root.join("repo/Cargo.toml"),b"[package]\nname = \"onboarding-starter\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[workspace]\n")?;
    write(&root.join("repo/src/lib.rs"),b"pub fn greeting() -> &'static str { \"Hello\" }\n#[cfg(test)] mod tests { #[test] fn greets() { assert_eq!(super::greeting(), \"Hello, studio\"); } }\n")?;
    git(&root.join("repo"), &["init", "-q"])?;
    commit(&root.join("repo"), "Create isolated onboarding starter")?;
    write(
        &root.join("onboarding-starter.json"),
        &serde_json::to_vec(&scope).map_err(|e| e.to_string())?,
    )?;
    let book = Book {
        schema: SCHEMA.into(),
        scope,
        snapshot: Some(snapshot),
        terminal: None,
        records: vec![],
        inspection: None,
        direct: vec![],
    };
    write(
        &config.book,
        &serde_json::to_vec(&book).map_err(|e| e.to_string())?,
    )?;
    write(
        &root.join("config.json"),
        &serde_json::to_vec(&config).map_err(|e| e.to_string())?,
    )?;
    Ok(config)
}
fn proof(
    config: &Config,
    snapshot: &Snapshot,
    operation: Operation,
    outcome: Outcome,
    review: Option<TaskReview>,
    request: &str,
) -> Result<StudioProof, String> {
    use route_contract::{
        binding::HostPlacement,
        snapshot::{CheckScope, Surface, WorkspaceBinding},
    };
    let intent =
        serde_json::from_value(serde_json::to_value(&operation).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
    let situation = openagents_chat::route::Situation {
        surface: Surface::Terminal,
        caller: "onboarding-simulation".into(),
        request: request.into(),
        thread: None,
        computer: config.scope.host.clone(),
        project: Some(WorkspaceBinding {
            project: config.scope.workspace.clone(),
            path: None,
        }),
        ready: false,
        bound: None,
        check: CheckScope::ExecutorExit,
    };
    let mut admission = openagents_chat::route::admit(
        &route_contract::RouteResult::LocalCommand {
            action: route_contract::route::LocalAction::Screen {
                screen: "studio".into(),
                target: None,
            },
        },
        &situation,
        None,
        "Simulated owner reply; no real work admitted",
        None,
    );
    admission.placement.workspace = situation.project.clone();
    admission.route.explicit = true;
    admission.input.request = route_contract::digest_of(&intent);
    let binding = WorkbenchBinding {
        schema: route_contract::BINDING_SCHEMA.into(),
        snapshot: admission.digest(),
        parent: None,
        placement: HostPlacement {
            computer: config.scope.host.clone(),
            recipient: config.scope.host.clone(),
            generation: snapshot.stream.clone(),
        },
        run: None,
        terminal: None,
        resources: vec![],
    };
    let route = StudioRoute {
        schema: route_contract::studio::SCHEMA.into(),
        request: request.into(),
        snapshot: admission.digest(),
        binding: binding.digest(),
        intent,
    };
    route.check(&admission, &binding)?;
    outcome.validate().map_err(|e| e.to_string())?;
    if !outcome.answers(&operation) {
        return Err("Simulated owner reply answers another operation".into());
    }
    let result = ResultView {
        schema: route_contract::studio::RESULT_SCHEMA.into(),
        request: request.into(),
        route: route.digest(),
        state: State::Completed {
            outcome: Box::new(outcome),
        },
    };
    Ok(StudioProof {
        route,
        admission,
        binding,
        result,
        snapshot: snapshot.clone(),
        review,
    })
}
/// Complete the labeled practice once. Reopening never dispatches or repeats a goal.
pub fn practice(root: &Path) -> Result<Config, String> {
    let existed = root.join("config.json").is_file();
    let config = starter(root, Lane::Simulated)?;
    if existed {
        if config.rows()?.iter().all(|r| r.complete) {
            return Ok(config);
        }
        return Err("Incomplete practice requires explicit inspection; no automatic replay".into());
    }
    let mut book = config.book()?;
    let mut tasks = Store::open(&config.tasks).map_err(|e| e.to_string())?;
    let mut studio = Studio::open(&config.tasks).map_err(|e| e.to_string())?;
    studio
        .set_seat(Seat {
            name: "apprentice".into(),
            role: Role::Lead,
            route: coder::task::studio::parse_route("codex:onboarding-simulation")
                .map_err(|e| e.to_string())?,
            look: "starter".into(),
            desk: 0,
        })
        .map_err(|e| e.to_string())?;
    let (goal, released) = studio
        .submit_goal(
            &mut tasks,
            NewGoal {
                text: "Make the greeting test pass in this scratch repository; push nothing."
                    .into(),
                repository: Repository {
                    label: config.scope.workspace.clone(),
                    path: config.starter.join("repo").display().to_string(),
                },
                lead: Some("apprentice".into()),
            },
            1_790_000_000,
        )
        .map_err(|e| e.to_string())?;
    let mut snapshot = Snapshot {
        stream: book.snapshot.as_ref().unwrap().stream.clone(),
        sequence: 1,
        view: studio.wire(&tasks, &config.tasks),
    };
    snapshot.validate().map_err(|e| e.to_string())?;
    book.snapshot = Some(snapshot.clone());
    let open = coder_pty::wire::Open::new(
        &"6".repeat(64),
        &coder_host::mailbox::workspace_id(&config.scope.workspace),
        "",
        coder_pty::wire::Launch::Shell,
        coder_pty::wire::Size::new(24, 80),
    );
    book.terminal = Some(TerminalProof {
        owner: TerminalOwner {
            studio_stream: snapshot.stream.clone(),
            host_key: "11".repeat(32),
            host_generation: 1,
        },
        result: coder_pty::wire::TerminalResult::from_outcome(
            &open.request,
            Ok((
                coder_pty::wire::Status::Accepted,
                coder_pty::wire::Value::Opened {
                    terminal: coder_pty::wire::TerminalRef {
                        generation: coder_host::mailbox::terminal_generation(&"11".repeat(32), 1),
                        terminal: "7".repeat(64),
                    },
                    size: open.size,
                },
            )),
        ),
        open,
    });
    // Scripted replies are explicitly simulated, not a real engine run or answered task.
    snapshot
        .view
        .decisions
        .push(coder_access::studio::Decision {
            decision: released.task_id.clone(),
            goal,
            task: Some(released.task_id.clone()),
            seat: Some("apprentice".into()),
            kind: coder_access::studio::DecisionKind::Question,
            text: "Use Hello, studio exactly?".into(),
            based_on: 1,
            approval: None,
        });
    snapshot.view.canonicalize();
    snapshot.validate().map_err(|e| e.to_string())?;
    book.records.push(proof(
        &config,
        &snapshot,
        Operation::AnswerDecision {
            decision: released.task_id.clone(),
            based_on: 1,
            text: "Use Hello, studio exactly.".into(),
            command: "1".repeat(64),
            issued_at: 1_790_000_001,
        },
        Outcome::Dispatched {
            receipt: coder_access::protocol::Receipt {
                operation: "studio.decision.answer".into(),
                reference: released.task_id.clone(),
            },
        },
        None,
        &"2".repeat(64),
    )?);
    let repo = config.starter.join("repo");
    let base = git(&repo, &["rev-parse", "HEAD"])?;
    replace(&repo.join("src/lib.rs"),b"pub fn greeting() -> &'static str { \"Hello, studio\" }\n#[cfg(test)] mod tests { #[test] fn greets() { assert_eq!(super::greeting(), \"Hello, studio\"); } }\n")?;
    git(&repo, &["add", "."])?;
    let tree = git(&repo, &["write-tree"])?;
    let review = TaskReview {
        task: released.task_id.clone(),
        base: base.clone(),
        head_commit: base.clone(),
        head: tree.clone(),
        files: vec![coder_access::review::FileCount {
            path: "src/lib.rs".into(),
            status: coder_access::review::FileStatus::Modified,
            added: Some(1),
            removed: Some(1),
        }],
        files_total: 1,
        added: 1,
        removed: 1,
        uncounted: 0,
        diff: git(
            &repo,
            &["diff", "--cached", "--no-ext-diff", "--no-textconv", &base],
        )?,
        completeness: coder_access::review::Completeness::Complete,
        publication: None,
    };
    let commit = commit(&repo, "Complete simulated greeting locally")?;
    let decision = coder_access::studio::MergeDecision {
        task: released.task_id.clone(),
        base: base.clone(),
        head_commit: base.clone(),
        head: tree.clone(),
        verdict: coder_access::studio::Verdict::Merge,
        text: String::new(),
        command: "3".repeat(64),
        issued_at: 1_790_000_002,
    };
    let publication = coder_access::review::Publication {
        operation: "4".repeat(64),
        task: released.task_id.clone(),
        base: base.clone(),
        head_commit: base.clone(),
        head: tree.clone(),
        landing: coder_access::review::Landing::Branch,
        state: coder_access::review::PublishState::Published,
        branch: Some("local-practice".into()),
        commit: Some(commit),
        url: None,
        note: "Simulated local landing; no remote and nothing pushed.".into(),
    };
    let merged = coder_access::studio::Merged {
        task: released.task_id,
        base: base.clone(),
        head_commit: base,
        head: tree,
        verdict: coder_access::studio::Verdict::Merge,
        publication: Some(publication),
    };
    book.records.push(proof(
        &config,
        &snapshot,
        Operation::DecideMerge {
            decision: Box::new(decision),
        },
        Outcome::Merged {
            merged: Box::new(merged),
        },
        Some(review),
        &"5".repeat(64),
    )?);
    let draft = config.starter.join("contribution-draft");
    mkdir(&draft.join("skills"))?;
    const AUTHOR: &str = "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798";
    write(&draft.join("skills/greeting.md"),b"Inspect local greeting changes and retained owner evidence. Reading grants no execution or XP.")?;
    write(
        &draft.join("README.md"),
        b"Simulated onboarding contribution; no publication or awards.",
    )?;
    write(
        &draft.join("package.json"),
        json!({"v":1,"slug":"starter-greeting","version":"1","publisher":AUTHOR})
            .to_string()
            .as_bytes(),
    )?;
    let owner_root = config.starter.join("contribution-owner");
    Owner::open(owner_root.clone(), DraftOnly).freeze(
        Source {
            flow: "practice".into(),
            thread: "practice".into(),
            task: "practice-contribution".into(),
        },
        Flow::at(Step::Tests, Some("starter-greeting".into())),
        &draft,
        Declarations {
            author: AUTHOR.into(),
            fee_msat: None,
            payout: None,
        },
    )?;
    let contribution = contribution_workbench::host::Config {
        plugins: vec![owner_root],
        knowledge: vec![],
        reviews: vec![],
        events: vec![],
        documents: vec![],
        operators: BTreeSet::new(),
        evaluators: BTreeSet::new(),
        referees: BTreeSet::new(),
        ledger: None,
    };
    let path = config.starter.join("contribution.json");
    write(
        &path,
        &serde_json::to_vec(&contribution).map_err(|e| e.to_string())?,
    )?;
    let row = contribution.read(1_790_000_003)?.remove(0);
    book.inspection = Some(Inspection {
        config: path,
        source: row.source_record.clone(),
        revision: digest(&row),
    });
    replace(
        &config.book,
        &serde_json::to_vec(&book).map_err(|e| e.to_string())?,
    )?;
    if !config.rows()?.iter().all(|r| r.complete) {
        return Err("Practice owner facts did not complete the bounded path".into());
    }
    Ok(config)
}
