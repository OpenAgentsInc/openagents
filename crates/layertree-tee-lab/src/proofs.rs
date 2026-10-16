//! Proofs of the monetary core called by the production predicate.
//! Tree lookup, Merkle witnesses, and Bitcoin script construction stay
//! outside this boundary. Hash bytes and crypto validity are abstract.

#![allow(non_snake_case, dead_code)]

use crate::predicate::{self, Oracle, Output, OutputRules, Params};
use crate::tree::{Leaf, MAX_AMOUNT, Node, TreeError};

struct CryptoFacts {
    key: bool,
    signature: bool,
}
impl Oracle for CryptoFacts {
    fn valid_key(&self, _: &[u8; 32]) -> bool {
        self.key
    }
    fn voucher(&self, _: &[u8; 32], _: [u8; 32], _: &[u8; 64]) -> bool {
        self.signature
    }
}

fn hash(_: &str, _: &[&[u8]]) -> [u8; 32] {
    kani::any()
}

fn leaf() -> Leaf {
    let amount: u64 = kani::any();
    kani::assume(amount > 0 && amount < MAX_AMOUNT);
    Leaf {
        amount,
        exit_key: kani::any(),
        owner_key: [3; 32],
        evictor_key: [4; 32],
        evict_after: 100,
        nonce: kani::any(),
        seq: kani::any(),
    }
}

/// `ExitK.no_inflation`: arbitrary proposed amounts and payout scripts can
/// pass the real output checker only if the tree and payout conserve value.
#[kani::proof]
#[kani::unwind(5)]
fn ExitK_no_inflation() {
    let member = leaf();
    let before: u64 = kani::any();
    kani::assume(before >= member.amount);
    let terms = predicate::exit_terms(before, &member).unwrap();
    let payouts = [terms.payout];
    let rules = OutputRules {
        sum: terms.sum,
        payouts: &payouts,
        min_locktime: None,
        terminal: terms.sum == 0,
    };
    let proposed: [u8; 34] = kani::any();
    let out0: u64 = kani::any();
    let paid: u64 = kani::any();
    let outputs = [
        Output {
            amount: out0,
            script: &[1],
        },
        Output {
            amount: paid,
            script: &proposed,
        },
        Output {
            amount: 0,
            script: &[0x6a],
        },
    ];
    let expected = (!rules.terminal).then_some(Output {
        amount: terms.sum,
        script: &[1],
    });
    if predicate::check_outputs(&rules, &outputs, expected, &[0x6a], Some(100), true).is_ok() {
        let tree_value = if rules.terminal { 0 } else { out0 };
        assert!(
            tree_value as u128 + paid as u128 == before as u128,
            "ExitK.no_inflation"
        );
    }
    let script = terms.payout.script();
    let honest = [
        Output {
            amount: terms.sum,
            script: &[1],
        },
        Output {
            amount: member.amount,
            script: &script,
        },
        outputs[2],
    ];
    assert!(predicate::check_outputs(&rules, &honest, expected, &[0x6a], Some(100), true).is_ok());
}

/// `exit_available`: the exact exit terms pay this member's own key and
/// amount, including the terminal case.
#[kani::proof]
#[kani::unwind(5)]
fn exit_available() {
    let member = leaf();
    let before: u64 = kani::any();
    kani::assume(before >= member.amount);
    let terms = predicate::exit_terms(before, &member).unwrap();
    assert_eq!(terms.payout.amount, member.amount);
    assert_eq!(
        terms.sum as u128 + terms.payout.amount as u128,
        before as u128
    );
    for i in 0..4 {
        let range = i * 8..i * 8 + 8;
        let got = u64::from_le_bytes(terms.payout.exit_key[range.clone()].try_into().unwrap());
        let want = u64::from_le_bytes(member.exit_key[range].try_into().unwrap());
        assert_eq!(got, want);
    }
}

/// `Steps.sum` and `Reachable.ledger`: up to three accepted applications of
/// the actual edit arithmetic, for all u64 balances and old/new amounts.
#[kani::proof]
#[kani::unwind(5)]
fn Steps_sum_Reachable_ledger() {
    let original: u64 = kani::any();
    let mut sum = original;
    let (mut old_sum, mut new_sum, mut removed, mut added) = (0u128, 0u128, 0u128, 0u128);
    for _ in 0..3 {
        let old: u64 = kani::any();
        let new: u64 = kani::any();
        if let Ok(next) = predicate::change_sum(sum, old, new) {
            old_sum += old as u128;
            new_sum += new as u128;
            if old > new {
                removed += (old - new) as u128;
            } else {
                added += (new - old) as u128;
            }
            sum = next;
            assert_eq!(sum as u128 + old_sum, original as u128 + new_sum);
            assert_eq!(sum as u128 + removed, original as u128 + added);
        }
    }
}

/// `voucher_no_replay`: an accepted partial claim strictly advances the
/// sequence; a complete claim removes the source; the final sequence refuses.
#[kani::proof]
#[kani::unwind(5)]
#[kani::stub(crate::tree::tagged_hash, hash)]
fn voucher_no_replay() {
    let src = leaf();
    let claim = leaf();
    let p = Params {
        tree_id: [0; 32],
        depth: 4,
        dust_min: 330,
        migration_m: None,
    };
    let facts = CryptoFacts {
        key: kani::any(),
        signature: kani::any(),
    };
    if let Ok(replacement) = predicate::claim_terms(&facts, &p, &src, &claim, &[0; 64]) {
        assert!(facts.key && facts.signature);
        assert!(src.seq < u32::MAX);
        let residual = replacement.map(|l| l.amount).unwrap_or(0);
        assert_eq!(residual as u128 + claim.amount as u128, src.amount as u128);
        if let Some(after) = replacement {
            assert!(after.seq > src.seq);
            assert_eq!(after.seq, src.seq + 1);
        }
    }
}

/// Checked Merkle-node addition never wraps, for every pair of u64 sums.
#[kani::proof]
#[kani::unwind(5)]
#[kani::stub(crate::tree::tagged_hash, hash)]
fn checked_addition_never_wraps() {
    let a: u64 = kani::any();
    let b: u64 = kani::any();
    match Node::parent(
        &Node {
            hash: [0; 32],
            sum: a,
        },
        &Node {
            hash: [0; 32],
            sum: b,
        },
    ) {
        Ok(n) => assert_eq!(n.sum as u128, a as u128 + b as u128),
        Err(e) => {
            assert_eq!(e, TreeError::SumOverflow);
            assert!(a as u128 + b as u128 > u64::MAX as u128);
        }
    }
}

/// A successful script comparison implies equality at every byte position.
#[kani::proof]
#[kani::unwind(5)]
fn script_equality_checks_every_byte() {
    let a: [u8; 34] = kani::any();
    let b: [u8; 34] = kani::any();
    let position: u8 = kani::any();
    kani::assume(position < 34);
    if predicate::same_bytes(&a, &b) {
        assert_eq!(a[position as usize], b[position as usize]);
    }
    assert!(predicate::same_bytes(&a, &a));
}
