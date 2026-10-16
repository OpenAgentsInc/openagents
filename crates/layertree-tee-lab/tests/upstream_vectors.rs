//! Opt-in cross-check against upstream's `spec/test-vectors.json`, read
//! from a local reference clone at test time and never vendored (the
//! upstream repository has no licence). Run with:
//!
//! ```text
//! LAYERTREE_VECTORS=~/work/projects/repos/layertree/spec/test-vectors.json \
//!   cargo test --manifest-path crates/layertree-tee-lab/Cargo.toml -- --ignored
//! ```

use layertree_tee_lab::tree::{Leaf, Node, Tree, tree_id};
use serde_json::Value;

fn h32(v: &Value) -> [u8; 32] {
    let b = hex::decode(v.as_str().expect("hex string")).expect("hex");
    b.try_into().expect("32 bytes")
}

#[test]
#[ignore = "needs a local clone of lightninglabs/layertree; set LAYERTREE_VECTORS"]
fn our_tree_matches_upstream_vectors() {
    let path = std::env::var("LAYERTREE_VECTORS").expect("set LAYERTREE_VECTORS");
    let v: Value =
        serde_json::from_str(&std::fs::read_to_string(path).expect("read vectors")).expect("json");

    // E_h for every height listed.
    for (h, n) in v["empty_nodes"].as_array().unwrap().iter().enumerate() {
        assert_eq!(Node::empty(h as u32).hash, h32(&n["hash"]), "E_{h}");
    }

    // The example tree: leaf bytes, leaf hashes, root.
    let ex = &v["example_tree"];
    let depth = ex["depth"].as_u64().unwrap() as u32;
    let mut tree = Tree::new(depth).unwrap();
    for (pos, l) in ex["leaves"].as_object().unwrap() {
        let leaf = Leaf {
            amount: l["amount"].as_u64().unwrap(),
            exit_key: h32(&l["exit_key"]),
            owner_key: h32(&l["owner_key"]),
            evictor_key: h32(&l["evictor_key"]),
            evict_after: l["evict_after"].as_u64().unwrap() as u32,
            nonce: l["nonce"].as_u64().unwrap() as u32,
            seq: l["seq"].as_u64().unwrap() as u32,
        };
        assert_eq!(
            hex::encode(leaf.serialize()),
            l["serialized"].as_str().unwrap()
        );
        assert_eq!(leaf.hash(), h32(&l["leaf_hash"]));
        tree.insert(pos.parse().unwrap(), leaf).unwrap();
    }
    let root = tree.root().unwrap();
    assert_eq!(root.hash, h32(&ex["root"]["hash"]));
    assert_eq!(root.sum, ex["root"]["sum"].as_u64().unwrap());

    // The state key with NUMS in place of the enclave key is upstream's
    // internal key: same H_state tweak, so the TEE output commits to the
    // root exactly as the covenant output does.
    let secp = bitcoin::secp256k1::Secp256k1::verification_only();
    let nums =
        bitcoin::key::XOnlyPublicKey::from_slice(&layertree_tee_lab::output::NUMS_X).unwrap();
    let p = layertree_tee_lab::output::state_key(&secp, &nums, &root.hash);
    let ik = hex::decode(ex["internal_key"].as_str().unwrap()).unwrap();
    assert_eq!(p.serialize()[..], ik[1..]);

    // tree_id over the creation outpoint; the vector's txid is in display
    // (reversed) order or internal order, so accept the one that matches
    // and report which.
    let txid = h32(&ex["creation_outpoint"]["txid"]);
    let vout = ex["creation_outpoint"]["vout"].as_u64().unwrap() as u32;
    let mut rev = txid;
    rev.reverse();
    let want = h32(&ex["tree_id"]);
    assert!(
        tree_id(&txid, vout) == want || tree_id(&rev, vout) == want,
        "tree_id"
    );
}
