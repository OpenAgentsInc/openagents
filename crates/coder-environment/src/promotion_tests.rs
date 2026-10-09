//! ENV-06: reviewed Save, promotion, rollback, history, and job pins.

use super::*;

fn promote(e: &Environment, request: &str, selection: u64, verification: &str) -> Command {
    Command::Promote {
        request_id: request.into(),
        expected_selection_revision: selection,
        review: review(e, &format!("rev-{request}"), verification),
    }
}

/// A second passed verification of the current draft, on `job`.
fn verified_again(e: &Environment, build_id: &str, n: usize, ev: char) -> Environment {
    let (e, _) = step(e, start_verify(&format!("req-v{n}"), build_id));
    let id = format!("verify-{n}");
    let v = verify(&id);
    let (e, _) = step(
        &e,
        v(V::Linked {
            run: run(&format!("job-v{n}")),
        }),
    );
    let (e, _) = step(
        &e,
        v(V::Passed {
            evidence_digest: d(ev),
            evidence: crate::evidence::EvidenceStatus::Complete,
        }),
    );
    e
}

/// v1 promoted on draft 1, then a recipe edit, build-2, verify-2 promoted as v2.
fn two_versions() -> Environment {
    let e = verified();
    let (e, _) = step(&e, promote(&e, "req-p1", 0, "verify-1"));
    let (e, _) = step(
        &e,
        Command::UpdateRecipe {
            expected_draft_revision: 1,
            recipe: recipe('2'),
        },
    );
    let (e, _) = step(&e, start_build("req-b2", 2));
    let b = build("build-2");
    let (e, _) = step(&e, b(B::Linked { run: run("job-b2") }));
    let (e, _) = step(&e, b(B::ImageReady { image: image('2') }));
    let e = verified_again(&e, "build-2", 2, '8');
    let (e, _) = step(&e, promote(&e, "req-p2", 1, "verify-2"));
    e
}

#[test]
fn reviewed_promotion_saves_an_immutable_version_and_moves_the_selection() {
    let e = verified();
    let candidate = e.propose("verify-1").unwrap();
    assert_eq!(candidate.recipe_revision, 1);
    assert_eq!(candidate.image, image('1'));
    assert_eq!(candidate.build_run, run("job-b1"));
    assert_eq!(candidate.verification_run, run("job-v1"));
    assert_eq!(candidate.evidence_digest, d('9'));
    // Proposing has no side effects.
    assert_eq!(e, verified());

    let command = promote(&e, "req-p1", 0, "verify-1");
    let (next, effect) = step(&e, command.clone());
    assert_eq!(
        effect,
        Effect::Promoted {
            version_id: "v1".into(),
            selection_revision: 1,
            previous: None,
        }
    );
    let v1 = next.version("v1").unwrap();
    let stamp = v1.review.as_ref().unwrap();
    assert_eq!(stamp.id, "rev-req-p1");
    assert_eq!(stamp.candidate_digest, candidate.digest());
    assert_eq!(
        next.selections,
        vec![SelectionChange {
            revision: 1,
            kind: SelectionKind::Promoted,
            version_id: "v1".into(),
            previous: None,
            request_id: "req-p1".into(),
            at_ms: e.revision + 100,
        }]
    );
    let pin = next.pin().unwrap();
    assert_eq!(
        (pin.version_id.as_str(), pin.selection_revision, &pin.image),
        ("v1", 1, &image('1'))
    );

    // Lost reply: the original request returns the original result.
    assert_eq!(replay(&next, command), effect);
    // The request id with another review conflicts.
    let mut other = promote(&e, "req-p1", 0, "verify-1");
    if let Command::Promote { review, .. } = &mut other {
        review.id = "rev-other".into();
    }
    assert_eq!(
        refuse(&next, other),
        Refusal::RequestConflict("req-p1".into())
    );
    // Promoting the saved verification again with a new request.
    assert_eq!(
        refuse(&next, promote(&e, "req-p9", 1, "verify-1")),
        Refusal::AlreadySaved("v1".into())
    );

    // A plain reviewed Save never moves the selection.
    let (saved, effect) = step(&e, save(&e, "req-s1", "verify-1"));
    assert_eq!(
        effect,
        Effect::VersionSaved {
            version_id: "v1".into()
        }
    );
    assert_eq!(saved.selection, Selection::default());
    assert!(saved.pin().is_none());
}

#[test]
fn a_review_is_refused_when_anything_displayed_changed() {
    let e = verified();
    let base = review(&e, "rev-1", "verify-1");
    let attempt = |mutate: &dyn Fn(&mut Candidate)| {
        let mut r = base.clone();
        mutate(&mut r.candidate);
        refuse(
            &e,
            Command::Promote {
                request_id: "req-p".into(),
                expected_selection_revision: 0,
                review: r,
            },
        )
    };
    let cases: Vec<(&str, Box<dyn Fn(&mut Candidate)>)> = vec![
        ("recipe_digest", Box::new(|c| c.recipe_digest = d('7'))),
        ("source", Box::new(|c| c.source.revision = "1".repeat(40))),
        ("base", Box::new(|c| c.base.digest = d('7'))),
        ("runtime", Box::new(|c| c.runtime.revision = "rt-2".into())),
        ("image", Box::new(|c| c.image.manifest_digest = d('7'))),
        (
            "image",
            Box::new(|c| c.image.snapshot_id = Some("snap-x".into())),
        ),
        ("build_run", Box::new(|c| c.build_run = run("job-x"))),
        (
            "verification_run",
            Box::new(|c| c.verification_run = run("job-y")),
        ),
        ("plan", Box::new(|c| c.plan_digest = d('7'))),
        ("evidence", Box::new(|c| c.evidence_digest = d('7'))),
        ("build", Box::new(|c| c.build_id = "build-9".into())),
    ];
    for (field, mutate) in cases {
        assert_eq!(attempt(&*mutate), Refusal::StaleReview(field), "{field}");
    }
    assert_eq!(
        attempt(&|c| c.environment = "env-2".into()),
        Refusal::StaleReview("environment")
    );

    // A draft edit after display: the reviewed draft is stale.
    let (edited, _) = step(
        &e,
        Command::UpdateRecipe {
            expected_draft_revision: 1,
            recipe: recipe('2'),
        },
    );
    assert_eq!(
        refuse(
            &edited,
            Command::Promote {
                request_id: "req-p".into(),
                expected_selection_revision: 0,
                review: base.clone(),
            }
        ),
        Refusal::StaleDraft {
            expected: 1,
            current: 2
        }
    );

    // Stale grant: expired, or already spent on another version.
    let expired = Command::Promote {
        request_id: "req-p".into(),
        expected_selection_revision: 0,
        review: base.clone(),
    };
    assert_eq!(
        apply(&e, &expired, base.expires_ms + 1).unwrap_err(),
        Refusal::ReviewExpired("rev-1".into())
    );
    let mut unbounded = base.clone();
    unbounded.expires_ms = unbounded.granted_ms + promotion::MAX_REVIEW_MS + 1;
    assert!(matches!(
        refuse(
            &e,
            Command::SaveVersion {
                request_id: "req-s".into(),
                review: unbounded
            }
        ),
        Refusal::Invalid(_)
    ));
    let e2 = verified_again(&e, "build-1", 2, '8');
    let (e2, _) = step(
        &e2,
        Command::SaveVersion {
            request_id: "req-s1".into(),
            review: base.clone(),
        },
    );
    let mut reused = review(&e2, "rev-1", "verify-2");
    reused.granted_ms = base.granted_ms;
    assert_eq!(
        refuse(
            &e2,
            Command::SaveVersion {
                request_id: "req-s2".into(),
                review: reused
            }
        ),
        Refusal::ReviewUsed {
            review: "rev-1".into(),
            version: "v1".into()
        }
    );
}

#[test]
fn concurrent_promotions_against_one_selection_have_one_winner_and_no_partial_save() {
    let e = verified_again(&verified(), "build-1", 2, '8');
    let a = promote(&e, "req-a", 0, "verify-1");
    let b = promote(&e, "req-b", 0, "verify-2");
    let (won, _) = step(&e, a.clone());
    assert_eq!(
        refuse(&won, b),
        Refusal::StaleSelection {
            expected: 0,
            current: 1
        }
    );
    // The loser saved nothing: no orphan version beside the winner.
    assert_eq!(won.versions.len(), 1);
    assert_eq!(won.version("v1").unwrap().verification_id, "verify-1");

    // Through the store, under its lease and fence, from two threads.
    let dir = tempfile::tempdir().unwrap();
    let store = store::Store::under(dir.path().join("environments"));
    store.create(&e).unwrap();
    let outcomes = std::thread::scope(|s| {
        let handles = [a.clone(), promote(&e, "req-b", 0, "verify-2")].map(|command| {
            let store = store.clone();
            s.spawn(move || {
                loop {
                    match store.apply("env-1", &command, 500) {
                        Err(store::StoreError::Busy) => std::thread::yield_now(),
                        other => break other,
                    }
                }
            })
        });
        handles.map(|h| h.join().unwrap())
    });
    let wins = outcomes
        .iter()
        .filter(|o| matches!(o, Ok(Applied::Changed(..))))
        .count();
    let stale = outcomes
        .iter()
        .filter(|o| {
            matches!(
                o,
                Err(store::StoreError::Refused(Refusal::StaleSelection { .. }))
            )
        })
        .count();
    assert_eq!((wins, stale), (1, 1), "{outcomes:?}");
    let retained = store.read("env-1").unwrap();
    assert_eq!(retained.versions.len(), 1);
    assert_eq!(retained.selection.revision, 1);
    // The lost reply of the winner returns its retained result.
    let winner = if retained.versions[0].verification_id == "verify-1" {
        a
    } else {
        promote(&e, "req-b", 0, "verify-2")
    };
    assert!(matches!(
        store.apply("env-1", &winner, 600).unwrap(),
        Applied::Replayed(Effect::Promoted { .. })
    ));
}

#[test]
fn rollback_selects_an_earlier_version_without_changing_any_version() {
    let e = two_versions();
    let before = e.versions.clone();
    assert_eq!(e.pin().unwrap().version_id, "v2");
    // A pin taken by a job admitted now.
    let running = e.pin().unwrap();

    let rollback = Command::Select {
        request_id: "req-r1".into(),
        expected_selection_revision: 2,
        version_id: "v1".into(),
    };
    let (rolled, effect) = step(&e, rollback.clone());
    assert_eq!(
        effect,
        Effect::Selected {
            version_id: "v1".into(),
            selection_revision: 3,
            previous: Some("v2".into()),
            change: SelectionKind::RolledBack,
        }
    );
    assert_eq!(rolled.versions, before);
    assert_eq!(replay(&rolled, rollback), effect);
    // A stale rollback loses to the one that won.
    assert_eq!(
        refuse(
            &rolled,
            Command::Select {
                request_id: "req-r2".into(),
                expected_selection_revision: 2,
                version_id: "v1".into(),
            }
        ),
        Refusal::StaleSelection {
            expected: 2,
            current: 3
        }
    );

    // New jobs pin v1 exactly; the job admitted earlier keeps v2.
    let new_job = rolled.pin().unwrap();
    assert_eq!(
        (new_job.version_id.as_str(), new_job.image.clone()),
        ("v1", image('1'))
    );
    assert_eq!(running.version_id, "v2");
    assert_eq!(running.image, image('2'));
    assert_eq!(running.recipe_revision, 2);

    // Saved history, newest first, with selection marks.
    let rows = rolled.history(None, 10);
    assert_eq!(
        rows.iter()
            .map(|r| (r.version_id.as_str(), r.selected, r.selected_at.clone()))
            .collect::<Vec<_>>(),
        vec![("v2", false, vec![2]), ("v1", true, vec![1, 3])]
    );
    assert_eq!(rows[0].reviewer.as_deref(), Some("reviewer-1"));
    assert_eq!(rolled.history(Some(2), 10).len(), 1);
    assert_eq!(rolled.history(None, 1)[0].version_id, "v2");

    // Rolling forward again is a plain selection.
    let (forward, effect) = step(
        &rolled,
        Command::Select {
            request_id: "req-f".into(),
            expected_selection_revision: 3,
            version_id: "v2".into(),
        },
    );
    assert!(matches!(
        effect,
        Effect::Selected {
            change: SelectionKind::Selected,
            ..
        }
    ));
    forward.validate().unwrap();

    // A rewritten selection history is not a valid successor.
    let mut tampered = forward.clone();
    tampered.revision += 1;
    tampered.selections[0].version_id = "v2".into();
    assert!(!forward.preserves_history_of(&tampered));

    // Retirement stops future selection; nothing is pinned for new jobs.
    let (retired, _) = step(&forward, Command::Retire);
    assert!(retired.pin().is_none());
}

#[test]
fn the_store_resolves_one_project_selection_for_new_jobs() {
    let dir = tempfile::tempdir().unwrap();
    let store = store::Store::under(dir.path().join("environments"));
    let project = ProjectLink {
        workspace: "ws".into(),
        project: "proj".into(),
    };
    assert_eq!(store.selected(&project).unwrap(), None);
    let e = two_versions();
    store.create(&e).unwrap();
    assert_eq!(store.selected(&project).unwrap().unwrap().version_id, "v2");
    let other = ProjectLink {
        workspace: "ws".into(),
        project: "other".into(),
    };
    assert_eq!(store.selected(&other).unwrap(), None);

    // A second live environment selecting for the same project is refused.
    let mut twin = e.clone();
    twin.id = "env-2".into();
    store.create(&twin).unwrap();
    assert_eq!(store.selected(&project), Err(store::StoreError::Ambiguous));
    store.apply("env-2", &Command::Retire, 900).unwrap();
    assert_eq!(
        store.selected(&project).unwrap().unwrap().environment,
        "env-1"
    );
}
