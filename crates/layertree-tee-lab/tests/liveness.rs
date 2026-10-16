//! The enclave's liveness and publication rules (#11786): two-phase
//! signing, forced inclusion of exits, mirror receipts before any
//! signature, and the owner's `HALT`.

mod common;

use bitcoin::key::Keypair;
use bitcoin::secp256k1::{Message, SecretKey};
use bitcoin::{Amount, OutPoint};
use common::{World, guardian, sk};
use layertree_tee_lab::enclave::{EXIT_DEADLINE, Prepared, halt_msg};
use layertree_tee_lab::mirror::{self, MemMirror, Mirror, MirrorPolicy, Receipt};
use layertree_tee_lab::rules::{Op, Reject};
use layertree_tee_lab::tree::exit_auth_msg;
use layertree_tee_lab::unroll::verify_package;

/// A member's client: check the prepared unroll of the new state with
/// public data only, and sign the transaction only if it pays every leaf.
fn member_signs(
    w: &World,
    prep: &Prepared,
    op: &Op,
) -> Option<Vec<bitcoin::secp256k1::schnorr::Signature>> {
    let p = w.params();
    if let Some(next) = &prep.next {
        let pkg = prep.unroll.as_ref()?;
        for leaf in next.tree.leaves().values() {
            verify_package(
                &w.secp,
                &p,
                next.output,
                next.outpoint,
                next.amount(),
                next.tree.leaves(),
                pkg,
                leaf,
            )
            .ok()?;
        }
    }
    Some(w.sign(&prep.tx, &prep.prevouts, op))
}

#[test]
fn two_phase_members_sign_only_after_checking_the_unroll() {
    let mut w = World::new(6, 4, None);
    let joiner = w.member_leaf(40, 7_000);
    let op = Op::Join {
        height: 0,
        index: 50,
        leaves: vec![(0, joiner)],
    };
    let (tx, prevouts, _) = w.build(&op);
    let before = w.enclave.state().unwrap().outpoint;
    let mut prep = w.enclave.prepare(tx, &prevouts, &op).unwrap();
    // The unroll is signed and its key gone before anyone signs.
    assert_eq!(w.enclave.ephemeral_keys_held(), 0);
    assert!(prep.tx.input[0].witness.is_empty());

    // A tampered unroll (one sat moved between two payouts): the member
    // refuses, so the enclave has nothing to sign with.
    let good = prep.unroll.clone().unwrap();
    let mut bad = good.clone();
    let last = bad.txs.last_mut().unwrap();
    last.output[0].value = Amount::from_sat(last.output[0].value.to_sat() - 1);
    last.output[1].value = Amount::from_sat(last.output[1].value.to_sat() + 1);
    prep.unroll = Some(bad);
    assert!(member_signs(&w, &prep, &op).is_none());
    assert_eq!(
        w.enclave.finish(&[], &[]).unwrap_err(),
        Reject::SignatureCount
    );
    assert_eq!(w.enclave.state().unwrap().outpoint, before, "nothing moved");

    // Signatures over another transaction do not count either.
    let other = {
        let mut t = prep.tx.clone();
        t.lock_time = bitcoin::absolute::LockTime::from_height(7).unwrap();
        w.sign(&t, &prep.prevouts, &op)
    };
    assert_eq!(
        w.enclave.finish(&other, &[]).unwrap_err(),
        Reject::BadSignature("transaction")
    );

    // The real unroll checks out; the member signs and the enclave
    // finishes, publishing exactly the unroll the member checked.
    prep.unroll = Some(good.clone());
    let sigs = member_signs(&w, &prep, &op).expect("a valid unroll");
    let signed = w.enclave.finish(&sigs, &[]).unwrap();
    assert_eq!(signed.unroll, Some(good));
    assert_eq!(
        w.enclave.state().unwrap().outpoint,
        OutPoint {
            txid: signed.tx.compute_txid(),
            vout: 0
        }
    );
    assert!(
        w.enclave.finish(&sigs, &[]).is_err(),
        "nothing left to finish"
    );
}

fn exit_request_sig(w: &World, pos: u64) -> bitcoin::secp256k1::schnorr::Signature {
    let leaf = w.leaf(pos);
    let msg = Message::from_digest(exit_auth_msg(&w.params().tree_id, &leaf));
    w.sign_with(&leaf.owner_key, &msg)
}

#[test]
fn a_censored_exit_halts_every_non_exit_transition_after_the_deadline() {
    let mut w = World::new(6, 128, None);
    w.enclave.set_height(100);
    // Only the leaf's owner can file the request.
    let forged = w.sign_with(&w.leaf(3).owner_key, &Message::from_digest([1; 32]));
    assert_eq!(
        w.enclave.request_exit(2, &forged).unwrap_err(),
        Reject::BadSignature("exit request")
    );
    let sig = exit_request_sig(&w, 2);
    w.enclave.request_exit(2, &sig).unwrap();
    assert_eq!(w.enclave.exit_requests(), &[(2, 100)]);

    // Inside the deadline the operator may still do other work.
    w.enclave.set_height(100 + EXIT_DEADLINE - 1);
    w.step(Op::Refresh).unwrap();
    // At the deadline, everything but exits is refused.
    w.enclave.set_height(100 + EXIT_DEADLINE);
    assert_eq!(w.step(Op::Refresh).unwrap_err(), Reject::ExitCensored);
    let joiner = w.member_leaf(41, 5_000);
    assert_eq!(
        w.step(Op::Join {
            height: 0,
            index: 60,
            leaves: vec![(0, joiner)],
        })
        .unwrap_err(),
        Reject::ExitCensored
    );
    // Another member's exit still goes through; the request stays.
    w.step(Op::Exit {
        pos: 4,
        evict: false,
    })
    .unwrap();
    assert_eq!(w.step(Op::Refresh).unwrap_err(), Reject::ExitCensored);
    // Including the requested exit clears it, and the tree goes on.
    w.step(Op::Exit {
        pos: 2,
        evict: false,
    })
    .unwrap();
    assert!(w.enclave.exit_requests().is_empty());
    w.step(Op::Refresh).unwrap();
}

#[test]
fn no_signature_without_m_of_m_mirror_receipts() {
    let mut w = World::new(6, 128, None);
    let mut mirrors: Vec<MemMirror> = (0..3).map(|i| MemMirror::new(&sk(900 + i))).collect();
    let keys = mirrors.iter().map(Mirror::key).collect();
    w.enclave.set_mirror_policy(MirrorPolicy {
        mirrors: keys,
        m: 2,
    });

    let op = Op::Refresh;
    let (tx, prevouts, sigs) = w.build(&op);
    let before = w.enclave.state().unwrap().outpoint;
    // The one-call path has no receipts to show.
    let prep = w.enclave.prepare(tx, &prevouts, &op).unwrap();
    assert_eq!(
        w.enclave.finish(&sigs, &[]).unwrap_err(),
        Reject::MissingReceipts
    );

    // Two mirrors down: one receipt is not enough.
    mirrors[1].up = false;
    mirrors[2].up = false;
    let one = mirror::collect(&prep.record, prep.unroll.as_ref(), &mut mirrors);
    assert_eq!(one.len(), 1);
    assert_eq!(
        w.enclave.finish(&sigs, &one).unwrap_err(),
        Reject::MissingReceipts
    );
    // The same mirror twice counts once.
    let twice = [one[0], one[0]];
    assert_eq!(
        w.enclave.finish(&sigs, &twice).unwrap_err(),
        Reject::MissingReceipts
    );
    // A mirror the enclave was not configured with does not count.
    let mut stranger = MemMirror::new(&sk(950));
    let extra = stranger.store(&prep.record, prep.unroll.as_ref()).unwrap();
    assert_eq!(
        w.enclave.finish(&sigs, &[one[0], extra]).unwrap_err(),
        Reject::MissingReceipts
    );
    // A receipt for other data (no unroll) does not count.
    let mut m1 = MemMirror::new(&sk(901));
    let wrong = m1.store(&prep.record, None).unwrap();
    assert_eq!(
        w.enclave.finish(&sigs, &[one[0], wrong]).unwrap_err(),
        Reject::MissingReceipts
    );
    // A forged signature from a configured mirror does not count.
    let forged = Receipt {
        mirror: mirrors[1].key(),
        sig: one[0].sig,
    };
    assert_eq!(
        w.enclave.finish(&sigs, &[one[0], forged]).unwrap_err(),
        Reject::MissingReceipts
    );
    assert_eq!(w.enclave.state().unwrap().outpoint, before);

    // Back up: two receipts, and the mirrors now hold the record and unroll.
    mirrors[1].up = true;
    let two = mirror::collect(&prep.record, prep.unroll.as_ref(), &mut mirrors);
    assert_eq!(two.len(), 2);
    let signed = w.enclave.finish(&sigs, &two).unwrap();
    assert!(mirrors[1].records.contains_key(&signed.record.hash()));
    assert_eq!(mirrors[1].unrolls.len(), 1);
}

#[test]
fn after_halt_only_exits_are_signed() {
    let mut w = World::new(6, 128, None);
    let g = guardian();
    let msg = Message::from_digest(halt_msg(&w.params().tree_id));
    // No guardian configured: nobody can halt.
    let sig = w
        .secp
        .sign_schnorr(&msg, &Keypair::from_secret_key(&w.secp, &g));
    assert_eq!(w.enclave.halt(&sig).unwrap_err(), Reject::NotAllowed);
    w.enclave
        .set_guardian(bitcoin::key::XOnlyPublicKey::from(g.public_key(&w.secp)));
    // Only the guardian's signature halts it.
    let other: SecretKey = sk(77);
    let not_g = w
        .secp
        .sign_schnorr(&msg, &Keypair::from_secret_key(&w.secp, &other));
    assert_eq!(
        w.enclave.halt(&not_g).unwrap_err(),
        Reject::BadSignature("halt")
    );
    assert!(!w.enclave.halted());
    w.enclave.halt(&sig).unwrap();
    assert!(w.enclave.halted());

    // No heartbeat, no join, no update, no claim; exits and evictions only.
    assert_eq!(w.step(Op::Refresh).unwrap_err(), Reject::Halted);
    let joiner = w.member_leaf(42, 5_000);
    assert_eq!(
        w.step(Op::Join {
            height: 0,
            index: 70,
            leaves: vec![(0, joiner)],
        })
        .unwrap_err(),
        Reject::Halted
    );
    let l = w.leaf(0);
    assert_eq!(
        w.step(Op::Update {
            pos: 0,
            new_leaf: layertree_tee_lab::tree::Leaf { nonce: 9, ..l },
        })
        .unwrap_err(),
        Reject::Halted
    );
    w.step(Op::Exit {
        pos: 1,
        evict: false,
    })
    .unwrap();
    // The members' current unroll stays valid: it is the way out.
    let st = w.enclave.state().unwrap().clone();
    let pkg = w.last_unroll.clone().unwrap();
    for leaf in st.tree.leaves().values() {
        verify_package(
            &w.secp,
            &w.params(),
            st.output,
            st.outpoint,
            st.amount(),
            st.tree.leaves(),
            &pkg,
            leaf,
        )
        .unwrap();
    }
}
