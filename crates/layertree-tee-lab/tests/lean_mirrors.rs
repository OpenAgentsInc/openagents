//! Property tests that mirror, one to one, the theorems upstream proves in
//! Lean 4 about the abstract rules (`formal/LayerTree/*.lean` at commit
//! f6c4d29). Here they run against our Rust predicate and enclave, which
//! replace the scripts as the trust anchor. Each test is named after the
//! theorem it mirrors; the statement is quoted in its doc comment.
//!
//! These are tests, not proofs: they sample. The plan to prove them about
//! the Rust code (Kani) is in the spec's Verification section.

#![allow(non_snake_case)]

mod common;

use bitcoin::Amount;
use common::World;
use layertree_tee_lab::light::Member;
use layertree_tee_lab::output::p2tr_raw;
use layertree_tee_lab::rules::{self, Op, Reject};
use layertree_tee_lab::tree::{Leaf, Node, Proof, Tree, enc64, fold, maintain_path};
use proptest::prelude::*;

const CASES: u32 = 32;

fn tree_of(w: &mut World, depth: u32, amounts: &[(u64, u64)]) -> Tree {
    let mut t = Tree::new(depth).unwrap();
    for (i, (pos, amt)) in amounts.iter().enumerate() {
        let pos = pos % t.capacity();
        if t.get(pos).is_none() {
            let l = w.member_leaf(500 + i as u32, *amt);
            t.insert(pos, l).unwrap();
        }
    }
    t
}

/// A random op on the current state that the rules should accept, or None.
#[derive(Debug, Clone)]
enum Act {
    Exit(u8, bool),
    Join(u8, u8, u32),
    Claim(u8, u8, u32),
    Update(u8, u32),
}

fn act() -> impl Strategy<Value = Act> {
    prop_oneof![
        (any::<u8>(), any::<bool>()).prop_map(|(p, e)| Act::Exit(p, e)),
        (0u8..3, any::<u8>(), 330u32..80_000).prop_map(|(h, i, a)| Act::Join(h, i, a)),
        (any::<u8>(), any::<u8>(), 330u32..80_000).prop_map(|(s, c, a)| Act::Claim(s, c, a)),
        (any::<u8>(), 330u32..80_000).prop_map(|(p, a)| Act::Update(p, a)),
    ]
}

fn realise(w: &mut World, a: &Act, fresh: &mut u32) -> Option<Op> {
    let st = w.enclave.state()?.clone();
    let occ: Vec<u64> = st.tree.leaves().keys().copied().collect();
    if occ.is_empty() {
        return None;
    }
    Some(match *a {
        Act::Exit(i, evict) => Op::Exit {
            pos: occ[i as usize % occ.len()],
            evict,
        },
        Act::Join(h, i, amt) => {
            let h = h as u32;
            let index = (i as u64) % (st.tree.capacity() >> h);
            if st.tree.subtree(h, index).ok()? != st.tree.empty_node(h) {
                return None;
            }
            *fresh += 1;
            let l = w.member_leaf(*fresh, amt as u64);
            Op::Join {
                height: h,
                index,
                leaves: vec![(0, l)],
            }
        }
        Act::Claim(s, c, amt) => {
            let sp = occ[s as usize % occ.len()];
            let src = w.leaf(sp);
            let amt = amt as u64;
            let resid = src.amount.checked_sub(amt)?;
            if (resid != 0 && resid < 330) || st.tree.get(c as u64).is_some() || c as u64 == sp {
                return None;
            }
            *fresh += 1;
            let mut cl = w.member_leaf(*fresh, amt);
            cl.evictor_key = src.evictor_key;
            cl.evict_after = src.evict_after;
            let v = w.voucher(&src, &cl);
            Op::Claim {
                source_pos: sp,
                claim_pos: c as u64,
                claim_leaf: cl,
                voucher_sig: v,
            }
        }
        Act::Update(p, amt) => {
            let pos = occ[p as usize % occ.len()];
            let mut nl = w.leaf(pos);
            nl.amount = amt as u64;
            nl.evict_after += 1;
            Op::Update { pos, new_leaf: nl }
        }
    })
}

proptest! {
    #![proptest_config(ProptestConfig { cases: CASES, .. ProptestConfig::default() })]

    /// `ExitK.no_inflation`: `out0 + payoutSum blocks ≤ r.sum`. A sequence
    /// of K exits never pays out, together with what stays in the tree,
    /// more than the tree held; and the predicate refuses any exit whose
    /// payout or output 0 is raised by even one sat. (When the exit empties
    /// the tree, output 0 carries no tree value and is free, spec 7.8; the
    /// tree's part of it is then 0.)
    #[test]
    fn ExitK_no_inflation(n in 2u32..12, k in 1usize..6, bump in 1u64..10_000) {
        let mut w = World::new(n, 128, None);
        let r_sum = w.enclave.state().unwrap().amount();
        let mut paid = 0u64;
        let mut out0 = r_sum;
        for _ in 0..k.min(n as usize) {
            let pos = *w.enclave.state().unwrap().tree.leaves().keys().next().unwrap();
            let op = Op::Exit { pos, evict: false };
            let terminal = w.enclave.state().unwrap().tree.leaves().len() == 1;
            for raise_out0 in [false, true] {
                if raise_out0 && terminal { continue; }
                let (mut tx, prevouts, _) = w.build(&op);
                let o = if raise_out0 { 0 } else { 1 };
                tx.output[o].value = Amount::from_sat(tx.output[o].value.to_sat() + bump);
                let sigs = w.sign(&tx, &prevouts, &op);
                prop_assert!(w.enclave.authorize(tx, &prevouts, &op, &sigs).is_err());
            }
            let s = w.step(op).unwrap();
            paid += s.tx.output[1].value.to_sat();
            out0 = w.enclave.state().map(|s| s.amount()).unwrap_or(0);
        }
        prop_assert!(out0 + paid <= r_sum);
    }

    /// `Steps.sum`: `r'.sum + oldSum ss = r.sum + newSum ss`. Over any
    /// accepted sequence of steps, the published records' old and new
    /// nodes account for every change of the root's sum.
    #[test]
    fn Steps_sum(acts in prop::collection::vec(act(), 1..12)) {
        let mut w = World::new(6, 128, None);
        let r0 = w.enclave.state().unwrap().tree.root().unwrap().sum;
        let (mut old_sum, mut new_sum, mut fresh) = (0u64, 0u64, 1_000u32);
        let mut r_last = r0;
        for a in &acts {
            let Some(op) = realise(&mut w, a, &mut fresh) else { continue };
            let Ok(s) = w.step(op) else { continue };
            for c in &s.record.changes {
                old_sum += c.old_node().sum;
                new_sum += c.new_node().sum;
            }
            r_last = s.record.new_root.sum;
        }
        prop_assert_eq!(r_last + old_sum, r0 + new_sum);
    }

    /// `Reachable.ledger`: `t'.total + removedSum xs t = t.total +
    /// addedSum xs t`. Leaf totals (not node sums) across a random history:
    /// what left by exit, carve residue or update decrease, plus what stays,
    /// equals what was there plus what joined or was added.
    #[test]
    fn Reachable_ledger(acts in prop::collection::vec(act(), 1..12)) {
        let mut w = World::new(6, 128, None);
        let t0 = w.enclave.state().unwrap().tree.total();
        let (mut removed, mut added, mut fresh) = (0u64, 0u64, 2_000u32);
        for a in &acts {
            let Some(op) = realise(&mut w, a, &mut fresh) else { continue };
            let before = w.enclave.state().unwrap().tree.clone();
            if w.step(op.clone()).is_err() { continue; }
            match &op {
                Op::Exit { pos, .. } => removed += before.get(*pos).unwrap().amount,
                Op::Join { leaves, .. } => added += leaves.iter().map(|(_, l)| l.amount).sum::<u64>(),
                Op::Update { pos, new_leaf } => {
                    let old = before.get(*pos).unwrap().amount;
                    if new_leaf.amount >= old { added += new_leaf.amount - old } else { removed += old - new_leaf.amount }
                }
                _ => {}
            }
            if w.enclave.state().is_none() { break; }
        }
        let t1 = w.enclave.state().map(|s| s.tree.total()).unwrap_or(0);
        prop_assert_eq!(t1 + removed, t0 + added);
    }

    /// `payable`: for every leaf `l` at `p`, the step that empties `p`
    /// is valid against the root, `(t.set p none).root.sum + l.amount =
    /// t.root.sum`, and every other position is unchanged.
    #[test]
    fn payable(amounts in prop::collection::vec((any::<u64>(), 1u64..1_000_000), 1..30), pick in any::<usize>()) {
        let mut w = World::new(0, 128, None);
        let t = tree_of(&mut w, 6, &amounts);
        let occ: Vec<u64> = t.leaves().keys().copied().collect();
        let p = occ[pick % occ.len()];
        let l = *t.get(p).unwrap();
        let path = t.path(p, 0).unwrap();
        let mut t2 = t.clone();
        t2.set(p, None).unwrap();
        prop_assert_eq!(fold(l.node(), &path).unwrap(), t.root().unwrap());
        prop_assert_eq!(fold(Node::EMPTY, &path).unwrap(), t2.root().unwrap());
        prop_assert_eq!(t2.root().unwrap().sum + l.amount, t.root().unwrap().sum);
        for q in occ.iter().filter(|q| **q != p) {
            prop_assert_eq!(t2.get(*q), t.get(*q));
        }
    }

    /// `exit_available` (with the block's `payoutOk`): every leaf has an
    /// exit the rules accept, by its owner, or by its evictor at the lease,
    /// paying exactly `l.amount` to `P2TR(l.exit_key)` and leaving output 0
    /// at `root.sum − l.amount`.
    #[test]
    fn exit_available(n in 1u32..16, pick in any::<usize>(), evict in any::<bool>()) {
        let mut w = World::new(n, 128, None);
        let st = w.enclave.state().unwrap().clone();
        let occ: Vec<u64> = st.tree.leaves().keys().copied().collect();
        let p = occ[pick % occ.len()];
        let l = *st.tree.get(p).unwrap();
        let s = w.step(Op::Exit { pos: p, evict }).unwrap();
        prop_assert_eq!(&s.tx.output[1].script_pubkey, &p2tr_raw(&l.exit_key));
        prop_assert_eq!(s.tx.output[1].value.to_sat(), l.amount);
        if n > 1 {
            prop_assert_eq!(s.tx.output[0].value.to_sat(), st.amount() - l.amount);
        }
    }

    /// `voucher_no_replay`: after a claim against `src`, no leaf reachable
    /// by any later sequence of steps has `src`'s body, so the voucher's
    /// message never verifies again, whatever amount the leaf carries; the
    /// predicate refuses the replay.
    #[test]
    fn voucher_no_replay(amt in 330u64..9_000, acts in prop::collection::vec(act(), 0..8)) {
        let mut w = World::new(5, 128, None);
        let src = w.leaf(4); // 50,000 sat
        let mut c = w.member_leaf(900, amt);
        c.evictor_key = src.evictor_key;
        c.evict_after = src.evict_after;
        let v = w.voucher(&src, &c);
        w.step(Op::Claim { source_pos: 4, claim_pos: 100, claim_leaf: c, voucher_sig: v }).unwrap();
        let mut fresh = 3_000u32;
        for a in &acts {
            if let Some(op) = realise(&mut w, a, &mut fresh) { let _ = w.step(op); }
        }
        let Some(st) = w.enclave.state().cloned() else { return Ok(()) };
        for l in st.tree.leaves().values() {
            prop_assert_ne!(l.body(), src.body());
        }
        if let (Some(_), true) = (st.tree.get(4), st.tree.get(101).is_none()) {
            let op = Op::Claim { source_pos: 4, claim_pos: 101, claim_leaf: c, voucher_sig: v };
            prop_assert_eq!(rules::effects(&w.secp, &w.params(), &st, &op).unwrap_err(), Reject::BadSignature("voucher"));
        }
    }

    /// `path_maintenance`: after any position changes, a member's new path
    /// is its old path with at most one sibling replaced, the one where the
    /// two paths meet, and that sibling is derivable from the changed
    /// position's own path. Checked directly and through a light client
    /// following the enclave's records.
    #[test]
    fn path_maintenance(amounts in prop::collection::vec((any::<u64>(), 1u64..1_000_000), 2..30),
                        edits in prop::collection::vec((any::<u64>(), proptest::option::of(1u64..1_000_000)), 1..10),
                        pick in any::<usize>()) {
        let mut w = World::new(0, 128, None);
        let mut t = tree_of(&mut w, 6, &amounts);
        let occ: Vec<u64> = t.leaves().keys().copied().collect();
        let me = occ[pick % occ.len()];
        let mut path = t.path(me, 0).unwrap();
        for (i, (q, amt)) in edits.iter().enumerate() {
            let q = q % t.capacity();
            if q == me { continue; }
            let new: Option<Leaf> = amt.map(|a| w.member_leaf(700 + i as u32, a));
            if new.is_none() && t.get(q).is_none() { continue; }
            let qpath = t.path(q, 0).unwrap();
            let new_node = new.map(|l| l.node()).unwrap_or(Node::EMPTY);
            let updated = maintain_path(me, &path, q, new_node, &qpath).unwrap();
            let changed: usize = updated.iter().zip(&path).filter(|(a, b)| a != b).count();
            prop_assert!(changed <= 1);
            match new { Some(l) if t.get(q).is_none() => t.insert(q, l).unwrap(), l => t.set(q, l).unwrap() }
            path = updated;
            prop_assert_eq!(&path, &t.path(me, 0).unwrap());
            let proof = Proof { position: me, leaf: *t.get(me).unwrap(), path: path.clone() };
            prop_assert!(proof.verify(&t.root().unwrap(), 6));
        }
    }

    /// `path_maintenance`, through the light client: a member that sees
    /// only the chain and the records keeps the same path as the full tree.
    #[test]
    fn path_maintenance_light_client(acts in prop::collection::vec(act(), 1..10)) {
        let mut w = World::new(6, 128, None);
        let p = w.params();
        let st = w.enclave.state().unwrap().clone();
        let mut me = Member { pos: 0, leaf: st.tree.get(0).copied(), path: st.tree.path(0, 0).unwrap(),
                              root: st.tree.root().unwrap(), outpoint: st.outpoint, unroll_key: st.output.unroll_key };
        let mut fresh = 4_000u32;
        for a in &acts {
            let Some(op) = realise(&mut w, a, &mut fresh) else { continue };
            let Ok(s) = w.step(op) else { continue };
            prop_assert!(me.follow(&w.secp, &p, &s.tx, &s.record).is_ok());
            let Some(st) = w.enclave.state() else { break };
            prop_assert_eq!(&me.path, &st.tree.path(0, 0).unwrap());
            prop_assert_eq!(me.root, st.tree.root().unwrap());
        }
    }

    /// `join_sound`: a graft lands only in an empty aligned slot, and
    /// `out0 = t.root.sum + new.sum`.
    #[test]
    fn join_sound(h in 0u32..4, i in any::<u64>(), amt in 330u64..100_000) {
        let mut w = World::new(6, 128, None); // positions 0..6 occupied
        let st = w.enclave.state().unwrap().clone();
        let index = i % (st.tree.capacity() >> h);
        let l = w.member_leaf(800, amt);
        let op = Op::Join { height: h, index, leaves: vec![(0, l)] };
        let empty = st.tree.subtree(h, index).unwrap() == st.tree.empty_node(h);
        match rules::effects(&w.secp, &w.params(), &st, &op) {
            Ok(_) => {
                prop_assert!(empty);
                let s = w.step(op).unwrap();
                prop_assert_eq!(s.tx.output[0].value.to_sat(), st.amount() + amt);
            }
            Err(e) => { prop_assert!(!empty); prop_assert_eq!(e, Reject::SlotNotEmpty); }
        }
    }

    /// `root_inj`: equal roots mean equal trees. Sampled as its
    /// contrapositive: changing any one leaf field, or moving a leaf,
    /// changes the root.
    #[test]
    fn root_inj(amounts in prop::collection::vec((any::<u64>(), 1u64..1_000_000), 1..20), pick in any::<usize>(), field in 0u8..5, to in any::<u64>()) {
        let mut w = World::new(0, 128, None);
        let t = tree_of(&mut w, 6, &amounts);
        let occ: Vec<u64> = t.leaves().keys().copied().collect();
        let p = occ[pick % occ.len()];
        let mut l = *t.get(p).unwrap();
        let mut t2 = t.clone();
        match field {
            0 => { l.amount += 1; t2.set(p, Some(l)).unwrap(); }
            1 => { l.seq += 1; t2.set(p, Some(l)).unwrap(); }
            2 => { l.nonce ^= 1; t2.set(p, Some(l)).unwrap(); }
            3 => { l.evict_after += 1; t2.set(p, Some(l)).unwrap(); }
            _ => {
                let q = to % t.capacity();
                if t.get(q).is_some() { return Ok(()); }
                t2.set(p, None).unwrap();
                t2.insert(q, l).unwrap();
            }
        }
        prop_assert_ne!(t.root().unwrap(), t2.root().unwrap());
    }

    /// `enc64_inj` and `le64_inj`: the sum and amount encodings are
    /// injective, so a sum cannot be re-read as another.
    #[test]
    fn enc64_inj(n in any::<u64>(), m in any::<u64>()) {
        prop_assume!(n != m);
        prop_assert_ne!(enc64(n), enc64(m));
        prop_assert_ne!(n.to_le_bytes(), m.to_le_bytes());
    }
}

/// `Steps.sound` (the half the enclave strengthens): every accepted state
/// is a real tree whose leaves all prove against its root; there is no
/// opaque node behind which value could hide.
#[test]
fn Steps_sound_no_opaque_nodes() {
    let mut w = World::new(4, 128, None);
    let a = w.member_leaf(60, 5_000);
    w.step(Op::Join {
        height: 3,
        index: 3,
        leaves: vec![(2, a)],
    })
    .unwrap();
    let st = w.enclave.state().unwrap();
    let root = st.tree.root().unwrap();
    for pos in st.tree.leaves().keys() {
        assert!(st.tree.proof(*pos).unwrap().verify(&root, st.tree.depth()));
    }
    assert_eq!(
        st.tree.leaves().values().map(|l| l.amount).sum::<u64>(),
        root.sum
    );
}
