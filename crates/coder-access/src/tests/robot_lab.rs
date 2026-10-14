//! Signed robot access and exact mutation retries with throwaway keys.
use super::*;
use ::robot_lab::remote::{Answer, Control, Request as RobotRequest};
#[derive(Default)]
struct Lab {
    reads: u64,
    effects: u64,
}
impl Dispatch for Lab {
    fn dispatch(&mut self, _: &str, _: &str, _: &Operation) -> std::result::Result<Receipt, Code> {
        Err(Code::Unsupported)
    }
    fn robot_lab(&mut self, _: &str, request: &RobotRequest) -> std::result::Result<Answer, Code> {
        if request.reads_only() {
            self.reads += 1;
        } else {
            self.effects += 1;
        }
        Ok(Answer::Data {
            request: request.clone(),
            value: serde_json::json!({"job":"fixture","step":self.reads,"effects":self.effects}),
        })
    }
}
fn enrolled(f: &Fixture, rights: &str, lab: &mut Lab) -> (SecretKey, Client) {
    let secret = key();
    let invitation = HostInvitation::parse(&f.invite(rights), now(), POLICY).unwrap();
    let pending = client::prepare_redeem(&invitation, &secret, now(), POLICY).unwrap();
    let reply = f
        .host()
        .handle(&pending.event, &f.relay, now(), lab)
        .unwrap();
    let access =
        client::finish_redeem(&invitation, &pending, &reply, &secret, now(), POLICY).unwrap();
    (secret, Client::device(access, secret, POLICY).unwrap())
}
#[test]
fn robot_reads_and_controls_require_distinct_current_rights_and_mutations_retry_once() {
    let fixture = Fixture::local();
    let mut lab = Lab::default();
    let (_, observer) = enrolled(&fixture, "observe", &mut lab);
    let read = Operation::RobotLab {
        robot: RobotRequest::Status {
            job: "fixture".into(),
        },
    };
    let control = Operation::RobotLab {
        robot: RobotRequest::Control {
            job: "fixture".into(),
            command: Control::Stop,
        },
    };
    let pending = observer.prepare(control.clone(), now()).unwrap();
    let reply = fixture
        .host()
        .handle(&pending.event, &fixture.relay, now(), &mut lab)
        .unwrap();
    assert_eq!(
        observer
            .verify_reply(&pending, &reply, now())
            .unwrap_err()
            .code,
        Code::MissingRight
    );
    assert_eq!(lab.effects, 0);
    let pending = observer.prepare(read.clone(), now()).unwrap();
    for _ in 0..2 {
        let reply = fixture
            .host()
            .handle(&pending.event, &fixture.relay, now(), &mut lab)
            .unwrap();
        assert!(matches!(
            observer.verify_reply(&pending, &reply, now()),
            Ok(Outcome::RobotLab { .. })
        ));
    }
    assert_eq!(lab.reads, 2);
    let (secret, operator) = enrolled(&fixture, "operate", &mut lab);
    let pending = operator.prepare(read, now()).unwrap();
    let reply = fixture
        .host()
        .handle(&pending.event, &fixture.relay, now(), &mut lab)
        .unwrap();
    assert_eq!(
        operator
            .verify_reply(&pending, &reply, now())
            .unwrap_err()
            .missing,
        Some(Right::Observe)
    );
    let pending = operator.prepare(control, now()).unwrap();
    let first = fixture
        .host()
        .handle(&pending.event, &fixture.relay, now(), &mut lab)
        .unwrap();
    let second = fixture
        .host()
        .handle(&pending.event, &fixture.relay, now(), &mut lab)
        .unwrap();
    assert_eq!(first.id, second.id);
    assert_eq!(lab.effects, 1);
    assert!(matches!(
        operator.verify_reply(&pending, &first, now()),
        Ok(Outcome::RobotLab { .. })
    ));
    fixture.host().revoke(&pubkey(&secret), now()).unwrap();
    let reply = fixture
        .host()
        .handle(&pending.event, &fixture.relay, now(), &mut lab)
        .unwrap();
    assert_eq!(
        operator
            .verify_reply(&pending, &reply, now())
            .unwrap_err()
            .code,
        Code::Revoked
    );
    assert_eq!(lab.effects, 1);
}
