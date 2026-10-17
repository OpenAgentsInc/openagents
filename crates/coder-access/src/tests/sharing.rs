use super::*;
use crate::client::{finish_redeem, prepare_redeem};
use crate::enrolled::{Action, Answer, Call, Scope};
use crate::host::Unconnected;
use std::collections::BTreeMap;

struct Projects {
    roots: BTreeMap<String, PathBuf>,
}
impl Dispatch for Projects {
    fn dispatch(&mut self, _: &str, _: &str, _: &Operation) -> std::result::Result<Receipt, Code> {
        panic!("scoped share reached broad dispatch")
    }
    fn project_scope(
        &mut self,
        account: &str,
        workspaces: &[String],
    ) -> std::result::Result<Scope, Code> {
        let roots = workspaces
            .iter()
            .map(|label| {
                self.roots
                    .get(label)
                    .map(|path| (label.clone(), path.clone()))
                    .ok_or(Code::Forbidden)
            })
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Scope::local(account, &roots).map_err(|error| error.code)
    }
    fn enrolled(
        &mut self,
        _: &str,
        _: &str,
        _: u64,
        _: u64,
        _: &Scope,
        call: &Call,
    ) -> std::result::Result<Answer, Code> {
        Ok(Answer::Computer {
            computer: call.action.computer().into(),
            attached: true,
        })
    }
}
fn call(
    f: &Fixture,
    client: &Client,
    operation: Operation,
    at: u64,
    dispatch: &mut dyn Dispatch,
) -> Result<Outcome> {
    let pending = client.prepare(operation, at)?;
    let reply = f.host().handle(&pending.event, &f.relay, at, dispatch)?;
    client.verify_reply(&pending, &reply, at)
}
fn share(device: &SecretKey, workspace: &str, at: u64) -> Operation {
    Operation::InviteProject {
        share: crate::sharing::Invite {
            account: "recipient".into(),
            device: pubkey(device),
            workspaces: vec![workspace.into()],
            rights: Rights::parse_list("observe,operate").unwrap(),
            expires_at: at + 30 * 86400,
        },
    }
}
fn invitation(
    f: &Fixture,
    device: &SecretKey,
    workspace: &str,
    at: u64,
    dispatch: &mut dyn Dispatch,
) -> String {
    match call(
        f,
        &f.owner_client(),
        share(device, workspace, at),
        at,
        dispatch,
    )
    .unwrap()
    {
        Outcome::Invitation { code, .. } => code,
        other => panic!("{other:?}"),
    }
}
fn redeem(f: &Fixture, code: &str, key: &SecretKey, at: u64) -> Result<Access> {
    let invitation = HostInvitation::parse(code, at, POLICY)?;
    let pending = prepare_redeem(&invitation, key, at, POLICY)?;
    let reply = f
        .host()
        .handle(&pending.event, &f.relay, at, &mut Unconnected)?;
    finish_redeem(&invitation, &pending, &reply, key, at, POLICY)
}
fn observe(account: &str) -> Operation {
    Operation::Enrolled {
        computer: Call {
            account: account.into(),
            action: Action::Observe {
                computer: random_id(),
            },
        },
    }
}
fn projects(a: &tempfile::TempDir, b: &tempfile::TempDir) -> Projects {
    Projects {
        roots: BTreeMap::from([("a".into(), a.path().into()), ("b".into(), b.path().into())]),
    }
}

#[test]
fn recipient_pinning_separate_shares_and_revocation_preserve_unrelated_access() {
    let f = Fixture::local();
    let recipient = key();
    let a = tempfile::tempdir().unwrap();
    let b = tempfile::tempdir().unwrap();
    let mut projects = projects(&a, &b);
    let at = now();
    let code = invitation(&f, &recipient, "a", at, &mut projects);
    assert_eq!(
        redeem(&f, &code, &key(), at).unwrap_err().code,
        Code::Forbidden
    );
    assert!(f.host().devices(at).unwrap().is_empty());
    let access_a = redeem(&f, &code, &recipient, at).unwrap();
    let code_b = invitation(&f, &recipient, "b", at, &mut projects);
    let access_b = redeem(&f, &code_b, &recipient, at).unwrap();
    let a = Client::device(access_a.clone(), recipient, POLICY).unwrap();
    let b = Client::device(access_b.clone(), recipient, POLICY).unwrap();
    call(&f, &a, observe("recipient"), at, &mut projects).unwrap();
    call(&f, &b, observe("recipient"), at, &mut projects).unwrap();
    assert_eq!(
        f.host()
            .devices(at)
            .unwrap()
            .iter()
            .filter(|entry| entry.state == DeviceState::Active)
            .count(),
        2
    );
    assert_eq!(
        call(&f, &a, observe("another-account"), at, &mut projects)
            .unwrap_err()
            .code,
        Code::Forbidden
    );
    assert_eq!(
        call(&f, &a, share(&key(), "b", at), at, &mut projects)
            .unwrap_err()
            .code,
        Code::Forbidden
    );
    assert_eq!(
        call(&f, &a, Operation::ListWorkspaces {}, at, &mut projects)
            .unwrap_err()
            .code,
        Code::Forbidden
    );
    let revoke = Operation::RevokeProject {
        grant: access_a.grant.grant.clone(),
    };
    call(&f, &f.owner_client(), revoke.clone(), at, &mut projects).unwrap();
    call(&f, &f.owner_client(), revoke, at, &mut projects).unwrap();
    assert_eq!(
        call(&f, &a, observe("recipient"), at, &mut projects)
            .unwrap_err()
            .code,
        Code::Revoked
    );
    call(&f, &b, observe("recipient"), at, &mut projects).unwrap();
    let current = f.host().devices(at).unwrap();
    assert!(
        current
            .iter()
            .any(|entry| entry.grant == access_b.grant.grant
                && entry.epoch == access_b.grant.epoch
                && entry.state == DeviceState::Active)
    );
    assert_eq!(
        redeem(&f, &code, &recipient, at).unwrap_err().code,
        Code::Revoked
    );
}

#[test]
fn unknown_projects_old_hosts_and_escalated_rights_never_issue_a_share() {
    let f = Fixture::local();
    let device = key();
    let a = tempfile::tempdir().unwrap();
    let b = tempfile::tempdir().unwrap();
    let mut projects = projects(&a, &b);
    let at = now();
    assert_eq!(
        call(
            &f,
            &f.owner_client(),
            share(&device, "../other", at),
            at,
            &mut projects
        )
        .unwrap_err()
        .code,
        Code::Forbidden
    );
    assert_eq!(
        call(
            &f,
            &f.owner_client(),
            share(&device, "a", at),
            at,
            &mut Unconnected
        )
        .unwrap_err()
        .code,
        Code::Unsupported
    );
    let mut request = share(&device, "a", at);
    if let Operation::InviteProject { share } = &mut request {
        share.rights = Rights::pairing();
    }
    assert_eq!(
        call(&f, &f.owner_client(), request, at, &mut projects)
            .unwrap_err()
            .code,
        Code::Malformed
    );
    assert!(f.host().outstanding_invitations(at).unwrap().is_empty());
    assert!(f.host().devices(at).unwrap().is_empty());
}

#[test]
fn an_owner_selected_expiry_cannot_be_extended_by_automatic_renewal() {
    let f = Fixture::local();
    let device = key();
    let a = tempfile::tempdir().unwrap();
    let b = tempfile::tempdir().unwrap();
    let mut projects = projects(&a, &b);
    let issued = now() - 24 * 86400;
    let code = invitation(&f, &device, "a", issued, &mut projects);
    let access = redeem(&f, &code, &device, issued).unwrap();
    assert!(
        f.host()
            .renew(
                &pubkey(&device),
                &access.grant.grant,
                access.grant.epoch,
                now()
            )
            .unwrap()
            .is_none()
    );
    let client = Client::device(access.clone(), device, POLICY).unwrap();
    call(&f, &client, observe("recipient"), now(), &mut projects).unwrap();
    assert_eq!(
        call(
            &f,
            &client,
            observe("recipient"),
            access.grant.expires_at,
            &mut projects
        )
        .unwrap_err()
        .code,
        Code::Expired
    );
}
