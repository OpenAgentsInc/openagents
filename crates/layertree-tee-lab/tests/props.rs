//! Property tests: random transition sequences through the enclave keep
//! conservation, every proof, and a valid unroll for every member.

mod common;

use common::World;
use layertree_tee_lab::rules::Op;
use layertree_tee_lab::tree::{Node, Proof, Tree};
use layertree_tee_lab::unroll::verify_package;
use proptest::prelude::*;

#[derive(Debug, Clone)]
enum Act {
    Exit(u8),
    Join(u8, u32),
    Claim(u8, u8, u32),
    Refresh,
}

fn act() -> impl Strategy<Value = Act> {
    prop_oneof![
        any::<u8>().prop_map(Act::Exit),
        (any::<u8>(), 330u32..50_000).prop_map(|(p, a)| Act::Join(p, a)),
        (any::<u8>(), any::<u8>(), 330u32..50_000).prop_map(|(s, c, a)| Act::Claim(s, c, a)),
        Just(Act::Refresh),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 24, .. ProptestConfig::default() })]

    #[test]
    fn random_histories_conserve_and_stay_exitable(acts in prop::collection::vec(act(), 1..10), rules_on in any::<bool>()) {
        let mut w = World::with_rules(6, 3, None, rules_on);
        let mut next_member = 100u32;
        for a in acts {
            let Some(st) = w.enclave.state().cloned() else { break };
            let occupied: Vec<u64> = st.tree.leaves().keys().copied().collect();
            let op = match a {
                Act::Exit(i) => Op::Exit { pos: occupied[i as usize % occupied.len()], evict: false },
                Act::Join(p, amt) => {
                    let pos = p as u64;
                    if st.tree.get(pos).is_some() { continue; }
                    next_member += 1;
                    let l = w.member_leaf(next_member, amt as u64);
                    Op::Join { height: 0, index: pos, leaves: vec![(0, l)] }
                }
                Act::Claim(s, c, amt) => {
                    let sp = occupied[s as usize % occupied.len()];
                    let src = w.leaf(sp);
                    let amt = amt as u64;
                    let resid = src.amount.saturating_sub(amt);
                    if amt > src.amount || (resid != 0 && resid < 330) || st.tree.get(c as u64).is_some() { continue; }
                    next_member += 1;
                    let mut cl = w.member_leaf(next_member, amt);
                    cl.evictor_key = src.evictor_key;
                    cl.evict_after = src.evict_after;
                    let v = w.voucher(&src, &cl);
                    Op::Claim { source_pos: sp, claim_pos: c as u64, claim_leaf: cl, voucher_sig: v }
                }
                Act::Refresh => Op::Refresh,
            };
            let before = st.amount();
            let s = w.step(op.clone()).expect("valid op accepted");
            // Conservation: output 0 + payouts == input (plus joined value).
            let joined: u64 = match &op { Op::Join { leaves, .. } => leaves.iter().map(|(_, l)| l.amount).sum(), _ => 0 };
            let paid: u64 = match &op { Op::Exit { .. } => s.tx.output[1].value.to_sat(), _ => 0 };
            if let Some(st2) = w.enclave.state() {
                prop_assert_eq!(st2.amount() + paid, before + joined);
                prop_assert_eq!(st2.amount(), st2.tree.total());
                let root = st2.tree.root().unwrap();
                for pos in st2.tree.leaves().keys() {
                    prop_assert!(st2.tree.proof(*pos).unwrap().verify(&root, st2.tree.depth()));
                }
                let p = w.params();
                let pkg = w.last_unroll.as_ref().unwrap();
                for leaf in st2.tree.leaves().values() {
                    prop_assert!(verify_package(&w.secp, &p, st2.output, st2.outpoint, st2.amount(), st2.tree.leaves(), pkg, leaf).is_ok());
                }
            }
            prop_assert_eq!(w.enclave.ephemeral_keys_held(), 0);
        }
    }

    #[test]
    fn any_tampered_sum_breaks_the_proof(amounts in prop::collection::vec(1u64..1_000_000, 2..40), pick in any::<usize>(), level in any::<usize>(), delta in 1u64..1_000) {
        let mut t = Tree::new(6).unwrap();
        let mut w = World::new(0, 128, None);
        for (i, a) in amounts.iter().enumerate() {
            let l = w.member_leaf(i as u32 + 10, *a);
            t.insert(i as u64, l).unwrap();
        }
        let root = t.root().unwrap();
        let pos = (pick % amounts.len()) as u64;
        let good = t.proof(pos).unwrap();
        prop_assert!(good.verify(&root, 6));
        let mut bad: Proof = good.clone();
        let k = level % bad.path.len();
        let s = bad.path[k].sibling.sum;
        bad.path[k].sibling.sum = s.checked_add(delta).unwrap_or_else(|| s - delta);
        prop_assert!(!bad.verify(&root, 6));
        // Claiming a larger total under the same hash fails too.
        let inflated = Node { hash: root.hash, sum: root.sum + delta };
        prop_assert!(!good.verify(&inflated, 6));
    }
}
