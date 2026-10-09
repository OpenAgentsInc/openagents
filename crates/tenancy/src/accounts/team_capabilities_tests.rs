use super::team_capabilities::*;
use super::{Accounts, Role, WorkspaceKind};
use crate::{Manifest, Registry, Tenant, keys};
use serde_json::json;
use std::cell::Cell;
use std::collections::BTreeMap;

struct Fixture {
    dir: tempfile::TempDir,
    accounts: Accounts,
    owner: String,
    member: String,
    workspace: String,
    owner_token: String,
    member_token: String,
    reader_token: String,
    member_key: String,
    release: Release,
}
struct Native {
    release: Release,
}
impl Sources for Native {
    fn current(&mut self, _: &[serde_json::Value]) -> Result<Verified, String> {
        Ok(Verified {
            release: self.release.clone(),
            evidence: vec![
                json!({"signed_fixture":"native unit source; process tests validate actual signed evidence"}),
            ],
        })
    }
}
impl Fixture {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let manifest = Manifest {
            v: crate::SCHEMA.into(),
            sequence: 0,
            supersedes: None,
            shared: BTreeMap::new(),
            tenants: BTreeMap::from([(
                "team".into(),
                Tenant {
                    credential: "key-ref:fixture".into(),
                    principals: vec![],
                    doors: BTreeMap::new(),
                    quota: None,
                },
            )]),
            digest: String::new(),
        };
        let registry = Registry::install(dir.path(), manifest).unwrap();
        let accounts = Accounts::install(dir.path()).unwrap();
        let issue = |actions: &[&str]| {
            keys::issue_scoped(
                dir.path(),
                registry.manifest(),
                "team",
                None,
                Some(keys::Scopes {
                    models: None,
                    actions: Some(actions.iter().map(|s| s.to_string()).collect()),
                }),
            )
            .unwrap()
        };
        let owner_key = issue(&[READ, REVIEW]);
        let member_key = issue(&[READ, ENABLE, USE]);
        let reader_key = issue(&[READ]);
        let owner = accounts
            .create_account("Owner", &[format!("key:{}", owner_key.key.id)])
            .unwrap();
        let member = accounts
            .create_account(
                "Member",
                &[
                    format!("key:{}", member_key.key.id),
                    format!("key:{}", reader_key.key.id),
                ],
            )
            .unwrap();
        let ws = accounts
            .create_workspace(&owner.id, "Team", WorkspaceKind::Organization, "team", None)
            .unwrap();
        let invitation = accounts
            .invite(&owner.id, &ws.id, Role::Member, 3600)
            .unwrap();
        accounts.accept(&member.id, &invitation.token).unwrap();
        let pin = bytes_digest(b"source");
        let release = Release {
            source: "explicit native source".into(),
            catalog: pin.clone(),
            package: format!("{}:fixture", "a".repeat(64)),
            publisher: "a".repeat(64),
            release: "b".repeat(64),
            manifest: pin.clone(),
            version: "1.0.0".into(),
            component: "explain-error".into(),
            program: pin.clone(),
            operation: "explain".into(),
            wasm: pin,
            evaluations: vec!["c".repeat(64)],
            data_requirements: vec!["explicit-request-text".into()],
            source_recipients: vec!["local-wasm".into()],
        };
        Self {
            dir,
            accounts,
            owner: owner.id,
            member: member.id,
            workspace: ws.id,
            owner_token: owner_key.token,
            member_token: member_key.token,
            reader_token: reader_key.token,
            member_key: member_key.key.id,
            release,
        }
    }
    fn source(&self) -> Native {
        Native {
            release: self.release.clone(),
        }
    }
    fn request(&self) -> Request {
        Request {
            schema: SCHEMA.into(),
            id: "grant-1".into(),
            workspace: self.workspace.clone(),
            member: self.member.clone(),
            release: self.release.clone(),
            input: bytes_digest(b"private input"),
            input_bytes: 13,
            recipient: format!("local-member:{}", self.member),
            purpose: "Separately reviewed input rights".into(),
            expires_at: super::unix_now() + 60,
        }
    }
    fn grant(&self) -> Revision {
        let request = self.request();
        self.accounts
            .team_grant(
                &self.owner_token,
                request.clone(),
                &request.digest(),
                &mut self.source(),
            )
            .unwrap()
    }
    fn use_with(
        &self,
        id: &str,
        source: &mut dyn Sources,
        calls: &Cell<u32>,
    ) -> Result<ResultOfUse, String> {
        self.accounts.team_apply(&self.workspace,&self.member_token,"grant-1",id,Action::Use,Some((b"private input",&bytes_digest(b"private input"))),source,|r,fence|{fence.before_effect()?;calls.set(calls.get()+1);Ok(Completed{receipt:json!({"original_grant":r.digest,"output_digest":bytes_digest(b"result")}),output:Some(json!("private output"))})})
    }
}
#[test]
fn exact_grant_scope_is_immutable_and_read_permission_cannot_execute_or_review() {
    let f = Fixture::new();
    let reader = f
        .accounts
        .team_permissions(&f.workspace, &f.reader_token)
        .unwrap();
    assert!(!reader.review && !reader.enable && !reader.use_capability);
    let member = f
        .accounts
        .team_permissions(&f.workspace, &f.member_token)
        .unwrap();
    assert!(!member.review && member.enable && member.use_capability);
    let owner = f
        .accounts
        .team_permissions(&f.workspace, &f.owner_token)
        .unwrap();
    assert!(owner.review && !owner.enable && !owner.use_capability);
    let request = f.request();
    assert!(
        f.accounts
            .team_grant(
                &f.member_token,
                request.clone(),
                &request.digest(),
                &mut f.source()
            )
            .is_err()
    );
    assert!(
        f.accounts
            .team_grant(
                &f.owner_token,
                request.clone(),
                "wrong approval",
                &mut f.source()
            )
            .is_err()
    );
    let r = f.grant();
    assert_eq!(
        f.accounts
            .team_grant(
                &f.owner_token,
                request.clone(),
                &request.digest(),
                &mut f.source()
            )
            .unwrap()
            .digest,
        r.digest
    );
    let mut wider = request;
    wider.recipient = "remote-recipient".into();
    assert!(
        f.accounts
            .team_grant(
                &f.owner_token,
                wider.clone(),
                &wider.digest(),
                &mut f.source()
            )
            .is_err()
    );
    let calls = Cell::new(0);
    assert!(
        f.accounts
            .team_apply(
                &f.workspace,
                &f.reader_token,
                "grant-1",
                "read-refusal",
                Action::Use,
                Some((b"private input", &bytes_digest(b"private input"))),
                &mut f.source(),
                |_, _| {
                    calls.set(1);
                    unreachable!()
                }
            )
            .is_err()
    );
    assert_eq!(calls.get(), 0);
    assert!(
        f.accounts
            .team_apply(
                &f.workspace,
                &f.owner_token,
                "grant-1",
                "owner-refusal",
                Action::Enable,
                None,
                &mut f.source(),
                |_, _| unreachable!()
            )
            .is_err()
    );
}
#[test]
fn changed_release_operation_sources_recipients_and_input_refuse_before_dispatch() {
    let f = Fixture::new();
    f.grant();
    let calls = Cell::new(0);
    for mutate in [0, 1, 2, 3, 4] {
        let mut source = f.source();
        match mutate {
            0 => source.release.manifest = bytes_digest(b"changed"),
            1 => source.release.release = "d".repeat(64),
            2 => source.release.operation = "wider".into(),
            3 => source.release.source_recipients.push("remote".into()),
            _ => source.release.source = "another source".into(),
        };
        assert!(f.use_with("new-use", &mut source, &calls).is_err());
    }
    assert!(
        f.accounts
            .team_apply(
                &f.workspace,
                &f.member_token,
                "grant-1",
                "changed-input",
                Action::Use,
                Some((b"private input plus more", &bytes_digest(b"private input"))),
                &mut f.source(),
                |_, _| unreachable!()
            )
            .is_err()
    );
    assert_eq!(calls.get(), 0);
    assert!(
        super::load(f.dir.path())
            .unwrap()
            .team_capabilities
            .effects
            .is_empty()
    );
}
#[test]
fn removed_member_and_revoked_key_or_grant_refuse_new_use() {
    let f = Fixture::new();
    let r = f.grant();
    let calls = Cell::new(0);
    f.accounts
        .team_revoke(&f.workspace, &f.owner_token, "grant-1", &r.digest)
        .unwrap();
    assert!(f.use_with("revoked-use", &mut f.source(), &calls).is_err());
    let f = Fixture::new();
    f.grant();
    keys::revoke(f.dir.path(), &f.member_key).unwrap();
    assert!(f.use_with("key-revoked", &mut f.source(), &calls).is_err());
    let f = Fixture::new();
    f.grant();
    f.accounts
        .remove_member(&f.owner, &f.workspace, &f.member)
        .unwrap();
    assert!(f.use_with("removed-use", &mut f.source(), &calls).is_err());
    assert_eq!(calls.get(), 0);
}
#[test]
fn recipient_membership_changes_require_a_new_review_without_invalidating_other_members() {
    let f = Fixture::new();
    let original = f.grant();
    let unrelated = f.accounts.create_account("Other member", &[]).unwrap();
    let invitation = f
        .accounts
        .invite(&f.owner, &f.workspace, Role::Member, 3600)
        .unwrap();
    f.accounts.accept(&unrelated.id, &invitation.token).unwrap();
    let calls = Cell::new(0);
    f.use_with("unrelated-membership", &mut f.source(), &calls)
        .unwrap();
    assert_eq!(calls.get(), 1);
    f.accounts
        .remove_member(&f.owner, &f.workspace, &f.member)
        .unwrap();
    let invitation = f
        .accounts
        .invite(&f.owner, &f.workspace, Role::Member, 3600)
        .unwrap();
    f.accounts.accept(&f.member, &invitation.token).unwrap();
    assert!(
        f.use_with("rejoined-member", &mut f.source(), &calls)
            .unwrap_err()
            .contains("membership changed")
    );
    assert!(
        f.accounts
            .team_grant(
                &f.owner_token,
                original.request.clone(),
                &original.request.digest(),
                &mut f.source()
            )
            .is_err()
    );
    let mut request = f.request();
    request.id = "rejoined-grant".into();
    let rejoined = f
        .accounts
        .team_grant(
            &f.owner_token,
            request.clone(),
            &request.digest(),
            &mut f.source(),
        )
        .unwrap();
    assert_ne!(rejoined.member_epoch, original.member_epoch);
    f.accounts
        .team_apply(
            &f.workspace,
            &f.member_token,
            "rejoined-grant",
            "fresh-review",
            Action::Use,
            Some((b"private input", &bytes_digest(b"private input"))),
            &mut f.source(),
            |_, fence| {
                fence.before_effect()?;
                calls.set(calls.get() + 1);
                Ok(Completed {
                    receipt: json!({"accepted":true}),
                    output: None,
                })
            },
        )
        .unwrap();
    f.accounts
        .set_role(&f.owner, &f.workspace, &f.member, Role::Admin)
        .unwrap();
    assert!(
        f.accounts
            .team_apply(
                &f.workspace,
                &f.member_token,
                "rejoined-grant",
                "role-changed",
                Action::Use,
                Some((b"private input", &bytes_digest(b"private input"))),
                &mut f.source(),
                |_, _| {
                    calls.set(999);
                    unreachable!()
                }
            )
            .is_err()
    );
    assert_eq!(calls.get(), 2);
}
#[test]
fn expiry_is_enforced_and_unknown_or_changed_retries_never_dispatch() {
    crate::files_only!();
    let f = Fixture::new();
    let mut expired = f.request();
    expired.expires_at = super::unix_now();
    assert!(
        f.accounts
            .team_grant(
                &f.owner_token,
                expired.clone(),
                &expired.digest(),
                &mut f.source()
            )
            .is_err()
    );
    let r = f.grant();
    let calls = Cell::new(0);
    let first = f.use_with("use-1", &mut f.source(), &calls).unwrap();
    assert!(!first.replayed);
    assert_eq!(first.effect.grant, r.digest);
    assert_eq!(calls.get(), 1);
    let reopened = Accounts::open(f.dir.path()).unwrap();
    let retry = reopened
        .team_apply(
            &f.workspace,
            &f.member_token,
            "grant-1",
            "use-1",
            Action::Use,
            Some((b"private input", &bytes_digest(b"private input"))),
            &mut f.source(),
            |_, _| unreachable!(),
        )
        .unwrap();
    assert!(retry.replayed);
    assert!(retry.output.is_none());
    assert!(
        reopened
            .team_apply(
                &f.workspace,
                &f.member_token,
                "grant-1",
                "use-1",
                Action::Enable,
                None,
                &mut f.source(),
                |_, _| unreachable!()
            )
            .is_err()
    );
    assert!(
        reopened
            .team_apply(
                &f.workspace,
                &f.member_token,
                "grant-1",
                "interrupted",
                Action::Use,
                Some((b"private input", &bytes_digest(b"private input"))),
                &mut f.source(),
                |_, _| Err("process interrupted after dispatch".into())
            )
            .is_err()
    );
    assert!(
        f.use_with("interrupted", &mut f.source(), &calls)
            .unwrap_err()
            .contains("unknown")
    );
    assert_eq!(calls.get(), 1);
    let retained = std::fs::read_to_string(f.dir.path().join("accounts.json")).unwrap();
    assert!(!retained.contains("private input"));
    assert!(!retained.contains("private output"));
    assert!(!retained.contains(&f.member_token));
}
#[test]
fn replacing_writer_lock_during_source_read_refuses_and_preserves_new_lock() {
    crate::files_only!();
    let f = Fixture::new();
    f.grant();
    struct Replaced<'a> {
        f: &'a Fixture,
    }
    impl Sources for Replaced<'_> {
        fn current(&mut self, prior: &[serde_json::Value]) -> Result<Verified, String> {
            let value = self.f.source().current(prior)?;
            std::fs::remove_file(self.f.dir.path().join("accounts.lock")).unwrap();
            std::fs::write(
                self.f.dir.path().join("accounts.lock"),
                b"replacement writer",
            )
            .unwrap();
            Ok(value)
        }
    }
    let calls = Cell::new(0);
    assert!(
        f.use_with("custody-refusal", &mut Replaced { f: &f }, &calls)
            .is_err()
    );
    assert_eq!(calls.get(), 0);
    assert_eq!(
        std::fs::read(f.dir.path().join("accounts.lock")).unwrap(),
        b"replacement writer"
    );
}
#[test]
fn native_dispatch_fence_refuses_replaced_state_after_checkpoint_without_rerun() {
    let f = Fixture::new();
    f.grant();
    let calls = Cell::new(0);
    assert!(
        f.accounts
            .team_apply(
                &f.workspace,
                &f.member_token,
                "grant-1",
                "blocked-lookup",
                Action::Use,
                Some((b"private input", &bytes_digest(b"private input"))),
                &mut f.source(),
                |_, fence| {
                    let mut store = super::load(f.dir.path()).unwrap();
                    store.sequence += 1;
                    store.supersedes = Some(store.digest.clone());
                    store.seal();
                    super::save(f.dir.path(), &store).unwrap();
                    fence.before_effect()?;
                    calls.set(1);
                    unreachable!()
                }
            )
            .is_err()
    );
    assert_eq!(calls.get(), 0);
    assert_eq!(
        super::load(f.dir.path())
            .unwrap()
            .team_capabilities
            .effects
            .values()
            .next()
            .unwrap()
            .state,
        "unknown"
    );
}
#[test]
fn current_key_is_rechecked_after_blocking_source_read_before_any_effect() {
    let f = Fixture::new();
    f.grant();
    struct Revoked<'a> {
        f: &'a Fixture,
    }
    impl Sources for Revoked<'_> {
        fn current(&mut self, prior: &[serde_json::Value]) -> Result<Verified, String> {
            let value = self.f.source().current(prior)?;
            keys::revoke(self.f.dir.path(), &self.f.member_key).unwrap();
            Ok(value)
        }
    }
    let calls = Cell::new(0);
    assert!(
        f.use_with("revoked-during-source", &mut Revoked { f: &f }, &calls)
            .is_err()
    );
    assert_eq!(calls.get(), 0);
}
#[test]
fn original_admitted_revision_survives_later_source_change_without_relabeling() {
    let f = Fixture::new();
    let r = f.grant();
    let calls = Cell::new(0);
    let first = f.use_with("first", &mut f.source(), &calls).unwrap();
    let mut changed = f.source();
    changed.release.release = "d".repeat(64);
    assert!(f.use_with("next", &mut changed, &calls).is_err());
    assert_eq!(first.effect.grant, r.digest);
    assert_eq!(first.effect.receipt.unwrap()["original_grant"], r.digest);
    assert_eq!(calls.get(), 1);
}

#[test]
fn expired_while_source_waits_refuses_before_checkpoint_or_dispatch() {
    let f = Fixture::new();
    let mut request = f.request();
    request.expires_at = super::unix_now() + 2;
    f.accounts
        .team_grant(
            &f.owner_token,
            request.clone(),
            &request.digest(),
            &mut f.source(),
        )
        .unwrap();
    struct Delayed {
        source: Native,
        expires: u64,
    }
    impl Sources for Delayed {
        fn current(&mut self, prior: &[serde_json::Value]) -> Result<Verified, String> {
            while super::unix_now() < self.expires {
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
            self.source.current(prior)
        }
    }
    let calls = Cell::new(0);
    assert!(
        f.use_with(
            "expired-after-read",
            &mut Delayed {
                source: f.source(),
                expires: request.expires_at
            },
            &calls
        )
        .unwrap_err()
        .contains("expired")
    );
    assert_eq!(calls.get(), 0);
    assert!(
        super::load(f.dir.path())
            .unwrap()
            .team_capabilities
            .effects
            .is_empty()
    );
}
#[test]
fn expiry_after_admitted_effect_does_not_relabel_its_final_receipt() {
    let f = Fixture::new();
    let mut request = f.request();
    request.expires_at = super::unix_now() + 2;
    let r = f
        .accounts
        .team_grant(
            &f.owner_token,
            request.clone(),
            &request.digest(),
            &mut f.source(),
        )
        .unwrap();
    let result = f
        .accounts
        .team_apply(
            &f.workspace,
            &f.member_token,
            "grant-1",
            "running",
            Action::Use,
            Some((b"private input", &bytes_digest(b"private input"))),
            &mut f.source(),
            |_, fence| {
                fence.before_effect()?;
                while super::unix_now() < request.expires_at {
                    std::thread::sleep(std::time::Duration::from_millis(20));
                }
                fence.after_effect()?;
                Ok(Completed {
                    receipt: json!({"original":r.digest}),
                    output: None,
                })
            },
        )
        .unwrap();
    assert_eq!(result.effect.state, "complete");
    assert_eq!(result.effect.grant, r.digest);
}
#[test]
fn another_member_sees_only_scrubbed_shared_cards_and_cannot_review() {
    let f = Fixture::new();
    f.grant();
    let registry = Registry::open(f.dir.path()).unwrap();
    let key = keys::issue_scoped(
        f.dir.path(),
        registry.manifest(),
        "team",
        None,
        Some(keys::Scopes {
            models: None,
            actions: Some([READ.into(), REVIEW.into()].into_iter().collect()),
        }),
    )
    .unwrap();
    let other = f
        .accounts
        .create_account("Other", &[format!("key:{}", key.key.id)])
        .unwrap();
    let invite = f
        .accounts
        .invite(&f.owner, &f.workspace, Role::Member, 3600)
        .unwrap();
    f.accounts.accept(&other.id, &invite.token).unwrap();
    assert!(
        f.accounts
            .team_list(&f.workspace, &key.token)
            .unwrap()
            .is_empty()
    );
    let cards = f.accounts.team_cards(&f.workspace, &key.token).unwrap();
    assert_eq!(cards.len(), 1);
    for field in ["input", "purpose", "evidence", "source"] {
        assert!(cards[0].get(field).is_none());
    }
    assert_eq!(cards[0]["execution_authorized"], false);
    let mut request = f.request();
    request.id = "other-review".into();
    request.member = other.id.clone();
    request.recipient = format!("local-member:{}", other.id);
    assert!(
        f.accounts
            .team_grant(
                &key.token,
                request.clone(),
                &request.digest(),
                &mut f.source()
            )
            .unwrap_err()
            .contains("owner or admin")
    );
    assert_eq!(
        f.accounts
            .team_list(&f.workspace, &f.owner_token)
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn refused_older_observation_cannot_erase_retained_newer_signed_knowledge() {
    struct Refused {
        observation: serde_json::Value,
    }
    impl Sources for Refused {
        fn current(&mut self, _: &[serde_json::Value]) -> Result<Verified, String> {
            Err("Refused native source observation.".into())
        }
        fn knowledge(&self) -> Vec<serde_json::Value> {
            vec![self.observation.clone()]
        }
    }
    let f = Fixture::new();
    f.grant();
    let calls = Cell::new(0);
    let newer = json!({"native_verified_signed_fixture":"newer-withdrawal"});
    let older = json!({"native_verified_signed_fixture":"older-published-head"});
    assert!(
        f.use_with(
            "observed-withdrawal",
            &mut Refused {
                observation: newer.clone()
            },
            &calls
        )
        .is_err()
    );
    assert!(
        f.use_with(
            "withheld-newer-head",
            &mut Refused {
                observation: older.clone()
            },
            &calls
        )
        .is_err()
    );
    let restarted = Accounts::open(f.dir.path()).unwrap();
    assert_eq!(
        restarted
            .team_list(&f.workspace, &f.member_token)
            .unwrap()
            .len(),
        1
    );
    let store = super::load(f.dir.path()).unwrap();
    let knowledge = store.team_capabilities.knowledge.values().next().unwrap();
    assert!(knowledge.contains(&newer) && knowledge.contains(&older));
    assert_eq!(calls.get(), 0);
    assert!(store.team_capabilities.effects.is_empty());
}

#[test]
fn oversized_combined_history_refuses_before_replacing_readable_native_state() {
    crate::files_only!();
    struct Large {
        release: Release,
        evidence: serde_json::Value,
    }
    impl Sources for Large {
        fn current(&mut self, _: &[serde_json::Value]) -> Result<Verified, String> {
            Ok(Verified {
                release: self.release.clone(),
                evidence: vec![self.evidence.clone()],
            })
        }
    }
    let f = Fixture::new();
    let mut request = f.request();
    for index in 0..3 {
        request.id = format!("bounded-{index}");
        request.release.package = format!("{}:bounded-{index}", request.release.publisher);
        let mut source = Large {
            release: request.release.clone(),
            evidence: json!({"native_fixture": "x".repeat(3 * 1024 * 1024)}),
        };
        let result = f.accounts.team_grant(
            &f.owner_token,
            request.clone(),
            &request.digest(),
            &mut source,
        );
        if index < 2 {
            assert!(result.is_ok());
        } else {
            assert!(result.unwrap_err().contains("reader bound"));
        }
    }
    let restarted = Accounts::open(f.dir.path()).unwrap();
    assert_eq!(
        restarted
            .team_list(&f.workspace, &f.member_token)
            .unwrap()
            .len(),
        2
    );
    assert!(
        std::fs::metadata(f.dir.path().join(super::ACCOUNTS))
            .unwrap()
            .len()
            < 16 * 1024 * 1024
    );
}

#[test]
fn active_native_policy_denies_new_local_plugin_effects_and_keeps_original_inspection() {
    let f = Fixture::new();
    f.grant();
    let calls = Cell::new(0);
    let original = f
        .use_with("original-before-policy", &mut f.source(), &calls)
        .unwrap();
    assert_eq!(calls.get(), 1);
    f.accounts
        .review_team_policy(
            &f.workspace,
            receipts::team_policy::Change {
                expected_digest: None,
                terms: receipts::team_policy::Terms {
                    version: 1,
                    expires_unix: super::unix_now() + 60,
                    rules: vec![],
                },
            },
            |_| {
                f.accounts
                    .authorize(&f.workspace, &f.owner)
                    .map_err(|e| e.to_string())
            },
        )
        .unwrap();
    let sequence = f.accounts.store().unwrap().sequence;
    for action in [Action::Install, Action::Enable, Action::Use] {
        let id = format!("unsupported-{action:?}").to_lowercase();
        let input = matches!(action, Action::Use)
            .then_some((b"private input".as_slice(), bytes_digest(b"private input")));
        let denied = f.accounts.team_apply(
            &f.workspace,
            &f.member_token,
            "grant-1",
            &id,
            action,
            input
                .as_ref()
                .map(|(bytes, digest)| (*bytes, digest.as_str())),
            &mut f.source(),
            |_, fence| {
                fence.before_effect()?;
                calls.set(calls.get() + 1);
                Ok(Completed {
                    receipt: json!({"unexpected":true}),
                    output: None,
                })
            },
        );
        assert!(denied.unwrap_err().contains("team policy"));
        assert_eq!(calls.get(), 1);
        assert_eq!(f.accounts.store().unwrap().sequence, sequence);
    }
    let mut new = f.request();
    new.id = "new-under-policy".into();
    assert!(
        f.accounts
            .team_grant(&f.owner_token, new.clone(), &new.digest(), &mut f.source())
            .unwrap_err()
            .contains("team policy")
    );
    assert_eq!(f.accounts.store().unwrap().sequence, sequence);
    assert_eq!(
        f.accounts
            .team_list(&f.workspace, &f.reader_token)
            .unwrap()
            .len(),
        1
    );
    let replay = f
        .use_with("original-before-policy", &mut f.source(), &calls)
        .unwrap();
    assert!(replay.replayed);
    assert_eq!(
        serde_json::to_value(replay.effect).unwrap(),
        serde_json::to_value(original.effect).unwrap()
    );
    assert_eq!(calls.get(), 1);
}
