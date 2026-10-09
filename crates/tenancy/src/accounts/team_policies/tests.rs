use super::super::super::{Role, WorkspaceKind};
use super::*;
use receipts::team_policy::{Capability, Placement, PlacementKind, Rule, Source, Terms, digest};
struct Fixture {
    _dir: tempfile::TempDir,
    accounts: Accounts,
    owner: String,
    admin: String,
    member: String,
    ws: String,
}
fn fixture() -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let accounts = Accounts::install(dir.path()).unwrap();
    let owner = accounts.create_account("Owner", &[]).unwrap().id;
    let admin = accounts.create_account("Admin", &[]).unwrap().id;
    let member = accounts.create_account("Member", &[]).unwrap().id;
    let ws = accounts
        .create_workspace(&owner, "Team", WorkspaceKind::Organization, "tenant", None)
        .unwrap()
        .id;
    for (account, role) in [(&admin, Role::Admin), (&member, Role::Member)] {
        let invite = accounts.invite(&owner, &ws, role, 3600).unwrap();
        accounts.accept_into(account, &invite.token, &ws).unwrap();
    }
    Fixture {
        _dir: dir,
        accounts,
        owner,
        admin,
        member,
        ws,
    }
}
fn actor(f: &Fixture, who: &str) -> MemberRef {
    f.accounts.authorize(&f.ws, who).unwrap()
}
fn effect() -> Effect {
    Effect {
        capability: Capability::SystemOne,
        release: digest(&"release"),
        model: Some("model".into()),
        plugin: None,
        recipients: vec![digest(&"recipient")],
        source: Source {
            request: digest(&"request"),
            material: digest(&"request"),
        },
        placement: Placement {
            kind: PlacementKind::LocalGateway,
            identity: digest(&"recipient"),
        },
    }
}
fn terms() -> Terms {
    Terms {
        version: 1,
        expires_unix: unix_now() + 3600,
        rules: vec![Rule {
            effect: effect(),
            data_classes: vec!["owner-reviewed-private".into()],
        }],
    }
}
fn review(f: &Fixture) -> Revision {
    f.accounts
        .review_team_policy(
            &f.ws,
            Change {
                expected_digest: None,
                terms: terms(),
            },
            |_| Ok(actor(f, &f.owner)),
        )
        .unwrap()
}
#[test]
fn owner_and_admin_authority_only_narrow_and_historical_snapshot_stays_original() {
    let f = fixture();
    let original = review(&f);
    let mut next = terms();
    next.version = 2;
    next.rules[0].effect.recipients.push(digest(&"new"));
    assert!(
        f.accounts
            .review_team_policy(
                &f.ws,
                Change {
                    expected_digest: Some(original.digest.clone()),
                    terms: next
                },
                |_| Ok(actor(&f, &f.admin))
            )
            .is_err()
    );
    assert!(
        f.accounts
            .review_team_policy(
                &f.ws,
                Change {
                    expected_digest: Some(original.digest.clone()),
                    terms: Terms {
                        version: 2,
                        ..terms()
                    }
                },
                |_| Ok(actor(&f, &f.member))
            )
            .is_err()
    );
    let guard = f
        .accounts
        .team_policy_guard(&effect(), None, Some("request"), |_| {
            Ok((actor(&f, &f.member), "key-one".into()))
        })
        .unwrap();
    let snapshot = guard.snapshot.clone();
    drop(guard);
    let next = f
        .accounts
        .review_team_policy(
            &f.ws,
            Change {
                expected_digest: Some(original.digest),
                terms: Terms {
                    version: 2,
                    rules: vec![],
                    expires_unix: original.terms.expires_unix,
                    ..terms()
                },
            },
            |_| Ok(actor(&f, &f.admin)),
        )
        .unwrap();
    assert_ne!(next.digest, snapshot.policy.digest);
    assert!(
        f.accounts
            .team_policy_guard(&effect(), Some(&snapshot), None, |_| Ok((
                actor(&f, &f.member),
                "key-one".into()
            )))
            .is_err()
    );
    assert_eq!(
        f.accounts.store().unwrap().team_policies.dispatches["request"].snapshot,
        snapshot
    );
    assert!(f.accounts.team_policy_handed_off("request").unwrap());
    let view = f
        .accounts
        .read_team_policy(|_| Ok(actor(&f, &f.member)))
        .unwrap();
    assert!(view.1.is_none());
}
#[test]
fn exact_scope_substitutions_and_reconnect_never_create_a_second_handoff() {
    let f = fixture();
    review(&f);
    for field in 0..7 {
        let mut proposed = effect();
        match field {
            0 => proposed.release = digest(&"replacement"),
            1 => proposed.model = Some("foreign-model".into()),
            2 => proposed.recipients = vec![digest(&"foreign")],
            3 => proposed.source.request = digest(&"changed-source"),
            4 => proposed.source.material = digest(&"private-injection"),
            5 => proposed.placement.kind = PlacementKind::CloudGateway,
            _ => {
                proposed.plugin = Some(receipts::team_policy::PluginPin {
                    publisher: "a".repeat(64),
                    release: digest(&"plugin"),
                    module: digest(&"wasm"),
                })
            }
        };
        assert!(
            f.accounts
                .team_policy_guard(&proposed, None, None, |_| Ok((
                    actor(&f, &f.member),
                    "key-one".into()
                )))
                .is_err()
        );
    }
    let guard = f
        .accounts
        .team_policy_guard(&effect(), None, Some("unknown"), |_| {
            Ok((actor(&f, &f.member), "key-one".into()))
        })
        .unwrap();
    drop(guard);
    let reopened = Accounts::open(f._dir.path()).unwrap();
    assert!(
        reopened
            .team_policy_guard(&effect(), None, Some("unknown"), |_| Ok((
                actor(&f, &f.member),
                "key-one".into()
            )))
            .is_err()
    );
    let current = actor(&f, &f.member);
    f.accounts
        .remove_member(&f.owner, &f.ws, &f.member)
        .unwrap();
    assert!(
        reopened
            .team_policy_guard(&effect(), None, None, |_| Ok((current, "key-one".into())))
            .is_err()
    );
    assert!(reopened.team_policy_handed_off("unknown").unwrap());
}
#[test]
fn expired_policy_and_changed_owner_refuse_new_effects_without_erasing_history() {
    let f = fixture();
    let r = review(&f);
    let old = f
        .accounts
        .team_policy_guard(&effect(), None, Some("old"), |_| {
            Ok((actor(&f, &f.member), "key-one".into()))
        })
        .unwrap()
        .snapshot
        .clone();
    f.accounts
        .transfer_ownership(&f.owner, &f.ws, &f.admin)
        .unwrap();
    assert!(
        f.accounts
            .team_policy_guard(&effect(), None, None, |_| Ok((
                actor(&f, &f.member),
                "key-one".into()
            )))
            .is_err()
    );
    assert_eq!(
        f.accounts.store().unwrap().team_policies.dispatches["old"].snapshot,
        old
    );
    let mut expired = terms();
    expired.version = 2;
    expired.expires_unix = unix_now();
    assert!(
        f.accounts
            .review_team_policy(
                &f.ws,
                Change {
                    expected_digest: Some(r.digest),
                    terms: expired
                },
                |_| Ok(actor(&f, &f.admin))
            )
            .is_err()
    );
}

#[test]
fn policy_disclosure_guard_refuses_replaced_native_state_after_sealed_admission() {
    crate::files_only!();
    let f = fixture();
    review(&f);
    let member = actor(&f, &f.member);
    let guard = f
        .accounts
        .team_policy_guard(&effect(), None, Some("custody-original"), |_| {
            Ok((member.clone(), "key:fixture".into()))
        })
        .unwrap();
    guard.before_effect().unwrap();
    let current = f._dir.path().join("accounts.json");
    let original = f._dir.path().join("held-original.json");
    std::fs::rename(&current, &original).unwrap();
    std::fs::copy(&original, &current).unwrap();
    assert!(guard.before_effect().unwrap_err().contains("custody"));
    drop(guard);
    assert!(
        f.accounts
            .store()
            .unwrap()
            .team_policies
            .dispatches
            .contains_key("custody-original")
    );
}
