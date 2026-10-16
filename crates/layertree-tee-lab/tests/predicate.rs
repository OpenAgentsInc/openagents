//! The pure boundary preserves byte equality and refuses invalid positions.

mod common;

use common::World;
use layertree_tee_lab::predicate::same_bytes;
use layertree_tee_lab::rules::{self, Op};
use proptest::prelude::*;

proptest! {
    #[test]
    fn word_comparison_matches_byte_equality(a in prop::collection::vec(any::<u8>(), 0..140),
        b in prop::collection::vec(any::<u8>(), 0..140)) {
        prop_assert_eq!(same_bytes(&a, &b), a == b);
        prop_assert!(same_bytes(&a, &a));
        if !a.is_empty() {
            let mut changed = a.clone();
            let last = changed.len() - 1;
            changed[last] ^= 1;
            prop_assert!(!same_bytes(&a, &changed));
        }
    }
}

#[test]
fn an_out_of_range_join_is_refused_before_shifting_the_index() {
    let mut world = World::new(2, 128, None);
    let leaf = world.member_leaf(10, 1_000);
    let state = world.enclave.state().unwrap();
    for (height, index) in [(0, 256), (8, 1), (8, u64::MAX), (33, 0)] {
        let op = Op::Join {
            height,
            index,
            leaves: vec![(0, leaf)],
        };
        assert!(rules::effects(&world.secp, &world.params(), state, &op).is_err());
    }
}
