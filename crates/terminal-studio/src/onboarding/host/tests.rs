use super::*;
#[test]
fn practice_reopens_without_duplicate_work_or_real_completion() {
    let root = tempfile::tempdir().unwrap();
    let config = practice::practice(root.path()).unwrap();
    assert!(config.rows().unwrap().iter().all(|r| r.complete));
    let before = std::fs::read(&config.book).unwrap();
    let head = git(&config.starter.join("repo"), &["rev-parse", "HEAD"]).unwrap();
    let again = practice::practice(root.path()).unwrap();
    assert_eq!(std::fs::read(&again.book).unwrap(), before);
    assert_eq!(
        git(&config.starter.join("repo"), &["rev-parse", "HEAD"]).unwrap(),
        head
    );
    assert_eq!(git(&config.starter.join("repo"), &["remote"]).unwrap(), "");
    let mut real = config;
    real.scope.lane = Lane::Real;
    assert!(real.rows().is_err());
}
#[test]
fn real_retained_scratch_task_is_inert_and_reader_changes_nothing() {
    use coder::task::{
        Store,
        studio::{NewGoal, Repository, Role, Seat, Studio},
    };
    let root = tempfile::tempdir().unwrap();
    let config = practice::starter(root.path(), Lane::Real).unwrap();
    let mut tasks = Store::open(&config.tasks).unwrap();
    let mut studio = Studio::open(&config.tasks).unwrap();
    studio
        .set_seat(Seat {
            name: "apprentice".into(),
            role: Role::Lead,
            route: coder::task::studio::parse_route("codex:inert-fixture").unwrap(),
            look: "starter".into(),
            desk: 0,
        })
        .unwrap();
    let (_, released) = studio
        .submit_goal(
            &mut tasks,
            NewGoal {
                text: "Inspect the isolated starter; no execution admitted.".into(),
                repository: Repository {
                    label: config.scope.workspace.clone(),
                    path: config.starter.join("repo").display().to_string(),
                },
                lead: Some("apprentice".into()),
            },
            1,
        )
        .unwrap();
    let task = tasks.show(&released.task_id).unwrap();
    assert_eq!(task.status, coder::task::Status::Queued);
    assert!(task.run.is_none());
    let mut book = config.book().unwrap();
    book.snapshot.as_mut().unwrap().view = studio.wire(&tasks, &config.tasks);
    practice::replace(&config.book, &serde_json::to_vec(&book).unwrap()).unwrap();
    let marker = config.tasks.join(coder::task::STORE_FILE);
    let marker_before = std::fs::read(&marker).unwrap();
    std::fs::write(
        config.tasks.join("pending.json"),
        b"retained pending fixture",
    )
    .unwrap();
    std::fs::write(config.tasks.join("tasks.json"), b"retained legacy fixture").unwrap();
    let rows = config.rows().unwrap();
    assert!(
        rows.iter()
            .find(|r| r.objective == Objective::ScratchGoal)
            .unwrap()
            .complete
    );
    assert_eq!(rows.iter().filter(|r| r.complete).count(), 1);
    assert_eq!(std::fs::read(&marker).unwrap(), marker_before);
    assert_eq!(
        std::fs::read(config.tasks.join("pending.json")).unwrap(),
        b"retained pending fixture"
    );
    assert_eq!(
        std::fs::read(config.tasks.join("tasks.json")).unwrap(),
        b"retained legacy fixture"
    );
    for _ in 0..4 {
        assert_eq!(config.rows().unwrap(), rows);
    }
    let missing = root.path().join("missing");
    assert!(coder::task::retained_task(&missing, "task").is_err());
    assert!(!missing.exists());
}

#[test]
fn retained_reader_refuses_legacy_and_corrupt_sources_without_writes() {
    let root = tempfile::tempdir().unwrap();
    let legacy = root.path().join("legacy");
    std::fs::create_dir(&legacy).unwrap();
    practice::write(
        &legacy.join(coder::task::LEGACY_STORE_FILE),
        b"legacy fixture",
    )
    .unwrap();
    assert!(coder::task::retained_task(&legacy, "task").is_err());
    assert_eq!(std::fs::read_dir(&legacy).unwrap().count(), 1);
    assert_eq!(
        std::fs::read(legacy.join(coder::task::LEGACY_STORE_FILE)).unwrap(),
        b"legacy fixture"
    );
    let config = practice::practice(&root.path().join("practice")).unwrap();
    let book = config.book().unwrap();
    let id = &book.snapshot.as_ref().unwrap().view.tasks[0].task;
    let file = config
        .tasks
        .join(coder::task::TASK_DIR)
        .join(format!("{id}.json"));
    std::fs::write(&file, b"corrupt retained replay").unwrap();
    assert!(coder::task::retained_task(&config.tasks, id).is_err());
    assert_eq!(std::fs::read(&file).unwrap(), b"corrupt retained replay");
}
#[test]
fn owner_proofs_refuse_other_decisions_hosts_and_stale_review() {
    let root = tempfile::tempdir().unwrap();
    let config = practice::practice(root.path()).unwrap();
    let original = std::fs::read(&config.book).unwrap();
    let mut book = config.book().unwrap();
    if let State::Completed { outcome } = &mut book.records[0].result.state {
        if let Outcome::Dispatched { receipt } = outcome.as_mut() {
            receipt.reference = "another-decision".into();
        }
    }
    practice::replace(&config.book, &serde_json::to_vec(&book).unwrap()).unwrap();
    assert!(config.rows().is_err());
    practice::replace(&config.book, &original).unwrap();
    let mut book = config.book().unwrap();
    book.terminal.as_mut().unwrap().owner.host_generation += 1;
    practice::replace(&config.book, &serde_json::to_vec(&book).unwrap()).unwrap();
    assert!(config.rows().is_err());
    practice::replace(&config.book, &original).unwrap();
    let mut book = config.book().unwrap();
    book.records[1].review.as_mut().unwrap().head = "0".repeat(40);
    practice::replace(&config.book, &serde_json::to_vec(&book).unwrap()).unwrap();
    assert!(config.rows().is_err());
}
#[test]
fn pane_preflight_preserves_existing_products_and_statuses_are_prominent() {
    let root = tempfile::tempdir().unwrap();
    let config = practice::practice(root.path()).unwrap();
    let mut products = terminal_core::resources::Products::default();
    let host = Host::Local {
        instance: digest(&config),
    };
    for index in 0..terminal_core::resources::PRODUCTS_MAX {
        products
            .open(
                PaneKind::Thread,
                &Subject::Resource {
                    resource: workbench::ResourceRef::new(
                        workbench::Kind::Thread,
                        host.clone(),
                        format!("thread-{index}"),
                    ),
                },
            )
            .unwrap();
    }
    let before = format!("{:?}", products);
    assert!(preflight(&products, &host).is_err());
    assert_eq!(format!("{:?}", products), before);
    let host = Host::Local {
        instance: digest(&config),
    };
    let desc = Adapter {
        config,
        host: host.clone(),
    }
    .describe(&Subject::Record {
        host,
        id: "onboarding".into(),
        revision: None,
    });
    assert!(desc.actions.is_empty());
    assert!(desc.detail.contains("Cost:"));
    assert_eq!(desc.detail.matches("COMPLETE").count(), 5);
    assert!(desc.detail.len() <= 2048);
}

#[test]
fn recorder_preserves_newer_owner_state_and_duplicate_acknowledgments() {
    let root = tempfile::tempdir().unwrap();
    let config = practice::practice(root.path()).unwrap();
    let book = config.book().unwrap();
    let proof = &book.records[0];
    let State::Completed { outcome } = &proof.result.state else {
        panic!()
    };
    let owner = OwnerProof {
        request: "8".repeat(64),
        operation: openagents_chat::studio::operation(&proof.route).unwrap(),
        outcome: *outcome.clone(),
        snapshot: proof.snapshot.clone(),
        review: None,
    };
    let mut fresh = book.snapshot.unwrap();
    fresh.sequence += 1;
    config.record_snapshot(fresh.clone()).unwrap();
    config.record_owner(owner.clone()).unwrap();
    assert_eq!(config.book().unwrap().snapshot.unwrap(), fresh);
    let before = std::fs::read(&config.book).unwrap();
    config.record_owner(owner).unwrap();
    assert_eq!(std::fs::read(&config.book).unwrap(), before);
    config.record_snapshot(fresh.clone()).unwrap();
    assert_eq!(std::fs::read(&config.book).unwrap(), before);
    fresh.view.tasks.clear();
    assert!(config.record_snapshot(fresh).is_err());
}
#[cfg(unix)]
#[test]
fn external_repository_symlink_is_not_an_isolated_starter() {
    let root = tempfile::tempdir().unwrap();
    let config = practice::starter(&root.path().join("starter"), Lane::Real).unwrap();
    let external = root.path().join("external");
    std::fs::rename(config.starter.join("repo"), &external).unwrap();
    std::os::unix::fs::symlink(&external, config.starter.join("repo")).unwrap();
    assert!(config.rows().is_err());
}
