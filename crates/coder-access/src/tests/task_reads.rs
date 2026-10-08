//! Signed read admission over a synthetic host, without a relay or owner home.
use super::*;
use crate::task_read;

#[derive(Default)]
struct Reader {
    reads: u64,
    effects: u64,
}
impl Dispatch for Reader {
    fn dispatch(&mut self, _: &str, _: &str, _: &Operation) -> std::result::Result<Receipt, Code> {
        self.effects += 1;
        Err(Code::Unsupported)
    }
    fn task_list(
        &mut self,
        _: &str,
        query: &task_read::ListQuery,
    ) -> std::result::Result<task_read::List, Code> {
        if query.workspace != "fixture" {
            return Err(Code::Forbidden);
        }
        self.reads += 1;
        Ok(task_read::List {
            workspace: query.workspace.clone(),
            snapshot_digest: format!("sha256:{:064x}", self.reads),
            rows: vec![],
            next: None,
            more_available: false,
        })
    }
}
fn enrolled(f: &Fixture, rights: &str, reader: &mut Reader) -> (SecretKey, Client) {
    let phone = key();
    let invitation = HostInvitation::parse(&f.invite(rights), now(), POLICY).unwrap();
    let pending = client::prepare_redeem(&invitation, &phone, now(), POLICY).unwrap();
    let reply = f
        .host()
        .handle(&pending.event, &f.relay, now(), reader)
        .unwrap();
    let access =
        client::finish_redeem(&invitation, &pending, &reply, &phone, now(), POLICY).unwrap();
    (phone, Client::device(access, phone, POLICY).unwrap())
}
fn list(workspace: &str) -> Operation {
    Operation::ListTasks {
        query: task_read::ListQuery {
            workspace: workspace.into(),
            cursor: None,
            limit: 1,
        },
    }
}

#[test]
fn observe_reads_are_signed_current_and_not_retained_or_effectful() {
    let fixture = Fixture::local();
    let mut reader = Reader::default();
    let (phone, observer) = enrolled(&fixture, "observe", &mut reader);
    let pending = observer.prepare(list("fixture"), now()).unwrap();
    let first = fixture
        .host()
        .handle(&pending.event, &fixture.relay, now(), &mut reader)
        .unwrap();
    let second = fixture
        .host()
        .handle(&pending.event, &fixture.relay, now(), &mut reader)
        .unwrap();
    assert_ne!(first.id, second.id);
    for reply in [&first, &second] {
        assert!(matches!(
            observer.verify_reply(&pending, reply, now()),
            Ok(Outcome::Tasks { .. })
        ));
    }
    assert_eq!((reader.reads, reader.effects), (2, 0));
    fixture.host().revoke(&pubkey(&phone), now()).unwrap();
    let reply = fixture
        .host()
        .handle(&pending.event, &fixture.relay, now(), &mut reader)
        .unwrap();
    assert_eq!(
        observer
            .verify_reply(&pending, &reply, now())
            .unwrap_err()
            .code,
        Code::Revoked
    );
    assert_eq!((reader.reads, reader.effects), (2, 0));
}

#[test]
fn operate_without_observe_and_wrong_workspace_disclose_nothing() {
    let fixture = Fixture::local();
    let mut reader = Reader::default();
    let (_, operator) = enrolled(&fixture, "operate", &mut reader);
    let pending = operator.prepare(list("fixture"), now()).unwrap();
    let reply = fixture
        .host()
        .handle(&pending.event, &fixture.relay, now(), &mut reader)
        .unwrap();
    let denied = operator.verify_reply(&pending, &reply, now()).unwrap_err();
    assert_eq!(
        (denied.code, denied.missing),
        (Code::MissingRight, Some(Right::Observe))
    );
    let (_, observer) = enrolled(&fixture, "observe", &mut reader);
    let pending = observer.prepare(list("unadmitted"), now()).unwrap();
    let reply = fixture
        .host()
        .handle(&pending.event, &fixture.relay, now(), &mut reader)
        .unwrap();
    assert_eq!(
        observer
            .verify_reply(&pending, &reply, now())
            .unwrap_err()
            .code,
        Code::Forbidden
    );
    assert_eq!((reader.reads, reader.effects), (0, 0));
}

#[test]
fn a_read_that_outlives_its_signed_window_never_discloses_a_reply() {
    let fixture = Fixture::local();
    let mut reader = Reader::default();
    let (_, observer) = enrolled(&fixture, "observe", &mut reader);
    let pending = observer.prepare(list("fixture"), now()).unwrap();
    let mut tick = 0;
    let result = fixture.host().handle_with_clock(
        &pending.event,
        &fixture.relay,
        || {
            tick += 1;
            Ok(if tick == 1 {
                pending.request.issued_at
            } else {
                pending.request.expires_at + 1
            })
        },
        &mut reader,
    );
    assert_eq!(result.unwrap_err().code, Code::Expired);
    assert_eq!((reader.reads, reader.effects), (1, 0));
}
