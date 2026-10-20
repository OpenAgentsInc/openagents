use super::*;
use coder_host::Tasks;
use serde_json::json;
use std::collections::BTreeMap;
use std::io::Write;
use std::path::PathBuf;

struct Fixture {
    _temporary: tempfile::TempDir,
    inbox: Inbox,
    root: PathBuf,
    id: String,
}

impl Fixture {
    fn new(prompt: &str) -> Self {
        // The synthetic owner intentionally refuses every store below the
        // real ~/.openagents, including a build lease's scratch directory.
        let temporary = tempfile::tempdir().unwrap();
        let root = temporary.path().canonicalize().unwrap().join("checkout");
        std::fs::create_dir(&root).unwrap();
        let store = temporary.path().canonicalize().unwrap().join("tasks");
        let inbox = Inbox::new(
            &store,
            BTreeMap::from([
                ("checkout".into(), root.clone()),
                ("other".into(), temporary.path().join("other")),
            ]),
        );
        let id = "a".repeat(64);
        let mut store = task::Store::open(&store).unwrap();
        submit(&mut store, &id, &root, prompt);
        drop(store);
        Self {
            _temporary: temporary,
            inbox,
            root,
            id,
        }
    }
    fn query(&self, limit: u16) -> wire::PageQuery {
        wire::PageQuery {
            workspace: "checkout".into(),
            task: self.id.clone(),
            revision: None,
            cursor: None,
            limit,
        }
    }
    fn page(&self) -> wire::Page {
        self.inbox.task_read(&self.query(64)).unwrap()
    }
    fn run(&self, reply: &str) {
        task::owner::allow_scripted(&self.inbox.store, "isolated observation acceptance").unwrap();
        task::owner::scripted(&self.inbox.store, &self.id, |_, root| {
            std::fs::write(root.join("result.txt"), b"synthetic candidate\n").unwrap();
            Ok(task::owner::Scripted {
                ending: "model_finished".into(),
                reply: reply.into(),
            })
        })
        .unwrap();
    }
    fn original(&self, scope: &wire::Scope, pin: &wire::Original) -> Vec<u8> {
        let mut query = wire::OriginalQuery {
            scope: scope.clone(),
            original: pin.clone(),
            cursor: None,
            limit: 127,
        };
        let mut bytes = Vec::new();
        loop {
            let chunk = self.inbox.task_original(&query).unwrap();
            assert!(chunk.answers(&query));
            assert!(serde_json::to_vec(&chunk).unwrap().len() <= wire::MAX_REPLY_BYTES);
            bytes.extend(STANDARD.decode(chunk.data).unwrap());
            if !chunk.more_available {
                break;
            }
            query.cursor = chunk.next;
        }
        assert_eq!(task::digest_bytes(&bytes), pin.digest);
        bytes
    }
}

fn submit(store: &mut task::Store, id: &str, root: &std::path::Path, prompt: &str) {
    let command = task::Command {
        schema: task::COMMAND_SCHEMA.into(),
        command_id: format!("submit-{id}"),
        task_id: id.into(),
        expected_revision: None,
        action: task::Action::Submit {
            intent: task::TaskIntent {
                title: "Synthetic observation task".into(),
                prompt: prompt.into(),
                workspace: task::Workspace {
                    path: root.to_string_lossy().into_owned(),
                    source_revision: None,
                },
                configuration: task::RequestedConfiguration {
                    adapter: task::adapter::NAME.into(),
                    model: Some("synthetic".into()),
                },
                images: Vec::new(),
            },
        },
    };
    store.apply(&serde_json::to_vec(&command).unwrap()).unwrap();
}

fn append(path: &std::path::Path, step: atif::Step) {
    let mut file = std::fs::OpenOptions::new().append(true).open(path).unwrap();
    serde_json::to_writer(&mut file, &json!({"record":"step","step":step})).unwrap();
    file.write_all(b"\n").unwrap();
    file.sync_all().unwrap();
}

#[test]
fn lists_only_admitted_workspace_and_binds_the_complete_snapshot() {
    let fixture = Fixture::new("Read original evidence.");
    let mut store = task::Store::open(&fixture.inbox.store).unwrap();
    submit(
        &mut store,
        &"b".repeat(64),
        &fixture.root,
        "Second synthetic task.",
    );
    submit(
        &mut store,
        &"c".repeat(64),
        &fixture._temporary.path().join("other"),
        "Other workspace.",
    );
    drop(store);
    let query = wire::ListQuery {
        workspace: "checkout".into(),
        cursor: None,
        limit: 1,
    };
    let first = fixture.inbox.task_list(&query).unwrap();
    assert_eq!(first.rows[0].task, fixture.id);
    assert!(first.more_available);
    assert!(first.answers(&query));
    let next = wire::ListQuery {
        cursor: first.next.clone(),
        ..query.clone()
    };
    let second = fixture.inbox.task_list(&next).unwrap();
    assert_eq!(second.rows[0].task, "b".repeat(64));
    assert!(!second.more_available);
    fixture
        .inbox
        .cancel(
            &"d".repeat(64),
            "device",
            &fixture.id,
            1,
            "Synthetic cancellation",
        )
        .unwrap();
    assert_eq!(fixture.inbox.task_list(&next), Err(Code::Stale));
    assert_eq!(
        fixture.inbox.task_list(&wire::ListQuery {
            workspace: "unknown".into(),
            ..query
        }),
        Err(Code::Forbidden)
    );
    let read = fixture.inbox.task_read(&wire::PageQuery {
        workspace: "other".into(),
        ..fixture.query(1)
    });
    assert_eq!(read, Err(Code::Forbidden));
}

#[test]
fn queued_and_finished_evidence_keeps_outcomes_separate_and_originals_exact() {
    let fixture = Fixture::new("Preserve every original byte.");
    let queued = fixture.page();
    assert_eq!(queued.evidence.state, "not_started");
    assert_eq!(queued.scope.attempt, None);
    assert_eq!(
        (
            &*queued.verification,
            &*queued.integration,
            &*queued.termination,
            &*queued.delivery,
            &*queued.cleanup
        ),
        ("not_run", "unknown", "unknown", "unknown", "unknown")
    );
    let journal = fixture.original(
        &queued.scope,
        queued.artifacts[0].original.as_ref().unwrap(),
    );
    assert_eq!(
        journal,
        std::fs::read(
            fixture
                .inbox
                .store
                .join("task")
                .join(format!("{}.json", fixture.id))
        )
        .unwrap()
    );
    fixture.run("Synthetic run finished.");
    let finished = fixture.page();
    assert_eq!(finished.evidence.state, "sealed");
    assert_eq!(finished.scope.attempt, Some(1));
    assert_eq!(
        (
            &*finished.verification,
            &*finished.integration,
            &*finished.termination,
            &*finished.delivery,
            &*finished.cleanup
        ),
        ("not_run", "unknown", "model_finished", "complete", "clear")
    );
    assert_eq!(finished.cost_microusd, Some(0));
    let trace = finished.evidence.original.as_ref().unwrap();
    assert_eq!(
        fixture.original(&finished.scope, trace),
        std::fs::read(
            fixture
                .inbox
                .store
                .join(format!("{}.1.atif.jsonl", fixture.id))
        )
        .unwrap()
    );
    let retained = finished
        .artifacts
        .iter()
        .find(|row| row.label == "result.txt")
        .unwrap();
    assert_eq!(
        fixture.original(&finished.scope, retained.original.as_ref().unwrap()),
        b"synthetic candidate\n"
    );
    let manifest = finished
        .artifacts
        .iter()
        .find(|row| row.label == "Candidate artifact manifest")
        .unwrap();
    let manifest_bytes = fixture.original(&finished.scope, manifest.original.as_ref().unwrap());
    assert_eq!(
        serde_json::from_slice::<artifact::Manifest>(&manifest_bytes)
            .unwrap()
            .entries[0]
            .path,
        PathBuf::from("result.txt")
    );
    assert!(finished.answers(&fixture.query(64)));
}

#[test]
fn append_reconnect_preserves_the_original_prefix_and_rejects_rewrites() {
    let fixture = Fixture::new("Append-only acceptance.");
    task::owner::allow_scripted(&fixture.inbox.store, "isolated append acceptance").unwrap();
    task::owner::scripted(&fixture.inbox.store, &fixture.id, |_, _| {
        let first = fixture.inbox.task_read(&fixture.query(1)).unwrap();
        assert_eq!(first.evidence.state, "unsealed");
        let path = fixture
            .inbox
            .store
            .join(format!("{}.1.atif.jsonl", fixture.id));
        let old_pin = first.evidence.original.clone().unwrap();
        append(
            &path,
            atif::Step::said(atif::Source::System, "Synthetic appended evidence."),
        );
        let next_query = wire::PageQuery {
            revision: Some(first.scope.revision),
            cursor: first.next.clone(),
            ..fixture.query(64)
        };
        let next = fixture.inbox.task_read(&next_query).unwrap();
        assert!(next.evidence.total_steps > first.evidence.total_steps);
        assert!(next.answers(&next_query));
        assert_ne!(next.evidence.original, first.evidence.original);
        let original_query = wire::OriginalQuery {
            scope: first.scope.clone(),
            original: old_pin,
            cursor: None,
            limit: 64,
        };
        assert_eq!(
            fixture.inbox.task_original(&original_query),
            Err(Code::Stale)
        );
        let bytes = std::fs::read(&path).unwrap();
        let rewritten = String::from_utf8(bytes.clone())
            .unwrap()
            .replace("Append-only acceptance.", "Changed old prefix.");
        std::fs::write(&path, rewritten).unwrap();
        assert_eq!(fixture.inbox.task_read(&next_query), Err(Code::Stale));
        std::fs::write(&path, &bytes).unwrap();
        let mut changed = next_query.clone();
        changed.cursor.as_mut().unwrap().prefix_digest = format!("sha256:{}", "0".repeat(64));
        assert_eq!(fixture.inbox.task_read(&changed), Err(Code::Stale));
        Ok(task::owner::Scripted {
            ending: "model_finished".into(),
            reply: "Done.".into(),
        })
    })
    .unwrap();
}

#[test]
fn large_steps_and_prompts_have_explicit_gaps_with_pinned_original_chunks() {
    let fixture = Fixture::new(&"prompt ".repeat(4500));
    fixture.run(&"original reply ".repeat(4000));
    let page = fixture.page();
    assert!(page.prompt.is_empty());
    assert_eq!(page.artifacts[0].state, "prompt_oversized");
    assert!(page.evidence.steps.iter().any(|step| matches!(
        step,
        wire::Step::Gap {
            reason: wire::GapReason::Oversized,
            ..
        }
    )));
    assert!(serde_json::to_vec(&page).unwrap().len() <= wire::MAX_REPLY_BYTES);
    assert!(
        String::from_utf8(
            fixture.original(&page.scope, page.artifacts[0].original.as_ref().unwrap())
        )
        .unwrap()
        .contains(&"prompt ".repeat(4500))
    );
    let trace = fixture.original(&page.scope, page.evidence.original.as_ref().unwrap());
    assert!(
        String::from_utf8(trace)
            .unwrap()
            .contains(&"original reply ".repeat(4000))
    );
}

#[test]
fn scope_revision_source_digest_and_original_prefix_substitutions_refuse() {
    let fixture = Fixture::new("Exact source acceptance.");
    fixture.run("Done.");
    let page = fixture.page();
    let exact = wire::OriginalQuery {
        scope: page.scope.clone(),
        original: page.evidence.original.clone().unwrap(),
        cursor: None,
        limit: 64,
    };
    let mut changed = exact.clone();
    changed.scope.workspace = "other".into();
    assert_eq!(fixture.inbox.task_original(&changed), Err(Code::Forbidden));
    changed = exact.clone();
    changed.scope.revision += 1;
    assert_eq!(fixture.inbox.task_original(&changed), Err(Code::Stale));
    changed = exact.clone();
    changed.scope.attempt = Some(2);
    assert_eq!(fixture.inbox.task_original(&changed), Err(Code::Stale));
    changed = exact.clone();
    changed.scope.intent_digest = format!("sha256:{}", "0".repeat(64));
    assert_eq!(fixture.inbox.task_original(&changed), Err(Code::Stale));
    changed = exact.clone();
    changed.original.digest = format!("sha256:{}", "0".repeat(64));
    assert_eq!(fixture.inbox.task_original(&changed), Err(Code::Stale));
    changed = exact.clone();
    changed.original.source = "trace:2".into();
    assert!(fixture.inbox.task_original(&changed).is_err());
    changed = exact.clone();
    changed.original.source = "../../secret".into();
    assert!(fixture.inbox.task_original(&changed).is_err());
    changed = exact.clone();
    changed.original.bytes += 1;
    assert_eq!(fixture.inbox.task_original(&changed), Err(Code::Stale));
    let chunk = fixture.inbox.task_original(&exact).unwrap();
    changed = exact.clone();
    changed.cursor = chunk.next;
    changed.cursor.as_mut().unwrap().prefix_digest = format!("sha256:{}", "0".repeat(64));
    assert_eq!(fixture.inbox.task_original(&changed), Err(Code::Stale));
    assert_eq!(
        fixture.inbox.task_read(&wire::PageQuery {
            revision: Some(page.scope.revision + 1),
            ..fixture.query(1)
        }),
        Err(Code::Stale)
    );
    let mut cursor_query = fixture.query(1);
    cursor_query.cursor = page.next;
    assert!(fixture.inbox.task_read(&cursor_query).is_err());
}

#[test]
fn missing_damaged_mismatched_and_unavailable_sources_are_explicit() {
    let fixture = Fixture::new("Damaged evidence acceptance.");
    fixture.run("Done.");
    let path = fixture
        .inbox
        .store
        .join(format!("{}.1.atif.jsonl", fixture.id));
    let saved = std::fs::read(&path).unwrap();
    std::fs::write(&path, b"malformed\n").unwrap();
    let malformed = fixture.page();
    assert_eq!(malformed.evidence.state, "malformed");
    assert!(malformed.evidence.original.is_some());
    std::fs::write(&path, &saved).unwrap();
    std::fs::OpenOptions::new()
        .append(true)
        .open(&path)
        .unwrap()
        .write_all(b"{\"record\":")
        .unwrap();
    let damaged = fixture.page();
    assert_eq!(damaged.evidence.state, "digest_mismatch");
    assert_eq!(damaged.evidence.faults.last().unwrap().kind, "torn");
    std::fs::write(&path, &saved).unwrap();
    std::fs::remove_file(&path).unwrap();
    assert_eq!(fixture.page().evidence.state, "missing");
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(fixture.root.join("result.txt"), &path).unwrap();
        assert_eq!(fixture.page().evidence.state, "unavailable");
    }
    let entry = fixture
        .page()
        .artifacts
        .into_iter()
        .find(|row| row.label == "result.txt")
        .unwrap();
    let pin = entry.original.unwrap();
    let blob = fixture.inbox.store.join(format!(
        "artifact-{}.blob",
        pin.digest.trim_start_matches("sha256:")
    ));
    std::fs::write(blob, b"changed retained artifact").unwrap();
    let page = fixture.page();
    assert_eq!(
        page.artifacts
            .iter()
            .find(|row| row.label == "result.txt")
            .unwrap()
            .state,
        "damaged_or_unavailable"
    );
    let query = wire::OriginalQuery {
        scope: page.scope,
        original: pin,
        cursor: None,
        limit: 64,
    };
    assert_eq!(fixture.inbox.task_original(&query), Err(Code::Unavailable));
}

#[test]
fn child_positions_come_only_from_structured_original_calls() {
    let fixture = Fixture::new("A prose mention of child session fake-child is not evidence.");
    task::owner::allow_scripted(&fixture.inbox.store, "isolated structured child acceptance")
        .unwrap();
    task::owner::scripted(&fixture.inbox.store, &fixture.id, |_, _| {
        let path = fixture.inbox.store.join(format!("{}.1.atif.jsonl", fixture.id));
        for index in 0..90 {
            append(&path, atif::Step::called(atif::Call { id: format!("call-{index}"), name: "delegate".into(), arguments: json!({"agent":"codex"}), output: "Synthetic child reference.".into(),
                outcome: atif::Outcome::Completed, milliseconds: 0, purpose: None,
                extra: serde_json::from_value(json!({"schema":crate::trace::DELEGATE_CALL_SCHEMA,"capability":"codex","session_id":format!("synthetic-child-{index}")})).unwrap() }));
        }
        let page = fixture.page(); assert!(page.more_children); assert!(!page.children.is_empty());
        assert!(page.children.len() <= wire::MAX_ITEMS);
        for child in &page.children { assert_eq!(child.state, "reference_only"); assert!(child.reference.as_ref().unwrap().starts_with("synthetic-child-")); assert!(child.source_step < page.evidence.total_steps); }
        let original = fixture.original(&page.scope, page.evidence.original.as_ref().unwrap());
        assert!(String::from_utf8(original).unwrap().contains("synthetic-child-89"));
        Ok(task::owner::Scripted { ending: "model_finished".into(), reply: "Done.".into() })
    }).unwrap();
}

#[test]
fn absent_and_legacy_stores_are_never_initialized_or_migrated_by_observation() {
    let temporary = tempfile::tempdir().unwrap();
    let store = temporary.path().join("absent");
    let inbox = Inbox::new(
        &store,
        BTreeMap::from([("checkout".into(), temporary.path().join("checkout"))]),
    );
    let query = wire::ListQuery {
        workspace: "checkout".into(),
        cursor: None,
        limit: 1,
    };
    assert!(inbox.task_list(&query).is_err());
    assert!(!store.exists());
    let fixture = Fixture::new("Readonly migration refusal.");
    let marker = fixture.inbox.store.join(task::STORE_FILE);
    // Substitute a legacy marker and retain every byte for comparison.
    let native = std::fs::read(&marker).unwrap();
    let legacy = String::from_utf8(native.clone())
        .unwrap()
        .replace(task::STORE_SCHEMA, task::LEGACY_STORE_SCHEMA);
    assert_ne!(legacy.as_bytes(), native);
    std::fs::write(&marker, legacy.as_bytes()).unwrap();
    assert!(fixture.inbox.task_list(&query).is_err());
    assert_eq!(std::fs::read(marker).unwrap(), legacy.as_bytes());
}
