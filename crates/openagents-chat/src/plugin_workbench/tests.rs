use super::*;
use std::sync::Arc;

#[derive(Default)]
struct Fake {
    calls: Mutex<Vec<Vec<String>>>,
    unknown: bool,
    failed: bool,
    malformed_reuse: bool,
}
impl Engine for Arc<Fake> {
    fn tests(&self, dir: &Path) -> Result<Vec<Test>, String> {
        if !dir.join("skills/hello.md").is_file() {
            return Err("Missing skill.".into());
        }
        Ok(vec![Test {
            name: "hello".into(),
            kind: "should-fire".into(),
            task: "Greet Ada".into(),
        }])
    }
    fn run(&self, argv: &[String], _: std::time::Duration) -> Result<Ran, String> {
        self.calls.lock().unwrap().push(argv.to_vec());
        if self.unknown {
            return Err("Acknowledgment lost.".into());
        }
        if let Some(at) = argv.iter().position(|s| s == "--output-dir") {
            let into = PathBuf::from(&argv[at + 1]).join("run1");
            fs::create_dir_all(&into).unwrap();
            let mut report: serde_json::Value = serde_json::from_str(include_str!(
                "../../../ext-eval/tests/fixtures/expected/better.report.json"
            ))
            .unwrap();
            let bytes = fs::read(Path::new(&argv[3]).join("package.json")).unwrap();
            report["subject"]["definition"]["id"] =
                serde_json::json!(format!("{}:hello/hello", "a".repeat(64)));
            report["subject"]["definition"]["artifact"]["digest"] =
                serde_json::json!(Digest::of_bytes(&bytes).as_str());
            fs::write(
                into.join("report.json"),
                serde_json::to_vec(&report).unwrap(),
            )
            .unwrap();
        }
        if argv.get(1).is_some_and(|word| word == "use") {
            if self.malformed_reuse {
                return Ok(Ran {
                    ok: true,
                    output: "Human text is not a retained route result.".into(),
                });
            }
            return Ok(Ran { ok: true, output: serde_json::json!({
                "v":"openagents.plugin-use.v1", "pin":{"id":argv[2],"version":argv[4],"digest":argv[6]},
                "thread":argv[10], "request":"use-scripted", "dispatched":"ran", "outputs":[{"digest":"sha256:scripted"}]}).to_string() });
        }
        Ok(Ran {
            ok: !self.failed,
            output: "Baseline: 1/3; subject: 3/3. Cost: unknown (scripted fixture).".into(),
        })
    }
}
fn fixture(fake: Arc<Fake>) -> (tempfile::TempDir, Owner<Arc<Fake>>, Record) {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("author-task/plugins/hello");
    fs::create_dir_all(dir.join("skills")).unwrap();
    fs::write(dir.join("skills/hello.md"), "Greet the named person.").unwrap();
    fs::write(dir.join("README.md"), "A greeting skill.").unwrap();
    fs::write(
        dir.join("package.json"),
        serde_json::to_vec(
            &serde_json::json!({"v":1,"slug":"hello","version":"0.1.0","publisher":"a".repeat(64)}),
        )
        .unwrap(),
    )
    .unwrap();
    let owner = Owner::open(tmp.path().join("kept"), fake);
    let record = owner
        .freeze(
            Source {
                flow: "flow1".into(),
                thread: "author-thread".into(),
                task: "author-task".into(),
            },
            Flow::at(crate::plugin_flow::Step::Tests, Some("hello".into())),
            &dir,
            Declarations {
                author: "a".repeat(64),
                fee_msat: None,
                payout: None,
            },
        )
        .unwrap();
    (tmp, owner, record)
}
fn request(record: &Record, id: &str, action: Action) -> Request {
    Request {
        id: id.into(),
        source: record.source.clone(),
        release: record.release.clone(),
        tree: record.tree.clone(),
        action,
    }
}

#[test]
fn scratch_flow_keeps_identity_and_separate_choices_across_reopen() {
    let fake = Arc::new(Fake::default());
    let (tmp, owner, record) = fixture(fake.clone());
    assert!(fake.calls.lock().unwrap().is_empty());
    assert!(
        owner
            .apply(request(&record, "early", Action::Publish))
            .is_err()
    );
    assert!(
        owner
            .apply(request(&record, "early-enable", Action::Enable))
            .is_err()
    );
    for (id, action) in [
        ("compare", Action::Compare),
        ("publish", Action::Publish),
        ("install", Action::Install),
        ("enable", Action::Enable),
    ] {
        let req = request(&record, id, action);
        let first = owner.apply(req.clone()).unwrap();
        assert_eq!(owner.apply(req).unwrap(), first);
    }
    let reopened = Owner::open(tmp.path().join("kept"), fake.clone());
    assert_eq!(reopened.read().unwrap().source, record.source);
    assert_eq!(fake.calls.lock().unwrap().len(), 4);
    assert!(
        reopened
            .apply(request(&record, "another-publish", Action::Publish))
            .is_err()
    );
    let reuse = Action::Reuse {
        source_task: "independent-task".into(),
        thread: "independent-thread".into(),
        request: "Greet Grace".into(),
        workspace: tmp.path().into(),
    };
    let req = request(&record, "reuse", reuse);
    reopened.apply(req.clone()).unwrap();
    reopened.apply(req).unwrap();
    let calls = fake.calls.lock().unwrap();
    assert_eq!(calls.len(), 5);
    assert_eq!(
        calls[3][..3],
        ["plugin", "enable", record.release.id.as_str()]
    );
    assert!(calls[3].contains(&record.release.digest.to_string()));
    assert!(calls[4].contains(&"independent-thread".into()));
    assert!(calls[4].contains(&record.release.version));
    assert_eq!(reopened.read().unwrap().attempts.len(), 5);
}

#[test]
fn unknown_effect_is_retained_before_send_and_never_replayed() {
    let fake = Arc::new(Fake {
        unknown: true,
        ..Fake::default()
    });
    let (tmp, owner, record) = fixture(fake.clone());
    let req = request(&record, "comparison", Action::Compare);
    assert_eq!(owner.apply(req.clone()).unwrap(), Outcome::Unknown);
    let reopened = Owner::open(tmp.path().join("kept"), fake.clone());
    assert_eq!(reopened.apply(req).unwrap(), Outcome::Unknown);
    assert!(
        reopened
            .apply(request(&record, "another", Action::Compare))
            .is_err()
    );
    assert_eq!(fake.calls.lock().unwrap().len(), 1);
}

#[test]
fn changed_release_context_request_or_reviewed_bytes_cannot_execute() {
    let fake = Arc::new(Fake::default());
    let (_tmp, owner, record) = fixture(fake.clone());
    let mut changed = request(&record, "id", Action::Compare);
    changed.release.version = "0.2.0".into();
    assert!(owner.apply(changed).is_err());
    let mut changed = request(&record, "id", Action::Compare);
    changed.source.task = "another".into();
    assert!(owner.apply(changed).is_err());
    assert!(
        owner
            .apply(request(&record, "../../escape", Action::Compare))
            .is_err()
    );
    let req = request(&record, "id", Action::Compare);
    owner.apply(req).unwrap();
    assert!(
        owner
            .apply(request(&record, "id", Action::Publish))
            .is_err()
    );
    fs::write(owner.root.join("draft/skills/hello.md"), "Other guidance.").unwrap();
    assert!(owner.reviewed_files().is_err());
    assert!(
        owner
            .apply(request(&record, "publish", Action::Publish))
            .is_err()
    );
    assert_eq!(fake.calls.lock().unwrap().len(), 1);
}

#[test]
fn failed_comparison_stays_visible_and_reuse_requires_independent_sources() {
    let fake = Arc::new(Fake {
        failed: true,
        ..Fake::default()
    });
    let (_tmp, owner, record) = fixture(fake.clone());
    owner
        .apply(request(&record, "comparison", Action::Compare))
        .unwrap();
    let kept = owner.read().unwrap();
    assert!(matches!(
        kept.attempts["comparison"].outcome,
        Outcome::Finished { ok: false, .. }
    ));
    assert!(
        owner
            .apply(request(
                &record,
                "reuse",
                Action::Reuse {
                    source_task: record.source.task.clone(),
                    thread: record.source.thread.clone(),
                    request: "Hello".into(),
                    workspace: owner.root.clone()
                }
            ))
            .is_err()
    );
    let json = serde_json::to_vec(&kept).unwrap();
    assert_eq!(serde_json::from_slice::<Record>(&json).unwrap(), kept);
}

#[cfg(unix)]
#[test]
fn private_review_refuses_links_and_keeps_private_permissions() {
    use std::os::unix::fs::{PermissionsExt, symlink};
    let fake = Arc::new(Fake::default());
    let (_tmp, owner, record) = fixture(fake.clone());
    assert_eq!(
        fs::metadata(&owner.root).unwrap().permissions().mode() & 0o777,
        0o700
    );
    assert_eq!(
        fs::metadata(owner.root.join("record.json"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
    symlink("/etc/passwd", owner.root.join("draft/secret")).unwrap();
    assert!(
        owner
            .apply(request(&record, "comparison", Action::Compare))
            .is_err()
    );
    assert!(fake.calls.lock().unwrap().is_empty());
}

#[test]
fn the_mounted_evaluation_pane_reads_only_and_exposes_exact_costs_and_failures() {
    use workbench::pane::Panes;
    use workbench::{Host, Kind, ResourceRef, Revision};
    let fake = Arc::new(Fake {
        failed: true,
        ..Fake::default()
    });
    let (tmp, owner, record) = fixture(fake.clone());
    let inspected = owner.reviewed_files().unwrap();
    assert!(
        inspected
            .iter()
            .any(|(path, bytes)| path == "README.md" && bytes == b"A greeting skill.")
    );
    owner
        .apply(request(&record, "comparison", Action::Compare))
        .unwrap();
    let subject = Subject::Resource {
        resource: ResourceRef::new(
            Kind::Evidence,
            Host::Local {
                instance: local_instance(&owner.root),
            },
            record.source.flow.clone(),
        ),
    };
    let mut wrong = subject.clone();
    if let Subject::Resource { resource } = &mut wrong {
        resource.host = Host::Local {
            instance: "b".repeat(64),
        };
    }
    assert_eq!(owner.describe(&wrong).state, PaneState::Missing);
    assert!(owner.describe(&wrong).detail.is_empty());
    let panes = Panes::default().adapter(Box::new(owner));
    for _ in 0..3 {
        let pane = panes.resolve(PaneKind::Evaluation, &subject).unwrap();
        assert_eq!(pane.state, PaneState::Ready);
        assert_eq!(pane.actions, vec!["inspect"]);
        assert!(pane.detail.contains("cost_usd"), "{}", pane.detail);
        assert!(pane.detail.contains("baseline") && pane.detail.contains("subject"));
        assert!(pane.detail.contains("unreported"));
    }
    let mut stale = subject;
    if let Subject::Resource { resource } = &mut stale {
        resource.revision = Some(Revision::Sha256("c".repeat(64)));
    }
    assert!(matches!(
        panes.resolve(PaneKind::Evaluation, &stale).unwrap().state,
        PaneState::Stale { .. }
    ));
    assert_eq!(fake.calls.lock().unwrap().len(), 1);
    assert!(tmp.path().join("kept/record.json").is_file());
}

#[test]
fn reviewed_file_panes_bind_owner_and_tree_without_execution() {
    let fake = Arc::new(Fake::default());
    let (_tmp, owner, _) = fixture(fake.clone());
    let draft = DraftPane::open(owner.root.clone());
    let subjects = draft.subjects().unwrap();
    let descriptions = subjects
        .iter()
        .map(|s| draft.describe(s))
        .collect::<Vec<_>>();
    assert!(
        descriptions
            .iter()
            .any(|d| d.title == "README.md" && d.detail == "A greeting skill.")
    );
    assert!(descriptions.iter().any(|d| d.title == "skills/hello.md"));
    assert!(descriptions.iter().any(|d| d.title == "package.json"));
    let mut other = subjects[0].clone();
    if let Subject::Resource { resource } = &mut other {
        resource.host = workbench::Host::Local {
            instance: "d".repeat(64),
        };
    }
    assert_eq!(draft.describe(&other).state, PaneState::Missing);
    fs::write(owner.root.join("draft/README.md"), "Changed after review").unwrap();
    assert_eq!(draft.describe(&subjects[0]).state, PaneState::Unavailable);
    assert!(fake.calls.lock().unwrap().is_empty());
}

#[test]
fn success_without_attributable_reuse_result_is_retained_as_failed() {
    let fake = Arc::new(Fake {
        malformed_reuse: true,
        ..Default::default()
    });
    let (tmp, owner, record) = fixture(fake.clone());
    for (id, action) in [
        ("compare", Action::Compare),
        ("install", Action::Install),
        ("enable", Action::Enable),
    ] {
        owner.apply(request(&record, id, action)).unwrap();
    }
    let req = request(
        &record,
        "reuse",
        Action::Reuse {
            source_task: "independent-task".into(),
            thread: "independent-thread".into(),
            request: "Greet Grace".into(),
            workspace: tmp.path().into(),
        },
    );
    let outcome = owner.apply(req.clone()).unwrap();
    assert!(matches!(outcome, Outcome::Finished { ok: false, .. }));
    assert!(
        matches!(&outcome, Outcome::Finished { output, .. } if output.contains("Human text is not a retained route result."))
    );
    assert_eq!(owner.apply(req).unwrap(), outcome);
    assert_eq!(fake.calls.lock().unwrap().len(), 4);
}
