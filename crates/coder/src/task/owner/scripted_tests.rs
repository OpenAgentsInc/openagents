//! Scripted turns (#10572): the owner records them like any run, only in
//! a task store that admits them, and a real engine's admission rules are
//! unchanged.
use super::*;

/// A task store and a Git checkout under temporary directories, with one
/// queued studio-shaped task (Coder's adapter and a model) on the checkout.
fn studio_task() -> (tempfile::TempDir, PathBuf, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let checkout = dir.path().join("checkout");
    std::fs::create_dir(&checkout).unwrap();
    let git = |args: &[&str]| {
        let status = std::process::Command::new("git")
            .args([
                "-c",
                "user.name=Fixture",
                "-c",
                "user.email=fixture@example.invalid",
                "-c",
                "commit.gpgsign=false",
            ])
            .args(args)
            .current_dir(&checkout)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .status()
            .unwrap();
        assert!(status.success(), "git {args:?}");
    };
    git(&["init", "-q"]);
    std::fs::write(checkout.join("greeting.txt"), "hello\n").unwrap();
    git(&["add", "-A"]);
    git(&["commit", "-qm", "Seed"]);
    let store = dir.path().join("tasks");
    let mut tasks = Store::open(&store).unwrap();
    let command = Command {
        schema: COMMAND_SCHEMA.into(),
        command_id: "submit-studio".into(),
        task_id: "studio-task".into(),
        expected_revision: None,
        action: Action::Submit {
            intent: TaskIntent {
                title: "Greet".into(),
                prompt: "Greet with Hello, studio.".into(),
                workspace: Workspace {
                    path: checkout.canonicalize().unwrap().display().to_string(),
                    source_revision: None,
                },
                configuration: RequestedConfiguration {
                    adapter: super::super::adapter::NAME.into(),
                    model: Some("studio-sim".into()),
                },
                images: Vec::new(),
            },
        },
    };
    tasks.apply(&serde_json::to_vec(&command).unwrap()).unwrap();
    (dir, store, checkout)
}

fn finished(reply: &str) -> impl FnOnce(&Task, &Path) -> Result<Scripted, String> {
    let reply = reply.to_owned();
    move |_task: &Task, workspace: &Path| {
        std::fs::write(workspace.join("greeting.txt"), "Hello, studio\n")
            .map_err(|error| error.to_string())?;
        Ok(Scripted {
            ending: "model_finished".into(),
            reply,
        })
    }
}

/// The last agent message in a trace.
fn last_reply(path: &Path) -> Option<String> {
    let recording = atif::log::read(path).ok()?;
    recording.document()["steps"]
        .as_array()?
        .iter()
        .rev()
        .find(|step| {
            step["source"]
                .as_str()
                .is_some_and(|source| source.eq_ignore_ascii_case("agent"))
        })
        .and_then(|step| step["message"].as_str().map(str::to_owned))
}

#[test]
fn a_store_that_does_not_admit_scripted_turns_refuses_them() {
    let (_dir, store, _) = studio_task();
    let refused = scripted(&store, "studio-task", finished("Done.")).unwrap_err();
    assert!(matches!(refused, Error::InvalidCommand(_)), "{refused:?}");
    let task = Store::open(&store).unwrap().show("studio-task").unwrap();
    assert_eq!(task.status, Status::Queued);
    assert!(task.run.is_none());
}

#[test]
fn a_scripted_turn_is_recorded_like_any_run_and_replays() {
    let (_dir, store, checkout) = studio_task();
    allow_scripted(&store, "an owner test").unwrap();
    assert!(scripted_allowed(&store));
    let task = scripted(&store, "studio-task", finished("Greeted.")).unwrap();
    assert_eq!(task.status, Status::Finished);
    assert_eq!(task.execution, Execution::Finished);
    let run = task.run.as_ref().unwrap();
    assert_eq!(run.admission.adapter, SCRIPTED_ADAPTER);
    assert_eq!(run.admission.network, SCRIPTED_NETWORK);
    assert!(run.admission.grant.adapter_configuration.is_none());
    assert_eq!(run.effect_id.as_deref(), Some("studio-task:1:command"));
    let result = run.result.as_ref().unwrap();
    assert_eq!(result.ending, "model_finished");
    assert_eq!(result.cost_microusd, Some(0));
    assert_eq!(result.cost_status, "priced");
    assert!(result.candidate_snapshot.is_some());
    assert_eq!(
        std::fs::read_to_string(checkout.join("greeting.txt")).unwrap(),
        "Hello, studio\n"
    );
    assert_eq!(
        last_reply(&store.join(&run.admission.trace_file)).as_deref(),
        Some("Greeted.")
    );
    // The journal replays through the same transitions.
    let again = Store::open(&store).unwrap().show("studio-task").unwrap();
    assert_eq!(again, task);
    // A finished task takes no second scripted turn.
    assert!(matches!(
        scripted(&store, "studio-task", finished("Again.")),
        Err(Error::InvalidTransition)
    ));
}

#[test]
fn a_scripted_question_waits_and_a_failed_script_fails_the_turn() {
    let (_dir, store, _) = studio_task();
    allow_scripted(&store, "an owner test").unwrap();
    let asked = scripted(&store, "studio-task", |_, _| {
        Ok(Scripted {
            ending: super::super::interaction::QUESTION_ENDING.into(),
            reply: "Which greeting?".into(),
        })
    })
    .unwrap();
    assert_eq!(
        super::super::interaction::pending(&asked),
        Some(super::super::interaction::Kind::Question)
    );

    let (_dir, store, _) = studio_task();
    allow_scripted(&store, "an owner test").unwrap();
    let failed = scripted(&store, "studio-task", |_, _| Err("no script".into())).unwrap();
    assert_eq!(failed.status, Status::Finished);
    assert_eq!(failed.execution, Execution::Failed);
    assert_eq!(
        failed.run.unwrap().result.unwrap().ending,
        "scripted_failed"
    );
}

#[test]
fn only_the_scripted_shape_is_admitted_and_real_admissions_are_unchanged() {
    let (_dir, store, _) = studio_task();
    allow_scripted(&store, "an owner test").unwrap();
    let queued = Store::open(&store).unwrap().show("studio-task").unwrap();
    let recorded = scripted(&store, "studio-task", finished("Done.")).unwrap();
    let admission = recorded.run.unwrap().admission;
    let admit = |admission: Admission| {
        let record = Record {
            sequence: 1,
            task_id: queued.task_id.clone(),
            epoch: 1,
            event: Event::Admitted {
                admission: Box::new(admission),
            },
        };
        let mut tasks = BTreeMap::from([(queued.task_id.clone(), queued.clone())]);
        transition(&record, &mut tasks)
    };
    admit(admission.clone()).unwrap();

    // A scripted admission that claims another network or read scope.
    let mut network = admission.clone();
    network.network = network_policy().into();
    assert!(matches!(admit(network), Err(Error::InvalidTransition)));
    let mut scope = admission.clone();
    scope.read_scope = "workspace_and_system".into();
    assert!(matches!(admit(scope), Err(Error::InvalidTransition)));

    // The same grant under the task's own adapter is a real admission,
    // and Coder's adapter without an engine configuration is refused.
    let mut real = admission.clone();
    real.adapter = super::super::adapter::NAME.into();
    real.network = network_policy().into();
    real.read_scope = "workspace_and_system".into();
    assert!(matches!(admit(real), Err(Error::InvalidTransition)));

    // A bounded command on a task that names a model is still refused.
    let mut bounded = admission.clone();
    bounded.adapter = "bounded-command".into();
    bounded.network = network_policy().into();
    bounded.read_scope = "workspace_and_system".into();
    assert!(matches!(admit(bounded), Err(Error::InvalidTransition)));

    // A scripted admission whose program is not the store's marker.
    let mut program = admission;
    program.grant.program = Path::new("/bin/sh").canonicalize().unwrap();
    let bytes = serde_json::to_vec_pretty(&program.grant).unwrap();
    program.grant_digest = digest_bytes(&bytes);
    program.grant_request = String::from_utf8(bytes).unwrap();
    assert!(matches!(admit(program), Err(Error::InvalidTransition)));
}
