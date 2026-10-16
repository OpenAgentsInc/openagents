//! Attestation against a fake report, and a light client that follows the
//! tree from chain data and records, and catches a broken enclave.

mod common;

use bitcoin::Amount;
use common::{MEASUREMENT, World};
use layertree_tee_lab::attest::{AttestError, FakeVendor, LogEntry, Platform, Policy, verify};
use layertree_tee_lab::enclave::Enclave;
use layertree_tee_lab::light::{LightError, Member};
use layertree_tee_lab::rules::Op;

fn policy(w: &World) -> Policy {
    Policy {
        vendor_root: w.vendor.root(&w.secp),
        releases: vec![LogEntry {
            measurement: MEASUREMENT,
            source_commit: "abc123".into(),
        }],
    }
}

#[test]
fn a_matching_digest_passes() {
    let w = World::new(2, 128, None);
    assert_eq!(
        verify(&w.secp, &policy(&w), &w.report, &w.params()),
        Ok("attested")
    );
}

#[test]
fn a_digest_not_in_the_release_log_fails() {
    let w = World::new(2, 128, None);
    let mut pol = policy(&w);
    pol.releases[0].measurement[0] ^= 1;
    assert_eq!(
        verify(&w.secp, &pol, &w.report, &w.params()),
        Err(AttestError::UnknownMeasurement)
    );
}

#[test]
fn a_report_for_another_key_or_other_parameters_fails() {
    let w = World::new(2, 128, None);
    let mut p = w.params();
    p.unroll_delay += 1;
    assert_eq!(
        verify(&w.secp, &policy(&w), &w.report, &p),
        Err(AttestError::ReportDataMismatch)
    );
    // Another enclave's key, same code.
    let cfg = layertree_tee_lab::enclave::Config {
        operator_key: p.operator_key,
        tree_id: p.tree_id,
        depth: p.depth,
        dust_min: p.dust_min,
        unroll_delay: w.params().unroll_delay,
        migration_m: None,
        radix: 128,
    };
    let (other, _) = Enclave::launch(cfg, MEASUREMENT, &w.vendor, Platform::Tdx);
    assert_eq!(
        verify(&w.secp, &policy(&w), &w.report, other.params()),
        Err(AttestError::ReportDataMismatch)
    );
}

#[test]
fn a_report_not_signed_by_the_vendor_fails() {
    let w = World::new(2, 128, None);
    let impostor = FakeVendor::new(&w.secp);
    let forged = impostor.sign_report(&w.secp, Platform::SevSnp, MEASUREMENT, w.report.report_data);
    assert_eq!(
        verify(&w.secp, &policy(&w), &forged, &w.params()),
        Err(AttestError::VendorSignature)
    );
}

fn member(w: &World, pos: u64) -> Member {
    let st = w.enclave.state().unwrap();
    Member {
        pos,
        leaf: st.tree.get(pos).copied(),
        path: st.tree.path(pos, 0).unwrap(),
        root: st.tree.root().unwrap(),
        outpoint: st.outpoint,
        unroll_key: st.output.unroll_key,
    }
}

#[test]
fn a_light_client_follows_spends() {
    let mut w = World::new(10, 128, None);
    let p = w.params();
    let mut me = member(&w, 4);
    let src = w.leaf(4);
    let mut c = w.member_leaf(30, 2_000);
    c.evictor_key = src.evictor_key;
    c.evict_after = src.evict_after;
    let v = w.voucher(&src, &c);
    let new = w.member_leaf(31, 9_000);
    let ops = vec![
        Op::Exit {
            pos: 1,
            evict: false,
        },
        Op::Join {
            height: 2,
            index: 5,
            leaves: vec![(1, new)],
        },
        Op::Claim {
            source_pos: 4,
            claim_pos: 70,
            claim_leaf: c,
            voucher_sig: v,
        },
        Op::Refresh,
        Op::Exit {
            pos: 9,
            evict: false,
        },
    ];
    for op in ops {
        let s = w.step(op).unwrap();
        me.follow(&w.secp, &p, &s.tx, &s.record).unwrap();
        let st = w.enclave.state().unwrap();
        assert_eq!(me.root, st.tree.root().unwrap());
        assert_eq!(me.path, st.tree.path(4, 0).unwrap());
        assert_eq!(me.leaf, st.tree.get(4).copied());
        assert_eq!(me.outpoint, st.outpoint);
    }
    // Our leaf paid 2,000 out by voucher and advanced seq.
    assert_eq!(me.leaf.unwrap().amount, src.amount - 2_000);
}

#[test]
fn a_light_client_catches_a_broken_enclave() {
    let mut w = World::new(5, 128, None);
    let p = w.params();
    let mut me = member(&w, 0);
    w.enclave.break_for_test();
    // The broken enclave signs an exit whose output 0 keeps less than the
    // tree's new sum: the difference goes to "fee", or to an attacker.
    let op = Op::Exit {
        pos: 3,
        evict: false,
    };
    let (mut tx, prevouts, sigs) = w.build(&op);
    tx.output[0].value = Amount::from_sat(tx.output[0].value.to_sat() - 50_000);
    let s = w.enclave.authorize(tx, &prevouts, &op, &sigs).unwrap();
    assert_eq!(
        me.follow(&w.secp, &p, &s.tx, &s.record),
        Err(LightError::OutputMismatch)
    );

    // A record that lies about the old state is caught too.
    let mut w = World::new(5, 128, None);
    let mut me = member(&w, 0);
    let s = w
        .step(Op::Exit {
            pos: 2,
            evict: false,
        })
        .unwrap();
    let mut rec = s.record.clone();
    rec.changes[0].old.as_mut().unwrap().amount += 1;
    // The forged record is not the anchored one.
    assert_eq!(
        me.follow(&w.secp, &p, &s.tx, &rec),
        Err(LightError::RecordNotAnchored)
    );
}
