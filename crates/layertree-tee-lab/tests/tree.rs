//! The Merkle-sum tree: build, prove, verify, and a tampered sum fails.

mod common;

use common::World;
use layertree_tee_lab::tree::{Node, PathStep, Proof, Tree, TreeError, fold, maintain_path};

#[test]
fn every_leaf_proves_against_the_root() {
    let w = World::new(13, 128, None);
    let tree = &w.enclave.state().unwrap().tree;
    let root = tree.root().unwrap();
    assert_eq!(root.sum, tree.total());
    for pos in tree.leaves().keys() {
        let proof = tree.proof(*pos).unwrap();
        assert!(proof.verify(&root, tree.depth()), "position {pos}");
    }
}

#[test]
fn an_empty_tree_has_the_empty_root() {
    let t = Tree::new(8).unwrap();
    assert_eq!(t.root().unwrap(), Node::empty(8));
    assert_eq!(t.root().unwrap().sum, 0);
    assert_eq!(Node::empty(0), Node::EMPTY);
}

#[test]
fn a_tampered_sum_fails() {
    let w = World::new(9, 128, None);
    let tree = &w.enclave.state().unwrap().tree;
    let root = tree.root().unwrap();
    let good = tree.proof(3).unwrap();

    // Lower a sibling's sum: the hash no longer matches.
    let mut bad = good.clone();
    bad.path[2].sibling.sum -= 1;
    assert!(!bad.verify(&root, tree.depth()));

    // Claim a bigger leaf: the leaf hash changes.
    let mut bad = good.clone();
    bad.leaf.amount += 1;
    assert!(!bad.verify(&root, tree.depth()));

    // A third party that hands out a root with an inflated total is caught
    // because the leaf's path folds to the real sum.
    let inflated = Node {
        hash: root.hash,
        sum: root.sum + 1,
    };
    assert!(!good.verify(&inflated, tree.depth()));

    // The same proof for another slot fails.
    let mut moved = good.clone();
    moved.position = 4;
    assert!(!moved.verify(&root, tree.depth()));

    // A short path fails.
    let mut short = good;
    short.path.pop();
    assert!(!short.verify(&root, tree.depth()));
}

#[test]
fn a_wrapping_sum_is_an_error_not_a_small_root() {
    let big = Node {
        hash: [1u8; 32],
        sum: u64::MAX,
    };
    let path = [PathStep {
        right: false,
        sibling: Node {
            hash: [2u8; 32],
            sum: 1,
        },
    }];
    assert_eq!(fold(big, &path), Err(TreeError::SumOverflow));
}

#[test]
fn path_maintenance_matches_a_recomputed_path() {
    let mut w = World::new(20, 128, None);
    let mut tree = w.enclave.state().unwrap().tree.clone();
    let me = 5u64;
    let mut my_path = tree.path(me, 0).unwrap();
    let new_leaf = w.member_leaf(77, 4_242);
    for (pos, leaf) in [
        (17u64, None),
        (100, Some(new_leaf)),
        (4, None),
        (6, Some(new_leaf)),
    ] {
        let changed_path = tree.path(pos, 0).unwrap();
        let new_node = leaf
            .map(|l: layertree_tee_lab::tree::Leaf| l.node())
            .unwrap_or(Node::EMPTY);
        my_path = maintain_path(me, &my_path, pos, new_node, &changed_path).unwrap();
        match leaf {
            Some(l) if tree.get(pos).is_none() => tree.insert(pos, l).unwrap(),
            l => tree.set(pos, l).unwrap(),
        }
        assert_eq!(my_path, tree.path(me, 0).unwrap());
        let proof = Proof {
            position: me,
            leaf: *tree.get(me).unwrap(),
            path: my_path.clone(),
        };
        assert!(proof.verify(&tree.root().unwrap(), tree.depth()));
    }
}

#[test]
fn invalid_leaves_are_refused() {
    let mut w = World::new(1, 128, None);
    let mut l = w.member_leaf(5, 1_000);
    l.amount = 0;
    assert!(l.validate().is_err());
    l.amount = 1 << 51;
    assert!(l.validate().is_err());
    l.amount = 1_000;
    l.evict_after = 500_000_000;
    assert!(l.validate().is_err());
    l.evict_after = 0;
    l.exit_key = [0xffu8; 32];
    assert!(l.validate().is_err());
}
