use super::*;
use crate::client::{finish_redeem, prepare_redeem};
use crate::enrolled::{Action, Answer, Call, Command, FEATURE, Scope};
use crate::host::Unconnected;

#[derive(Default)]
struct Scoped(usize);
impl Dispatch for Scoped {
    fn dispatch(&mut self, _: &str, _: &str, _: &Operation) -> std::result::Result<Receipt, Code> {
        panic!("scope escaped into broad dispatch")
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
        self.0 += 1;
        Ok(Answer::Computer {
            computer: call.action.computer().into(),
            attached: true,
        })
    }
}

fn access(f: &Fixture, rights: &str, at: u64) -> (SecretKey, Access, Scope) {
    let device = key();
    let scope = Scope::local(
        "acct_scope",
        &[("project".into(), f.dir.parent().unwrap().into())],
    )
    .unwrap();
    let issued = f
        .host()
        .invite_scoped(
            &f.relay,
            Rights::parse_list(rights).unwrap(),
            scope.clone(),
            at,
            at + 30 * 86400,
        )
        .unwrap();
    let invitation = HostInvitation::parse(&issued.code, at, POLICY).unwrap();
    let pending = prepare_redeem(&invitation, &device, at, POLICY).unwrap();
    let reply = f
        .host()
        .handle(&pending.event, &f.relay, at, &mut Unconnected)
        .unwrap();
    let access = finish_redeem(&invitation, &pending, &reply, &device, at, POLICY).unwrap();
    (device, access, scope)
}

fn call(f: &Fixture, client: &Client, op: Operation, dispatch: &mut Scoped) -> Result<Outcome> {
    let pending = client.prepare(op, now())?;
    let reply = f.host().handle(&pending.event, &f.relay, now(), dispatch)?;
    client.verify_reply(&pending, &reply, now())
}

fn attach(account: &str) -> Operation {
    Operation::Enrolled {
        computer: Call {
            account: account.into(),
            action: Action::Attach {
                computer: random_id(),
                workspace: "project".into(),
            },
        },
    }
}

#[test]
fn scoped_grants_survive_restart_and_cannot_reach_broad_operations() {
    let f = Fixture::local();
    let (device, access, scope) = access(&f, "observe,operate", now());
    assert_eq!(access.grant.scope.as_ref(), Some(&scope));
    assert_eq!(access.grant.requires, [FEATURE]);
    assert_eq!(
        f.host().devices(now()).unwrap()[0].scope.as_ref(),
        Some(&scope)
    );
    let client = Client::device(access, device, POLICY).unwrap();
    let mut dispatch = Scoped::default();
    call(&f, &client, attach("acct_scope"), &mut dispatch).unwrap();
    assert_eq!(dispatch.0, 1);
    for op in [
        task(),
        terminal(),
        Operation::ListWorkspaces {},
        Operation::ListThreads {},
        Operation::ListDevices {},
        Operation::ListAgents {},
        Operation::AppRpc {
            app: crate::app_rpc::Request {
                method: crate::app_rpc::Method::ListHarnesses,
                params: serde_json::json!({}),
            },
        },
        Operation::InviteChats {},
    ] {
        assert_eq!(
            call(&f, &client, op, &mut dispatch).unwrap_err().code,
            Code::Forbidden
        );
    }
    assert_eq!(
        call(&f, &client, attach("another_account"), &mut dispatch)
            .unwrap_err()
            .code,
        Code::Forbidden
    );
    assert_eq!(dispatch.0, 1);
    f.host().revoke(&pubkey(&device), now()).unwrap();
    assert_eq!(
        call(&f, &client, attach("acct_scope"), &mut dispatch)
            .unwrap_err()
            .code,
        Code::Revoked
    );
    assert_eq!(dispatch.0, 1);
}

#[test]
fn a_read_only_scope_cannot_start_a_command_and_unscoped_access_is_refused() {
    let f = Fixture::local();
    let (device, access, _) = access(&f, "observe", now());
    let client = Client::device(access, device, POLICY).unwrap();
    let mut command = Command {
        id: random_id(),
        script: "printf test".into(),
        cwd: "".into(),
        env: Default::default(),
        timeout_seconds: 1,
        digest: String::new(),
        source_digest: random_id(),
    };
    command.digest = command.content_hash();
    let op = Operation::Enrolled {
        computer: Call {
            account: "acct_scope".into(),
            action: Action::Start {
                computer: random_id(),
                command,
            },
        },
    };
    let mut dispatch = Scoped::default();
    assert_eq!(
        call(&f, &client, op, &mut dispatch).unwrap_err().code,
        Code::MissingRight
    );
    assert_eq!(
        call(&f, &f.owner_client(), attach("acct_scope"), &mut dispatch)
            .unwrap_err()
            .code,
        Code::Forbidden
    );
    assert_eq!(dispatch.0, 0);
}

#[test]
fn renewal_keeps_the_scope_and_rejects_changed_terms_or_missing_feature() {
    let f = Fixture::local();
    let (device, access, scope) = access(&f, "observe,operate", now() - 24 * 86400);
    let envelope = f
        .host()
        .renew(
            &pubkey(&device),
            &access.grant.grant,
            access.grant.epoch,
            now(),
        )
        .unwrap()
        .unwrap();
    let renewed = access.renewed(envelope, &device, now(), POLICY).unwrap();
    assert_eq!(renewed.grant.scope.as_ref(), Some(&scope));
    assert_eq!(renewed.grant.requires, [FEATURE]);
    let mut widened = renewed.grant.clone();
    widened.grant = random_id();
    widened.expires_at += 1;
    widened.issued_at += 1;
    widened.scope.as_mut().unwrap().account = "another_account".into();
    let signed = seal(
        &widened,
        GRANT,
        &f.host().key().unwrap(),
        &pubkey(&device),
        &widened.grant,
        widened.issued_at,
        widened.expires_at,
    )
    .unwrap();
    assert_eq!(
        renewed
            .renewed(signed, &device, now() + 1, POLICY)
            .unwrap_err()
            .code,
        Code::Forbidden
    );
    let mut unsupported = renewed.grant;
    unsupported.requires.clear();
    assert_eq!(
        unsupported.validate(POLICY).unwrap_err().code,
        Code::Unsupported
    );
    assert!(scope.validate(&Rights::pairing()).is_err());
}
