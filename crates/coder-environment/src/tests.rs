use crate::store::{Store, StoreError};
use crate::transition::{BuildObservation as B, VerificationObservation as V};
use crate::*;

fn d(c: char) -> String {
    c.to_string().repeat(64)
}
fn recipe(install: char) -> Recipe {
    Recipe {
        schema: RECIPE_SCHEMA.into(),
        base: ImagePin {
            provider: Provider::Boat,
            image_id: "coder-base-2026-10-08".into(),
            digest: d('a'),
        },
        runtime: ArtifactPin {
            revision: "rt-1".into(),
            digest: d('b'),
        },
        platform: Platform {
            os: "linux".into(),
            architecture: "x86_64".into(),
        },
        install: Script {
            cwd: ".".into(),
            digest: d(install),
        },
        start: Start::default(),
        inputs: Inputs {
            toolchain: Some(d('c')),
            locks: [("Cargo.lock".to_string(), d('d'))].into(),
        },
        credential_names: ["GH_TOKEN".to_string()].into(),
        qualification: Qualification {
            profile: "rust-library".into(),
            plan_digest: d('e'),
        },
        limits: Limits {
            deadline_seconds: 3600,
            concurrent_machines: 2,
            total_machine_allocations: 4,
            output_bytes: 1 << 28,
        },
        capture: Default::default(),
    }
}
fn env() -> Environment {
    Environment::new(
        "env-1",
        ProjectLink {
            workspace: "ws".into(),
            project: "proj".into(),
        },
        SourcePin {
            repository: Some("OpenAgentsInc/example".into()),
            revision: "0".repeat(40),
            digest: d('f'),
        },
        recipe('1'),
        1,
    )
    .unwrap()
}
fn image(c: char) -> ImageIdentity {
    ImageIdentity {
        provider: Provider::Boat,
        image_id: format!("env-1-candidate-{c}"),
        snapshot_id: Some(format!("snap-{c}")),
        manifest_digest: d(c),
    }
}
fn run(job: &str) -> RunLink {
    RunLink {
        cloud_job: job.into(),
        task: Some(format!("task-{job}")),
    }
}
#[track_caller]
fn step(env: &Environment, command: Command) -> (Environment, Effect) {
    match apply(env, &command, env.revision + 100).unwrap() {
        Applied::Changed(next, effect) => {
            assert!(env.preserves_history_of(&next));
            next.validate().unwrap();
            (*next, effect)
        }
        Applied::Replayed(e) => panic!("expected a change, got replay {e:?}"),
    }
}
#[track_caller]
fn refuse(env: &Environment, command: Command) -> Refusal {
    apply(env, &command, 0).unwrap_err()
}
#[track_caller]
fn replay(env: &Environment, command: Command) -> Effect {
    match apply(env, &command, 0).unwrap() {
        Applied::Replayed(e) => e,
        Applied::Changed(_, e) => panic!("expected replay, got change {e:?}"),
    }
}
fn build(id: &str) -> impl Fn(B) -> Command {
    let id = id.to_string();
    move |observation| Command::ObserveBuild {
        build_id: id.clone(),
        observation,
    }
}
fn verify(id: &str) -> impl Fn(V) -> Command {
    let id = id.to_string();
    move |observation| Command::ObserveVerification {
        verification_id: id.clone(),
        observation,
    }
}
fn start_build(request: &str, draft: u64) -> Command {
    Command::StartBuild {
        request_id: request.into(),
        expected_draft_revision: draft,
    }
}
fn start_verify(request: &str, build_id: &str) -> Command {
    Command::StartVerification {
        request_id: request.into(),
        build_id: build_id.into(),
        plan_digest: d('e'),
    }
}
/// The review a reviewer grants after seeing `verification`'s candidate on
/// `e`. When `e` cannot propose it (already saved, not passed), the review
/// carries the candidate that was displayed on the verified record.
pub(crate) fn review(e: &Environment, id: &str, verification: &str) -> Review {
    let candidate = e.propose(verification).unwrap_or_else(|_| {
        let mut c = verified().propose("verify-1").unwrap();
        c.verification_id = verification.into();
        c
    });
    Review {
        id: id.into(),
        actor: "reviewer-1".into(),
        candidate,
        granted_ms: 1,
        expires_ms: 1_000_000,
    }
}
fn save(e: &Environment, request: &str, verification: &str) -> Command {
    Command::SaveVersion {
        request_id: request.into(),
        review: review(e, &format!("rev-{request}"), verification),
    }
}

/// Draft → ready build → passed verification, on draft revision 1.
fn verified() -> Environment {
    let e = env();
    let (e, _) = step(&e, start_build("req-b1", 1));
    let b = build("build-1");
    let (e, _) = step(&e, b(B::Linked { run: run("job-b1") }));
    let (e, _) = step(
        &e,
        b(B::Progress {
            state: BuildState::Installing,
        }),
    );
    let (e, _) = step(&e, b(B::ImageReady { image: image('1') }));
    let (e, _) = step(&e, start_verify("req-v1", "build-1"));
    let v = verify("verify-1");
    let (e, _) = step(&e, v(V::Linked { run: run("job-v1") }));
    let (e, _) = step(
        &e,
        v(V::Passed {
            evidence_digest: d('9'),
            evidence: crate::evidence::EvidenceStatus::Complete,
        }),
    );
    e
}

#[test]
fn saved_version_links_exact_source_recipe_build_and_verifier_runs() {
    let e = verified();
    let (e, effect) = step(&e, save(&e, "req-s1", "verify-1"));
    assert_eq!(
        effect,
        Effect::VersionSaved {
            version_id: "v1".into()
        }
    );
    let v = e.version("v1").unwrap();
    assert_eq!(v.parent, None);
    assert_eq!(v.recipe_revision, 1);
    assert_eq!(v.recipe_digest, recipe('1').digest());
    assert_eq!(v.source, e.source);
    assert_eq!(v.base, recipe('1').base);
    assert_eq!(v.image, image('1'));
    assert_eq!(
        (v.build_id.as_str(), &v.build_run),
        ("build-1", &run("job-b1"))
    );
    assert_eq!(
        (v.verification_id.as_str(), &v.verification_run),
        ("verify-1", &run("job-v1"))
    );
    assert_eq!(v.evidence_digest, d('9'));
    assert_eq!(v.review.as_ref().unwrap().actor, "reviewer-1");

    let (e, effect) = step(
        &e,
        Command::Select {
            request_id: "req-p1".into(),
            expected_selection_revision: 0,
            version_id: "v1".into(),
        },
    );
    assert_eq!(e.active().unwrap().id, "v1");
    assert_eq!(
        effect,
        Effect::Selected {
            version_id: "v1".into(),
            selection_revision: 1,
            previous: None,
            change: SelectionKind::Selected,
        }
    );
}

#[test]
fn stale_draft_revisions_are_refused_and_lost_replies_replay() {
    let e = env();
    let update = |expected, c| Command::UpdateRecipe {
        expected_draft_revision: expected,
        recipe: recipe(c),
    };
    let (e, effect) = step(&e, update(1, '2'));
    assert_eq!(
        effect,
        Effect::RecipeRevised {
            revision: 2,
            digest: recipe('2').digest()
        }
    );
    assert_eq!(e.draft().parent_digest, Some(recipe('1').digest()));
    // The same update after a lost reply is the retained revision.
    assert_eq!(replay(&e, update(1, '2')), effect);
    // A different edit against the old revision loses.
    assert_eq!(
        refuse(&e, update(1, '3')),
        Refusal::StaleDraft {
            expected: 1,
            current: 2
        }
    );
    assert_eq!(
        refuse(&e, start_build("req-b", 1)),
        Refusal::StaleDraft {
            expected: 1,
            current: 2
        }
    );
    let mut invalid = recipe('4');
    invalid.platform.architecture = "aarch64".into();
    assert!(matches!(
        refuse(
            &e,
            Command::UpdateRecipe {
                expected_draft_revision: 2,
                recipe: invalid
            }
        ),
        Refusal::Invalid(_)
    ));
}

#[test]
fn a_recipe_edit_stales_an_unsaved_build_and_saved_versions_never_change() {
    let e = verified();
    let (saved, _) = step(&e, save(&e, "req-s1", "verify-1"));
    let v1 = saved.version("v1").unwrap().clone();

    // Saving the same verification again: replay by request, refusal otherwise.
    assert_eq!(
        replay(&saved, save(&saved, "req-s1", "verify-1")),
        Effect::VersionSaved {
            version_id: "v1".into()
        }
    );
    assert_eq!(
        refuse(&saved, save(&saved, "req-s2", "verify-1")),
        Refusal::AlreadySaved("v1".into())
    );

    // Editing the draft stales the verified build for an unsaved copy.
    let edit = Command::UpdateRecipe {
        expected_draft_revision: 1,
        recipe: recipe('2'),
    };
    let (edited, _) = step(&e, edit.clone());
    assert_eq!(
        refuse(&edited, save(&edited, "req-s1", "verify-1")),
        Refusal::StaleDraft {
            expected: 1,
            current: 2
        }
    );

    // After an edit and a second version, v1 is byte-for-byte unchanged.
    let (e2, _) = step(&saved, edit);
    let (e2, _) = step(&e2, start_build("req-b2", 2));
    let b = build("build-2");
    let (e2, _) = step(&e2, b(B::Linked { run: run("job-b2") }));
    let (e2, _) = step(&e2, b(B::ImageReady { image: image('2') }));
    let (e2, _) = step(&e2, start_verify("req-v2", "build-2"));
    let v = verify("verify-2");
    let (e2, _) = step(&e2, v(V::Linked { run: run("job-v2") }));
    let (e2, _) = step(
        &e2,
        v(V::Passed {
            evidence_digest: d('8'),
            evidence: crate::evidence::EvidenceStatus::CompleteWithRedactions,
        }),
    );
    let (e2, _) = step(&e2, save(&e2, "req-s2b", "verify-2"));
    assert_eq!(e2.version("v1").unwrap(), &v1);
    assert_eq!(e2.version("v2").unwrap().parent.as_deref(), Some("v1"));
    assert_eq!(e2.version("v2").unwrap().recipe_revision, 2);

    // Any rewrite of a saved version or recipe revision is not a valid successor.
    let mut tampered = e2.clone();
    tampered.revision += 1;
    tampered.versions[0].image = image('3');
    assert!(!e2.preserves_history_of(&tampered));
    let mut tampered = e2.clone();
    tampered.revision += 1;
    tampered.builds[0].image = Some(image('3'));
    assert!(!e2.preserves_history_of(&tampered));
}

#[test]
fn unknown_outcomes_stay_visible_and_block_dependent_work_until_reconciled() {
    let e = env();
    let (e, _) = step(&e, start_build("req-b1", 1));
    let b = build("build-1");
    let (e, _) = step(
        &e,
        b(B::Progress {
            state: BuildState::Installing,
        }),
    );
    let (e, effect) = step(
        &e,
        b(B::Unknown {
            reason: "boat_direct_failed".into(),
        }),
    );
    assert_eq!(
        effect,
        Effect::BuildObserved {
            build_id: "build-1".into(),
            state: BuildState::NeedsReconciliation
        }
    );
    assert_eq!(e.unresolved(), vec!["build-1"]);
    // A second unknown is already recorded.
    replay(
        &e,
        b(B::Unknown {
            reason: "again".into(),
        }),
    );
    // No second install while the first may still be running.
    assert_eq!(
        refuse(&e, start_build("req-b2", 1)),
        Refusal::Unresolved("build-1".into())
    );
    assert_eq!(
        refuse(&e, start_verify("req-v1", "build-1")),
        Refusal::Unresolved("build-1".into())
    );
    // Reconciliation cannot claim an earlier phase than the one observed.
    assert_eq!(
        refuse(
            &e,
            b(B::Progress {
                state: BuildState::Provisioning
            })
        ),
        Refusal::Regression("build-1".into())
    );
    let (e, _) = step(&e, b(B::ImageReady { image: image('1') }));
    assert!(e.unresolved().is_empty());
    let history: Vec<_> = e
        .build("build-1")
        .unwrap()
        .history
        .iter()
        .map(|s| s.state)
        .collect();
    assert_eq!(
        history,
        [
            BuildState::Requested,
            BuildState::Installing,
            BuildState::NeedsReconciliation,
            BuildState::Ready
        ]
    );
    assert_eq!(
        e.build("build-1").unwrap().history[2].reason.as_deref(),
        Some("boat_direct_failed")
    );

    // The same rule holds for verification.
    let (e, _) = step(&e, start_verify("req-v1", "build-1"));
    let v = verify("verify-1");
    let (e, _) = step(
        &e,
        v(V::Unknown {
            reason: "verifier reply lost".into(),
        }),
    );
    assert_eq!(e.unresolved(), vec!["verify-1"]);
    assert_eq!(
        refuse(&e, save(&e, "req-s1", "verify-1")),
        Refusal::NotPassed("verify-1".into())
    );
    assert_eq!(
        refuse(&e, start_verify("req-v2", "build-1")),
        Refusal::Unresolved("verify-1".into())
    );
}

#[test]
fn terminal_attempts_are_final_and_conflicting_facts_are_refused() {
    let e = env();
    let (e, _) = step(&e, start_build("req-b1", 1));
    let b = build("build-1");
    let (e, _) = step(&e, b(B::Linked { run: run("job-1") }));
    assert_eq!(
        refuse(&e, b(B::Linked { run: run("job-2") })),
        Refusal::RunConflict("build-1".into())
    );
    let (e, _) = step(
        &e,
        b(B::Failed {
            reason: "install exited 1".into(),
        }),
    );
    replay(
        &e,
        b(B::Failed {
            reason: "install exited 1".into(),
        }),
    );
    assert_eq!(
        refuse(&e, b(B::ImageReady { image: image('1') })),
        Refusal::Terminal("build-1".into())
    );
    assert_eq!(
        refuse(&e, b(B::Unknown { reason: "x".into() })),
        Refusal::Terminal("build-1".into())
    );
    assert_eq!(
        refuse(&e, start_verify("req-v", "build-1")),
        Refusal::BuildNotReady("build-1".into())
    );
    assert!(matches!(
        refuse(
            &e,
            b(B::Progress {
                state: BuildState::Ready
            })
        ),
        Refusal::Invalid(_)
    ));

    // A ready build keeps exactly one image.
    let (e, _) = step(&e, start_build("req-b2", 1));
    let b2 = build("build-2");
    let (e, _) = step(&e, b2(B::ImageReady { image: image('1') }));
    replay(&e, b2(B::ImageReady { image: image('1') }));
    assert_eq!(
        refuse(&e, b2(B::ImageReady { image: image('2') })),
        Refusal::ImageConflict("build-2".into())
    );
    // The verifier must use the recipe's frozen plan.
    assert_eq!(
        refuse(
            &e,
            Command::StartVerification {
                request_id: "req-v".into(),
                build_id: "build-2".into(),
                plan_digest: d('7'),
            }
        ),
        Refusal::PlanMismatch
    );
}

#[test]
fn requests_are_idempotent_and_reused_ids_conflict() {
    let e = env();
    let (e, first) = step(&e, start_build("req-b1", 1));
    assert_eq!(replay(&e, start_build("req-b1", 1)), first);
    assert_eq!(e.builds.len(), 1);
    assert_eq!(
        refuse(&e, start_verify("req-b1", "build-1")),
        Refusal::RequestConflict("req-b1".into())
    );
    assert!(matches!(
        refuse(&e, start_build("bad id", 1)),
        Refusal::Invalid(_)
    ));
}

#[test]
fn concurrent_selections_against_one_revision_have_one_winner() {
    let (e, _) = {
        let e = verified();
        step(&e, save(&e, "req-s1", "verify-1"))
    };
    let select = |request: &str, expected| Command::Select {
        request_id: request.into(),
        expected_selection_revision: expected,
        version_id: "v1".into(),
    };
    let (won, _) = step(&e, select("req-a", 0));
    assert_eq!(
        refuse(&won, select("req-b", 0)),
        Refusal::StaleSelection {
            expected: 0,
            current: 1
        }
    );
    replay(&won, select("req-a", 0));
    assert_eq!(
        refuse(
            &won,
            Command::Select {
                request_id: "req-c".into(),
                expected_selection_revision: 1,
                version_id: "v9".into()
            }
        ),
        Refusal::UnknownVersion("v9".into())
    );
}

#[test]
fn retirement_stops_new_work_but_in_flight_attempts_still_reconcile() {
    let e = env();
    let (e, _) = step(&e, start_build("req-b1", 1));
    let (e, _) = step(&e, Command::Retire);
    replay(&e, Command::Retire);
    assert_eq!(refuse(&e, start_build("req-b2", 1)), Refusal::Retired);
    assert_eq!(
        refuse(
            &e,
            Command::UpdateRecipe {
                expected_draft_revision: 1,
                recipe: recipe('2')
            }
        ),
        Refusal::Retired
    );
    step(&e, build("build-1")(B::Cancelled));
}

#[test]
fn the_store_survives_restart_and_writes_nothing_for_replays_or_refusals() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("environments");
    {
        let store = Store::under(&root);
        store.create(&env()).unwrap();
        assert_eq!(store.create(&env()), Err(StoreError::Exists));
        store.apply("env-1", &start_build("req-b1", 1), 10).unwrap();
        store
            .apply(
                "env-1",
                &build("build-1")(B::Unknown {
                    reason: "host restarted".into(),
                }),
                11,
            )
            .unwrap();
    }
    // A fresh store over the same root: the unknown outcome is still there.
    let store = Store::under(&root);
    let e = store.read("env-1").unwrap();
    assert_eq!(e.unresolved(), vec!["build-1"]);
    let bytes = std::fs::read(root.join("env-1.json")).unwrap();

    // The original request after restart replays without a write.
    assert!(matches!(
        store.apply("env-1", &start_build("req-b1", 1), 12).unwrap(),
        Applied::Replayed(Effect::BuildRequested { .. })
    ));
    assert_eq!(
        store.apply("env-1", &start_build("req-b2", 1), 12),
        Err(StoreError::Refused(Refusal::Unresolved("build-1".into())))
    );
    assert_eq!(std::fs::read(root.join("env-1.json")).unwrap(), bytes);

    // A writer holding a stale revision is fenced; history rewrites are refused.
    let lease = store.lease("env-1").unwrap();
    assert_eq!(store.lease("env-1").err(), Some(StoreError::Busy));
    let mut next = e.clone();
    next.revision += 1;
    assert_eq!(
        lease.commit("env-1", e.revision - 1, &next),
        Err(StoreError::Fence {
            expected: e.revision - 1,
            current: e.revision
        })
    );
    next.builds[0].recipe_revision = 7;
    assert_eq!(
        lease.commit("env-1", e.revision, &next),
        Err(StoreError::Immutable)
    );
    drop(lease);
    assert_eq!(store.list().unwrap(), vec![e]);
}

#[test]
fn reads_have_no_side_effects() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("environments");
    let store = Store::under(&root);
    assert_eq!(store.read("env-1"), Err(StoreError::NotFound));
    assert_eq!(store.list(), Ok(vec![]));
    assert!(!root.exists());
    assert_eq!(store.read("../x"), Err(StoreError::InvalidId));

    store.create(&env()).unwrap();
    let listing = |root: &std::path::Path| {
        let mut names: Vec<_> = std::fs::read_dir(root)
            .unwrap()
            .map(|e| {
                let e = e.unwrap();
                let m = e.metadata().unwrap();
                (e.file_name(), m.len(), m.modified().unwrap())
            })
            .collect();
        names.sort();
        names
    };
    let before = listing(&root);
    store.read("env-1").unwrap();
    store.list().unwrap();
    assert_eq!(store.read("env-2"), Err(StoreError::NotFound));
    assert_eq!(listing(&root), before);
}

#[test]
fn incomplete_evidence_never_passes_verification_or_saves_a_version() {
    use crate::evidence::{CallIdentity, EvidenceStatus, Recorder, Redactor, StreamName};
    let tmp = tempfile::tempdir().unwrap();
    // A verifier whose output overran its evidence budget.
    let mut r =
        Recorder::create(tmp.path().join("ev"), "ev-v1", None, Redactor::new(), 64).unwrap();
    let call = CallIdentity {
        id: "check-1".into(),
        parent: None,
        run: run("job-v1"),
        tool: "shell".into(),
        request: None,
        operation: None,
    };
    r.start_call(call, &serde_json::json!({}), 1).unwrap();
    r.output("check-1", StreamName::Stdout, &[b'y'; 200])
        .unwrap();
    r.observe_boat(
        "check-1",
        &boat::CommandFrame::Exit {
            exit_code: Some(0),
            success: true,
            timed_out: false,
        },
        2,
    )
    .unwrap();
    let sealed = r.finish(3).unwrap();
    assert_eq!(sealed.status, EvidenceStatus::Incomplete);

    let e = env();
    let (e, _) = step(&e, start_build("req-b1", 1));
    let b = build("build-1");
    let (e, _) = step(&e, b(B::Linked { run: run("job-b1") }));
    let (e, _) = step(&e, b(B::ImageReady { image: image('1') }));
    let (e, _) = step(&e, start_verify("req-v1", "build-1"));
    let v = verify("verify-1");
    let (e, _) = step(&e, v(V::Linked { run: run("job-v1") }));
    let (e, effect) = step(&e, v(sealed.passed()));
    assert_eq!(
        effect,
        Effect::VerificationObserved {
            verification_id: "verify-1".into(),
            state: VerificationState::Incomplete,
        }
    );
    let attempt = e.verification("verify-1").unwrap();
    assert_eq!(
        attempt.evidence_digest.as_deref(),
        Some(sealed.digest.as_str())
    );
    assert_eq!(
        refuse(&e, save(&e, "req-s1", "verify-1")),
        Refusal::NotPassed("verify-1".into())
    );
}

#[path = "promotion_tests.rs"]
mod promotion_tests;
