//! Retention and orphan cleanup use isolated fake providers and journals.
mod common;
use common::*;
use pay_ledger::Ledger;
use retail_cloud::{
    Error, Result, dispatch,
    fake::{FakeProvider, FakeSandbox, FakeTaskOwner},
    journal::Journal,
    offer::FundedRequest,
    retain::{self, Artifact, Artifacts, Kind, Manifest},
    sha256_hex,
};
use std::collections::BTreeMap;
struct Owner {
    manifest: Manifest,
    bytes: BTreeMap<String, Vec<u8>>,
}
impl Artifacts for Owner {
    fn manifest(&self, _: &str, _: &str) -> Result<Manifest> {
        Ok(self.manifest.clone())
    }
    fn read(&self, _: &str, _: &str, name: &str, _: usize) -> Result<Vec<u8>> {
        self.bytes
            .get(name)
            .cloned()
            .ok_or(Error::Invalid("artifact unavailable"))
    }
}
fn start(j: &mut Journal, l: &mut Ledger, p: &FakeProvider) -> (FundedRequest, String, Owner) {
    funded_account(l, "acct", 1000);
    let f = confirmed(j, "acct", "offer", &request(600));
    let r = delivered(j, l, p, &FakeSandbox::new(), &f);
    let task = dispatch::dispatch(j, l, &FakeTaskOwner::new(), &f, &rights(&f), NOW + 20)
        .unwrap()
        .task;
    let bytes: BTreeMap<String, Vec<u8>> = [
        ("patch", b"diff --git a/parser.rs b/parser.rs\n".to_vec()),
        ("checks", b"cargo test: passed\n".to_vec()),
        ("log", b"completed fixture\n".to_vec()),
    ]
    .into_iter()
    .map(|(name, bytes)| (name.into(), bytes))
    .collect();
    let manifest = Manifest {
        execution: f.execution.clone(),
        task,
        resource: r.clone(),
        source: f.admission.source.clone(),
        engine: "codex".into(),
        artifacts: bytes
            .iter()
            .map(|(name, bytes)| Artifact {
                name: name.clone(),
                kind: match name.as_str() {
                    "patch" => Kind::Patch,
                    "checks" => Kind::Checks,
                    _ => Kind::Log,
                },
                digest: sha256_hex(bytes),
                size: bytes.len(),
            })
            .collect(),
    };
    (f, r, Owner { manifest, bytes })
}
#[test]
fn restart_retains_declared_bytes_and_deletes_one_exact_resource() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("journal.sqlite");
    let mut j = Journal::open(&path).unwrap();
    let mut l = Ledger::in_memory().unwrap();
    let p = FakeProvider::new();
    let (f, r, o) = start(&mut j, &mut l, &p);
    retain::request(&mut j, &f, NOW + 21).unwrap();
    drop(j);
    let mut j = Journal::open(&path).unwrap();
    let receipt = retain::advance(&mut j, &p, &o, &f.execution, NOW + 22).unwrap();
    assert!(receipt.complete && receipt.deleted());
    assert_eq!(receipt.resources[0].resource, r);
    assert_eq!(p.delete_calls(), 1);
    assert_eq!(
        j.retained_artifact(&f, &rights(&f), "patch", NOW + 22)
            .unwrap(),
        o.bytes.get("patch").cloned()
    );
    assert_eq!(
        retain::advance(&mut j, &p, &o, &f.execution, NOW + 23).unwrap(),
        receipt
    );
    assert_eq!(p.delete_calls(), 1);
    let expired = retain::advance(&mut j, &p, &o, &f.execution, receipt.expires_at).unwrap();
    assert!(expired.expired && expired.complete);
    assert_eq!(
        j.retained_artifact(&f, &rights(&f), "patch", receipt.expires_at)
            .unwrap(),
        None
    );
}
#[test]
fn missing_artifacts_never_hide_billing_resource_or_claim_complete() {
    let mut j = Journal::in_memory().unwrap();
    let mut l = Ledger::in_memory().unwrap();
    let p = FakeProvider::new();
    let (f, _, mut o) = start(&mut j, &mut l, &p);
    o.bytes.remove("log");
    retain::request(&mut j, &f, NOW + 21).unwrap();
    let receipt = retain::advance(&mut j, &p, &o, &f.execution, NOW + 22).unwrap();
    assert!(!receipt.complete);
    assert!(receipt.deleted());
    assert!(p.active().is_empty());
    assert!(receipt.manifest.is_some());
}
#[test]
fn failed_delete_acknowledgment_reconciles_without_new_execution() {
    let mut j = Journal::in_memory().unwrap();
    let mut l = Ledger::in_memory().unwrap();
    let p = FakeProvider::new();
    let (f, _, o) = start(&mut j, &mut l, &p);
    retain::request(&mut j, &f, NOW + 21).unwrap();
    p.set_unreachable(true);
    let waiting = retain::advance(&mut j, &p, &o, &f.execution, NOW + 22).unwrap();
    assert!(!waiting.deleted());
    assert!(!waiting.discovery_complete);
    p.set_unreachable(false);
    let done = retain::advance(&mut j, &p, &o, &f.execution, NOW + 23).unwrap();
    assert!(done.deleted());
    assert_eq!(p.create_calls(), 1);
    assert!(
        dispatch::dispatch(&mut j, &l, &FakeTaskOwner::new(), &f, &rights(&f), NOW + 24).is_err()
    );
}
#[test]
fn private_artifacts_refuse_revoked_readers_and_credentials_are_not_retained() {
    let mut j = Journal::in_memory().unwrap();
    let mut l = Ledger::in_memory().unwrap();
    let p = FakeProvider::new();
    let (f, _, mut o) = start(&mut j, &mut l, &p);
    let secret = format!("debug: {KEY}\n").into_bytes();
    o.bytes.insert("log".into(), secret.clone());
    let item = o
        .manifest
        .artifacts
        .iter_mut()
        .find(|a| a.name == "log")
        .unwrap();
    item.digest = sha256_hex(&secret);
    item.size = secret.len();
    retain::request(&mut j, &f, NOW + 21).unwrap();
    let receipt = retain::advance(&mut j, &p, &o, &f.execution, NOW + 22).unwrap();
    assert!(!receipt.complete && receipt.deleted());
    assert_eq!(
        j.retained_artifact(&f, &rights(&f), "log", NOW + 22)
            .unwrap(),
        None
    );
    let mut revoked = rights(&f);
    revoked.observe.as_mut().unwrap().revoked = true;
    assert!(matches!(
        j.retained_artifact(&f, &revoked, "patch", NOW + 22),
        Err(Error::Denied(_))
    ));
}
#[test]
fn mismatched_manifest_is_incomplete_and_cleanup_still_recovers() {
    let mut j = Journal::in_memory().unwrap();
    let mut l = Ledger::in_memory().unwrap();
    let p = FakeProvider::new();
    let (f, _, mut o) = start(&mut j, &mut l, &p);
    o.manifest.source.commit = "wrong-source".into();
    retain::request(&mut j, &f, NOW + 21).unwrap();
    assert!(
        !retain::advance(&mut j, &p, &o, &f.execution, NOW + 22)
            .unwrap()
            .complete
    );
    let recovered = retain::advance(&mut j, &p, &o, &f.execution, NOW + 23).unwrap();
    assert!(!recovered.complete && recovered.deleted());
}

#[test]
fn the_service_worker_recovers_without_a_launching_client() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("journal.sqlite");
    let mut j = Journal::open(&path).unwrap();
    let mut l = Ledger::in_memory().unwrap();
    let p = FakeProvider::new();
    let (f, _, o) = start(&mut j, &mut l, &p);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
    retain::request(&mut j, &f, NOW + 21).unwrap();
    drop(j);
    let mut j = Journal::open(&path).unwrap();
    p.set_unreachable(true);
    let waiting = retain::worker_step(&mut j, &p, &o, NOW + 22).unwrap();
    assert_eq!(waiting.len(), 1);
    assert!(!waiting[0].1.as_ref().unwrap().deleted());
    p.set_unreachable(false);
    let done = retain::worker_step(&mut j, &p, &o, NOW + 23).unwrap();
    assert!(done[0].1.as_ref().unwrap().deleted());
    assert!(
        retain::worker_step(&mut j, &p, &o, NOW + 24)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn cleanup_discovers_a_created_sandbox_after_its_create_ack_is_lost() {
    let mut j = Journal::in_memory().unwrap();
    let mut l = Ledger::in_memory().unwrap();
    let p = FakeProvider::new();
    funded_account(&mut l, "acct", 1000);
    let f = confirmed(&mut j, "acct", "offer", &request(600));
    retail_cloud::reserve::reserve(&mut l, &f, &rights(&f), NOW + 2).unwrap();
    p.lose_next_ack();
    let provision =
        retail_cloud::provision::advance(&mut j, &l, &p, &f, &rights(&f), TEMPLATE, NOW + 3)
            .unwrap();
    assert!(matches!(
        provision.state,
        retail_cloud::provision::ProvisionState::Creating
    ));
    // No task or artifacts exist, but the created computer must be tracked
    // and removed under its original provider label.
    let o = Owner {
        manifest: Manifest {
            execution: f.execution.clone(),
            task: "absent".into(),
            resource: "absent".into(),
            source: f.admission.source.clone(),
            engine: "codex".into(),
            artifacts: vec![],
        },
        bytes: BTreeMap::new(),
    };
    retain::request(&mut j, &f, NOW + 4).unwrap();
    let done = retain::worker_step(&mut j, &p, &o, NOW + 5).unwrap();
    let receipt = done[0].1.as_ref().unwrap();
    assert!(receipt.deleted());
    assert!(!receipt.complete);
    assert_eq!(receipt.resources.len(), 1);
    assert_eq!(p.create_calls(), 1);
    assert_eq!(p.delete_calls(), 1);
    assert!(
        retail_cloud::provision::advance(&mut j, &l, &p, &f, &rights(&f), TEMPLATE, NOW + 6)
            .is_err()
    );
}

#[test]
fn a_vanished_client_never_has_to_request_cleanup() {
    let mut j = Journal::in_memory().unwrap();
    let mut l = Ledger::in_memory().unwrap();
    let p = FakeProvider::new();
    let (f, _, o) = start(&mut j, &mut l, &p);
    // The client vanished before submitting cleanup; the service observes
    // the maximum admitted lifetime even when the task owner is lost.
    let owner = FakeTaskOwner::new();
    owner.set_unreachable(true);
    let outcomes = retain::service_step(&mut j, &p, &owner, &o, NOW + 1900).unwrap();
    assert_eq!(outcomes.len(), 1);
    assert!(outcomes[0].1.as_ref().unwrap().deleted());
    assert!(p.active().is_empty());
    assert_eq!(j.funded(&f.execution).unwrap(), Some(f));
}

#[test]
fn a_lost_delete_reply_is_observed_before_any_duplicate_delete() {
    use retail_cloud::provision::{CreateSpec, Provider, ProviderError, Resource, ResourceState};
    struct LostDelete<'a> {
        inner: &'a FakeProvider,
        hide: std::cell::Cell<bool>,
    }
    impl Provider for LostDelete<'_> {
        fn create(&self, s: &CreateSpec) -> std::result::Result<Resource, ProviderError> {
            self.inner.create(s)
        }
        fn find(&self, p: &str) -> std::result::Result<Option<Resource>, ProviderError> {
            self.inner.find(p)
        }
        fn state(&self, r: &str) -> std::result::Result<ResourceState, ProviderError> {
            if self.hide.replace(false) {
                Err(ProviderError::Unknown("reply lost".into()))
            } else {
                self.inner.state(r)
            }
        }
        fn delete(&self, r: &str) -> std::result::Result<(), ProviderError> {
            self.inner.delete(r)?;
            self.hide.set(true);
            Err(ProviderError::Unknown("reply lost".into()))
        }
        fn usage_seconds(&self, r: &str) -> std::result::Result<Option<u64>, ProviderError> {
            self.inner.usage_seconds(r)
        }
    }
    let mut j = Journal::in_memory().unwrap();
    let mut l = Ledger::in_memory().unwrap();
    let p = FakeProvider::new();
    let (f, _, o) = start(&mut j, &mut l, &p);
    retain::request(&mut j, &f, NOW + 21).unwrap();
    let lost = LostDelete {
        inner: &p,
        hide: std::cell::Cell::new(false),
    };
    let uncertain = retain::advance(&mut j, &lost, &o, &f.execution, NOW + 22).unwrap();
    assert!(!uncertain.deleted());
    assert_eq!(p.delete_calls(), 1);
    let known = retain::advance(&mut j, &lost, &o, &f.execution, NOW + 23).unwrap();
    assert!(known.deleted());
    assert_eq!(p.delete_calls(), 1);
}
