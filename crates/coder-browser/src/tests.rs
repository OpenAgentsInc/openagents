use super::*;
use coder_access::{
    Rights,
    protocol::{GRANT, Grant, Origin, OriginKind},
};
use coder_pty::wire::{Input, Status, TerminalRef, Value};
use coder_reach::channel::{Acceptor, GrantCheck, GrantRefusal};
fn key(n: u8) -> SecretKey {
    SecretKey::from_byte_array([n; 32]).unwrap()
}
fn admission(right: Right, generation: u64) -> Admission {
    let host = key(3);
    let secret = key(4);
    let grant = Grant {
        v: GRANT.into(),
        requires: vec![],
        grant: "a".repeat(64),
        host: coder_reach::pubkey(&host),
        owner: coder_reach::pubkey(&key(5)),
        device: coder_reach::pubkey(&secret),
        relay: "ws://127.0.0.1:1".into(),
        rights: Rights::new([right]).unwrap(),
        epoch: 1,
        origin: Origin {
            kind: OriginKind::Approval,
            id: "b".repeat(64),
            issuer: coder_reach::pubkey(&key(5)),
        },
        issued_at: 100,
        expires_at: 500,
    };
    let event =
        artifact::seal(&grant, GRANT, &host, &grant.device, &grant.grant, 100, 500).unwrap();
    let access =
        Access::from_authorization(event, &secret, &grant.host, 100, RelayPolicy::LoopbackTest)
            .unwrap();
    Admission::new(secret, access, RelayPolicy::LoopbackTest, generation, 100).unwrap()
}
fn input(admission: &Admission) -> TermRequest {
    TermRequest::Input(Input::new(
        "c".repeat(64),
        TerminalRef {
            generation: terminal_generation(admission.host(), admission.generation()),
            terminal: "d".repeat(64),
        },
        b"echo fixture\r".to_vec(),
    ))
}
#[test]
fn connectivity_never_grants_rights_or_uses_expired_admission() {
    let mut a = admission(Right::Observe, 7);
    assert_eq!(a.check(&input(&a), 100), Err(Error::NotAdmitted));
    a.revoke();
    assert_eq!(a.check(&input(&a), 100), Err(Error::Disconnected));
    let a = admission(Right::Terminal, 7);
    assert_eq!(a.check(&input(&a), 501), Err(Error::NotAdmitted));
}
#[test]
fn stale_generation_and_unknown_extensions_refuse() {
    let a = admission(Right::Terminal, 7);
    let mut request = input(&a);
    if let TermRequest::Input(r) = &mut request {
        r.terminal.generation = "f".repeat(64);
    }
    assert_eq!(a.check(&request, 100), Err(Error::Stale));
    if let TermRequest::Input(r) = &mut request {
        r.requires = vec!["unknown".into()];
    }
    assert_eq!(a.check(&request, 100), Err(Error::Malformed));
}
struct Grants(bool);
impl GrantCheck for Grants {
    fn check(&self, _: &str, _: &str, _: u64, _: u64) -> std::result::Result<(), GrantRefusal> {
        if self.0 {
            Ok(())
        } else {
            Err(GrantRefusal::Revoked)
        }
    }
}
#[tokio::test]
async fn scratch_direct_signed_channel_dispatches_once_then_disconnect_never_replays() {
    let a = admission(Right::Terminal, 7);
    let (client, host) = tokio::io::duplex(128 * 1024);
    let receiver = async move {
        let accept = Acceptor::new(key(3), 7, Grants(true), Duration::from_secs(1));
        let mut channel = accept.accept(host, 100).await.unwrap();
        let mut assembler = Assembler::default();
        let bytes = loop {
            if let Some(bytes) = assembler
                .push(&channel.recv().await.unwrap().unwrap())
                .unwrap()
            {
                break bytes;
            }
        };
        let ToHost::Terminal(request) = ToHost::decode(&bytes).unwrap() else {
            panic!()
        };
        let response = TerminalResult::from_outcome(
            request.request(),
            Ok((Status::Accepted, Value::Written { bytes: 13 })),
        );
        for part in fragments(&ToDevice::Result(response).encode()).unwrap() {
            channel.send(&part).await.unwrap();
        }
        channel.close().await.unwrap();
    };
    let caller = async move {
        let mut client = Direct::connect(client, a, 100).await.unwrap();
        let request = input(&client.admission);
        assert!(
            client
                .request(request.clone(), 100)
                .await
                .unwrap()
                .value
                .is_some()
        );
        assert_eq!(
            client.request(request.clone(), 100).await,
            Err(Error::Unknown)
        );
        assert_eq!(client.admission.state(), State::Unknown);
        assert_eq!(client.request(request, 100).await, Err(Error::Disconnected));
    };
    tokio::join!(receiver, caller);
}
#[tokio::test]
async fn revoked_host_grant_and_stale_host_generation_fail_before_dispatch() {
    for (current, allowed) in [(8, true), (7, false)] {
        let (client, host) = tokio::io::duplex(128 * 1024);
        let a = admission(Right::Terminal, 7);
        let accept = Acceptor::new(key(3), current, Grants(allowed), Duration::from_secs(1));
        let (result, _) = tokio::join!(Direct::connect(client, a, 100), accept.accept(host, 100));
        assert!(result.is_err());
    }
}
struct Fixture {
    wrong_key: bool,
    lost: bool,
    count: usize,
}
impl Relay for Fixture {
    async fn exchange(&mut self, event: &Event, _: &str, recipient: &str, _: u64) -> Result<Event> {
        self.count += 1;
        if self.lost {
            return Err(Error::Unknown);
        }
        let (value, _): (serde_json::Value, _) = artifact::open(
            event,
            &key(3),
            &coder_reach::pubkey(&key(4)),
            &coder_reach::pubkey(&key(3)),
            coder_pty::wire::INPUT,
        )
        .unwrap();
        let request = TermRequest::from_value(value).unwrap();
        let result = TerminalResult::from_outcome(
            request.request(),
            Ok((Status::Accepted, Value::Written { bytes: 13 })),
        );
        artifact::seal(
            &result,
            RESULT,
            &key(if self.wrong_key { 6 } else { 3 }),
            recipient,
            request.request(),
            100,
            160,
        )
        .map_err(|_| Error::Malformed)
    }
}
#[tokio::test]
async fn sealed_relay_result_checks_signer_and_unknown_dispatch_is_not_replayed() {
    for (wrong_key, lost) in [(false, false), (true, false), (false, true)] {
        let a = admission(Right::Terminal, 7);
        let request = input(&a);
        let mut relayed = Relayed::new(
            a,
            Fixture {
                wrong_key,
                lost,
                count: 0,
            },
        );
        let result = relayed.request(request.clone(), 100).await;
        assert_eq!(result.is_ok(), !wrong_key && !lost);
        if result.is_err() {
            assert_eq!(
                relayed.request(request, 100).await,
                Err(Error::Disconnected)
            );
        }
        assert_eq!(relayed.relay.count, 1);
    }
}

struct FramesFixture(Event);
impl Relay for FramesFixture {
    async fn exchange(&mut self, _: &Event, _: &str, _: &str, _: u64) -> Result<Event> {
        Err(Error::Unknown)
    }
    async fn next(&mut self) -> Result<Event> {
        Ok(self.0.clone())
    }
}
#[tokio::test]
async fn relay_frames_preserve_explicit_gaps_and_bind_generation_attachment_and_retention() {
    use coder_pty::{
        client::{Applied, TerminalState},
        wire::{Body, Frame},
    };
    for wrong in 0..4 {
        let a = admission(Right::Terminal, 7);
        let terminal = TerminalRef {
            generation: terminal_generation(a.host(), 7),
            terminal: "d".repeat(64),
        };
        let attachment = "e".repeat(64);
        let mut frame = Frame::new(
            terminal.clone(),
            attachment.clone(),
            Body::Gap {
                from: 1,
                to: 8,
                bytes: Some(42),
            },
        );
        if wrong == 1 {
            frame.terminal.generation = "f".repeat(64);
        }
        if wrong == 2 {
            frame.attachment = "f".repeat(64);
        }
        let event = artifact::seal(
            &frame,
            coder_pty::wire::FRAME,
            &key(3),
            &coder_reach::pubkey(&key(4)),
            &attachment,
            100,
            if wrong == 3 { 101 } else { 160 },
        )
        .unwrap();
        let mut relayed = Relayed::new(a, FramesFixture(event));
        relayed.attachment = Some(attachment);
        let result = relayed.next(110).await;
        if wrong != 0 {
            assert!(result.is_err());
            continue;
        }
        let Incoming::Frame(frame) = result.unwrap() else {
            panic!()
        };
        let mut state = TerminalState::new(terminal, 24, 80);
        assert!(matches!(
            state.apply(&frame),
            Applied::Gap {
                from: 1,
                to: 8,
                bytes: Some(42)
            }
        ));
    }
}

#[test]
fn idle_expiry_and_thread_read_authority_are_separate() {
    let a = admission(Right::Terminal, 7);
    assert!(a.current(499));
    assert!(!a.current(500));
    assert_eq!(a.expires_at(), 500);
    assert_eq!(a.device(), coder_reach::pubkey(&key(4)));
    assert!(matches!(
        a.prepare_thread(&"a".repeat(32), None, 100),
        Err(Error::NotAdmitted)
    ));
    let mut a = admission(Right::Terminal, 7);
    a.access.grant.rights = Rights::new([Right::Terminal, Right::Observe]).unwrap();
    // Changing the unsigned projection never changes the host-signed authority.
    assert!(matches!(
        a.prepare_thread(&"a".repeat(32), None, 100),
        Err(Error::NotAdmitted)
    ));
}

struct QueuedEvent(Option<Event>);
impl Relay for QueuedEvent {
    fn take_event(&mut self) -> Result<Option<Event>> {
        Ok(self.0.take())
    }
    async fn exchange(&mut self, _: &Event, _: &str, _: &str, _: u64) -> Result<Event> {
        Err(Error::Unknown)
    }
}
#[test]
fn queued_role_loss_is_verified_before_input_can_be_enabled() {
    let a = admission(Right::Terminal, 7);
    let attachment = "e".repeat(64);
    let frame = coder_pty::wire::Frame::new(
        TerminalRef {
            generation: terminal_generation(a.host(), 7),
            terminal: "d".repeat(64),
        },
        attachment.clone(),
        coder_pty::wire::Body::Typist {
            typist: Some("f".repeat(64)),
            size: coder_pty::wire::Size::new(24, 80),
        },
    );
    let event = artifact::seal(
        &frame,
        coder_pty::wire::FRAME,
        &key(3),
        a.device(),
        &attachment,
        100,
        160,
    )
    .unwrap();
    let mut link = Relayed::new(a, QueuedEvent(Some(event)));
    link.attachment = Some(attachment);
    let Incoming::Frame(current) = link.take_incoming(110).unwrap().unwrap() else {
        panic!()
    };
    assert_eq!(current, frame);
    assert!(link.take_incoming(110).unwrap().is_none());
    assert!(matches!(link.take_incoming(500), Err(Error::NotAdmitted)));
}
