//! The tree output on today's Bitcoin, with a TEE in place of covenants.
//!
//! Spec section 6 commits the root by tweaking a NUMS internal key, so the
//! key path is unspendable and every spend runs a covenant script. Without
//! covenant opcodes we keep the commitment and change the key:
//!
//! * internal key `P = Q + H_state(r)·G`, where `Q` is the enclave key, so
//!   the output still commits to the root and a light client can check it;
//! * the key path (signed by the enclave with the state tweak) carries every
//!   normal transition;
//! * one tapscript leaf, `<K_k> OP_CHECKSIGVERIFY <Δ> OP_CHECKSEQUENCEVERIFY`,
//!   is the unilateral exit: `K_k` is a per-state key the enclave uses once
//!   to pre-sign the unroll of this state and then deletes.

use bitcoin::ScriptBuf;
use bitcoin::blockdata::opcodes::all::{OP_CHECKSIGVERIFY, OP_CSV};
use bitcoin::hashes::{Hash, sha256};
use bitcoin::key::{TweakedPublicKey, XOnlyPublicKey};
use bitcoin::script::Builder;
use bitcoin::secp256k1::{Scalar, Secp256k1, Verification};
use bitcoin::taproot::{LeafVersion, TapLeafHash, TaprootBuilder, TaprootSpendInfo};

use crate::tree::{TAG_STATE, tagged_hash};

/// BIP-341's nothing-up-my-sleeve x coordinate (spec 5.1).
pub const NUMS_X: [u8; 32] = [
    0x50, 0x92, 0x9b, 0x74, 0xc1, 0xa0, 0x49, 0x54, 0xb7, 0x8b, 0x4b, 0x60, 0x35, 0xe9, 0x7a, 0x5e,
    0x07, 0x8a, 0x5a, 0x0f, 0x28, 0xec, 0x96, 0xd5, 0x47, 0xbf, 0xee, 0x9a, 0xce, 0x80, 0x3a, 0xc0,
];

/// The pay-to-anchor script `OP_1 <0x4e73>` (Bitcoin Core 28+ policy).
pub fn p2a_script() -> ScriptBuf {
    ScriptBuf::from_bytes(vec![0x51, 0x02, 0x4e, 0x73])
}

/// `OP_1 <key>`: a payout to a leaf's `exit_key`, used as the output key
/// as is (spec 7.2 rule 2), with no further tweak.
pub fn p2tr_raw(key: &[u8; 32]) -> ScriptBuf {
    let x = XOnlyPublicKey::from_slice(key).expect("validated leaf key");
    ScriptBuf::new_p2tr_tweaked(TweakedPublicKey::dangerous_assume_tweaked(x))
}

/// The state tweak `H_state(r)` (spec 6.1) as a scalar.
pub fn state_tweak(root_hash: &[u8; 32]) -> Scalar {
    Scalar::from_be_bytes(tagged_hash(TAG_STATE, &[root_hash])).expect("2^-128 chance; spec 6.1")
}

/// `x(Q + H_state(r)·G)`.
pub fn state_key<C: Verification>(
    secp: &Secp256k1<C>,
    enclave_key: &XOnlyPublicKey,
    root_hash: &[u8; 32],
) -> XOnlyPublicKey {
    enclave_key
        .add_tweak(secp, &state_tweak(root_hash))
        .expect("valid tweak")
        .0
}

/// The unilateral-exit tapscript for state key `K_k` and delay `Δ` blocks.
pub fn unroll_leaf_script(unroll_key: &XOnlyPublicKey, delay: u16) -> ScriptBuf {
    Builder::new()
        .push_x_only_key(unroll_key)
        .push_opcode(OP_CHECKSIGVERIFY)
        .push_int(delay as i64)
        .push_opcode(OP_CSV)
        .into_script()
}

/// Everything that fixes one state's tree output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TreeOutput {
    pub root_hash: [u8; 32],
    pub unroll_key: XOnlyPublicKey,
}

impl TreeOutput {
    pub fn spend_info<C: Verification>(
        &self,
        secp: &Secp256k1<C>,
        enclave_key: &XOnlyPublicKey,
        delay: u16,
    ) -> TaprootSpendInfo {
        let internal = state_key(secp, enclave_key, &self.root_hash);
        TaprootBuilder::new()
            .add_leaf(0, unroll_leaf_script(&self.unroll_key, delay))
            .expect("one leaf")
            .finalize(secp, internal)
            .expect("complete tree")
    }

    pub fn script_pubkey<C: Verification>(
        &self,
        secp: &Secp256k1<C>,
        enclave_key: &XOnlyPublicKey,
        delay: u16,
    ) -> ScriptBuf {
        ScriptBuf::new_p2tr_tweaked(self.spend_info(secp, enclave_key, delay).output_key())
    }

    pub fn unroll_leaf_hash(&self, delay: u16) -> TapLeafHash {
        TapLeafHash::from_script(
            &unroll_leaf_script(&self.unroll_key, delay),
            LeafVersion::TapScript,
        )
    }
}

/// `TREE_SPK_CCV(r, m)` (spec 6.3): the output a CCV-tier tree would use
/// for the same root. Internal key `NUMS + sha256(x(NUMS) || r)·G`, then
/// the ordinary BIP-341 tweak by `m`. The migration transition (see
/// `rules`) pays the TEE tree into exactly this output.
pub fn ccv_tree_spk<C: Verification>(
    secp: &Secp256k1<C>,
    root_hash: &[u8; 32],
    m: &[u8; 32],
) -> ScriptBuf {
    let nums = XOnlyPublicKey::from_slice(&NUMS_X).expect("NUMS is on the curve");
    let mut msg = [0u8; 64];
    msg[..32].copy_from_slice(&NUMS_X);
    msg[32..].copy_from_slice(root_hash);
    let t = Scalar::from_be_bytes(sha256::Hash::hash(&msg).to_byte_array()).expect("valid scalar");
    let internal = nums.add_tweak(secp, &t).expect("valid tweak").0;
    let root = bitcoin::taproot::TapNodeHash::from_byte_array(*m);
    ScriptBuf::new_p2tr(secp, internal, Some(root))
}
