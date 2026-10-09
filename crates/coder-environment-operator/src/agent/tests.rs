use super::*;
use crate::activity::Entry;
use crate::{Custody, Providers};
use coder_environment::evidence::Redactor;
use coder_environment::{
    ArtifactPin, Environment, ImagePin, Limits, Platform, ProjectLink, Provider, Recipe, Script,
    SourcePin, digest,
};
use coder_working_computer::provider::fake::{FakeProvider, FakeRun};
use codex_transport::TokenUsage;
use codex_transport::fake::{FakeTransport, call, say};
use std::collections::BTreeMap;

type Fake = Arc<FakeProvider>;

fn pin() -> SourcePin {
    SourcePin {
        repository: Some("https://github.com/example/repo".into()),
        revision: "a".repeat(40),
        digest: "b".repeat(64),
    }
}

fn recipe() -> Recipe {
    Recipe {
        schema: coder_environment::RECIPE_SCHEMA.into(),
        base: ImagePin {
            provider: Provider::Boat,
            image_id: "oa-coder-runtime-1".into(),
            digest: "c".repeat(64),
        },
        runtime: ArtifactPin {
            revision: "rt-1".into(),
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
        start: Default::default(),
        inputs: Default::default(),
        credential_names: Default::default(),
        qualification: Qualification {
            profile: "unset".into(),
            plan_digest: "f".repeat(64),
        },
        limits: Limits {
            deadline_seconds: 3600,
            concurrent_machines: 2,
            total_machine_allocations: 8,
            output_bytes: 1 << 24,
        },
        capture: Default::default(),
    }
}

fn profile() -> Profile {
    let p = pin();
    Profile {
        workspace: "ws-1".into(),
        project: "proj-1".into(),
        cwd: "/tmp".into(),
        source_revision: p.revision.clone(),
        source_digest: p.digest.clone(),
        repository: p.repository.clone(),
        branch: None,
        paths: vec![],
        include: vec![],
        pool: "boat".into(),
        placement: coder_cloud::Placement::Boat,
        mode: coder_cloud::Mode::Coder,
        executor: "codex".into(),
        model: None,
        reasoning: None,
        max_timeout_seconds: 3600,
        size: "small".into(),
        template: None,
        credentials: BTreeMap::from([("OA_CODEX_AUTH".into(), "/tmp/auth.json".into())]),
        adapter: coder_cloud::operator::Adapter::Boat {
            origin: "https://boat.example.invalid".into(),
            token_file: "/tmp/boat-token".into(),
        },
    }
}

fn brief() -> Brief {
    Brief {
        environment: "env-1".into(),
        owner: Principal {
            workspace: "ws-1".into(),
            principal: "alice".into(),
        },
        profile_alias: "environment-setup".into(),
        profile: profile(),
        credential_names: BTreeSet::new(),
        git_credential: None,
        deadline_seconds: 3600,
        size: "small".into(),
        objective: "Set up this repository.".into(),
        context: "Repository example/repo, branch main.".into(),
    }
}

/// Machines where the first install fails, the repaired one passes, and
/// every check prints its marker.
fn machines() -> Fake {
    let provider = Arc::new(FakeProvider::new(BTreeMap::new(), true));
    let sanitize = coder_environment_build::sanitize::Plan::new(
        &recipe().capture,
        "/tmp/oa-commands/sanitize",
    );
    provider.on_command(Box::new(move |spec, _env, files| {
        let id = spec.id.as_str();
        let c = spec.command.as_str();
        if id.ends_with("-inventory-before") || id.ends_with("-inventory-after") {
            FakeRun::exit(
                0,
                &format!(
                    "oa-inventory git:head {}\noa-inventory done\n",
                    pin().revision
                ),
                "",
            )
        } else if id == "sanitize" {
            FakeRun::exit(0, &format!("sanitized {}\n", sanitize.digest()), "")
        } else if c.contains("oa-source head=") && c.contains("no-checkout") {
            if !files.contains_key("README.md") {
                return FakeRun::exit(3, "oa-source error=no-checkout\n", "");
            }
            let r = coder_environment_setup::source::Report::verified_for(&pin(), false);
            FakeRun::exit(0, &r.render(), "")
        } else if c.contains("oa-source head=") {
            files.insert("README.md".into(), "# repo".into());
            let r = coder_environment_setup::source::Report::verified_for(&pin(), true);
            FakeRun::exit(0, &r.render(), "")
        } else if c.contains("INSTALL_V1") {
            FakeRun::exit(1, "", "error: libfoo is missing\n")
        } else if c.contains("INSTALL_V2") {
            files.insert("target/app".into(), "binary".into());
            FakeRun::exit(0, "installed\n", "")
        } else if c.contains("OA-CHECK") {
            FakeRun::exit(0, "ok\nOA-CHECK passed=1 failed=0\n", "")
        } else if c == "cat README.md" {
            FakeRun::exit(0, "# repo\n", "")
        } else {
            FakeRun::exit(0, "", "")
        }
    }));
    provider
}

fn custody() -> Custody {
    Arc::new(|_: &BTreeSet<String>| Ok(Redactor::new()))
}

fn agent(
    dir: &std::path::Path,
    provider: &Fake,
    transport: FakeTransport,
) -> Agent<Fake, FakeTransport> {
    let owners = Owners::open(
        &dir.join("state"),
        Providers {
            setup: provider.clone(),
            build: provider.clone(),
            verify: provider.clone(),
        },
        custody(),
    )
    .unwrap();
    let env = Environment::new(
        "env-1",
        ProjectLink {
            workspace: "ws-1".into(),
            project: "proj-1".into(),
        },
        pin(),
        recipe(),
        now_ms(),
    )
    .unwrap();
    owners.setup.environments.create(&env).unwrap();
    Agent {
        owners: Arc::new(owners),
        transport,
        model: "gpt-test".into(),
        logs: Arc::new(Logs::under(dir.join("studio"))),
        root: dir.join("studio"),
        poll: Duration::from_millis(1),
    }
}

fn u() -> TokenUsage {
    TokenUsage::default()
}

#[tokio::test]
async fn the_agent_explores_repairs_asks_builds_and_verifies() {
    let dir = tempfile::tempdir().unwrap();
    let provider = machines();
    let transport = FakeTransport::new(vec![
        call(
            "c1",
            "run_command",
            &json!({"command":"cat README.md"}),
            u(),
        ),
        call(
            "c2",
            "write_recipe",
            &json!({"install_script":"echo INSTALL_V1"}),
            u(),
        ),
        call("c3", "run_install", &json!({}), u()),
        call(
            "c4",
            "ask_user",
            &json!({"question":"libfoo is missing. May I add it?"}),
            u(),
        ),
    ]);
    let a = agent(dir.path(), &provider, transport);
    let b = brief();
    let state = a.run(&b, State::new("env-1")).await;
    assert_eq!(
        state.phase,
        Phase::Waiting {
            question: "libfoo is missing. May I add it?".into()
        },
        "{:?}",
        a.logs.read("env-1")
    );
    assert_eq!(a.load("env-1").unwrap(), state);
    // The model saw the command's output and the failed install.
    let requests = a.transport.requests();
    let last = serde_json::to_string(&requests.last().unwrap().input).unwrap();
    assert!(last.contains("# repo") && last.contains("libfoo is missing"));

    // The person answers; the agent repairs, declares checks, reruns,
    // and finishes.
    a.owners
        .setup
        .steer(&state.session(), "Yes, add it.", now_ms())
        .await
        .unwrap();
    for reply in [
        call(
            "c5",
            "write_recipe",
            &json!({"install_script":"echo INSTALL_V2"}),
            u(),
        ),
        call("c6", "run_install", &json!({}), u()),
        call(
            "c7",
            "set_checks",
            &json!({"checks":[{"name":"Build","command":"make build"}],"offline":false}),
            u(),
        ),
        call(
            "c8",
            "finish",
            &json!({"summary":"Installs and builds."}),
            u(),
        ),
        call("c9", "run_install", &json!({}), u()),
        call(
            "c10",
            "finish",
            &json!({"summary":"Installs and builds."}),
            u(),
        ),
    ] {
        a.transport.then(reply);
    }
    let state = a.run(&b, state).await;
    assert!(
        matches!(state.phase, Phase::Review { .. }),
        "{:?}\n{:?}",
        state.phase,
        a.logs.read("env-1")
    );
    // Finishing before the install passed on the checks' revision was
    // refused, so the agent reran it.
    let input = serde_json::to_string(&state.input).unwrap();
    assert!(input.contains("Run the install on the current recipe revision first"));
    assert!(input.contains("Yes, add it."));

    let kinds: Vec<&'static str> = a
        .logs
        .read("env-1")
        .iter()
        .map(|r| match &r.entry {
            Entry::Source { ok: true, .. } => "source",
            Entry::Explored { .. } => "explored",
            Entry::Recipe { .. } => "recipe",
            Entry::Install { exit: Some(0), .. } => "install-ok",
            Entry::Install { .. } => "install-failed",
            Entry::Question { .. } => "question",
            Entry::Checks { .. } => "checks",
            Entry::Build {
                stage: Stage::Passed,
                ..
            } => "built",
            Entry::Verify {
                stage: Stage::Passed,
                ..
            } => "verified",
            Entry::Ready { .. } => "ready",
            _ => "other",
        })
        .filter(|k| *k != "other")
        .collect();
    assert_eq!(
        kinds,
        [
            "source",
            "explored",
            "recipe",
            "install-failed",
            "question",
            "recipe",
            "install-ok",
            "checks",
            "install-ok",
            "built",
            "verified",
            "ready"
        ]
    );
    // The candidate is saveable.
    let Phase::Review { verification } = &state.phase else {
        unreachable!()
    };
    let env = a.owners.setup.environments.read("env-1").unwrap();
    let candidate = env.propose(verification).unwrap();
    assert_eq!(candidate.recipe_revision, env.draft_revision);
    // The check script the plan pins prints the marker.
    let plan = env.draft().recipe.qualification.plan_digest.clone();
    let bytes = fs::read(a.owners.layout.artifacts().join(&plan)).unwrap();
    let plan = CheckPlan::parse(&bytes).unwrap();
    let script =
        fs::read_to_string(a.owners.layout.artifacts().join(&plan.checks[0].script)).unwrap();
    assert!(script.contains("'make build'") && script.contains("OA-CHECK passed=1"));
    assert_eq!(digest(script.as_bytes()), plan.checks[0].script);
}

#[tokio::test]
async fn a_failed_attempt_retries_on_a_new_session_with_the_conversation() {
    let dir = tempfile::tempdir().unwrap();
    let provider = machines();
    let transport = FakeTransport::new(vec![]);
    transport.then_fail(TransportError::Failed("boom".into()));
    let a = agent(dir.path(), &provider, transport);
    let state = a.run(&brief(), State::new("env-1")).await;
    assert!(matches!(state.phase, Phase::Failed { .. }), "{state:?}");
    // A stopped setup keeps no machine: its session is cancelled.
    let first = a.owners.setup.sessions().read("env-1-s1").unwrap();
    assert!(first.state.terminal(), "{:?}", first.state);
    let mut state = state;
    state.retry();
    assert_eq!(state.attempt, 2);
    assert_eq!(state.session(), "env-1-s2");
    a.transport
        .then(say("Which package manager should I use?", u()));
    let state = a.run(&brief(), state).await;
    assert_eq!(
        state.phase,
        Phase::Waiting {
            question: "Which package manager should I use?".into()
        }
    );
    let text = serde_json::to_string(&state.input).unwrap();
    assert!(text.contains("The last attempt stopped"));
}

#[test]
fn the_tools_are_declared_once_each() {
    let names: BTreeSet<String> = tools()
        .iter()
        .map(|t| t["name"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(names.len(), 6);
    assert_eq!(slug("Unit tests!"), "unit-tests");
}
