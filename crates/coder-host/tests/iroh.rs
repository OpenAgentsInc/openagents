//! The host's iroh listener: a phone pairs from a connect code on the
//! enroll ALPN and then reaches the host on the reach ALPN, with every
//! NIP-HOST and NIP-TERM check unchanged. An `EndpointId` never admits
//! anything: an unknown key reaches only enrollment.

use std::time::Duration;

use coder_host::Error;
use coder_host::access::protocol::{Operation, Outcome};
use coder_host::access::{Code, Right};
use coder_host::client::Device;
use coder_host::mailbox::terminal_generation;
use coder_host::message::{Assembler, TermRequest, ToDevice, fragments};
use coder_host::pty::wire::{Attach, Mode, Reason, Status, TerminalRef};
use coder_host::reach::channel::ClientConfig;
use coder_host::reach::{Refusal, pubkey};
use openagents_connect::control::{Op, Reply};

#[path = "support/connect.rs"]
mod support;

use support::{Phone, call, host, key, now};

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_phone_redeems_a_connect_code_over_iroh_and_a_second_phone_is_forbidden() {
    let host = host().await;
    let (_, code) = host.code().await;
    // The code names the host's own keys and its computer name.
    assert_eq!(code.host(), host.running.host_key());
    assert_eq!(code.endpoint(), host.addr().id);
    assert_eq!(code.label(), "Studio Mac");

    let phone = Phone::new().await;
    let (reply, access, host_now) = phone.redeem(&code, &host.relay, now()).await;
    let access = access.unwrap();
    // The grant is signed by the host key the code names, for this phone,
    // with the connect code's rights and no more.
    assert_eq!(reply.unwrap().pubkey, code.host());
    assert_eq!(access.grant.host, code.host());
    assert_eq!(access.grant.device, pubkey(&phone.secret));
    assert_eq!(
        access.grant.rights.to_list(),
        "observe,operate,terminal,review,access_read,access_admin"
    );
    assert_eq!(access.grant.relay, host.relay);
    assert!(host_now.abs_diff(now()) <= 2);
    assert!(openagents_connect::clock_warning(host_now, now()).is_none());

    // A retry by the same phone gets the same grant.
    let (_, again, _) = phone.redeem(&code, &host.relay, now()).await;
    assert_eq!(again.unwrap().grant.grant, access.grant.grant);

    // Another phone redeeming the same code is refused with a signed
    // `forbidden`, and holds nothing.
    let thief = Phone::new().await;
    let (reply, refused, _) = thief.redeem(&code, &host.relay, now()).await;
    assert!(reply.is_some());
    assert_eq!(refused.unwrap_err().code, Code::Forbidden);
    let Reply::Devices { devices } = call(&host.socket, Op::DeviceList {}).await.unwrap() else {
        panic!("devices")
    };
    assert_eq!(devices.len(), 1);
    assert_eq!(devices[0].device, pubkey(&phone.secret));

    // The host tells the new phone where to reach it: a v2 hint record
    // with its `iroh` hint, beside the v1 record older phones read.
    let host_key = host.running.host_key().to_owned();
    let mut found = None;
    for _ in 0..50 {
        host.running.publish_reach().await;
        let events = host.events.lock().await.clone();
        found = events.values().find_map(|event| {
            coder_host::reach::hints::v2::HintsV2::open(event, &phone.secret, &host_key).ok()
        });
        if found.is_some() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    let record = found.expect("a v2 hint record");
    let iroh = record.iroh().expect("an iroh hint");
    assert_eq!(iroh.address, code.endpoint().to_string());
    assert!(!iroh.direct.is_empty());
    let events = host.events.lock().await.clone();
    assert!(events.values().any(|event| {
        coder_host::reach::hints::Hints::open(event, &phone.secret, &host_key).is_ok()
    }));
    host.running.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_phone_that_pairs_over_iroh_gets_a_chat_invitation_and_a_refused_one_does_not() {
    let host = support::host_with(support::Options {
        chats: true,
        ..support::Options::default()
    })
    .await;
    let (_, code) = host.code().await;
    // The grant comes with a single-use invitation to the host's Coder
    // chats, so the phone can read the tasks it starts there.
    let phone = Phone::new().await;
    let (_, _, answer) = phone.answer(&code, &host.relay, now()).await;
    assert!(answer.reply.is_some());
    let chats = answer.chats.expect("a chat invitation beside the grant");
    assert!(chats.starts_with("coder-pair:"));
    // A phone the host refuses gets its signed refusal and no invitation.
    let thief = Phone::new().await;
    let (_, _, answer) = thief.answer(&code, &host.relay, now()).await;
    assert!(answer.reply.is_some());
    assert!(answer.chats.is_none());
    host.running.shutdown().await;

    // A host that serves no chats sends none.
    let plain = support::host().await;
    let (_, code) = plain.code().await;
    let (_, _, answer) = Phone::new().await.answer(&code, &plain.relay, now()).await;
    assert!(answer.reply.is_some());
    assert!(answer.chats.is_none());
    plain.running.shutdown().await;
}

/// A chat grant lasts 29 days; a phone asks again well before it ends.
const CHAT_GRANT_SECS: u64 = 29 * 24 * 60 * 60;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_phone_paired_over_iroh_reads_its_chats_and_renews_them_on_its_link() {
    let host = support::host_with(support::Options {
        chats: true,
        ..support::Options::default()
    })
    .await;
    let (_, code) = host.code().await;
    let phone = Phone::new().await;
    let (invitation, pending, answer) = phone.answer(&code, &host.relay, now()).await;
    let reply: nostr::domain::Event =
        serde_json::from_str(answer.reply.as_deref().unwrap()).unwrap();
    let access = coder_host::access::client::finish_redeem(
        &invitation,
        &pending,
        &reply,
        &phone.secret,
        now(),
        support::POLICY,
    )
    .unwrap();
    // The invitation the enroll reply carried reads the host's Coder chats.
    let first = support::read_chats(&answer.chats.unwrap(), &phone.secret).await;
    assert!(first.expires_at.abs_diff(now() + CHAT_GRANT_SECS) <= 60);
    // Before that grant ends the phone asks again on its link, as a phone
    // paired by any other path does, and the new invitation reads too.
    let device = phone.device(access);
    let link = phone.link(&host, &device).await.unwrap();
    let (again, expires_at) = support::invite_chats(&link).await.unwrap();
    assert!(expires_at.abs_diff(now() + CHAT_GRANT_SECS) <= 60);
    let renewed = support::read_chats(&again, &phone.secret).await;
    assert_ne!(renewed.grant, first.grant);
    host.running.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_phone_that_redeems_a_code_on_the_relay_gets_its_chats_from_chats_invite() {
    let host = support::host_with(support::Options {
        chats: true,
        ..support::Options::default()
    })
    .await;
    let (_, code) = host.code().await;
    // iroh cannot connect, so the phone redeems the same code on the relay,
    // whose answer carries no chat invitation.
    let phone = Phone::new().await;
    let enrolled = coder_host::client::iroh::enroll_on_relay(
        &code.encode(),
        &host.relay,
        &phone.secret,
        support::POLICY,
    )
    .await
    .unwrap();
    assert!(enrolled.chats.is_none());
    assert!(enrolled.access.grant.rights.contains(Right::Observe));
    // It asks on the relay, where it now talks to the computer.
    let device = phone.device(enrolled.access);
    let link = coder_host::client::Link::relay(device, host.relay.clone());
    let (invitation, _) = support::invite_chats(&link).await.unwrap();
    support::read_chats(&invitation, &phone.secret).await;
    host.running.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_host_that_serves_no_chats_refuses_chats_invite_as_unavailable() {
    let host = host().await;
    let (_, code) = host.code().await;
    let phone = Phone::new().await;
    let (_, access, _) = phone.redeem(&code, &host.relay, now()).await;
    let device = phone.device(access.unwrap());
    let link = phone.link(&host, &device).await.unwrap();
    let error = support::invite_chats(&link).await.unwrap_err();
    assert!(
        matches!(&error, Error::Access(e) if e.code == Code::Unavailable),
        "{error:?}"
    );
    host.running.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_phone_whose_clock_is_off_by_thirty_seconds_still_pairs_and_works() {
    let host = host().await;
    for offset in [-30_i64, 30] {
        let (_, code) = host.code().await;
        let phone = Phone::new().await;
        let at = now().checked_add_signed(offset).unwrap();
        let (_, access, host_now) = phone.redeem(&code, &host.relay, at).await;
        let access = access.unwrap_or_else(|e| panic!("{offset}: {e}"));
        // Thirty seconds is inside the skew, so the phone names no problem.
        assert!(openagents_connect::clock_warning(host_now, at).is_none());
        // Its first request, dated by its own clock, is admitted too.
        let device = phone.device(access);
        let client = coder_host::access::Client::device(
            device.access().clone(),
            phone.secret,
            support::POLICY,
        )
        .unwrap();
        let pending = client.prepare(Operation::ListDevices {}, at).unwrap();
        let reply = host.running.authority().handle(
            &pending.event,
            &host.relay,
            &mut coder_host::access::host::Unconnected,
        );
        let outcome = client
            .verify_reply(&pending, &reply.unwrap(), at)
            .unwrap_or_else(|e| panic!("{offset}: {e}"));
        // Admitted as this device, which holds `access_read` like every
        // paired phone: it reads the device list.
        assert!(matches!(outcome, Outcome::Devices { .. }), "{offset}");
    }
    // A clock more than a minute off names itself.
    assert_eq!(
        openagents_connect::clock_warning(1_000, 1_000 - 90),
        Some(90)
    );
    host.running.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_terminal_opens_over_iroh_only_with_the_terminal_right() {
    let host = host().await;
    // Every connect code carries the terminal right.
    let (_, with) = host.code().await;
    let allowed = Phone::new().await;
    let (_, access, _) = allowed.redeem(&with, &host.relay, now()).await;
    let access = access.unwrap();
    assert!(access.grant.rights.contains(Right::Terminal));
    let operator = allowed.device(access);
    let link = allowed.link(&host, &operator).await.unwrap();
    let Outcome::Dispatched { receipt } = link
        .call(Operation::OpenTerminal { cols: 80, rows: 24 })
        .await
        .unwrap()
    else {
        panic!("terminal.open dispatches")
    };
    let reference = TerminalRef {
        generation: terminal_generation(host.running.host_key(), host.running.generation()),
        terminal: receipt.reference,
    };
    let attached = link
        .terminal(TermRequest::Attach(Attach::new(
            coder_host::reach::new_id(),
            reference.clone(),
            Mode::Interact,
            0,
            64 * 1024,
        )))
        .await
        .unwrap();
    assert_eq!(attached.status, Status::Accepted, "{attached:?}");

    // A grant without it (one an earlier code made) is refused a terminal.
    let refused = Phone::new().await;
    let at = now();
    let invitation = host
        .store
        .invite(
            &host.relay,
            coder_host::access::Rights::parse_list("observe,operate").unwrap(),
            at,
            at + 86_400,
        )
        .unwrap();
    let parsed =
        coder_host::access::protocol::HostInvitation::parse(&invitation.code, at, support::POLICY)
            .unwrap();
    let pending =
        coder_host::access::client::prepare_redeem(&parsed, &refused.secret, at, support::POLICY)
            .unwrap();
    let reply = host
        .store
        .handle(
            &pending.event,
            &host.relay,
            at,
            &mut coder_host::access::host::Unconnected,
        )
        .unwrap();
    let access = coder_host::access::client::finish_redeem(
        &parsed,
        &pending,
        &reply,
        &refused.secret,
        at,
        support::POLICY,
    )
    .unwrap();
    let observer = refused.device(access);
    let link = refused.link(&host, &observer).await.unwrap();
    let error = link
        .call(Operation::OpenTerminal { cols: 80, rows: 24 })
        .await
        .unwrap_err();
    assert!(
        matches!(&error, Error::Access(e) if e.code == Code::MissingRight && e.missing == Some(Right::Terminal)),
        "{error:?}"
    );
    let result = link
        .terminal(TermRequest::Attach(Attach::new(
            coder_host::reach::new_id(),
            reference,
            Mode::Observe,
            0,
            1024,
        )))
        .await
        .unwrap();
    assert_eq!(result.status, Status::Refused);
    assert_eq!(result.reason, Some(Reason::NotAdmitted));
    host.running.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn revocation_closes_an_open_iroh_channel_before_its_next_message() {
    let host = host().await;
    let (_, code) = host.code().await;
    let phone = Phone::new().await;
    let (_, access, _) = phone.redeem(&code, &host.relay, now()).await;
    let device = phone.device(access.unwrap());
    let link = phone.link(&host, &device).await.unwrap();
    link.ping().await.unwrap();

    let Reply::Revoked { epoch, .. } = call(
        &host.socket,
        Op::DeviceRevoke {
            device: pubkey(&phone.secret),
        },
    )
    .await
    .unwrap() else {
        panic!("revoked")
    };
    assert_eq!(epoch, 1);
    // The host rechecks at once: the channel closes naming the cause, and
    // the next message is never served. The open channel's periodic
    // recheck is 30 seconds here, so only the revocation closes it.
    let closed = tokio::time::timeout(Duration::from_secs(3), link.wait_closed())
        .await
        .expect("the channel closes within a round trip");
    assert_eq!(closed.as_deref(), Some("revoked"));
    assert!(link.ping().await.is_err());
    // A new channel under the revoked grant is refused.
    let refused = phone.link(&host, &device).await.unwrap_err();
    assert!(
        matches!(&refused, Error::Reach(r) if r.code == Refusal::Revoked || r.code == Refusal::Stale),
        "{refused:?}"
    );
    host.running.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_unknown_key_reaches_only_enrollment_and_a_grant_admits_only_its_device() {
    let host = host().await;
    let (_, code) = host.code().await;
    let phone = Phone::new().await;
    let (_, access, _) = phone.redeem(&code, &host.relay, now()).await;
    let access = access.unwrap();

    // A stranger dials the reach ALPN with the phone's grant ID: it proves
    // its own key, and the grant check refuses it.
    let stranger = Phone::new().await;
    let config = ClientConfig {
        device: key(),
        host: host.running.host_key().to_owned(),
        grant: access.grant.grant.clone(),
        epoch: 0,
        generation: host.running.generation(),
        timeout: Duration::from_secs(5),
    };
    let refused = openagents_connect::reach::dial(&stranger.endpoint, host.addr(), &config, now())
        .await
        .unwrap_err();
    assert_eq!(refused.code, Refusal::NotAdmitted);

    // A dialer that expects another host key is refused before anything
    // flows: the iroh key answering proves nothing about the Nostr key.
    let mut wrong = config.clone();
    wrong.device = phone.secret;
    wrong.host = pubkey(&key());
    let refused = openagents_connect::reach::dial(&phone.endpoint, host.addr(), &wrong, now())
        .await
        .unwrap_err();
    assert_eq!(refused.code, Refusal::IdentityMismatch);

    // On the enroll ALPN a stranger gets no signed reply for a request that
    // is not a redemption, for garbage, or for a wrong capability.
    let garbage = openagents_connect::enroll::EnrollRequest::new("{}".into());
    let answer = openagents_connect::enroll::redeem(&stranger.endpoint, host.addr(), &garbage)
        .await
        .unwrap();
    assert!(answer.reply.is_none());
    let client =
        coder_host::access::Client::device(access.clone(), phone.secret, support::POLICY).unwrap();
    let pending = client.prepare(Operation::ListDevices {}, now()).unwrap();
    let not_redeem = openagents_connect::enroll::EnrollRequest::new(
        serde_json::to_string(&pending.event).unwrap(),
    );
    let answer = openagents_connect::enroll::redeem(&phone.endpoint, host.addr(), &not_redeem)
        .await
        .unwrap();
    assert!(
        answer.reply.is_none(),
        "enrollment carries only redemptions"
    );
    let forged = openagents_connect::code::ConnectCode::from_invitation(
        openagents_connect::code::CodeParts {
            host: code.host(),
            endpoint: code.endpoint(),
            issued_at: code.issued_at(),
            relay: None,
            addrs: code.addrs().to_vec(),
            label: String::new(),
        },
        &code.invitation(),
        &"ab".repeat(32),
    )
    .unwrap();
    let (reply, refused, _) = stranger.redeem(&forged, &host.relay, now()).await;
    assert!(reply.is_none() && refused.is_err());

    // The phone itself reaches the host and is served.
    let device = phone.device(access);
    let link = phone.link(&host, &device).await.unwrap();
    link.ping().await.unwrap();
    host.running.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_grant_near_its_end_is_renewed_on_the_channel_and_the_channel_follows_it() {
    let host = host().await;
    // A grant issued 24 days ago, of 30: redeemed on the host's own clock
    // back then, as a phone paired that day would have.
    let day = 86_400;
    let issued = now() - 24 * day;
    let invitation = host
        .store
        .invite(
            &host.relay,
            coder_host::access::Rights::parse_list("observe,operate").unwrap(),
            issued,
            issued + 30 * day,
        )
        .unwrap();
    let phone = Phone::new().await;
    let parsed = coder_host::access::protocol::HostInvitation::parse(
        &invitation.code,
        issued,
        support::POLICY,
    )
    .unwrap();
    let pending =
        coder_host::access::client::prepare_redeem(&parsed, &phone.secret, issued, support::POLICY)
            .unwrap();
    let reply = host
        .store
        .handle(
            &pending.event,
            &host.relay,
            issued,
            &mut coder_host::access::host::Unconnected,
        )
        .unwrap();
    let old = coder_host::access::client::finish_redeem(
        &parsed,
        &pending,
        &reply,
        &phone.secret,
        issued,
        support::POLICY,
    )
    .unwrap();

    // Open the channel by hand, to read the host's unasked message.
    let connection = phone
        .endpoint
        .endpoint
        .connect(host.addr(), openagents_connect::REACH_ALPN)
        .await
        .unwrap();
    let stream = openagents_connect::stream::IrohStream::open(connection)
        .await
        .unwrap();
    let config = ClientConfig {
        device: phone.secret,
        host: host.running.host_key().to_owned(),
        grant: old.grant.grant.clone(),
        epoch: 0,
        generation: host.running.generation(),
        timeout: Duration::from_secs(5),
    };
    let channel = coder_host::reach::channel::connect(stream, &config, now())
        .await
        .unwrap();
    let (mut reader, mut writer) = channel.into_split();
    let mut assembler = Assembler::default();
    let renewal = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let payload = reader.recv().await.unwrap().unwrap();
            if let Some(message) = assembler.push(&payload).unwrap()
                && let Ok(ToDevice::Renewal(event)) = ToDevice::decode(&message)
            {
                return event;
            }
        }
    })
    .await
    .expect("the host renews at admission");
    let renewed = old
        .renewed(renewal, &phone.secret, now(), support::POLICY)
        .unwrap();
    assert_eq!(renewed.grant.rights, old.grant.rights);
    assert_eq!(renewed.grant.expires_at, renewed.grant.issued_at + 30 * day);

    // The channel now answers under the renewed grant, and a request under
    // it is admitted.
    let device = Device::new(renewed.clone(), phone.secret, support::POLICY).unwrap();
    let client =
        coder_host::access::Client::device(renewed, phone.secret, support::POLICY).unwrap();
    let pending = client.prepare(Operation::ListDevices {}, now()).unwrap();
    for part in
        fragments(&coder_host::message::ToHost::Call(pending.event.clone()).encode()).unwrap()
    {
        writer.send(&part).await.unwrap();
    }
    let answer = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let payload = reader.recv().await.unwrap().unwrap();
            if let Some(message) = assembler.push(&payload).unwrap()
                && let Ok(ToDevice::Answer(event)) = ToDevice::decode(&message)
            {
                return event;
            }
        }
    })
    .await
    .unwrap();
    let error = client.verify_reply(&pending, &answer, now()).unwrap_err();
    assert_eq!(error.code, Code::MissingRight);
    assert_ne!(device.grant(), old.grant.grant);

    // A revoked device never renews.
    host.store.revoke(&pubkey(&phone.secret), now()).unwrap();
    assert!(
        host.running
            .authority()
            .renew(&pubkey(&phone.secret), &old.grant.grant, 0)
            .is_none()
    );
    host.running.shutdown().await;
}
