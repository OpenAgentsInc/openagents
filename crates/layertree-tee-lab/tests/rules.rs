//! The rule predicate through the enclave: what it signs, and a rejection
//! for every rule a transaction can break.

mod common;

use bitcoin::secp256k1::Message;
use bitcoin::{Amount, OutPoint, ScriptBuf, Sequence, absolute::LockTime};
use common::{LEASE, World};
use layertree_tee_lab::output::{ccv_tree_spk, p2tr_raw};
use layertree_tee_lab::rules::{Op, Reject};
use layertree_tee_lab::tree::{Leaf, SEQ_FINAL, TreeError};

fn total(w: &World) -> u64 {
    w.enclave.state().map(|s| s.amount()).unwrap_or(0)
}

#[test]
fn owner_exit_is_signed_and_conserves() {
    let mut w = World::new(5, 128, None);
    let before = total(&w);
    let leaf = w.leaf(2);
    let s = w
        .step(Op::Exit {
            pos: 2,
            evict: false,
        })
        .unwrap();
    assert_eq!(s.tx.output[1].script_pubkey, p2tr_raw(&leaf.exit_key));
    assert_eq!(s.tx.output[1].value.to_sat(), leaf.amount);
    assert_eq!(s.tx.output[0].value.to_sat(), before - leaf.amount);
    assert_eq!(total(&w), before - leaf.amount);
    assert_eq!(w.enclave.state().unwrap().tree.total(), total(&w));
    // The next state's unroll key is gone from the enclave.
    assert_eq!(w.enclave.ephemeral_keys_held(), 0);
    // A one-element witness: a key-path spend.
    assert_eq!(s.tx.input[0].witness.len(), 1);
}

#[test]
fn the_enclave_key_path_signature_verifies_on_the_old_output() {
    use bitcoin::key::XOnlyPublicKey;
    use bitcoin::secp256k1::schnorr::Signature;
    let mut w = World::new(3, 128, None);
    let p = w.params();
    let state = w.enclave.state().unwrap().clone();
    let (tx, prevouts, sigs) = w.build(&Op::Exit {
        pos: 0,
        evict: false,
    });
    let s = w
        .enclave
        .authorize(
            tx,
            &prevouts,
            &Op::Exit {
                pos: 0,
                evict: false,
            },
            &sigs,
        )
        .unwrap();
    let spk = state
        .output
        .script_pubkey(&w.secp, &p.enclave_key, p.unroll_delay);
    let out_key = XOnlyPublicKey::from_slice(&spk.as_bytes()[2..]).unwrap();
    let sig = Signature::from_slice(s.tx.input[0].witness.nth(0).unwrap()).unwrap();
    let msg = Message::from_digest(layertree_tee_lab::rules::sighash(&s.tx, &prevouts));
    w.secp.verify_schnorr(&sig, &msg, &out_key).unwrap();
}

#[test]
fn eviction_waits_for_the_lease() {
    let mut w = World::new(3, 128, None);
    let op = Op::Exit {
        pos: 1,
        evict: true,
    };
    // Lock time below the lease.
    let (mut tx, prevouts, _) = w.build(&op);
    tx.lock_time = LockTime::from_height(LEASE - 1).unwrap();
    let sigs = w.sign(&tx, &prevouts, &op);
    assert_eq!(
        w.enclave.authorize(tx, &prevouts, &op, &sigs).unwrap_err(),
        Reject::Lease
    );
    // Lock time disabled by a final nSequence.
    let (mut tx, prevouts, _) = w.build(&op);
    tx.input[0].sequence = Sequence::MAX;
    let sigs = w.sign(&tx, &prevouts, &op);
    assert_eq!(
        w.enclave.authorize(tx, &prevouts, &op, &sigs).unwrap_err(),
        Reject::Lease
    );
    // At the lease, signed by the evictor: accepted, paid in full.
    let leaf = w.leaf(1);
    let s = w.step(op).unwrap();
    assert_eq!(s.tx.output[1].value.to_sat(), leaf.amount);
}

#[test]
fn eviction_needs_the_evictor_and_exit_needs_the_owner() {
    let mut w = World::new(3, 128, None);
    let op = Op::Exit {
        pos: 0,
        evict: false,
    };
    let (tx, prevouts, _) = w.build(&op);
    let msg = Message::from_digest(layertree_tee_lab::rules::sighash(&tx, &prevouts));
    // Operator signs in the owner's place.
    let forged = vec![w.sign_with(&w.operator.clone(), &msg)];
    assert_eq!(
        w.enclave
            .authorize(tx.clone(), &prevouts, &op, &forged)
            .unwrap_err(),
        Reject::BadSignature("transaction")
    );
    // No signature at all.
    assert_eq!(
        w.enclave.authorize(tx, &prevouts, &op, &[]).unwrap_err(),
        Reject::SignatureCount
    );
}

#[test]
fn a_signature_over_another_transaction_is_refused() {
    let mut w = World::new(3, 128, None);
    let op = Op::Exit {
        pos: 0,
        evict: false,
    };
    let (mut tx, prevouts, sigs) = w.build(&op);
    // The owner signed; then someone adds an output the rules leave free
    // (a change output). The owner's signature no longer covers the tx.
    tx.output.push(bitcoin::TxOut {
        value: Amount::from_sat(5_000),
        script_pubkey: ScriptBuf::new_p2tr(
            &w.secp,
            bitcoin::key::XOnlyPublicKey::from_slice(&w.operator).unwrap(),
            None,
        ),
    });
    assert_eq!(
        w.enclave.authorize(tx, &prevouts, &op, &sigs).unwrap_err(),
        Reject::BadSignature("transaction")
    );
}

#[test]
fn payouts_must_be_exact_and_to_the_exit_key() {
    let mut w = World::new(3, 128, None);
    let op = Op::Exit {
        pos: 0,
        evict: false,
    };
    for tamper in 0..3 {
        let (mut tx, prevouts, _) = w.build(&op);
        match tamper {
            0 => tx.output[1].value = Amount::from_sat(tx.output[1].value.to_sat() - 1),
            1 => tx.output[1].value = Amount::from_sat(tx.output[1].value.to_sat() + 1),
            _ => tx.output[1].script_pubkey = p2tr_raw(&w.operator),
        }
        let sigs = w.sign(&tx, &prevouts, &op);
        assert_eq!(
            w.enclave.authorize(tx, &prevouts, &op, &sigs).unwrap_err(),
            Reject::Payout,
            "tamper {tamper}"
        );
    }
}

#[test]
fn output_zero_must_carry_the_new_root_and_sum() {
    let mut w = World::new(3, 128, None);
    let op = Op::Exit {
        pos: 0,
        evict: false,
    };
    // Amount short by one sat.
    let (mut tx, prevouts, _) = w.build(&op);
    tx.output[0].value = Amount::from_sat(tx.output[0].value.to_sat() - 1);
    let sigs = w.sign(&tx, &prevouts, &op);
    assert_eq!(
        w.enclave.authorize(tx, &prevouts, &op, &sigs).unwrap_err(),
        Reject::OutputZero
    );
    // Output 0 under the old root (a replay of the state).
    let (mut tx, prevouts, _) = w.build(&op);
    tx.output[0].script_pubkey = prevouts[0].script_pubkey.clone();
    let sigs = w.sign(&tx, &prevouts, &op);
    assert_eq!(
        w.enclave.authorize(tx, &prevouts, &op, &sigs).unwrap_err(),
        Reject::OutputZero
    );
    // Built for an unroll key the enclave never made (a stale key).
    let (tx, prevouts, sigs) = w.build(&op);
    let _fresh = w.enclave.next_unroll_key();
    assert_eq!(
        w.enclave.authorize(tx, &prevouts, &op, &sigs).unwrap_err(),
        Reject::OutputZero
    );
}

#[test]
fn the_record_must_be_anchored() {
    let mut w = World::new(3, 128, None);
    let op = Op::Exit {
        pos: 0,
        evict: false,
    };
    let (mut tx, prevouts, _) = w.build(&op);
    tx.output.pop();
    let sigs = w.sign(&tx, &prevouts, &op);
    assert_eq!(
        w.enclave.authorize(tx, &prevouts, &op, &sigs).unwrap_err(),
        Reject::MissingRecord
    );
}

#[test]
fn input_zero_must_be_the_tree() {
    let mut w = World::new(3, 128, None);
    let op = Op::Exit {
        pos: 0,
        evict: false,
    };
    let (mut tx, prevouts, sigs) = w.build(&op);
    tx.input.swap(0, 1);
    assert_eq!(
        w.enclave.authorize(tx, &prevouts, &op, &sigs).unwrap_err(),
        Reject::WrongInput
    );
    let (mut tx, prevouts, sigs) = w.build(&op);
    tx.input[0].previous_output = OutPoint {
        vout: 1,
        ..tx.input[0].previous_output
    };
    assert_eq!(
        w.enclave.authorize(tx, &prevouts, &op, &sigs).unwrap_err(),
        Reject::WrongInput
    );
    let (tx, mut prevouts, sigs) = w.build(&op);
    prevouts[0].value = Amount::from_sat(prevouts[0].value.to_sat() + 1);
    assert_eq!(
        w.enclave.authorize(tx, &prevouts, &op, &sigs).unwrap_err(),
        Reject::PrevoutMismatch
    );
}

#[test]
fn join_grafts_real_leaves_into_an_empty_slot() {
    let mut w = World::new(4, 128, None);
    let before = total(&w);
    let a = w.member_leaf(10, 5_000);
    let b = w.member_leaf(11, 7_000);
    // Height 2, index 2: positions 8..12.
    let s = w
        .step(Op::Join {
            height: 2,
            index: 2,
            leaves: vec![(0, a), (3, b)],
        })
        .unwrap();
    assert_eq!(s.tx.output[0].value.to_sat(), before + 12_000);
    assert_eq!(w.leaf(8), a);
    assert_eq!(w.leaf(11), b);
    // Height 2, index 0 overlaps members 0..4.
    let c = w.member_leaf(12, 5_000);
    let op = Op::Join {
        height: 2,
        index: 0,
        leaves: vec![(0, c)],
    };
    let st = w.enclave.state().unwrap().clone();
    assert_eq!(
        layertree_tee_lab::rules::effects(&w.secp, &w.params(), &st, &op).unwrap_err(),
        Reject::SlotNotEmpty
    );
    // An offset outside the slot.
    let op = Op::Join {
        height: 1,
        index: 10,
        leaves: vec![(2, c)],
    };
    assert_eq!(
        layertree_tee_lab::rules::effects(&w.secp, &w.params(), &st, &op).unwrap_err(),
        Reject::SlotNotEmpty
    );
    // A zero-amount leaf.
    let mut z = c;
    z.amount = 0;
    let op = Op::Join {
        height: 0,
        index: 40,
        leaves: vec![(0, z)],
    };
    assert!(matches!(
        layertree_tee_lab::rules::effects(&w.secp, &w.params(), &st, &op).unwrap_err(),
        Reject::Tree(TreeError::InvalidLeaf(_))
    ));
}

#[test]
fn join_needs_the_operator() {
    let mut w = World::new(2, 128, None);
    let c = w.member_leaf(12, 5_000);
    let op = Op::Join {
        height: 0,
        index: 40,
        leaves: vec![(0, c)],
    };
    let (tx, prevouts, _) = w.build(&op);
    let msg = Message::from_digest(layertree_tee_lab::rules::sighash(&tx, &prevouts));
    let wrong = vec![w.sign_with(&c.owner_key, &msg)];
    assert_eq!(
        w.enclave.authorize(tx, &prevouts, &op, &wrong).unwrap_err(),
        Reject::BadSignature("transaction")
    );
}

#[test]
fn update_needs_owner_and_operator() {
    let mut w = World::new(3, 128, None);
    let mut renewed = w.leaf(1);
    renewed.evict_after = LEASE + 52_560;
    let op = Op::Update {
        pos: 1,
        new_leaf: renewed,
    };
    let (tx, prevouts, sigs) = w.build(&op);
    // Operator twice, no owner.
    let bad = vec![sigs[1], sigs[1]];
    assert_eq!(
        w.enclave
            .authorize(tx.clone(), &prevouts, &op, &bad)
            .unwrap_err(),
        Reject::BadSignature("transaction")
    );
    w.enclave.authorize(tx, &prevouts, &op, &sigs).unwrap();
    assert_eq!(w.leaf(1).evict_after, LEASE + 52_560);
}

fn claim_leaf(w: &mut World, src: &Leaf, member: u32, amount: u64) -> Leaf {
    let mut c = w.member_leaf(member, amount);
    c.evictor_key = src.evictor_key;
    c.evict_after = src.evict_after;
    c
}

#[test]
fn claim_carves_a_voucher_and_advances_seq() {
    let mut w = World::new(4, 128, None);
    let before = total(&w);
    let src = w.leaf(3); // 40,000 sat
    let c = claim_leaf(&mut w, &src, 50, 15_000);
    let v = w.voucher(&src, &c);
    w.step(Op::Claim {
        source_pos: 3,
        claim_pos: 60,
        claim_leaf: c,
        voucher_sig: v,
    })
    .unwrap();
    assert_eq!(total(&w), before);
    assert_eq!(w.leaf(3).amount, 25_000);
    assert_eq!(w.leaf(3).seq, 1);
    assert_eq!(w.leaf(60), c);

    // Replaying the same voucher fails: the source's seq moved.
    let c2 = claim_leaf(&mut w, &src, 51, 15_000);
    let _ = c2;
    let op = Op::Claim {
        source_pos: 3,
        claim_pos: 61,
        claim_leaf: c,
        voucher_sig: v,
    };
    let st = w.enclave.state().unwrap().clone();
    assert_eq!(
        layertree_tee_lab::rules::effects(&w.secp, &w.params(), &st, &op).unwrap_err(),
        Reject::BadSignature("voucher")
    );

    // A draining voucher empties the source position.
    let src = w.leaf(3);
    let all = claim_leaf(&mut w, &src, 52, src.amount);
    let v = w.voucher(&src, &all);
    w.step(Op::Claim {
        source_pos: 3,
        claim_pos: 62,
        claim_leaf: all,
        voucher_sig: v,
    })
    .unwrap();
    assert!(w.enclave.state().unwrap().tree.get(3).is_none());
    assert_eq!(total(&w), before);
}

#[test]
fn claim_rejections() {
    let mut w = World::new(4, 128, None);
    let src = w.leaf(3); // 40,000 sat
    let st = w.enclave.state().unwrap().clone();
    let p = w.params();
    let fx = |w: &World, op: &Op| layertree_tee_lab::rules::effects(&w.secp, &p, &st, op);

    let over = claim_leaf(&mut w, &src, 50, 40_001);
    let op = Op::Claim {
        source_pos: 3,
        claim_pos: 60,
        claim_leaf: over,
        voucher_sig: w.voucher(&src, &over),
    };
    assert_eq!(fx(&w, &op).unwrap_err(), Reject::Underflow);

    let dust = claim_leaf(&mut w, &src, 50, 329);
    let op = Op::Claim {
        source_pos: 3,
        claim_pos: 60,
        claim_leaf: dust,
        voucher_sig: w.voucher(&src, &dust),
    };
    assert_eq!(fx(&w, &op).unwrap_err(), Reject::Dust);

    let leaves_dust = claim_leaf(&mut w, &src, 50, 40_000 - 100);
    let op = Op::Claim {
        source_pos: 3,
        claim_pos: 60,
        claim_leaf: leaves_dust,
        voucher_sig: w.voucher(&src, &leaves_dust),
    };
    assert_eq!(fx(&w, &op).unwrap_err(), Reject::Dust);

    let mut later = claim_leaf(&mut w, &src, 50, 1_000);
    later.evict_after = src.evict_after + 1;
    let op = Op::Claim {
        source_pos: 3,
        claim_pos: 60,
        claim_leaf: later,
        voucher_sig: w.voucher(&src, &later),
    };
    assert_eq!(fx(&w, &op).unwrap_err(), Reject::EvictionTerms);

    let mut other_evictor = claim_leaf(&mut w, &src, 50, 1_000);
    other_evictor.evictor_key = other_evictor.owner_key;
    let op = Op::Claim {
        source_pos: 3,
        claim_pos: 60,
        claim_leaf: other_evictor,
        voucher_sig: w.voucher(&src, &other_evictor),
    };
    assert_eq!(fx(&w, &op).unwrap_err(), Reject::EvictionTerms);

    let ok = claim_leaf(&mut w, &src, 50, 1_000);
    let op = Op::Claim {
        source_pos: 3,
        claim_pos: 2,
        claim_leaf: ok,
        voucher_sig: w.voucher(&src, &ok),
    };
    assert_eq!(fx(&w, &op).unwrap_err(), Reject::SlotNotEmpty);

    // Signed by someone other than the source's owner.
    let msg = Message::from_digest(layertree_tee_lab::tree::voucher_msg(&p.tree_id, &src, &ok));
    let forged = w.sign_with(&w.operator.clone(), &msg);
    let op = Op::Claim {
        source_pos: 3,
        claim_pos: 60,
        claim_leaf: ok,
        voucher_sig: forged,
    };
    assert_eq!(fx(&w, &op).unwrap_err(), Reject::BadSignature("voucher"));

    // A voucher for another tree.
    let mut other_id = p.tree_id;
    other_id[0] ^= 1;
    let msg = Message::from_digest(layertree_tee_lab::tree::voucher_msg(&other_id, &src, &ok));
    let elsewhere = w.sign_with(&src.owner_key, &msg);
    let op = Op::Claim {
        source_pos: 3,
        claim_pos: 60,
        claim_leaf: ok,
        voucher_sig: elsewhere,
    };
    assert_eq!(fx(&w, &op).unwrap_err(), Reject::BadSignature("voucher"));
}

#[test]
fn a_seq_final_leaf_cannot_be_claimed() {
    let mut w = World::new(2, 128, None);
    let mut fin = w.leaf(0);
    fin.seq = SEQ_FINAL;
    w.step(Op::Update {
        pos: 0,
        new_leaf: fin,
    })
    .unwrap();
    let src = w.leaf(0);
    let c = claim_leaf(&mut w, &src, 50, 1_000);
    let v = w.voucher(&src, &c);
    let st = w.enclave.state().unwrap().clone();
    let op = Op::Claim {
        source_pos: 0,
        claim_pos: 60,
        claim_leaf: c,
        voucher_sig: v,
    };
    assert_eq!(
        layertree_tee_lab::rules::effects(&w.secp, &w.params(), &st, &op).unwrap_err(),
        Reject::SeqFinal
    );
}

#[test]
fn the_last_exit_is_terminal() {
    let mut w = World::new(1, 128, None);
    let s = w
        .step(Op::Exit {
            pos: 0,
            evict: false,
        })
        .unwrap();
    assert!(s.unroll.is_none());
    assert!(s.record.next_unroll_key.is_none());
    assert!(w.enclave.state().is_none());
}

#[test]
fn refresh_moves_the_tree_and_kills_the_old_unroll() {
    let mut w = World::new(3, 128, None);
    let old = w.enclave.state().unwrap().clone();
    let s = w.step(Op::Refresh).unwrap();
    let new = w.enclave.state().unwrap();
    assert_eq!(new.tree, old.tree);
    assert_ne!(new.output.unroll_key, old.output.unroll_key);
    // The old unroll spends the outpoint this refresh just spent.
    assert_eq!(
        w.genesis_unroll.txs[0].input[0].previous_output,
        s.tx.input[0].previous_output
    );
}

#[test]
fn migration_is_a_rotation_to_the_ccv_output() {
    let m = [9u8; 32];
    // Not allowed when no target was fixed at launch.
    let w = World::new(3, 128, None);
    let st = w.enclave.state().unwrap().clone();
    let op = Op::Migrate { m };
    assert_eq!(
        layertree_tee_lab::rules::effects(&w.secp, &w.params(), &st, &op).unwrap_err(),
        Reject::NotAllowed
    );

    let mut w = World::new(3, 128, Some(m));
    let st = w.enclave.state().unwrap().clone();
    let root = st.tree.root().unwrap();
    // The wrong m is refused.
    let wrong = Op::Migrate { m: [8u8; 32] };
    assert_eq!(
        layertree_tee_lab::rules::effects(&w.secp, &w.params(), &st, &wrong).unwrap_err(),
        Reject::NotAllowed
    );
    let s = w.step(Op::Migrate { m }).unwrap();
    assert_eq!(
        s.tx.output[0].script_pubkey,
        ccv_tree_spk(&w.secp, &root.hash, &m)
    );
    assert_eq!(s.tx.output[0].value.to_sat(), root.sum);
    assert_eq!(s.record.new_root, root);
    assert!(s.unroll.is_none());
    // Every member's proof is unchanged: same root.
    for pos in st.tree.leaves().keys() {
        assert!(
            st.tree
                .proof(*pos)
                .unwrap()
                .verify(&s.record.new_root, st.tree.depth())
        );
    }
}
