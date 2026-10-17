//! App dispatch uses the same current grants and retry records as host tasks.
use super::*;
use crate::app_rpc::{Answer, Method, Request};
use serde_json::json;

#[derive(Default)]
struct App {
    reads: usize,
    effects: usize,
    wrong_method: bool,
}
impl Dispatch for App {
    fn dispatch(&mut self, _: &str, _: &str, _: &Operation) -> std::result::Result<Receipt, Code> {
        Err(Code::Unsupported)
    }
    fn app_rpc(
        &mut self,
        _: &str,
        _: &str,
        grant: Option<(&str, u64)>,
        request: &Request,
    ) -> std::result::Result<Answer, Code> {
        assert!(grant.is_some());
        if request.method.reads_only() {
            self.reads += 1;
        } else {
            self.effects += 1;
        }
        Ok(Answer {
            method: if self.wrong_method {
                Method::ListSkills
            } else {
                request.method
            },
            value: json!({"reads":self.reads,"effects":self.effects}),
        })
    }
}
fn enrolled(f: &Fixture, rights: &str, app: &mut App) -> (SecretKey, Client) {
    let secret = key();
    let invitation = HostInvitation::parse(&f.invite(rights), now(), POLICY).unwrap();
    let pending = client::prepare_redeem(&invitation, &secret, now(), POLICY).unwrap();
    let reply = f
        .host()
        .handle(&pending.event, &f.relay, now(), app)
        .unwrap();
    let access =
        client::finish_redeem(&invitation, &pending, &reply, &secret, now(), POLICY).unwrap();
    (secret, Client::device(access, secret, POLICY).unwrap())
}
fn request(method: Method) -> Operation {
    Operation::AppRpc {
        app: Request {
            method,
            params: if method == Method::RelayCommand {
                json!({"chatId":"chat","entry":{"id":"command"}})
            } else if method == Method::ContinueChat {
                json!({"account":"fixture","chatId":"a1000000-0000-4000-8000-000000000001","target":{"id":"a1000000-0000-4000-8000-000000000002","home":"cloud"}})
            } else {
                json!({})
            },
        },
    }
}

#[test]
fn continuations_require_observe_and_operate_before_any_effect() {
    let fixture = Fixture::local();
    let mut app = App::default();
    for (rights, missing) in [("observe", Right::Operate), ("operate", Right::Observe)] {
        let (_, client) = enrolled(&fixture, rights, &mut app);
        let pending = client
            .prepare(request(Method::ContinueChat), now())
            .unwrap();
        let error = call(&fixture, &client, &pending, &mut app).unwrap_err();
        assert_eq!(error.code, Code::MissingRight);
        assert_eq!(error.missing, Some(missing));
    }
    assert_eq!(app.effects, 0);
    let (_, client) = enrolled(&fixture, "observe,operate", &mut app);
    let pending = client
        .prepare(request(Method::ContinueChat), now())
        .unwrap();
    let first = call(&fixture, &client, &pending, &mut app).unwrap();
    assert_eq!(first, call(&fixture, &client, &pending, &mut app).unwrap());
    assert_eq!(app.effects, 1);
}
fn call(f: &Fixture, client: &Client, pending: &Pending, app: &mut App) -> Result<Outcome> {
    let reply = f
        .host()
        .handle(&pending.event, &f.relay, now(), app)
        .unwrap();
    client.verify_reply(pending, &reply, now())
}
#[test]
fn app_rights_are_checked_before_dispatch_and_effect_retries_do_not_repeat() {
    let fixture = Fixture::local();
    let mut app = App::default();
    let (_, observer) = enrolled(&fixture, "observe", &mut app);
    let write = observer
        .prepare(request(Method::RelayCommand), now())
        .unwrap();
    assert_eq!(
        call(&fixture, &observer, &write, &mut app)
            .unwrap_err()
            .code,
        Code::MissingRight
    );
    assert_eq!(app.effects, 0);
    let read = observer
        .prepare(request(Method::ListModels), now())
        .unwrap();
    for _ in 0..2 {
        call(&fixture, &observer, &read, &mut app).unwrap();
    }
    assert_eq!(app.reads, 2);
    let (key, operator) = enrolled(&fixture, "operate", &mut app);
    let read = operator
        .prepare(request(Method::ListModels), now())
        .unwrap();
    assert_eq!(
        call(&fixture, &operator, &read, &mut app)
            .unwrap_err()
            .missing,
        Some(Right::Observe)
    );
    let write = operator
        .prepare(request(Method::RelayCommand), now())
        .unwrap();
    let first = call(&fixture, &operator, &write, &mut app).unwrap();
    assert_eq!(first, call(&fixture, &operator, &write, &mut app).unwrap());
    assert_eq!(app.effects, 1);
    fixture.host().revoke(&pubkey(&key), now()).unwrap();
    assert_eq!(
        call(&fixture, &operator, &write, &mut app)
            .unwrap_err()
            .code,
        Code::Revoked
    );
    assert_eq!(app.effects, 1);
}
#[test]
fn mismatched_app_reply_is_refused() {
    let fixture = Fixture::local();
    let mut app = App {
        wrong_method: true,
        ..Default::default()
    };
    let (_, observer) = enrolled(&fixture, "observe", &mut app);
    let read = observer
        .prepare(request(Method::ListModels), now())
        .unwrap();
    assert_eq!(
        call(&fixture, &observer, &read, &mut app).unwrap_err().code,
        Code::Unavailable
    );
}
