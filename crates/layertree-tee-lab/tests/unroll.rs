//! The timelocked unilateral exit: built and checked with the enclave gone.

mod common;

use bitcoin::{Amount, Sequence};
use common::{DELAY, World};
use layertree_tee_lab::output::unroll_leaf_script;
use layertree_tee_lab::rules::Op;
use layertree_tee_lab::unroll::{UnrollError, verify_package};

fn check_all(w: &World) -> usize {
    let p = w.params();
    let st = w.enclave.state().unwrap().clone();
    let pkg = w.last_unroll.as_ref().unwrap();
    let mut longest = 0;
    for leaf in st.tree.leaves().values() {
        let branch = verify_package(
            &w.secp,
            &p,
            st.output,
            st.outpoint,
            st.amount(),
            st.tree.leaves(),
            pkg,
            leaf,
        )
        .expect("member verifies its unroll");
        longest = longest.max(branch.len());
    }
    longest
}

#[test]
fn every_member_can_unroll_after_genesis_and_after_transitions() {
    // radix 4 with 20 members: a three-level unroll (4 groups of 16, 4 of 4).
    let mut w = World::new(20, 4, None);
    assert_eq!(check_all(&w), 3);
    w.step(Op::Exit {
        pos: 7,
        evict: false,
    })
    .unwrap();
    let a = w.member_leaf(40, 3_000);
    w.step(Op::Join {
        height: 0,
        index: 90,
        leaves: vec![(0, a)],
    })
    .unwrap();
    assert!(check_all(&w) >= 2);
    // Flat when the tree fits one transaction.
    let w = World::new(20, 128, None);
    assert_eq!(check_all(&w), 1);
}

#[test]
fn the_unroll_needs_nothing_from_the_enclave() {
    let w = World::new(12, 4, None);
    let p = w.params();
    let st = w.enclave.state().unwrap().clone();
    let pkg = w.last_unroll.clone().unwrap();
    let secp = w.secp.clone();
    let member = *st.tree.get(5).unwrap();
    // The enclave dies: drop it, and everything with it.
    drop(w);
    let branch = verify_package(
        &secp,
        &p,
        st.output,
        st.outpoint,
        st.amount(),
        st.tree.leaves(),
        &pkg,
        &member,
    )
    .unwrap();
    // The first transaction waits Δ blocks: in nSequence and in the script.
    let root = &branch[0];
    assert_eq!(root.input[0].sequence, Sequence::from_height(DELAY));
    assert!(root.version.0 >= 2);
    assert_eq!(
        root.input[0].witness.nth(1).unwrap(),
        unroll_leaf_script(&st.output.unroll_key, DELAY).as_bytes()
    );
    // The last pays the member exactly, and every transaction has a
    // zero-value anchor for fees.
    let last = branch.last().unwrap();
    assert!(last.output.iter().any(|o| o.value.to_sat() == member.amount
        && o.script_pubkey == layertree_tee_lab::output::p2tr_raw(&member.exit_key)));
    for tx in &branch {
        assert_eq!(tx.version.0, 3);
        assert!(tx.output.iter().any(|o| o.value == Amount::ZERO
            && o.script_pubkey == layertree_tee_lab::output::p2a_script()));
    }
}

#[test]
fn a_tampered_unroll_fails() {
    let w = World::new(10, 4, None);
    let p = w.params();
    let st = w.enclave.state().unwrap().clone();
    let me = *st.tree.get(0).unwrap();
    let good = w.last_unroll.clone().unwrap();
    let check = |pkg| {
        verify_package(
            &w.secp,
            &p,
            st.output,
            st.outpoint,
            st.amount(),
            st.tree.leaves(),
            pkg,
            &me,
        )
    };

    // Move value between two leaf outputs of a final transaction.
    let mut bad = good.clone();
    let last = bad.txs.last_mut().unwrap();
    last.output[0].value = Amount::from_sat(last.output[0].value.to_sat() - 1);
    last.output[1].value = Amount::from_sat(last.output[1].value.to_sat() + 1);
    assert!(matches!(check(&bad), Err(UnrollError::Signature(_))));

    // A shorter delay.
    let mut bad = good.clone();
    bad.txs[0].input[0].sequence = Sequence::from_height(1);
    assert_eq!(check(&bad), Err(UnrollError::RootDelay));

    // Drop a transaction: some leaves are no longer paid.
    let mut bad = good.clone();
    bad.txs.pop();
    assert_eq!(check(&bad), Err(UnrollError::Payouts));

    // The unroll of another state.
    let mut other = World::new(10, 4, None);
    other.step(Op::Refresh).unwrap();
    assert_eq!(
        check(other.last_unroll.as_ref().unwrap()),
        Err(UnrollError::RootInput)
    );
}

#[test]
fn every_state_gets_its_own_unroll() {
    let mut w = World::new(6, 128, None);
    let g = w.genesis_unroll.clone();
    let s = w
        .step(Op::Exit {
            pos: 1,
            evict: false,
        })
        .unwrap();
    let u = s.unroll.unwrap();
    // The new unroll spends the new tree output; the old one the old
    // output, which the transition spent.
    assert_eq!(u.txs[0].input[0].previous_output.txid, s.tx.compute_txid());
    assert_eq!(
        g.txs[0].input[0].previous_output,
        s.tx.input[0].previous_output
    );
}
