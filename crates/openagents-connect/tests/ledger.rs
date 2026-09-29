//! Single-use redemption: first device wins, retries repeat, others are
//! refused, and cancellation and expiry hold.

use openagents_connect::Code;
use openagents_connect::code::{CodeParts, ConnectCode, LIFETIME};
use openagents_connect::ledger::{Ledger, MAX_INVITATIONS, Redeem};

const NOW: u64 = 1_800_000_000;

fn code(issued_at: u64) -> ConnectCode {
    let host = secp256k1::SecretKey::from_byte_array([2; 32]).unwrap();
    ConnectCode::issue(CodeParts {
        host: coder_reach_pubkey(&host),
        endpoint: iroh::SecretKey::from_bytes(&[3; 32]).public(),
        issued_at,
        relay: None,
        addrs: vec![],
        label: String::new(),
    })
    .unwrap()
}

fn coder_reach_pubkey(secret: &secp256k1::SecretKey) -> String {
    secret
        .x_only_public_key(&secp256k1::Secp256k1::new())
        .0
        .to_string()
}

#[test]
fn first_redemption_binds_the_device() {
    let mut ledger = Ledger::new();
    let code = code(NOW);
    ledger.issue(&code, NOW).unwrap();
    assert_eq!(ledger.outstanding(NOW), 1);
    let (id, cap) = (code.invitation(), code.capability());
    assert_eq!(
        ledger.redeem(&id, &cap, "phone", NOW, NOW + 1),
        Redeem::Admitted
    );
    assert_eq!(ledger.outstanding(NOW + 1), 0);
    assert_eq!(
        ledger.redeem(&id, &cap, "phone", NOW, NOW + 2),
        Redeem::Retry
    );
    assert_eq!(
        ledger.redeem(&id, &cap, "thief", NOW, NOW + 2),
        Redeem::Refused(Code::Forbidden)
    );
    // After expiry the bound device can still retry; others still cannot.
    assert_eq!(
        ledger.redeem(&id, &cap, "phone", NOW, NOW + LIFETIME + 100),
        Redeem::Retry
    );
    // A redeemed invitation is not cancelled by cancel_all.
    assert!(!ledger.cancel(&id));
    assert_eq!(ledger.cancel_all(), 0);
    assert_eq!(
        ledger.redeem(&id, &cap, "phone", NOW, NOW + 3),
        Redeem::Retry
    );
}

#[test]
fn unknown_invitations_and_wrong_capabilities_are_silent() {
    let mut ledger = Ledger::new();
    let code = code(NOW);
    ledger.issue(&code, NOW).unwrap();
    assert_eq!(
        ledger.redeem(&"ab".repeat(32), &code.capability(), "phone", NOW, NOW),
        Redeem::Silent
    );
    assert_eq!(
        ledger.redeem(&code.invitation(), &"ab".repeat(32), "phone", NOW, NOW),
        Redeem::Silent
    );
    assert_eq!(
        ledger.redeem(&code.invitation(), "not hex", "phone", NOW, NOW),
        Redeem::Silent
    );
    // The silent attempts did not consume it.
    assert_eq!(
        ledger.redeem(&code.invitation(), &code.capability(), "phone", NOW, NOW),
        Redeem::Admitted
    );
}

#[test]
fn cancelled_and_expired_invitations_are_refused() {
    let mut ledger = Ledger::new();
    let (a, b, c) = (code(NOW), code(NOW), code(NOW));
    for code in [&a, &b, &c] {
        ledger.issue(code, NOW).unwrap();
    }
    assert!(ledger.cancel(&a.invitation()));
    assert!(!ledger.cancel(&a.invitation()));
    assert_eq!(
        ledger.redeem(&a.invitation(), &a.capability(), "phone", NOW, NOW),
        Redeem::Refused(Code::Revoked)
    );
    assert_eq!(
        ledger.redeem(
            &b.invitation(),
            &b.capability(),
            "phone",
            NOW,
            NOW + LIFETIME
        ),
        Redeem::Refused(Code::Expired)
    );
    // A request signed outside the invitation window.
    assert_eq!(
        ledger.redeem(
            &b.invitation(),
            &b.capability(),
            "phone",
            NOW + LIFETIME,
            NOW + 10
        ),
        Redeem::Refused(Code::Forbidden)
    );
    assert_eq!(ledger.cancel_all(), 2);
    assert_eq!(
        ledger.redeem(&c.invitation(), &c.capability(), "phone", NOW, NOW),
        Redeem::Refused(Code::Revoked)
    );
    assert_eq!(ledger.outstanding(NOW), 0);
}

#[test]
fn the_ledger_is_bounded_and_ids_are_unique() {
    let mut ledger = Ledger::new();
    let first = code(NOW);
    ledger.issue(&first, NOW).unwrap();
    assert_eq!(ledger.issue(&first, NOW).unwrap_err().code, Code::Forbidden);
    for _ in 1..MAX_INVITATIONS {
        ledger.issue(&code(NOW), NOW).unwrap();
    }
    assert_eq!(
        ledger.issue(&code(NOW), NOW).unwrap_err().code,
        Code::LimitExceeded
    );
    // Expired, unredeemed entries make room.
    ledger.issue(&code(NOW + LIFETIME), NOW + LIFETIME).unwrap();
}
