//! One grant per device key (#9908): enrolling again supersedes the device's
//! earlier grants without touching grants it delegated, and a dead grant
//! never blocks a new enrollment. Every exchange here runs on the host's
//! clock, without a relay, so the tests can move time past retention.
use super::*;
use crate::client::{finish_redeem, prepare_redeem};
use crate::host::Unconnected;

/// Past every invitation's and request's retention.
const LATER: u64 = INVITATION_LIFETIME + MAX_REQUEST_LIFETIME + 1;

fn invite_at(f: &Fixture, rights: &str, at: u64) -> String {
    f.host()
        .invite(
            &f.relay,
            Rights::parse_list(rights).unwrap(),
            at,
            at + 86_400,
        )
        .unwrap()
        .code
}

fn redeem_at(f: &Fixture, code: &str, device: &SecretKey, at: u64) -> Result<Access> {
    let invitation = HostInvitation::parse(code, at, POLICY)?;
    let pending = prepare_redeem(&invitation, device, at, POLICY)?;
    let reply = f
        .host()
        .handle(&pending.event, &f.relay, at, &mut Unconnected)?;
    finish_redeem(&invitation, &pending, &reply, device, at, POLICY)
}

fn call_at(f: &Fixture, client: &Client, op: Operation, at: u64) -> Result<Outcome> {
    let pending = client.prepare(op, at)?;
    let reply = f
        .host()
        .handle(&pending.event, &f.relay, at, &mut f.recorder.clone())?;
    client.verify_reply(&pending, &reply, at)
}

/// A request under an exact grant, as a device still holding it would send.
fn call_under(f: &Fixture, device: &SecretKey, access: &Access, at: u64) -> Code {
    let client = Client::device(access.clone(), *device, POLICY).unwrap();
    call_at(f, &client, task(), at).unwrap_err().code
}

fn states(f: &Fixture, at: u64) -> Vec<(String, String, DeviceState)> {
    let mut listed: Vec<_> = f
        .host()
        .devices(at)
        .unwrap()
        .into_iter()
        .map(|d| (d.device, d.grant, d.state))
        .collect();
    listed.sort_by(|a, b| (&a.0, &a.1).cmp(&(&b.0, &b.1)));
    listed
}

#[test]
fn a_device_that_enrolls_again_holds_one_grant() {
    let f = Fixture::local();
    // In the past, so the device's offline checks of its grants pass.
    let t = now() - 3_600;
    let phone = key();
    let laptop = key();
    let first_code = invite_at(&f, "standard", t);
    let first = redeem_at(&f, &first_code, &phone, t).unwrap();
    let other = redeem_at(&f, &invite_at(&f, "standard", t), &laptop, t).unwrap();

    let second = redeem_at(&f, &invite_at(&f, "standard", t + 10), &phone, t + 10).unwrap();
    // No epoch advanced: the new grant is current at the device's epoch.
    assert_eq!(second.grant.epoch, 0);
    assert_ne!(second.grant.grant, first.grant.grant);
    // The first grant stays, revoked, while its invitation is retained.
    let listed = states(&f, t + 10);
    let state = |grant: &str| listed.iter().find(|(_, g, _)| g == grant).unwrap().2;
    assert_eq!(listed.len(), 3);
    assert_eq!(state(&first.grant.grant), DeviceState::Revoked);
    assert_eq!(state(&second.grant.grant), DeviceState::Active);
    assert_eq!(state(&other.grant.grant), DeviceState::Active);
    assert_eq!(call_under(&f, &phone, &first, t + 11), Code::Revoked);
    // Redeeming the first invitation again says its grant was revoked.
    let again = redeem_at(&f, &first_code, &phone, t + 11).unwrap_err();
    assert_eq!(again.code, Code::Revoked);
    // A redemption retried under the same invitation recovers the same grant.
    let code = invite_at(&f, "standard", t + 12);
    let retried = redeem_at(&f, &code, &phone, t + 12).unwrap();
    assert_eq!(
        redeem_at(&f, &code, &phone, t + 13).unwrap().grant.grant,
        retried.grant.grant
    );
    let current = Client::device(retried.clone(), phone, POLICY).unwrap();
    assert!(call_at(&f, &current, task(), t + 14).is_ok());
    assert_eq!(call_under(&f, &phone, &second, t + 14), Code::Revoked);

    // Once no invitation names them, the superseded grants leave the book
    // at the device's next enrollment, and a request under them reads nothing.
    let later = t + 12 + LATER;
    let last = redeem_at(&f, &invite_at(&f, "standard", later), &phone, later).unwrap();
    let listed = states(&f, later);
    assert_eq!(
        listed
            .iter()
            .filter(|(device, _, _)| *device == pubkey(&phone))
            .map(|(_, grant, state)| (grant.clone(), *state))
            .collect::<Vec<_>>(),
        vec![(last.grant.grant.clone(), DeviceState::Active)]
    );
    for old in [&first, &second, &retried] {
        assert_eq!(call_under(&f, &phone, old, later + 1), Code::Forbidden);
    }
    // Another device's grant is untouched throughout.
    let laptop_client = Client::device(other, laptop, POLICY).unwrap();
    assert!(call_at(&f, &laptop_client, task(), later + 1).is_ok());
}

#[test]
fn revoked_grants_never_block_a_new_enrollment() {
    let f = Fixture::local();
    let t = now();
    // Fill the book's 128 grants.
    let devices: Vec<SecretKey> = (0..128).map(|_| key()).collect();
    let keys: Vec<String> = devices.iter().map(pubkey).collect();
    f.host()
        .issue_for_test(&keys, &f.relay, &Rights::standard(), t, t + 86_400)
        .unwrap();
    let mut at = t + 1;
    // Revoked grants: the earliest revoked leaves first.
    for (offset, device) in devices[..3].iter().enumerate() {
        f.host()
            .revoke(&pubkey(device), at + offset as u64)
            .unwrap();
    }
    at += 10;
    let newcomer = key();
    redeem_at(&f, &invite_at(&f, "standard", at), &newcomer, at).unwrap();
    let listed = states(&f, at);
    assert_eq!(listed.len(), 128);
    let holds = |d: &SecretKey| listed.iter().any(|(device, _, _)| *device == pubkey(d));
    assert!(!holds(&devices[0]));
    assert!(holds(&devices[1]) && holds(&devices[2]) && holds(&newcomer));

    // Two more newcomers take the other two revoked grants' places.
    for _ in 0..2 {
        at += LATER;
        redeem_at(&f, &invite_at(&f, "standard", at), &key(), at).unwrap();
    }
    let listed = states(&f, at);
    assert_eq!(listed.len(), 128);
    assert!(listed.iter().all(|(_, _, s)| *s == DeviceState::Active));

    // Only live grants to 128 devices refuse a new device.
    at += LATER;
    let refused = redeem_at(&f, &invite_at(&f, "standard", at), &key(), at).unwrap_err();
    assert_eq!(refused.code, Code::Bounds);
    // A device already there still enrolls again: its own grant makes room.
    at += LATER;
    let again = redeem_at(&f, &invite_at(&f, "standard", at), &devices[5], at).unwrap();
    let listed = states(&f, at);
    assert_eq!(listed.len(), 128);
    assert!(listed.iter().any(|(_, g, _)| *g == again.grant.grant));
}

#[test]
fn delegations_survive_their_issuer_enrolling_again() {
    let f = Fixture::local();
    let t = now() - 3_600;
    let admin_key = key();
    let admin_access = redeem_at(&f, &invite_at(&f, "all", t), &admin_key, t).unwrap();
    let admin = Client::device(admin_access.clone(), admin_key, POLICY).unwrap();
    let expires = admin_access.grant.expires_at;
    let delegate = |rights: &str, at| {
        let op = Operation::Invite {
            rights: Rights::parse_list(rights).unwrap(),
            grant_expires_at: expires,
        };
        let Outcome::Invitation { code, .. } = call_at(&f, &admin, op, at).unwrap() else {
            panic!("invitation expected")
        };
        code
    };
    // A redeemed delegation, and two still waiting.
    let helper_key = key();
    let helper_access =
        redeem_at(&f, &delegate("observe,operate", t + 1), &helper_key, t + 1).unwrap();
    let helper = Client::device(helper_access.clone(), helper_key, POLICY).unwrap();
    let waiting = delegate("observe,operate", t + 2);

    // The administrator enrolls again with the same rights: the waiting
    // invitation moves to its new grant, and the redeemed one is untouched.
    let renewed = redeem_at(&f, &invite_at(&f, "all", t + 3), &admin_key, t + 3).unwrap();
    assert_eq!(renewed.grant.epoch, 0);
    assert!(call_at(&f, &helper, task(), t + 4).is_ok());
    let late = redeem_at(&f, &waiting, &key(), t + 5).unwrap();
    assert_eq!(late.grant.origin.issuer, pubkey(&admin_key));
    let admin = Client::device(renewed.clone(), admin_key, POLICY).unwrap();

    // Enrolling again without access_admin hands on no delegation: what
    // waits under the superseded grant is refused, and the helpers keep theirs.
    let op = Operation::Invite {
        rights: Rights::parse_list("observe").unwrap(),
        grant_expires_at: renewed.grant.expires_at,
    };
    let Outcome::Invitation { code: stranded, .. } = call_at(&f, &admin, op, t + 6).unwrap() else {
        panic!("invitation expected")
    };
    redeem_at(
        &f,
        &invite_at(&f, "observe,operate", t + 7),
        &admin_key,
        t + 7,
    )
    .unwrap();
    let refused = redeem_at(&f, &stranded, &key(), t + 8).unwrap_err();
    assert_eq!(refused.code, Code::Revoked);
    assert!(call_at(&f, &helper, task(), t + 9).is_ok());

    // A revoked administrator's waiting invitations stay refused when it
    // enrolls again, even with every right: revocation is not undone.
    let boss_key = key();
    let boss_access = redeem_at(&f, &invite_at(&f, "all", t + 10), &boss_key, t + 10).unwrap();
    let boss = Client::device(boss_access.clone(), boss_key, POLICY).unwrap();
    let op = Operation::Invite {
        rights: Rights::parse_list("observe").unwrap(),
        grant_expires_at: boss_access.grant.expires_at,
    };
    let Outcome::Invitation { code: killed, .. } = call_at(&f, &boss, op, t + 11).unwrap() else {
        panic!("invitation expected")
    };
    f.host().revoke(&pubkey(&boss_key), t + 12).unwrap();
    let back = redeem_at(&f, &invite_at(&f, "all", t + 13), &boss_key, t + 13).unwrap();
    assert_eq!(back.grant.epoch, 1);
    let refused = redeem_at(&f, &killed, &key(), t + 14).unwrap_err();
    assert_eq!(refused.code, Code::Revoked);
}
