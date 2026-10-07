//! Philox 4x32-10 counter-based RNG: the standard for seeded work classes
//! (psionic#1116).
//!
//! Seeded work classes — ablation cells, scaling-sweep cells, rollout
//! generation, and future seeded generation kinds — verify by
//! `seeded_replication`: a same-class validator recomputes the work from the
//! same seed and compares. That verification class only holds if the RNG
//! produces identical output on every contributor device and backend. A
//! stateful generator whose output depends on call order, ambient entropy,
//! thread scheduling, or backend-specific library code breaks the class.
//!
//! Philox 4x32-10 (Salmon, Moraes, Dror, Shaw, "Parallel Random Numbers: As
//! Easy as 1, 2, 3", SC'11) is a counter-based block function: the triple
//! `(seed, stream, counter)` maps to output through pure 32-bit integer
//! arithmetic, with no mutable global state. Same triple, same output, on any
//! host — CPU, Metal-attached, or CUDA-attached — by construction. PyTorch's
//! CUDA generator (`at::Philox4_32`) uses the same algorithm and the same
//! `(seed, subsequence, offset)` parameterization, so this module's stream
//! layout matches what GPU training stacks already standardize on.
//!
//! Reference implementations read (read-only, never vendored):
//!
//! - `projects/tether/repos/qvac-ext-stable-diffusion.cpp/src/rng_philox.hpp`
//!   (the QVAC/Tether port absorbed via the 2026-06-10 QVAC edge-stack
//!   analysis; itself a port of AUTOMATIC1111 `modules/rng_philox.py`, which
//!   replicates `torch.randn` on CUDA)
//! - `DEShawResearch/random123` `tests/kat_vectors` (the published
//!   known-answer vectors pinned in [`PHILOX4X32_10_REFERENCE_VECTORS`] and in
//!   the committed fixture)
//!
//! The module exposes three layers:
//!
//! 1. [`philox4x32_10`]: the bare block function, pinned to the published
//!    random123 known-answer vectors.
//! 2. [`PhiloxStream`]: a `(seed, stream)` handle with random-access draws by
//!    counter/index — the PyTorch-compatible parameterization (`key` = split
//!    seed, counter words `[counter_lo, counter_hi, stream_lo, stream_hi]`).
//! 3. [`philox_counter_rng`] / [`philox_counter_unit`]: stateless
//!    `(seed, stream, counter)` draws with the same shape as the house
//!    splitmix64 mix (`sparta_counter_rng` in `psionic-train`), so seeded call
//!    sites migrate without restructuring.
//!
//! Floating-point scope: only [`PhiloxStream::unit_f64_at`] and
//! [`philox_counter_unit`] touch floats, and they use exact IEEE-754
//! operations (53-bit mantissa fill, one division by a power of two), which
//! are bit-identical on every conforming platform. No transcendental
//! functions: normal draws via Box–Muller depend on platform `libm`
//! `log`/`sin` and are intentionally not provided until a deterministic
//! transcendental policy exists (recorded follow-up in
//! `docs/PSIONIC_PHILOX_RNG.md`).

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Contract identifier for the Philox seeded-work RNG standard.
pub const PHILOX_RNG_CONTRACT_ID: &str = "psionic.core.philox_rng.v1";
/// Schema version of the committed reference-vector fixture.
pub const PHILOX_RNG_SCHEMA_VERSION: &str = "psionic.core.philox_rng.v1";
/// Committed reference-vector and determinism-receipt fixture.
pub const PHILOX_RNG_FIXTURE_PATH: &str = "fixtures/rng/philox4x32_reference_vectors.json";
/// Canonical doc for this standard.
pub const PHILOX_RNG_DOC_PATH: &str = "docs/PSIONIC_PHILOX_RNG.md";
/// Repo-local parity check script.
pub const PHILOX_RNG_CHECK_SCRIPT_PATH: &str = "scripts/check-philox-rng.sh";

/// Round count for the standard Philox 4x32-10 configuration.
pub const PHILOX4X32_ROUNDS: u32 = 10;

/// Philox 4x32 multiplier for counter word 0.
const PHILOX_M0: u32 = 0xD251_1F53;
/// Philox 4x32 multiplier for counter word 2.
const PHILOX_M1: u32 = 0xCD9E_8D57;
/// Weyl key-schedule increment for key word 0 (golden ratio).
const PHILOX_W0: u32 = 0x9E37_79B9;
/// Weyl key-schedule increment for key word 1 (sqrt 3 - 1).
const PHILOX_W1: u32 = 0xBB67_AE85;

/// Published random123 known-answer vectors for philox4x32-10, in
/// `(counter, key, expected)` order, copied verbatim from
/// `DEShawResearch/random123` `tests/kat_vectors`.
pub const PHILOX4X32_10_REFERENCE_VECTORS: [([u32; 4], [u32; 2], [u32; 4]); 3] = [
    (
        [0x0000_0000, 0x0000_0000, 0x0000_0000, 0x0000_0000],
        [0x0000_0000, 0x0000_0000],
        [0x6627_E8D5, 0xE169_C58D, 0xBC57_AC4C, 0x9B00_DBD8],
    ),
    (
        [0xFFFF_FFFF, 0xFFFF_FFFF, 0xFFFF_FFFF, 0xFFFF_FFFF],
        [0xFFFF_FFFF, 0xFFFF_FFFF],
        [0x408F_276D, 0x41C8_3B0E, 0xA20B_C7C6, 0x6D54_51FD],
    ),
    (
        [0x243F_6A88, 0x85A3_08D3, 0x1319_8A2E, 0x0370_7344],
        [0xA409_3822, 0x299F_31D0],
        [0xD16C_FE09, 0x94FD_CCEB, 0x5001_E420, 0x2412_6EA1],
    ),
];

/// Seed for the committed determinism-receipt workload.
pub const PHILOX_DETERMINISM_RECEIPT_SEED: u64 = 0x1116_5EED_C0DE_2026;
/// Stream count for the committed determinism-receipt workload.
pub const PHILOX_DETERMINISM_RECEIPT_STREAMS: u64 = 4;
/// Draws per stream for the committed determinism-receipt workload.
pub const PHILOX_DETERMINISM_RECEIPT_DRAWS_PER_STREAM: u64 = 1024;

#[inline]
fn mul_hi_lo(a: u32, b: u32) -> (u32, u32) {
    let product = u64::from(a) * u64::from(b);
    ((product >> 32) as u32, product as u32)
}

#[inline]
fn philox4x32_round(counter: [u32; 4], key: [u32; 2]) -> [u32; 4] {
    let (hi0, lo0) = mul_hi_lo(PHILOX_M0, counter[0]);
    let (hi1, lo1) = mul_hi_lo(PHILOX_M1, counter[2]);
    [
        hi1 ^ counter[1] ^ key[0],
        lo1,
        hi0 ^ counter[3] ^ key[1],
        lo0,
    ]
}

/// The Philox 4x32-10 block function: 128-bit counter plus 64-bit key in,
/// 128 random bits out. Pure integer arithmetic; pinned to the published
/// random123 known-answer vectors.
#[must_use]
pub fn philox4x32_10(mut counter: [u32; 4], mut key: [u32; 2]) -> [u32; 4] {
    for round in 0..PHILOX4X32_ROUNDS {
        counter = philox4x32_round(counter, key);
        if round + 1 < PHILOX4X32_ROUNDS {
            key[0] = key[0].wrapping_add(PHILOX_W0);
            key[1] = key[1].wrapping_add(PHILOX_W1);
        }
    }
    counter
}

/// One logical random stream identified by `(seed, stream)`, with
/// random-access draws by counter. Copy-cheap and stateless: the same
/// `(seed, stream, counter)` triple always yields the same value, independent
/// of call order, thread, host, or attached backend.
///
/// Layout matches PyTorch's `at::Philox4_32(seed, subsequence, offset)`:
/// `key = [seed_lo, seed_hi]`, counter words
/// `[counter_lo, counter_hi, stream_lo, stream_hi]`. The QVAC/Tether
/// `rng_philox.hpp` layout (per-element key = split seed, `counter[0]` =
/// offset, `counter[2]` = element index) is the 32-bit special case of this
/// parameterization.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PhiloxStream {
    /// Work-class seed. Caller-passed; never ambient entropy or wall clock.
    pub seed: u64,
    /// Stream / subsequence selector (for example: tensor, rollout, or cell
    /// identity), so independent draw sites never share a counter space.
    pub stream: u64,
}

impl PhiloxStream {
    /// Bind a logical stream to a caller-passed seed.
    #[must_use]
    pub fn new(seed: u64, stream: u64) -> Self {
        Self { seed, stream }
    }

    /// One 128-bit Philox block at `counter`.
    #[must_use]
    pub fn block(&self, counter: u64) -> [u32; 4] {
        let key = [self.seed as u32, (self.seed >> 32) as u32];
        let words = [
            counter as u32,
            (counter >> 32) as u32,
            self.stream as u32,
            (self.stream >> 32) as u32,
        ];
        philox4x32_10(words, key)
    }

    /// Random-access `u32` draw: lane `index % 4` of block `index / 4`.
    #[must_use]
    pub fn u32_at(&self, index: u64) -> u32 {
        self.block(index / 4)[(index % 4) as usize]
    }

    /// Random-access `u64` draw: lane pair `2 * (index % 2)` of block
    /// `index / 2`, low word first.
    #[must_use]
    pub fn u64_at(&self, index: u64) -> u64 {
        let block = self.block(index / 2);
        let lane = ((index % 2) * 2) as usize;
        (u64::from(block[lane + 1]) << 32) | u64::from(block[lane])
    }

    /// Random-access uniform draw in `[0, 1)` with a 53-bit mantissa. Uses
    /// only exact IEEE-754 operations, so the result is bit-identical on
    /// every conforming platform.
    #[must_use]
    pub fn unit_f64_at(&self, index: u64) -> f64 {
        (self.u64_at(index) >> 11) as f64 / (1_u64 << 53) as f64
    }
}

/// Stateless counter-based `u64` draw — the Philox-backed counterpart of the
/// house splitmix64 mix (`sparta_counter_rng`): the same
/// `(seed, stream, counter)` triple always yields the same value.
#[must_use]
pub fn philox_counter_rng(seed: u64, stream: u64, counter: u64) -> u64 {
    PhiloxStream::new(seed, stream).u64_at(counter)
}

/// Stateless counter-based uniform draw in `[0, 1)` — the Philox-backed
/// counterpart of `sparta_counter_unit`.
#[must_use]
pub fn philox_counter_unit(seed: u64, stream: u64, counter: u64) -> f64 {
    PhiloxStream::new(seed, stream).unit_f64_at(counter)
}

/// Recompute the committed determinism-receipt digest: SHA-256 over the
/// little-endian bytes of `philox_counter_rng` draws for the committed
/// workload (stream-major, then counter order). Any host — CPU-only,
/// Metal-attached, or CUDA-attached — recomputes this digest and compares it
/// against the value pinned in [`PHILOX_RNG_FIXTURE_PATH`]; a mismatch is a
/// determinism break in the seeded work-class substrate.
#[must_use]
pub fn philox_determinism_receipt_digest() -> String {
    let mut hasher = Sha256::new();
    for stream in 0..PHILOX_DETERMINISM_RECEIPT_STREAMS {
        for counter in 0..PHILOX_DETERMINISM_RECEIPT_DRAWS_PER_STREAM {
            hasher.update(
                philox_counter_rng(PHILOX_DETERMINISM_RECEIPT_SEED, stream, counter).to_le_bytes(),
            );
        }
    }
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic)]

    use std::{fs, path::Path};

    use super::{
        PHILOX_DETERMINISM_RECEIPT_DRAWS_PER_STREAM, PHILOX_DETERMINISM_RECEIPT_SEED,
        PHILOX_DETERMINISM_RECEIPT_STREAMS, PHILOX_RNG_CONTRACT_ID, PHILOX_RNG_FIXTURE_PATH,
        PHILOX_RNG_SCHEMA_VERSION, PHILOX4X32_10_REFERENCE_VECTORS, PhiloxStream,
        philox_counter_rng, philox_counter_unit, philox_determinism_receipt_digest, philox4x32_10,
    };

    fn fixture_json() -> serde_json::Value {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join(PHILOX_RNG_FIXTURE_PATH);
        let raw = fs::read_to_string(&path).expect("philox fixture must exist");
        serde_json::from_str(&raw).expect("philox fixture must be valid JSON")
    }

    fn parse_word(value: &serde_json::Value) -> u32 {
        let text = value.as_str().expect("fixture word must be a hex string");
        u32::from_str_radix(text.trim_start_matches("0x"), 16).expect("fixture word must parse")
    }

    #[test]
    fn philox4x32_10_matches_published_random123_vectors() {
        for (counter, key, expected) in PHILOX4X32_10_REFERENCE_VECTORS {
            assert_eq!(philox4x32_10(counter, key), expected);
        }
    }

    #[test]
    fn committed_fixture_vectors_match_pinned_vectors_and_implementation() {
        let fixture = fixture_json();
        assert_eq!(fixture["schema_version"], PHILOX_RNG_SCHEMA_VERSION);
        assert_eq!(fixture["contract_id"], PHILOX_RNG_CONTRACT_ID);

        let vectors = fixture["reference_vectors"]
            .as_array()
            .expect("fixture must carry reference_vectors");
        assert_eq!(vectors.len(), PHILOX4X32_10_REFERENCE_VECTORS.len());

        for (vector, (counter, key, expected)) in
            vectors.iter().zip(PHILOX4X32_10_REFERENCE_VECTORS)
        {
            let fixture_counter: Vec<u32> = vector["counter"]
                .as_array()
                .expect("vector counter")
                .iter()
                .map(parse_word)
                .collect();
            let fixture_key: Vec<u32> = vector["key"]
                .as_array()
                .expect("vector key")
                .iter()
                .map(parse_word)
                .collect();
            let fixture_expected: Vec<u32> = vector["expected"]
                .as_array()
                .expect("vector expected")
                .iter()
                .map(parse_word)
                .collect();

            assert_eq!(fixture_counter, counter);
            assert_eq!(fixture_key, key);
            assert_eq!(fixture_expected, expected);
            assert_eq!(philox4x32_10(counter, key).to_vec(), fixture_expected);
        }
    }

    #[test]
    fn determinism_receipt_digest_matches_committed_fixture() {
        let fixture = fixture_json();
        let receipt = &fixture["determinism_receipt"];
        assert_eq!(
            receipt["seed"],
            format!("0x{PHILOX_DETERMINISM_RECEIPT_SEED:016X}")
        );
        assert_eq!(
            receipt["streams"].as_u64(),
            Some(PHILOX_DETERMINISM_RECEIPT_STREAMS)
        );
        assert_eq!(
            receipt["draws_per_stream"].as_u64(),
            Some(PHILOX_DETERMINISM_RECEIPT_DRAWS_PER_STREAM)
        );
        assert_eq!(
            receipt["sha256"].as_str(),
            Some(philox_determinism_receipt_digest().as_str())
        );
    }

    #[test]
    fn stream_layout_matches_pytorch_parameterization() {
        // key = split seed, counter words = [ctr_lo, ctr_hi, stream_lo,
        // stream_hi] — at::Philox4_32(seed, subsequence, offset) with
        // stream = subsequence and counter = offset.
        let stream = PhiloxStream::new(0x1234_5678_9ABC_DEF0, 0x0FED_CBA9_8765_4321);
        let direct = philox4x32_10(
            [0x4444_3333, 0x0000_0000, 0x8765_4321, 0x0FED_CBA9],
            [0x9ABC_DEF0, 0x1234_5678],
        );
        assert_eq!(stream.block(0x4444_3333), direct);
    }

    #[test]
    fn stream_layout_covers_qvac_rng_philox_layout() {
        // QVAC/Tether rng_philox.hpp gives element i of a randn(n) call at
        // offset o the block (counter = [o, 0, i, 0], key = split seed).
        // That is the 32-bit special case of PhiloxStream: stream = i,
        // counter = o.
        let seed = 0x0000_00AB_CDEF_0123;
        let element_index = 7_u64;
        let offset = 3_u64;
        let qvac_block = philox4x32_10(
            [offset as u32, 0, element_index as u32, 0],
            [seed as u32, (seed >> 32) as u32],
        );
        assert_eq!(
            PhiloxStream::new(seed, element_index).block(offset),
            qvac_block
        );
    }

    #[test]
    fn draws_are_call_order_independent_and_distinct_across_streams() {
        let forward: Vec<u64> = (0..16)
            .map(|counter| philox_counter_rng(7, 1, counter))
            .collect();
        let mut reverse: Vec<u64> = (0..16)
            .rev()
            .map(|counter| philox_counter_rng(7, 1, counter))
            .collect();
        reverse.reverse();
        assert_eq!(forward, reverse);

        let other_stream: Vec<u64> = (0..16)
            .map(|counter| philox_counter_rng(7, 2, counter))
            .collect();
        let other_seed: Vec<u64> = (0..16)
            .map(|counter| philox_counter_rng(8, 1, counter))
            .collect();
        assert_ne!(forward, other_stream);
        assert_ne!(forward, other_seed);
    }

    #[test]
    fn u32_and_u64_indexing_tile_blocks_without_overlap() {
        let stream = PhiloxStream::new(11, 13);
        let block0 = stream.block(0);
        let block1 = stream.block(1);

        for lane in 0..4_u64 {
            assert_eq!(stream.u32_at(lane), block0[lane as usize]);
            assert_eq!(stream.u32_at(4 + lane), block1[lane as usize]);
        }
        assert_eq!(
            stream.u64_at(0),
            (u64::from(block0[1]) << 32) | u64::from(block0[0])
        );
        assert_eq!(
            stream.u64_at(1),
            (u64::from(block0[3]) << 32) | u64::from(block0[2])
        );
        assert_eq!(
            stream.u64_at(2),
            (u64::from(block1[1]) << 32) | u64::from(block1[0])
        );
    }

    #[test]
    fn unit_draws_stay_in_half_open_interval() {
        for counter in 0..512 {
            let unit = philox_counter_unit(0x5EED, 0, counter);
            assert!((0.0..1.0).contains(&unit));
        }
        // Exact construction: 53-bit mantissa over 2^53.
        let value = philox_counter_rng(0x5EED, 0, 0);
        assert!(
            (philox_counter_unit(0x5EED, 0, 0) - (value >> 11) as f64 / (1_u64 << 53) as f64).abs()
                == 0.0
        );
    }
}
