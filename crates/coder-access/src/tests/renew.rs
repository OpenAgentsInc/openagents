//! Silent renewal: a device that keeps connecting never has to pair again,
//! and a revoked device never renews. Every exchange runs on the host's
//! clock, without a relay.
use super::*;
use crate::client::{finish_redeem, prepare_redeem};
use crate::host::Unconnected;

const DAY: u64 = 86_400;
const LIFETIME: u64 = 30 * DAY;

fn redeem(f: &Fixture, rights: &str, device: &SecretKey, at: u64) -> Access {
    let code = f
        .host()
        .invite(
            &f.relay,
            Rights::parse_list(rights).unwrap(),
            at,
            at + LIFETIME,
        )
        .unwrap()
        .code;
    let invitation = HostInvitation::parse(&code, at, POLICY).unwrap();
    let pending = prepare_redeem(&invitation, device, at, POLICY).unwrap();
    let reply = f
        .host()
        .handle(&pending.event, &f.relay, at, &mut Unconnected)
        .unwrap();
    finish_redeem(&invitation, &pending, &reply, device, at, POLICY).unwrap()
}

fn call(
    f: &Fixture,
    device: &SecretKey,
    access: &Access,
    op: Operation,
    at: u64,
) -> Result<Outcome> {
    let client = Client::device(access.clone(), *device, POLICY)?;
    let pending = client.prepare(op, at)?;
    let reply = f
        .host()
        .handle(&pending.event, &f.relay, at, &mut f.recorder.clone())?;
    client.verify_reply(&pending, &reply, at)
}

#[test]
fn a_grant_renews_near_its_end_with_the_same_terms_and_the_device_accepts_it() {
    let f = Fixture::local();
    let now = now();
    // Issued 24 days ago: six of its thirty days remain.
    let t = now - 24 * DAY;
    let phone = key();
    let access = redeem(&f, "observe,operate", &phone, t);
    let (grant, epoch) = (access.grant.grant.clone(), access.grant.epoch);
    let device = pubkey(&phone);

    // Early in its life nothing is due.
    assert!(
        f.host()
            .renew(&device, &grant, epoch, t + DAY)
            .unwrap()
            .is_none()
    );
    // Another device, or a wrong epoch, never renews this grant.
    assert!(
        f.host()
            .renew(&pubkey(&key()), &grant, epoch, now)
            .unwrap()
            .is_none()
    );
    assert!(
        f.host()
            .renew(&device, &grant, epoch + 1, now)
            .unwrap()
            .is_none()
    );

    let envelope = f
        .host()
        .renew(&device, &grant, epoch, now)
        .unwrap()
        .unwrap();
    let renewed = access
        .renewed(envelope.clone(), &phone, now, POLICY)
        .unwrap();
    assert_ne!(renewed.grant.grant, grant);
    assert_eq!(renewed.grant.rights, access.grant.rights);
    assert_eq!(renewed.grant.epoch, epoch);
    assert_eq!(renewed.grant.origin, access.grant.origin);
    assert_eq!(renewed.grant.issued_at, now);
    assert_eq!(renewed.grant.expires_at, now + LIFETIME);

    // Both grants admit until the old one expires, so nothing in flight is
    // cut off; the renewed grant does not renew again.
    call(&f, &phone, &access, task(), now).unwrap();
    call(&f, &phone, &renewed, task(), now).unwrap();
    assert!(
        f.host()
            .renew(&device, &grant, epoch, now)
            .unwrap()
            .is_none()
    );
    assert!(
        f.host()
            .renew(&device, &renewed.grant.grant, epoch, now)
            .unwrap()
            .is_none()
    );

    // A renewal envelope with other terms, or for another device, is refused.
    assert_eq!(
        renewed
            .renewed(envelope.clone(), &phone, now, POLICY)
            .unwrap_err()
            .code,
        Code::Forbidden
    );
    assert!(access.renewed(envelope, &key(), now, POLICY).is_err());
}

#[test]
fn a_revoked_device_never_renews_and_loses_both_grants() {
    let f = Fixture::local();
    let now = now();
    let t = now - 24 * DAY;
    let phone = key();
    let access = redeem(&f, "observe,operate", &phone, t);
    let device = pubkey(&phone);
    let envelope = f
        .host()
        .renew(&device, &access.grant.grant, 0, now)
        .unwrap()
        .unwrap();
    let renewed = access.renewed(envelope, &phone, now, POLICY).unwrap();
    f.host().revoke(&device, now).unwrap();
    for grant in [&access.grant.grant, &renewed.grant.grant] {
        assert!(f.host().renew(&device, grant, 0, now).unwrap().is_none());
        assert!(f.host().renew(&device, grant, 1, now).unwrap().is_none());
    }
    assert_eq!(
        call(&f, &phone, &access, task(), now).unwrap_err().code,
        Code::Revoked
    );
    assert_eq!(
        call(&f, &phone, &renewed, task(), now).unwrap_err().code,
        Code::Revoked
    );
}

#[test]
fn a_delegated_grant_does_not_renew() {
    let f = Fixture::local();
    let now = now();
    let t = now - 24 * DAY;
    let admin = key();
    let admin_access = redeem(&f, "all", &admin, t);
    let Outcome::Invitation { code, .. } = call(
        &f,
        &admin,
        &admin_access,
        Operation::Invite {
            rights: Rights::parse_list("observe").unwrap(),
            grant_expires_at: t + 20 * DAY,
        },
        t,
    )
    .unwrap() else {
        panic!("an invitation");
    };
    let phone = key();
    let invitation = HostInvitation::parse(&code, t, POLICY).unwrap();
    let pending = prepare_redeem(&invitation, &phone, t, POLICY).unwrap();
    let reply = f
        .host()
        .handle(&pending.event, &f.relay, t, &mut Unconnected)
        .unwrap();
    let delegated = finish_redeem(&invitation, &pending, &reply, &phone, t, POLICY).unwrap();
    assert!(
        f.host()
            .renew(&pubkey(&phone), &delegated.grant.grant, 0, t + 19 * DAY)
            .unwrap()
            .is_none()
    );
}
