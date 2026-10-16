//! The decoders invert the encoders on real lab data, and refuse
//! malformed bytes with a typed error.

mod common;

use common::World;
use layertree_tee_lab::codec::{DecodeError, dec64};
use layertree_tee_lab::rules::{Op, Record, TAG_RECORD};
use layertree_tee_lab::tree::{Leaf, enc64, tagged_hash};
use layertree_tee_lab::unroll::UnrollPackage;
use proptest::prelude::*;

#[test]
fn records_leaves_and_unroll_packages_round_trip() {
    let mut w = World::new(12, 4, None);
    let pkg = w.last_unroll.clone().unwrap();
    assert_eq!(UnrollPackage::decode(&pkg.encode()).unwrap(), pkg);
    let leaf = w.leaf(3);
    assert_eq!(Leaf::parse(&leaf.serialize()).unwrap(), leaf);
    for op in [
        Op::Exit {
            pos: 1,
            evict: false,
        },
        Op::Refresh,
    ] {
        let s = w.step(op).unwrap();
        let bytes = s.record.encode();
        assert_eq!(Record::decode(&bytes).unwrap(), s.record);
        // The anchored hash is the tagged hash of exactly these bytes.
        assert_eq!(s.record.hash(), tagged_hash(TAG_RECORD, &[&bytes]));
    }
}

#[test]
fn malformed_bytes_are_refused_with_a_reason() {
    assert_eq!(dec64(&[0; 8]), Err(DecodeError::Length));
    assert_eq!(dec64(&[0; 9]), Err(DecodeError::Flag));
    assert_eq!(Leaf::parse(&[0; 115]), Err(DecodeError::Length));
    let mut w = World::new(4, 128, None);
    let s = w
        .step(Op::Exit {
            pos: 0,
            evict: false,
        })
        .unwrap();
    let bytes = s.record.encode();
    assert_eq!(
        Record::decode(&bytes[..bytes.len() - 1]),
        Err(DecodeError::Length)
    );
    let mut extra = bytes.clone();
    extra.push(0);
    assert_eq!(Record::decode(&extra), Err(DecodeError::Length));
    // The presence byte of the first change's old leaf must be 0 or 1.
    let mut flag = bytes.clone();
    flag[32 + 4 + 4 + 8] = 2;
    assert_eq!(Record::decode(&flag), Err(DecodeError::Flag));
    // A next key that is not a point.
    let mut key = bytes.clone();
    let n = key.len();
    key[n - 32..].copy_from_slice(&[0xff; 32]);
    assert_eq!(Record::decode(&key), Err(DecodeError::Key));
    // A huge change count refuses before allocating.
    let mut many = bytes.clone();
    many[36..40].copy_from_slice(&u32::MAX.to_le_bytes());
    assert_eq!(Record::decode(&many), Err(DecodeError::TooMany));
    // A truncated package, and a transaction cut short inside its length.
    let pkg = w.last_unroll.clone().unwrap();
    let wire = pkg.encode();
    assert_eq!(
        UnrollPackage::decode(&wire[..wire.len() - 1]),
        Err(DecodeError::Length)
    );
    let mut short = wire.clone();
    let len = u32::from_le_bytes(short[40..44].try_into().unwrap());
    short[40..44].copy_from_slice(&(len - 1).to_le_bytes());
    assert!(UnrollPackage::decode(&short).is_err());
}

proptest! {
    #[test]
    fn enc64_and_leaf_decoding_invert_encoding(n: u64, bytes in prop::collection::vec(any::<u8>(), 116)) {
        prop_assert_eq!(dec64(&enc64(n)), Ok(n));
        let leaf = Leaf::parse(&bytes).unwrap();
        prop_assert_eq!(leaf.serialize().to_vec(), bytes);
    }
}

/// `LAYERTREE_WRITE_SEEDS=1` writes the fuzz seed corpora from lab data
/// (test keys only).
#[test]
fn writes_fuzz_seeds_when_asked() {
    if std::env::var_os("LAYERTREE_WRITE_SEEDS").is_none() {
        return;
    }
    let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fuzz/corpus");
    let mut w = World::new(12, 4, None);
    let mut seeds: Vec<(&str, Vec<u8>)> = vec![
        ("leaf_roundtrip", w.leaf(2).serialize().to_vec()),
        ("enc64_roundtrip", enc64(330).to_vec()),
        (
            "unroll_package_decode",
            w.last_unroll.clone().unwrap().encode(),
        ),
    ];
    for op in [
        Op::Exit {
            pos: 1,
            evict: false,
        },
        Op::Refresh,
    ] {
        seeds.push(("record_decode", w.step(op).unwrap().record.encode()));
    }
    for (i, (target, bytes)) in seeds.into_iter().enumerate() {
        std::fs::create_dir_all(dir.join(target)).unwrap();
        std::fs::write(dir.join(target).join(format!("seed-{i}")), bytes).unwrap();
    }
}
