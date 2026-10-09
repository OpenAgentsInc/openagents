use crate::service::{CommandInput, RecipeEdit, Setup, SetupError};
use crate::*;
use coder_cloud::operator::{Adapter, Profile};
use coder_environment::evidence::{EvidenceStatus, Redactor, StreamState, load_manifest};
use coder_environment::{
    ArtifactPin, ImagePin, Inputs, Limits, Platform, ProjectLink, Provider, Qualification, Recipe,
    Script, Start,
};
use coder_working_computer::driver::Driver;
use coder_working_computer::provider::fake::{FakeProvider, FakeRun, Inject};
use coder_working_computer::{Computer, Phase, Purpose};
use std::path::PathBuf;

const GH_SECRET: &str = "ghp_setup_secret_value_0123456789";
const SESSION: &str = "setup-1";

fn recipe() -> Recipe {
    Recipe {
        schema: coder_environment::RECIPE_SCHEMA.into(),
        base: ImagePin {
            provider: Provider::Boat,
            image_id: "boat-base-1".into(),
            digest: "c".repeat(64),
        },
        runtime: ArtifactPin {
            revision: "runtime-1".into(),
            digest: "d".repeat(64),
        },
        platform: Platform {
            os: "linux".into(),
            architecture: "x86_64".into(),
        },
        install: Script {
            cwd: ".".into(),
            digest: "e".repeat(64),
        },
        start: Start::default(),
        inputs: Inputs::default(),
        credential_names: BTreeSet::new(),
        qualification: Qualification {
            profile: "rust-library".into(),
            plan_digest: "f".repeat(64),
        },
        limits: Limits {
            deadline_seconds: 3600,
            concurrent_machines: 2,
            total_machine_allocations: 4,
            output_bytes: 1 << 20,
        },
        capture: Default::default(),
    }
}
fn environment() -> Environment {
    Environment::new(
        "env-1",
        ProjectLink {
            workspace: "ws-1".into(),
            project: "proj-1".into(),
        },
        SourcePin {
            repository: Some("example/repo".into()),
            revision: "a".repeat(40),
            digest: "b".repeat(64),
        },
        recipe(),
        0,
    )
    .unwrap()
}
fn profile() -> Profile {
    Profile {
        workspace: "ws-1".into(),
        project: "proj-1".into(),
        cwd: PathBuf::from("/srv/repo"),
        source_revision: "a".repeat(40),
        source_digest: "b".repeat(64),
        repository: Some("example/repo".into()),
        branch: None,
        paths: vec![],
        include: vec![],
        pool: "pool-1".into(),
        placement: coder_cloud::Placement::Boat,
        mode: coder_cloud::Mode::Coder,
        executor: "codex".into(),
        model: None,
        reasoning: None,
        max_timeout_seconds: 3600,
        size: "small".into(),
        template: None,
        credentials: BTreeMap::from([
            ("OA_CODEX_AUTH".into(), PathBuf::from("/secrets/codex")),
            ("GH_TOKEN".into(), PathBuf::from("/secrets/gh")),
        ]),
        adapter: Adapter::Boat {
            origin: "https://boat.example".into(),
            token_file: PathBuf::from("/secrets/boat"),
        },
    }
}
fn request() -> SetupRequest {
    SetupRequest {
        session: SESSION.into(),
        environment: "env-1".into(),
        owner: Principal {
            workspace: "ws-1".into(),
            principal: "user-1".into(),
        },
        profile: "rust".into(),
        objective: "Set up the Rust workspace.".into(),
        credential_names: ["GH_TOKEN".to_string()].into(),
        git_credential: Some("GH_TOKEN".into()),
        deadline_seconds: 600,
    }
}

/// Scripted machine behavior keyed by command text.
fn handler() -> coder_working_computer::provider::fake::Handler {
    Box::new(|spec, env, files| {
        let c = spec.command.as_str();
        if c.contains("oa-source head=") {
            // The real script runs under `sh` in `tests/source_script.rs`.
            let pin = environment().source;
            let ok = crate::source::Report::verified_for(&pin, true);
            FakeRun::exit(0, &ok.render(), "")
        } else if c == "ls" {
            FakeRun::exit(0, "Cargo.toml\n", "")
        } else if c.contains("install-v1") {
            FakeRun::exit(1, "resolving\n", "error: libfoo is missing\n")
        } else if c.contains("install-v2") {
            files.insert("/usr/lib/libfoo.so".into(), "lib".into());
            FakeRun::exit(0, "installed libfoo\n", "")
        } else if c == "echo $GH_TOKEN" {
            let v = env.get("GH_TOKEN").cloned().unwrap_or_default();
            FakeRun::exit(0, &format!("{v}\n"), "")
        } else if c.starts_with("git clone") {
            // Git writes only the remote URL; the helper is per-process.
            files.insert(
                ".git/config".into(),
                "[remote \"origin\"]\n\turl = https://github.com/example/repo\n".into(),
            );
            let helper = env.get("GIT_CONFIG_VALUE_1").cloned().unwrap_or_default();
            FakeRun::exit(0, &format!("helper={helper}\n"), "")
        } else if c == "cat .git/config" {
            FakeRun::exit(0, files.get(".git/config").map_or("", String::as_str), "")
        } else if c.starts_with("sleep") {
            FakeRun {
                stdout: "waiting\n".into(),
                ..Default::default()
            }
        } else {
            FakeRun::exit(0, "", "")
        }
    })
}

fn harness(dir: &tempfile::TempDir) -> Setup<FakeProvider> {
    let root = dir.path();
    let environments = coder_environment::store::Store::under(root.join("environments"));
    environments.create(&environment()).unwrap();
    let provider = FakeProvider::new(
        BTreeMap::from([
            ("GH_TOKEN".into(), GH_SECRET.into()),
            ("OA_CODEX_AUTH".into(), "codex-login-value-123".into()),
        ]),
        true,
    );
    provider.on_command(handler());
    let computers = coder_working_computer::store::Store::under(root.join("computers"));
    Setup::new(
        root.join("setup"),
        store::Store::under(root.join("sessions")),
        environments,
        Driver::new(computers, provider),
        Box::new(|names: &BTreeSet<String>| {
            let mut r = Redactor::new();
            if names.contains("GH_TOKEN") {
                r.select(GH_SECRET).map_err(|e| e.to_string())?;
            }
            Ok(r)
        }),
    )
}
fn input(command: &str) -> CommandInput {
    CommandInput {
        command: command.into(),
        cwd: ".".into(),
        credential_names: BTreeSet::new(),
        git_auth: false,
        timeout_seconds: 60,
    }
}
fn edit(script: &str) -> RecipeEdit {
    RecipeEdit {
        install_script: script.into(),
        install_cwd: ".".into(),
        start: None,
        inputs: None,
        credential_names: None,
        qualification: None,
        limits: None,
        capture: None,
    }
}
fn started(r: ToolResult) -> String {
    match r {
        ToolResult::CommandStarted { command } => command,
        other => panic!("expected a command, got {other:?}"),
    }
}
fn resource(s: &Setup<FakeProvider>) -> String {
    let c = s.computers.store.read("setup-setup-1").unwrap();
    c.resource().unwrap().to_owned()
}
fn refused(e: SetupError) -> Refusal {
    match e {
        SetupError::Refused(r) => r,
        other => panic!("expected a refusal, got {other:?}"),
    }
}

#[tokio::test]
async fn install_failure_repair_and_rerun_are_recipe_revisions_with_evidence() {
    let dir = tempfile::tempdir().unwrap();
    let setup = harness(&dir);
    let s = setup.open(&request(), &profile(), 1_000).await.unwrap();
    assert_eq!(s.state, SetupState::Discovering);
    assert_eq!(s.generation, Some(1));
    // A dedicated setup computer, not a chat computer.
    let computer = setup.computers.store.read(&s.computer).unwrap();
    assert_eq!(
        computer.purpose,
        Purpose::EnvironmentSetup {
            environment: "env-1".into()
        }
    );
    assert_eq!(computer.credential_names, s.admission.credential_names);
    assert_eq!(s.admission.base.image_id, "boat-base-1");

    let cmd = started(
        setup
            .run_command(SESSION, "q1", &input("ls"), 2_000)
            .await
            .unwrap(),
    );
    let v = setup.poll(SESSION, &cmd, 2_100).await.unwrap();
    assert_eq!(v.command.run, Run::Exited { code: 0 });
    assert_eq!(v.stdout, "Cargo.toml\n");

    let r = setup
        .update_recipe(SESSION, "q2", 1, &edit("./install-v1.sh"), 3_000)
        .await
        .unwrap();
    assert!(matches!(r, ToolResult::RecipeRevised { revision: 2, .. }));
    // The install fence names the draft revision it runs.
    assert_eq!(
        refused(
            setup
                .run_install(SESSION, "q3", 1, 600, 3_100)
                .await
                .unwrap_err()
        ),
        Refusal::StaleDraft {
            expected: 1,
            current: 2
        }
    );
    // An install needs the pinned source on the machine first.
    assert_eq!(
        refused(
            setup
                .run_install(SESSION, "q3", 2, 600, 3_110)
                .await
                .unwrap_err()
        ),
        Refusal::SourceNotReady
    );
    let src = started(
        setup
            .materialize_source(SESSION, "qs", 600, 3_120)
            .await
            .unwrap(),
    );
    let v = setup.poll(SESSION, &src, 3_130).await.unwrap();
    assert_eq!(v.command.run, Run::Exited { code: 0 });
    let spec = &v.command.spec;
    // The exact pin, with ephemeral Git auth for the fetch only.
    assert!(spec.command.contains(&"a".repeat(40)));
    assert!(spec.command.contains("https://github.com/example/repo.git"));
    assert_eq!(spec.env, git_auth_env("GH_TOKEN"));
    assert!(!spec.command.contains(GH_SECRET));
    let report = crate::source::Report::parse(&v.stdout).unwrap();
    assert!(report.verified(&environment().source));
    assert!(setup.sessions.read(SESSION).unwrap().source_ready());
    let first = started(
        setup
            .run_install(SESSION, "q3", 2, 600, 3_200)
            .await
            .unwrap(),
    );
    let v = setup.poll(SESSION, &first, 3_300).await.unwrap();
    assert_eq!(v.command.run, Run::Exited { code: 1 });
    assert_eq!(v.stderr, "error: libfoo is missing\n");
    assert_eq!(
        setup.sessions.read(SESSION).unwrap().state,
        SetupState::Repairing
    );

    // Repair: a new revision, then a rerun that keeps the failed attempt.
    setup
        .update_recipe(SESSION, "q4", 2, &edit("./install-v2.sh"), 4_000)
        .await
        .unwrap();
    let second = started(
        setup
            .run_install(SESSION, "q5", 3, 600, 4_100)
            .await
            .unwrap(),
    );
    let v = setup.poll(SESSION, &second, 4_200).await.unwrap();
    assert_eq!(v.command.run, Run::Exited { code: 0 });
    let s = setup.sessions.read(SESSION).unwrap();
    assert_eq!(s.state, SetupState::Installed { recipe_revision: 3 });
    assert_eq!(s.recipe_revisions, vec![2, 3]);
    let attempts: Vec<_> = s
        .installs()
        .map(|c| (c.purpose.clone(), c.run.clone()))
        .collect();
    let env = setup.environments.read("env-1").unwrap();
    assert_eq!(env.recipes.len(), 3);
    assert_eq!(
        env.recipes[2].parent_digest.as_deref(),
        Some(env.recipes[1].digest.as_str())
    );
    assert_eq!(
        attempts,
        vec![
            (
                CommandPurpose::Install {
                    recipe_revision: 2,
                    recipe_digest: env.recipes[1].digest.clone(),
                    attempt: 1
                },
                Run::Exited { code: 1 }
            ),
            (
                CommandPurpose::Install {
                    recipe_revision: 3,
                    recipe_digest: env.recipes[2].digest.clone(),
                    attempt: 2
                },
                Run::Exited { code: 0 }
            ),
        ]
    );
    let inspection = setup.inspect(SESSION).unwrap();
    assert_eq!(inspection.draft_revision, 3);
    assert_eq!(
        inspection.install_script.as_deref(),
        Some("./install-v2.sh")
    );

    let s = setup.end(SESSION, 5_000).await.unwrap();
    assert_eq!(s.state, SetupState::Ended);
    assert!(s.evidence_complete());
    let sealed = s.segments[0].sealed.clone().unwrap();
    assert_eq!(sealed.status, EvidenceStatus::Complete);
    let manifest = load_manifest(&setup.evidence_dir(SESSION, "seg-1"), &sealed.digest).unwrap();
    let failed = manifest
        .calls
        .iter()
        .find(|c| c.identity.id == first)
        .unwrap();
    assert_eq!(failed.identity.tool, "environment.install");
    // The source step and its report are in the evidence.
    let source = manifest
        .calls
        .iter()
        .find(|c| c.identity.id == src)
        .unwrap();
    assert_eq!(source.identity.tool, "environment.source.materialize");
    assert_eq!(source.stdout.state, StreamState::Complete);
    assert_eq!(
        failed.stderr.length,
        "error: libfoo is missing\n".len() as u64
    );
    assert_eq!(failed.stderr.state, StreamState::Complete);
    assert!(
        manifest
            .calls
            .iter()
            .filter(|c| c.identity.tool == "environment.recipe.update")
            .count()
            == 2
    );
    // Ending the setup cleans its machine up as separate facts.
    let computer = setup.computers.store.read(&s.computer).unwrap();
    assert_eq!(computer.phase, Phase::Deleted);
}

#[tokio::test]
async fn repeated_requests_replay_without_running_again() {
    let dir = tempfile::tempdir().unwrap();
    let setup = harness(&dir);
    let s = setup.open(&request(), &profile(), 1_000).await.unwrap();
    let again = setup.open(&request(), &profile(), 1_100).await.unwrap();
    assert_eq!(again.generation, s.generation);

    let a = setup
        .run_command(SESSION, "q1", &input("ls"), 2_000)
        .await
        .unwrap();
    let b = setup
        .run_command(SESSION, "q1", &input("ls"), 2_050)
        .await
        .unwrap();
    assert_eq!(a, b);
    let process = setup
        .computers
        .provider
        .process(&resource(&setup), "cmd-1")
        .unwrap();
    assert_eq!(process.runs, 1);
    let starts = setup
        .computers
        .provider
        .calls()
        .iter()
        .filter(|c| *c == "command_start")
        .count();
    assert_eq!(starts, 1);
    assert_eq!(
        refused(
            setup
                .run_command(SESSION, "q1", &input("pwd"), 2_100)
                .await
                .unwrap_err()
        ),
        Refusal::RequestConflict("q1".into())
    );

    setup.poll(SESSION, "cmd-1", 2_200).await.unwrap();
    let r1 = setup
        .update_recipe(SESSION, "q2", 1, &edit("./install-v2.sh"), 3_000)
        .await
        .unwrap();
    let r2 = setup
        .update_recipe(SESSION, "q2", 1, &edit("./install-v2.sh"), 3_100)
        .await
        .unwrap();
    assert_eq!(r1, r2);
    assert_eq!(setup.environments.read("env-1").unwrap().recipes.len(), 2);
    let mut other = request();
    other.objective = "Something else.".into();
    assert_eq!(
        refused(setup.open(&other, &profile(), 3_200).await.unwrap_err()),
        Refusal::RequestConflict(SESSION.into())
    );
}

#[tokio::test]
async fn credentials_apply_only_when_named_and_git_auth_stays_out_of_files() {
    let dir = tempfile::tempdir().unwrap();
    let setup = harness(&dir);
    setup.open(&request(), &profile(), 1_000).await.unwrap();
    let res = resource(&setup);

    // Not named: the command's environment does not hold the token.
    let cmd = started(
        setup
            .run_command(SESSION, "q1", &input("echo $GH_TOKEN"), 2_000)
            .await
            .unwrap(),
    );
    setup.poll(SESSION, &cmd, 2_100).await.unwrap();
    let p = setup.computers.provider.process(&res, &cmd).unwrap();
    assert!(!p.env.contains_key("GH_TOKEN"));
    assert!(!p.env.contains_key("OA_CODEX_AUTH"));

    // Named: present, and redacted before the evidence keeps it.
    let mut named = input("echo $GH_TOKEN");
    named.credential_names = ["GH_TOKEN".to_string()].into();
    let cmd = started(
        setup
            .run_command(SESSION, "q2", &named, 3_000)
            .await
            .unwrap(),
    );
    let v = setup.poll(SESSION, &cmd, 3_100).await.unwrap();
    assert_eq!(
        setup.computers.provider.process(&res, &cmd).unwrap().env["GH_TOKEN"],
        GH_SECRET
    );
    assert!(!v.stdout.contains(GH_SECRET));

    // A credential the session did not name is refused, as is a URL that
    // carries one, and Git auth without naming its credential.
    let mut engine = input("ls");
    engine.credential_names = ["OA_CODEX_AUTH".to_string()].into();
    assert_eq!(
        refused(
            setup
                .run_command(SESSION, "q3", &engine, 4_000)
                .await
                .unwrap_err()
        ),
        Refusal::CredentialNotAdmitted("OA_CODEX_AUTH".into())
    );
    let leaky = input("git clone https://x-access-token:abc123@github.com/example/repo");
    assert_eq!(
        refused(
            setup
                .run_command(SESSION, "q4", &leaky, 4_100)
                .await
                .unwrap_err()
        ),
        Refusal::EmbeddedCredential
    );
    let mut unnamed_git = input("git clone https://github.com/example/repo");
    unnamed_git.git_auth = true;
    assert!(matches!(
        refused(
            setup
                .run_command(SESSION, "q5", &unnamed_git, 4_200)
                .await
                .unwrap_err()
        ),
        Refusal::CredentialNotAdmitted(_)
    ));

    // Ephemeral Git auth: the helper names the variable; no file holds it.
    let mut clone = unnamed_git.clone();
    clone.credential_names = ["GH_TOKEN".to_string()].into();
    let cmd = started(
        setup
            .run_command(SESSION, "q6", &clone, 5_000)
            .await
            .unwrap(),
    );
    setup.poll(SESSION, &cmd, 5_100).await.unwrap();
    let p = setup.computers.provider.process(&res, &cmd).unwrap();
    assert!(p.env["GIT_CONFIG_VALUE_1"].contains("${GH_TOKEN}"));
    assert!(
        p.env
            .iter()
            .filter(|(k, _)| k.starts_with("GIT_"))
            .all(|(_, v)| !v.contains(GH_SECRET))
    );
    let audit = started(
        setup
            .run_command(SESSION, "q7", &input("cat .git/config"), 6_000)
            .await
            .unwrap(),
    );
    let v = setup.poll(SESSION, &audit, 6_100).await.unwrap();
    assert!(v.stdout.contains("url = https://github.com/example/repo"));
    let machine = setup.computers.provider.machine(&res).unwrap();
    assert!(machine.files.values().all(|f| !f.contains(GH_SECRET)));

    let s = setup.end(SESSION, 7_000).await.unwrap();
    assert_eq!(
        s.segments[0].sealed.as_ref().unwrap().status,
        EvidenceStatus::CompleteWithRedactions
    );
}

#[tokio::test]
async fn an_ambiguous_start_is_reconciled_by_identity_not_rerun() {
    let dir = tempfile::tempdir().unwrap();
    let setup = harness(&dir);
    setup.open(&request(), &profile(), 1_000).await.unwrap();
    let res = resource(&setup);

    // The start happened but its reply was lost.
    setup
        .computers
        .provider
        .inject("command_start", Inject::LostReply);
    let cmd = started(
        setup
            .run_command(SESSION, "q1", &input("ls"), 2_000)
            .await
            .unwrap(),
    );
    let s = setup.sessions.read(SESSION).unwrap();
    assert!(matches!(s.command(&cmd).unwrap().run, Run::Unknown { .. }));
    assert_eq!(
        refused(
            setup
                .run_command(SESSION, "q2", &input("pwd"), 2_100)
                .await
                .unwrap_err()
        ),
        Refusal::Unresolved(cmd.clone())
    );
    let v = setup.reconcile(SESSION, &cmd, 2_200).await.unwrap();
    assert_eq!(v.command.run, Run::Exited { code: 0 });
    assert_eq!(v.stdout, "Cargo.toml\n");
    assert_eq!(
        setup.computers.provider.process(&res, &cmd).unwrap().runs,
        1
    );

    // The start definitely did not happen but the owner could not tell:
    // the read proves it absent, and the same identity starts once.
    setup
        .computers
        .provider
        .inject("command_start", Inject::Unknown);
    let cmd = started(
        setup
            .run_command(SESSION, "q2", &input("ls"), 3_000)
            .await
            .unwrap(),
    );
    assert!(setup.computers.provider.process(&res, &cmd).is_none());
    let v = setup.reconcile(SESSION, &cmd, 3_100).await.unwrap();
    assert_eq!(v.command.run, Run::Exited { code: 0 });
    assert_eq!(
        setup.computers.provider.process(&res, &cmd).unwrap().runs,
        1
    );
    let states: Vec<_> = v
        .command
        .history
        .iter()
        .map(|h| match h.run {
            Run::Requested => "requested",
            Run::Unknown { .. } => "unknown",
            Run::Running { .. } => "running",
            Run::Exited { .. } => "exited",
            _ => "other",
        })
        .collect();
    assert_eq!(
        states,
        ["requested", "unknown", "requested", "running", "exited"]
    );

    // A read the provider cannot answer leaves the command as it was.
    let cmd = started(
        setup
            .run_command(SESSION, "q3", &input("sleep 100"), 4_000)
            .await
            .unwrap(),
    );
    setup
        .computers
        .provider
        .inject("command_read", Inject::Unknown);
    let v = setup.poll(SESSION, &cmd, 4_100).await.unwrap();
    assert!(v.uncertain.is_some());
    assert!(matches!(v.command.run, Run::Running { .. }));
}

#[tokio::test]
async fn deadlines_stop_commands_and_cancel_the_session() {
    let dir = tempfile::tempdir().unwrap();
    let setup = harness(&dir);
    setup.open(&request(), &profile(), 1_000).await.unwrap();
    let res = resource(&setup);

    let mut slow = input("sleep 100");
    slow.timeout_seconds = 10;
    let cmd = started(
        setup
            .run_command(SESSION, "q1", &slow, 2_000)
            .await
            .unwrap(),
    );
    let v = setup.poll(SESSION, &cmd, 5_000).await.unwrap();
    assert!(matches!(v.command.run, Run::Running { .. }));
    // Bytes that could still begin a selected credential are held back
    // until the stream decides them.
    assert_eq!(v.stdout, "");
    let v = setup.poll(SESSION, &cmd, 12_000).await.unwrap();
    assert_eq!(v.command.run, Run::TimedOut);
    assert_eq!(v.stdout, "waiting\n");
    assert_eq!(
        setup.computers.provider.process(&res, &cmd).unwrap().exit,
        Some(143)
    );

    let mut long = input("sleep 1000");
    long.timeout_seconds = 3600;
    let cmd = started(
        setup
            .run_command(SESSION, "q2", &long, 13_000)
            .await
            .unwrap(),
    );
    // The command's deadline is capped by the session's.
    let s = setup.sessions.read(SESSION).unwrap();
    assert_eq!(
        s.command(&cmd).unwrap().deadline_ms,
        s.admission.deadline_ms
    );
    let s = setup.tick(SESSION, s.admission.deadline_ms).await.unwrap();
    assert!(matches!(s.state, SetupState::Cancelled { .. }));
    assert_eq!(s.command(&cmd).unwrap().run, Run::TimedOut);
    assert!(s.segments[0].sealed.is_some());
    assert_eq!(
        setup.computers.store.read(&s.computer).unwrap().phase,
        Phase::Deleted
    );
    assert_eq!(
        refused(
            setup
                .run_command(SESSION, "q3", &input("ls"), 2_000_000)
                .await
                .unwrap_err()
        ),
        Refusal::Ended
    );
}

#[tokio::test]
async fn cancellation_stops_the_running_command_and_cleans_up() {
    let dir = tempfile::tempdir().unwrap();
    let setup = harness(&dir);
    setup.open(&request(), &profile(), 1_000).await.unwrap();
    let cmd = started(
        setup
            .run_command(SESSION, "q1", &input("sleep 5"), 2_000)
            .await
            .unwrap(),
    );
    let s = setup
        .cancel(SESSION, "The user cancelled.", 3_000)
        .await
        .unwrap();
    assert_eq!(
        s.state,
        SetupState::Cancelled {
            reason: "The user cancelled.".into()
        }
    );
    assert_eq!(
        s.command(&cmd).unwrap().run,
        Run::Stopped {
            reason: "session_ended".into()
        }
    );
    assert!(s.evidence_complete());
    let computer = setup.computers.store.read(&s.computer).unwrap();
    assert_eq!(computer.phase, Phase::Deleted);
    // Cancelling again is the same outcome.
    assert_eq!(setup.cancel(SESSION, "again", 4_000).await.unwrap(), s);
}

#[tokio::test]
async fn steering_pauses_and_resumes_the_dedicated_computer() {
    let dir = tempfile::tempdir().unwrap();
    let setup = harness(&dir);
    setup.open(&request(), &profile(), 1_000).await.unwrap();
    let cmd = started(
        setup
            .run_command(SESSION, "q1", &input("sleep 5"), 2_000)
            .await
            .unwrap(),
    );
    let s = setup
        .steer(SESSION, "Prefer the pinned toolchain.", 2_100)
        .await
        .unwrap();
    assert_eq!(s.steering.len(), 1);
    assert_eq!(
        refused(
            setup
                .pause(SESSION, "Install apt packages?", 2_200)
                .await
                .unwrap_err()
        ),
        Refusal::Busy(cmd.clone())
    );
    let v = setup.stop(SESSION, &cmd, 2_300).await.unwrap();
    assert_eq!(
        v.command.run,
        Run::Stopped {
            reason: "owner".into()
        }
    );
    let s = setup
        .pause(SESSION, "Install apt packages?", 2_400)
        .await
        .unwrap();
    assert!(matches!(s.state, SetupState::AwaitingInput { .. }));
    let computer = setup.computers.store.read(&s.computer).unwrap();
    assert_eq!(computer.phase, Phase::Stopped);
    assert_eq!(
        refused(
            setup
                .run_command(SESSION, "q2", &input("ls"), 2_500)
                .await
                .unwrap_err()
        ),
        Refusal::AwaitingInput
    );
    let s = setup
        .steer(SESSION, "Yes, install them.", 3_000)
        .await
        .unwrap();
    assert_eq!(s.state, SetupState::Discovering);
    assert_eq!(s.generation, Some(2));
    // The restored boot re-applied the named credential.
    let m = setup.computers.provider.machine(&resource(&setup)).unwrap();
    assert_eq!(m.env["GH_TOKEN"], GH_SECRET);
    setup
        .run_command(SESSION, "q2", &input("ls"), 3_100)
        .await
        .unwrap();
}

#[tokio::test]
async fn an_owner_restart_opens_a_new_segment_and_discloses_the_gap() {
    let dir = tempfile::tempdir().unwrap();
    let setup = harness(&dir);
    setup.open(&request(), &profile(), 1_000).await.unwrap();
    let res = resource(&setup);
    let cmd = started(
        setup
            .run_command(SESSION, "q1", &input("sleep 5"), 2_000)
            .await
            .unwrap(),
    );
    setup.poll(SESSION, &cmd, 2_100).await.unwrap();
    setup.restart();
    setup
        .computers
        .provider
        .finish_command(&res, &cmd, 0, "done\n");
    let v = setup.poll(SESSION, &cmd, 3_000).await.unwrap();
    assert_eq!(v.command.run, Run::Exited { code: 0 });
    assert_eq!(v.stdout, "done\n");
    assert_eq!(v.command.evidence.len(), 2);
    let s = setup.end(SESSION, 4_000).await.unwrap();
    assert!(s.segments[0].interrupted);
    assert!(s.segments[1].sealed.is_some());
    assert!(!s.evidence_complete());
}

#[tokio::test]
async fn a_chat_computer_is_never_a_setup_computer() {
    let dir = tempfile::tempdir().unwrap();
    let setup = harness(&dir);
    let env = environment();
    let chat = Computer::new(
        coder_working_computer::Spec {
            id: "setup-setup-1".into(),
            owner: request().owner,
            chat: "chat-1".into(),
            project: env.project.clone(),
            source: env.source.clone(),
            base: None,
            size: "small".into(),
            credential_names: BTreeSet::new(),
            services: vec![],
            bounds: coder_working_computer::Bounds {
                idle_ms: 1000,
                observed_extension_ms: 0,
                absolute_ms: 1000,
            },
        },
        0,
    )
    .unwrap();
    setup.computers.store.create(&chat).unwrap();
    assert!(matches!(
        refused(setup.open(&request(), &profile(), 1_000).await.unwrap_err()),
        Refusal::Admission(_)
    ));
}

#[test]
fn admission_binds_the_admitted_codex_profile_pins_and_named_credentials() {
    let env = environment();
    let a = admit(&request(), &profile(), &env, 1_000).unwrap();
    assert_eq!(a.base, recipe().base);
    assert_eq!(a.runtime, recipe().runtime);
    assert_eq!(a.deadline_ms, 601_000);
    assert!(a.pins(&recipe()));

    let check = |r: SetupRequest, p: Profile| admit(&r, &p, &env, 1_000).unwrap_err();
    let mut p = profile();
    p.executor = "claude".into();
    p.credentials = BTreeMap::from([("ANTHROPIC_API_KEY".into(), PathBuf::from("/k"))]);
    assert!(matches!(check(request(), p), Refusal::Admission(_)));
    let mut p = profile();
    p.placement = coder_cloud::Placement::Gce;
    assert!(matches!(check(request(), p), Refusal::Admission(_)));
    let mut p = profile();
    p.project = "proj-2".into();
    assert!(matches!(check(request(), p), Refusal::Admission(_)));
    let mut p = profile();
    p.source_revision = "9".repeat(40);
    assert!(matches!(check(request(), p), Refusal::Admission(_)));
    let mut r = request();
    r.credential_names.insert("NPM_TOKEN".into());
    assert_eq!(
        check(r, profile()),
        Refusal::CredentialNotAdmitted("NPM_TOKEN".into())
    );
    let mut r = request();
    r.credential_names.insert("CLAUDE_CODE_OAUTH_TOKEN".into());
    assert_eq!(
        check(r, profile()),
        Refusal::CredentialNotAdmitted("CLAUDE_CODE_OAUTH_TOKEN".into())
    );
    let mut r = request();
    r.credential_names.clear();
    assert!(matches!(check(r, profile()), Refusal::Admission(_)));
    let mut r = request();
    r.deadline_seconds = 7200;
    assert!(matches!(check(r, profile()), Refusal::Admission(_)));

    let mut moved = recipe();
    moved.base.image_id = "boat-base-2".into();
    assert!(!a.pins(&moved));
}

#[tokio::test]
async fn a_recipe_revision_cannot_move_the_session_pins() {
    let dir = tempfile::tempdir().unwrap();
    let setup = harness(&dir);
    setup.open(&request(), &profile(), 1_000).await.unwrap();
    // Someone moves the draft's base outside this session.
    let mut moved = recipe();
    moved.base.image_id = "boat-base-2".into();
    setup
        .environments
        .apply(
            "env-1",
            &coder_environment::Command::UpdateRecipe {
                expected_draft_revision: 1,
                recipe: moved,
            },
            1_500,
        )
        .unwrap();
    assert_eq!(
        refused(
            setup
                .update_recipe(SESSION, "q1", 2, &edit("./install-v2.sh"), 2_000)
                .await
                .unwrap_err()
        ),
        Refusal::PinChanged
    );
    assert_eq!(
        refused(
            setup
                .run_install(SESSION, "q2", 2, 60, 2_100)
                .await
                .unwrap_err()
        ),
        Refusal::PinChanged
    );
}

#[test]
fn url_credentials_are_detected() {
    assert!(embeds_url_credential(
        "git clone https://tok@github.com/a/b"
    ));
    assert!(embeds_url_credential("curl 'https://u:p@host/x'"));
    assert!(!embeds_url_credential("git clone https://github.com/a/b"));
    assert!(!embeds_url_credential("git log --author=me@example.com"));
}

#[tokio::test]
async fn the_panel_lists_sessions_and_retains_steering_before_the_owner_wakes() {
    use coder_cloud::operator::SetupSessions;
    let dir = tempfile::tempdir().unwrap();
    let setup = std::sync::Arc::new(harness(&dir));
    let environment = request().environment;
    let (panel, mut wakes) = crate::panel::Panel::new(setup.clone());
    assert!(panel.sessions(&environment).unwrap().is_empty());
    setup.open(&request(), &profile(), 1_000).await.unwrap();
    let cmd = started(
        setup
            .run_command(SESSION, "q1", &input("sleep 5"), 1_100)
            .await
            .unwrap(),
    );
    setup.stop(SESSION, &cmd, 1_150).await.unwrap();
    let s = setup
        .pause(SESSION, "Which toolchain?", 1_200)
        .await
        .unwrap();
    assert!(matches!(s.state, SetupState::AwaitingInput { .. }));

    let rows = panel.sessions(&environment).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].state, "awaiting_input");
    assert_eq!(rows[0].question.as_deref(), Some("Which toolchain?"));
    assert!(rows[0].steerable);
    assert_eq!(rows[0].commands.len(), 1);
    // The projection never carries a credential value.
    assert!(!serde_json::to_string(&rows).unwrap().contains(GH_SECRET));
    assert!(panel.sessions("env-other").unwrap().is_empty());

    // Another environment's panel cannot steer this session.
    assert_eq!(
        panel.steer("env-other", SESSION, "Use stable.", 1_300),
        Err(coder_access::Code::Forbidden)
    );
    assert_eq!(
        panel.steer(&environment, SESSION, "Use stable.", 1_300),
        Ok("steering_retained_wake_requested".into())
    );
    // Retained before the answer, with no provider call on the way.
    let s = setup.sessions().read(SESSION).unwrap();
    assert_eq!(s.steering.len(), 1);
    assert!(matches!(s.state, SetupState::AwaitingInput { .. }));
    assert_eq!(wakes.try_recv().unwrap(), SESSION);
    // The owner's loop wakes the dedicated computer for the next turn.
    let s = setup.resume(SESSION, 1_400).await.unwrap();
    assert_eq!(s.state, SetupState::Discovering);
    assert_eq!(
        panel.sessions(&environment).unwrap()[0].steering[0].text,
        "Use stable."
    );
}

/// #11059: the setup agent beats while it runs; once it goes silent past
/// the window, its command and computer stop and the setup waits for the
/// person with a plain message. A message picks it up again.
#[tokio::test]
async fn a_silent_setup_turn_stops_its_computer_and_waits() {
    use coder_working_computer::{STALE_AFTER_MS, StopReason as ComputerStop};
    let dir = tempfile::tempdir().unwrap();
    let setup = harness(&dir);
    setup.open(&request(), &profile(), 1_000).await.unwrap();
    let mut long = input("sleep 1000");
    long.timeout_seconds = 3600;
    let cmd = started(
        setup
            .run_command(SESSION, "q1", &long, 2_000)
            .await
            .unwrap(),
    );
    // A restarted owner's first visit counts as alive.
    let s = setup.tick(SESSION, 3_000).await.unwrap();
    assert_eq!(s.state, SetupState::Discovering);
    // A beating agent keeps the turn however long it runs.
    let mut now = 3_000;
    for _ in 0..10 {
        now += 30_000;
        setup.heartbeat(SESSION, now).await.unwrap();
        let s = setup.tick(SESSION, now + 1).await.unwrap();
        assert_eq!(s.state, SetupState::Discovering);
    }
    // Silent past the window.
    let s = setup.tick(SESSION, now + STALE_AFTER_MS).await.unwrap();
    assert_eq!(
        s.state,
        SetupState::AwaitingInput {
            question: crate::service::STALLED.into()
        }
    );
    assert_eq!(
        s.command(&cmd).unwrap().run,
        Run::Stopped {
            reason: "owner".into()
        }
    );
    let computer = setup.computers.store.read(&s.computer).unwrap();
    assert_eq!(computer.phase, Phase::Stopped);
    assert_eq!(
        computer.boot().unwrap().stop_reason,
        Some(ComputerStop::Stale)
    );
    // Plain words for the person.
    for word in ["stale", "heartbeat", "generation", "turn"] {
        assert!(!crate::service::STALLED.to_lowercase().contains(word));
    }
    // The person answers: a restored computer, a new turn.
    let s = setup
        .steer(SESSION, "Keep going.", now + STALE_AFTER_MS + 1_000)
        .await
        .unwrap();
    assert_eq!(s.state, SetupState::Discovering);
    assert_eq!(s.generation, Some(2));
}
