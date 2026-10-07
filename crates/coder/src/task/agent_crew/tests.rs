use super::*;
use coder_host::Principal;
use coder_host::access::crew::{Evidence, ResultKind, Subject};
use coder_host::access::{Code, Operation};

fn setup(name: &str) -> (tempfile::TempDir, Store, Record) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::new(&dir.path().join("host"), name).unwrap();
    let record = store.open(dir.path(), 100).unwrap();
    let record = store.ensure_key(record, 100).unwrap();
    (dir, store, record)
}

fn input() -> VerdictInput {
    VerdictInput {
        id: "review-1".into(),
        subject: Subject {
            kind: "issue".into(),
            reference: "github:issue/1".into(),
            revision: 3,
            sha256: "a".repeat(64),
        },
        evidence: vec![Evidence {
            reference: "host:receipt/1".into(),
            sha256: "b".repeat(64),
        }],
        result: ResultKind::NeedsEvidence,
        reason: "The owner records that independent evidence is still missing.".into(),
        question_set_sha256: Some("c".repeat(64)),
    }
}

#[test]
fn old_alice_keeps_key_attestation_and_memory_namespace() {
    let (_dir, store, record) = setup("alice");
    let owner = secp256k1::SecretKey::from_byte_array([7; 32]).unwrap();
    let record = store.attest(record, &owner, 100_000, 100).unwrap();
    let original_key = store.key().unwrap().unwrap();
    let mut old = serde_json::to_value(&record).unwrap();
    old.as_object_mut().unwrap().remove("job_role");
    old.as_object_mut().unwrap().remove("crew_charter");
    std::fs::write(
        store.dir().join("agent.json"),
        serde_json::to_vec(&old).unwrap(),
    )
    .unwrap();
    let reopened = store
        .open(std::path::Path::new(&record.workspace), 101)
        .unwrap();
    assert_eq!(reopened.pubkey, record.pubkey);
    assert_eq!(reopened.attestation, record.attestation);
    assert_eq!(store.key().unwrap().unwrap(), original_key);
    assert_eq!(reopened.job_role, None);
    assert!(reopened.requires.is_empty());
}

#[test]
fn six_templates_use_distinct_keys_and_private_memory() {
    let dir = tempfile::tempdir().unwrap();
    let mut keys = std::collections::BTreeSet::new();
    for name in ["paul", "erin", "frank", "pat", "arthur", "vanna"] {
        let store = Store::new(&dir.path().join("host"), name).unwrap();
        let record = store.open(dir.path(), 100).unwrap();
        let record = store.ensure_key(record, 100).unwrap();
        assert_eq!(record.job_role.unwrap().preset(), name);
        assert!(keys.insert(record.pubkey.unwrap()));
        assert_eq!(record.requires, ["crew-sales.v1"]);
        let memory = crate::task::agent_memory::Memory::new(store.clone(), Default::default());
        memory
            .add(
                crate::task::agent_memory::MemoryKind::Note,
                crate::task::agent_memory::Author::Owner,
                name,
                vec![],
                100,
            )
            .unwrap();
        assert_eq!(memory.entries().unwrap().len(), 1);
        assert_eq!(memory.entries().unwrap()[0].text, name);
        assert!(!store.dir().join("grants.json").exists());
        assert!(!store.dir().join("policy.json").exists());
    }
    assert_eq!(keys.len(), 6);
}

#[test]
fn signed_recommendations_survive_restart_and_replay_without_approval() {
    let (dir, store, record) = setup("paul");
    let owner = "d".repeat(64);
    let verdict = store.crew_verdict(&input(), 200, &owner).unwrap();
    assert_eq!(verdict.author, record.pubkey.unwrap());
    assert_eq!(verdict.basis, "owner_recorded_recommendation");
    let reopened = Store::new(&dir.path().join("host"), "paul").unwrap();
    assert_eq!(reopened.crew_verdicts().unwrap(), [verdict.clone()]);
    assert_eq!(
        reopened.crew_verdict(&input(), 300, &owner).unwrap(),
        verdict
    );
    let mut changed = input();
    changed.evidence[0].sha256 = "e".repeat(64);
    assert!(
        reopened
            .crew_verdict(&changed, 301, &owner)
            .unwrap_err()
            .contains("different")
    );
    assert!(!store.dir().join("approvals.json").exists());
    assert!(!store.dir().join("policy.json").exists());
    assert_eq!(
        store
            .journal(100)
            .unwrap()
            .iter()
            .filter(|e| e.kind == Kind::Judgment)
            .count(),
        1
    );
    let path = store.dir().join("verdicts/review-1.json");
    let mut body = serde_json::to_value(&verdict).unwrap();
    body["input"]["reason"] = serde_json::json!("Run a payment now.");
    std::fs::write(path, serde_json::to_vec(&body).unwrap()).unwrap();
    assert!(reopened.crew_verdicts().unwrap_err().contains("signature"));
}

#[test]
#[cfg(unix)]
fn verdict_namespace_refuses_renames_links_and_shared_files() {
    use std::os::unix::fs::PermissionsExt;
    let (_dir, store, _) = setup("paul");
    store.crew_verdict(&input(), 200, &"d".repeat(64)).unwrap();
    let original = store.dir().join("verdicts/review-1.json");
    let renamed = store.dir().join("verdicts/renamed.json");
    std::fs::rename(&original, &renamed).unwrap();
    assert!(store.crew_verdicts().unwrap_err().contains("original ID"));
    std::fs::rename(&renamed, &original).unwrap();
    let shared = store.dir().join("shared.json");
    std::fs::hard_link(&original, &shared).unwrap();
    assert!(store.crew_verdicts().unwrap_err().contains("unshared"));
    std::fs::remove_file(shared).unwrap();
    std::fs::set_permissions(&original, std::fs::Permissions::from_mode(0o644)).unwrap();
    assert!(store.crew_verdicts().unwrap_err().contains("private"));
    std::fs::set_permissions(&original, std::fs::Permissions::from_mode(0o600)).unwrap();
    let saved = store.dir().join("saved.json");
    std::fs::rename(&original, &saved).unwrap();
    std::os::unix::fs::symlink(saved, original).unwrap();
    assert!(store.crew_verdicts().unwrap_err().contains("regular file"));
}

#[test]
fn maximum_typed_verdict_collection_fits_the_native_reply_and_refuses_more() {
    let (dir, store, _) = setup("paul");
    let mut maximum = input();
    maximum.subject.reference = format!("host:{}", "x".repeat(251));
    maximum.evidence = vec![
        Evidence {
            reference: maximum.subject.reference.clone(),
            sha256: "b".repeat(64),
        };
        16
    ];
    maximum.reason = "x".repeat(1024);
    for index in 0..MAX_VERDICTS {
        maximum.id = format!(
            "review-{}{}",
            char::from(b'a' + u8::try_from(index).unwrap()),
            "x".repeat(24)
        );
        store.crew_verdict(&maximum, 200, &"d".repeat(64)).unwrap();
    }
    let agents = crate::task::agent_host::Agents::new(
        dir.path().join("host"),
        dir.path().join("tasks"),
        Default::default(),
    );
    let principal = Principal {
        device: "d".repeat(64),
        grant: None,
        epoch: None,
    };
    let data = agents
        .answer(
            "list",
            &principal,
            &Operation::ListAgentVerdicts {
                agent: "paul".into(),
            },
        )
        .unwrap();
    assert_eq!(data["verdicts"].as_array().unwrap().len(), MAX_VERDICTS);
    coder_host::access::Outcome::Agent {
        agent: Box::new(data),
    }
    .validate()
    .unwrap();
    maximum.id = "one-more".into();
    assert!(
        store
            .crew_verdict(&maximum, 201, &"d".repeat(64))
            .unwrap_err()
            .contains("retention")
    );
}

#[test]
fn a_key_replaced_after_custody_is_checked_cannot_sign_as_the_old_author() {
    #[derive(Debug)]
    struct ReplacedKey {
        original: secp256k1::SecretKey,
        calls: std::sync::atomic::AtomicUsize,
    }
    impl crate::task::agent_key::KeyStore for ReplacedKey {
        fn custody(&self) -> &'static str {
            "file"
        }
        fn load(
            &self,
            _: crate::task::agent_key::Slot<'_>,
        ) -> Result<Option<secp256k1::SecretKey>, String> {
            let call = self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(Some(if call == 0 {
                self.original
            } else {
                secp256k1::SecretKey::from_byte_array([7; 32]).unwrap()
            }))
        }
        fn store(
            &self,
            _: crate::task::agent_key::Slot<'_>,
            _: &secp256k1::SecretKey,
        ) -> Result<(), String> {
            Err("The fixture grants no key writes.".into())
        }
        fn delete(&self, _: crate::task::agent_key::Slot<'_>) -> Result<bool, String> {
            Err("The fixture grants no key deletion.".into())
        }
    }
    let (_dir, store, _) = setup("paul");
    let original = store.key().unwrap().unwrap();
    let store = Store::with_keys(
        &_dir.path().join("host"),
        "paul",
        std::sync::Arc::new(ReplacedKey {
            original,
            calls: Default::default(),
        }),
    )
    .unwrap();
    assert!(
        store
            .crew_verdict(&input(), 200, &"d".repeat(64))
            .unwrap_err()
            .contains("identity changed")
    );
    assert!(!store.dir().join("verdicts/review-1.json").exists());
}

#[test]
fn moved_members_keep_verdict_history_without_signing_or_reactivation() {
    let (_dir, store, mut record) = setup("paul");
    let original = store.crew_verdict(&input(), 200, &"d".repeat(64)).unwrap();
    record.state = super::super::agent::State::Moved;
    store.save(&record).unwrap();
    assert_eq!(
        store.crew_verdict(&input(), 201, &"d".repeat(64)).unwrap(),
        original
    );
    let mut new = input();
    new.id = "new-review".into();
    assert!(
        store
            .crew_verdict(&new, 201, &"d".repeat(64))
            .unwrap_err()
            .contains("moved")
    );
    assert!(
        store
            .crew_charter(
                JobRole::SalesLead,
                1,
                true,
                "Reactivate.",
                201,
                &"d".repeat(64)
            )
            .unwrap_err()
            .contains("moved")
    );
    assert_eq!(store.crew_verdicts().unwrap(), [original]);
    assert!(!store.dir().join("verdicts/new-review.json").exists());
}

#[test]
fn malformed_roles_verdicts_and_scope_extensions_are_refused() {
    assert!(JobRole::parse("sales-administrator").is_err());
    assert!(serde_json::from_value::<JobRole>(serde_json::json!("authority")).is_err());
    let mut bad = input();
    bad.subject.revision = 0;
    assert!(bad.validate().is_err());
    bad = input();
    bad.evidence[0].reference = "$(pay somebody)".into();
    assert!(bad.validate().is_err());
    bad = input();
    bad.question_set_sha256 = Some("fake".into());
    assert!(bad.validate().is_err());
    let mut body = serde_json::to_value(input()).unwrap();
    body["approve"] = serde_json::json!("another-member");
    assert!(serde_json::from_value::<VerdictInput>(body).is_err());
    let mut charter = serde_json::to_value(Charter::initial(JobRole::SalesLead)).unwrap();
    charter["tools"] = serde_json::json!(["pay"]);
    assert!(serde_json::from_value::<Charter>(charter).is_err());
    let (_dir, store, _) = setup("paul");
    let mut secret = input();
    secret.evidence[0].reference = format!("host:oak_{}.{}", "test", "x".repeat(32));
    assert!(store.crew_verdict(&secret, 200, &"d".repeat(64)).is_err());
    assert!(!store.dir().join("verdicts").exists());
}

#[test]
fn concurrent_native_recording_keeps_the_collection_bound() {
    let (dir, store, _) = setup("paul");
    let agents = std::sync::Arc::new(crate::task::agent_host::Agents::new(
        dir.path().join("host"),
        dir.path().join("tasks"),
        Default::default(),
    ));
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(MAX_VERDICTS + 1));
    let threads: Vec<_> = (0..=MAX_VERDICTS)
        .map(|n| {
            let agents = agents.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                let mut verdict = input();
                verdict.id = format!("review-{n}");
                barrier.wait();
                agents.owner_crew(
                    &Principal {
                        device: "d".repeat(64),
                        grant: None,
                        epoch: None,
                    },
                    &Operation::RecordAgentVerdict {
                        agent: "paul".into(),
                        verdict,
                    },
                )
            })
        })
        .collect();
    let accepted = threads
        .into_iter()
        .filter_map(|thread| thread.join().unwrap().ok())
        .count();
    assert_eq!(accepted, MAX_VERDICTS);
    assert_eq!(store.crew_verdicts().unwrap().len(), MAX_VERDICTS);
}

#[test]
fn narrowing_keeps_identity_and_disabled_drafting_refuses_requests() {
    let (dir, store, record) = setup("paul");
    let narrowed = store
        .crew_charter(
            JobRole::SalesLead,
            1,
            false,
            "Wait for owner review.",
            200,
            &"d".repeat(64),
        )
        .unwrap();
    assert_eq!(narrowed.pubkey, record.pubkey);
    assert_eq!(narrowed.roles, record.roles);
    assert_eq!(narrowed.crew_charter.as_ref().unwrap().revision, 2);
    assert!(
        store
            .crew_charter(
                JobRole::SalesLead,
                1,
                true,
                "Stale edit.",
                201,
                &"d".repeat(64)
            )
            .is_err()
    );
    let agents = crate::task::agent_host::Agents::new(
        dir.path().join("host"),
        dir.path().join("tasks"),
        Default::default(),
    );
    let request = Operation::AskAgent {
        agent: "paul".into(),
        text: "Draft a recommendation.".into(),
        workspace: None,
        context: String::new(),
        mode: coder_host::access::agent::Mode::Terminal,
        typist: false,
    };
    let owner = Principal {
        device: "d".repeat(64),
        grant: None,
        epoch: None,
    };
    assert_eq!(
        agents.answer("request", &owner, &request).unwrap_err(),
        Code::Forbidden
    );
    assert_eq!(agents.list().agents[0].job_role, Some(JobRole::SalesLead));
    assert!(!agents.list().agents[0].busy);
    let mut paused = store.load().unwrap().unwrap();
    paused.state = crate::task::agent::State::Paused;
    store.save(&paused).unwrap();
    assert_eq!(
        agents
            .answer("refused-paused", &owner, &request)
            .unwrap_err(),
        Code::Forbidden
    );
    assert_eq!(
        store.load().unwrap().unwrap().state,
        crate::task::agent::State::Paused
    );
}

#[test]
fn native_owner_mutations_refuse_a_granted_device() {
    let (dir, store, _) = setup("paul");
    let agents = crate::task::agent_host::Agents::new(
        dir.path().join("host"),
        dir.path().join("tasks"),
        Default::default(),
    );
    let device = Principal {
        device: "e".repeat(64),
        grant: Some("grant".into()),
        epoch: Some(1),
    };
    for op in [
        Operation::SetAgentCharter {
            agent: "paul".into(),
            job_role: JobRole::SalesLead,
            expected: 1,
            drafting: false,
            purpose: "Narrow the scope.".into(),
        },
        Operation::RecordAgentVerdict {
            agent: "paul".into(),
            verdict: input(),
        },
    ] {
        assert!(op.owner_agent());
        assert_eq!(
            agents.owner_crew(&device, &op).unwrap_err(),
            Code::Forbidden
        );
    }
    assert!(store.crew_verdicts().unwrap().is_empty());
    assert_eq!(
        store
            .load()
            .unwrap()
            .unwrap()
            .crew_charter
            .unwrap()
            .revision,
        1
    );
}

#[test]
fn snapshot_preserves_the_machine_scope_and_rotation_keeps_old_verdict_authors() {
    let (dir, store, record) = setup("paul");
    let owner = secp256k1::SecretKey::from_byte_array([7; 32]).unwrap();
    let record = store.attest(record, &owner, 100_000, 100).unwrap();
    let verdict = store
        .crew_verdict(&input(), 200, &super::super::agent::public_hex(&owner))
        .unwrap();
    let snapshot = crate::task::agent_lifecycle::export(
        &store,
        &Default::default(),
        crate::task::agent_lifecycle::MemoryChoice::None,
        Some(&owner),
        201,
    )
    .unwrap();
    let imported = Store::new(&dir.path().join("host"), "sales-copy").unwrap();
    let copy = crate::task::agent_lifecycle::import(
        &imported,
        &Default::default(),
        &snapshot,
        dir.path(),
        Some(&owner),
        100_000,
        202,
    )
    .unwrap();
    assert_eq!(copy.job_role, record.job_role);
    assert_eq!(copy.crew_charter, record.crew_charter);
    assert_ne!(copy.pubkey, record.pubkey);
    assert!(copy.requires.contains(&"crew-sales.v1".into()));
    crate::task::agent_lifecycle::rotate(
        &store,
        &Default::default(),
        &owner,
        "fixture rotation",
        100_000,
        203,
    )
    .unwrap();
    assert_eq!(store.crew_verdicts().unwrap(), [verdict.clone()]);
    assert_eq!(
        store
            .crew_verdict(&input(), 204, &super::super::agent::public_hex(&owner))
            .unwrap(),
        verdict
    );
}

#[test]
fn sales_scheduler_never_reads_workspace_facts_or_enables_a_generic_job() {
    let (dir, store, _) = setup("paul");
    struct NoFacts;
    impl crate::task::agent_jobs::Facts for NoFacts {
        fn head(&self, _: &std::path::Path) -> Option<String> {
            panic!("sales scope cannot read workspace facts")
        }
        fn issues(
            &self,
            _: &str,
            _: &str,
        ) -> Result<
            (
                Vec<crate::task::issue_pick::Open>,
                Vec<crate::task::issue_pick::Pull>,
            ),
            String,
        > {
            panic!("sales scope cannot read issue facts")
        }
        fn capacity(&self) -> bool {
            panic!("no autonomous sales call")
        }
    }
    let agents = crate::task::agent_host::Agents::new(
        dir.path().join("host"),
        dir.path().join("tasks"),
        Default::default(),
    )
    .with_facts(std::sync::Arc::new(NoFacts));
    agents.tick();
    assert!(
        crate::task::agent_jobs::Jobs::new(store)
            .edit("nightly-check", crate::task::agent_jobs::Edit::On, 200)
            .unwrap_err()
            .contains("separately admitted")
    );
}
