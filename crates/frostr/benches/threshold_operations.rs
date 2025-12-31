//! Benchmarks for FROST threshold signature operations
//!
//! Run with: cargo bench -p frostr --bench threshold_operations

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use frost_secp256k1::SigningPackage;
use frostr::ecdh::threshold_ecdh;
use frostr::keygen::generate_key_shares;
use frostr::signing::{aggregate_signatures, round1_commit, round2_sign};
use std::collections::BTreeMap;
use std::hint::black_box;

fn bench_key_generation(c: &mut Criterion) {
    let mut group = c.benchmark_group("frost_keygen");

    for (k, n) in [(2, 3), (3, 5), (5, 7), (7, 10)].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}_of_{}", k, n)),
            &(k, n),
            |b, (k, n)| {
                b.iter(|| {
                    generate_key_shares(black_box(**k), black_box(**n))
                        .expect("Key generation failed")
                });
            },
        );
    }

    group.finish();
}

fn bench_signing_2_of_3(c: &mut Criterion) {
    let shares = generate_key_shares(2, 3).expect("Key generation failed");
    let message = b"Test message for benchmarking FROST signatures";

    c.bench_function("sign_2_of_3", |b| {
        b.iter(|| {
            // Round 1: Generate commitments
            let (nonces1, commitments1) = round1_commit(black_box(&shares[0]));
            let (nonces2, commitments2) = round1_commit(black_box(&shares[1]));

            // Create signing package
            let mut signing_commitments = BTreeMap::new();
            signing_commitments.insert(*shares[0].key_package.identifier(), commitments1);
            signing_commitments.insert(*shares[1].key_package.identifier(), commitments2);
            let signing_package = SigningPackage::new(signing_commitments, black_box(message));

            // Round 2: Generate partial signatures
            let sig1 = round2_sign(black_box(&shares[0]), &nonces1, &signing_package)
                .expect("Signing failed");
            let sig2 = round2_sign(black_box(&shares[1]), &nonces2, &signing_package)
                .expect("Signing failed");

            // Aggregate signatures
            let mut signature_shares = BTreeMap::new();
            signature_shares.insert(*shares[0].key_package.identifier(), sig1);
            signature_shares.insert(*shares[1].key_package.identifier(), sig2);

            aggregate_signatures(
                black_box(&signing_package),
                black_box(&signature_shares),
                black_box(&shares[0]),
            )
            .expect("Aggregation failed")
        });
    });
}

fn bench_signing_3_of_5(c: &mut Criterion) {
    let shares = generate_key_shares(3, 5).expect("Key generation failed");
    let message = b"Test message for benchmarking FROST signatures";

    c.bench_function("sign_3_of_5", |b| {
        b.iter(|| {
            // Round 1: Generate commitments
            let (nonces0, commitments0) = round1_commit(black_box(&shares[0]));
            let (nonces1, commitments1) = round1_commit(black_box(&shares[1]));
            let (nonces2, commitments2) = round1_commit(black_box(&shares[2]));

            // Create signing package
            let mut signing_commitments = BTreeMap::new();
            signing_commitments.insert(*shares[0].key_package.identifier(), commitments0);
            signing_commitments.insert(*shares[1].key_package.identifier(), commitments1);
            signing_commitments.insert(*shares[2].key_package.identifier(), commitments2);
            let signing_package = SigningPackage::new(signing_commitments, black_box(message));

            // Round 2: Generate partial signatures
            let sig0 = round2_sign(black_box(&shares[0]), &nonces0, &signing_package)
                .expect("Signing failed");
            let sig1 = round2_sign(black_box(&shares[1]), &nonces1, &signing_package)
                .expect("Signing failed");
            let sig2 = round2_sign(black_box(&shares[2]), &nonces2, &signing_package)
                .expect("Signing failed");

            // Aggregate signatures
            let mut signature_shares = BTreeMap::new();
            signature_shares.insert(*shares[0].key_package.identifier(), sig0);
            signature_shares.insert(*shares[1].key_package.identifier(), sig1);
            signature_shares.insert(*shares[2].key_package.identifier(), sig2);

            aggregate_signatures(
                black_box(&signing_package),
                black_box(&signature_shares),
                black_box(&shares[0]),
            )
            .expect("Aggregation failed")
        });
    });
}

fn bench_signing_5_of_7(c: &mut Criterion) {
    let shares = generate_key_shares(5, 7).expect("Key generation failed");
    let message = b"Test message for benchmarking FROST signatures";

    c.bench_function("sign_5_of_7", |b| {
        b.iter(|| {
            // Round 1: Generate commitments for first 5 shares
            let nonces_and_commitments: Vec<_> = (0..5)
                .map(|i| round1_commit(black_box(&shares[i])))
                .collect();

            // Create signing package
            let mut signing_commitments = BTreeMap::new();
            for (i, (_, commitment)) in nonces_and_commitments.iter().enumerate() {
                signing_commitments.insert(*shares[i].key_package.identifier(), *commitment);
            }
            let signing_package = SigningPackage::new(signing_commitments, black_box(message));

            // Round 2: Generate partial signatures
            let mut signature_shares = BTreeMap::new();
            for (i, (nonces, _)) in nonces_and_commitments.iter().enumerate() {
                let sig = round2_sign(black_box(&shares[i]), nonces, &signing_package)
                    .expect("Signing failed");
                signature_shares.insert(*shares[i].key_package.identifier(), sig);
            }

            // Aggregate signatures
            aggregate_signatures(
                black_box(&signing_package),
                black_box(&signature_shares),
                black_box(&shares[0]),
            )
            .expect("Aggregation failed")
        });
    });
}

fn bench_individual_signature(c: &mut Criterion) {
    let shares = generate_key_shares(2, 3).expect("Key generation failed");

    c.bench_function("round1_commit", |b| {
        b.iter(|| round1_commit(black_box(&shares[0])));
    });
}

fn bench_signature_aggregation(c: &mut Criterion) {
    let mut group = c.benchmark_group("signature_aggregation");

    let message = b"Test message for benchmarking FROST signatures";

    for num_sigs in [2, 3, 5, 7].iter() {
        let shares = generate_key_shares(*num_sigs, *num_sigs + 1).expect("Key generation failed");

        let num_sigs_usize = *num_sigs as usize;

        // Pre-generate nonces and commitments
        let nonces_and_commitments: Vec<_> = (0..num_sigs_usize)
            .map(|i| round1_commit(&shares[i]))
            .collect();

        // Create signing package
        let mut signing_commitments = BTreeMap::new();
        for (i, (_, commitment)) in nonces_and_commitments.iter().enumerate() {
            signing_commitments.insert(*shares[i].key_package.identifier(), *commitment);
        }
        let signing_package = SigningPackage::new(signing_commitments, message);

        // Generate partial signatures
        let mut signature_shares = BTreeMap::new();
        for (i, (nonces, _)) in nonces_and_commitments.iter().enumerate() {
            let sig = round2_sign(&shares[i], nonces, &signing_package).expect("Signing failed");
            signature_shares.insert(*shares[i].key_package.identifier(), sig);
        }

        group.bench_with_input(BenchmarkId::from_parameter(num_sigs), num_sigs, |b, _| {
            b.iter(|| {
                aggregate_signatures(
                    black_box(&signing_package),
                    black_box(&signature_shares),
                    black_box(&shares[0]),
                )
                .expect("Aggregation failed")
            });
        });
    }

    group.finish();
}

fn bench_threshold_ecdh(c: &mut Criterion) {
    let shares = generate_key_shares(2, 3).expect("Key generation failed");

    // Generate a peer public key (x-only, 32 bytes)
    let peer_pk: [u8; 32] = [0x42u8; 32];

    c.bench_function("threshold_ecdh_2_of_3", |b| {
        b.iter(|| {
            threshold_ecdh(black_box(&shares[0..2]), black_box(&peer_pk)).expect("ECDH failed")
        });
    });
}

fn bench_threshold_ecdh_various_configs(c: &mut Criterion) {
    let mut group = c.benchmark_group("threshold_ecdh");

    let peer_pk: [u8; 32] = [0x42u8; 32];

    for (k, n) in [(2, 3), (3, 5), (5, 7)].iter() {
        let shares = generate_key_shares(*k, *n).expect("Key generation failed");
        let k_usize = *k as usize;

        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}_of_{}", k, n)),
            &(k, n),
            |b, _| {
                b.iter(|| {
                    threshold_ecdh(black_box(&shares[0..k_usize]), black_box(&peer_pk))
                        .expect("ECDH failed")
                });
            },
        );
    }

    group.finish();
}

fn bench_verifying_key_derivation(c: &mut Criterion) {
    let shares = generate_key_shares(2, 3).expect("Key generation failed");

    c.bench_function("verifying_key_from_share", |b| {
        b.iter(|| shares[0].public_key_package.verifying_key());
    });
}

criterion_group!(
    benches,
    bench_key_generation,
    bench_signing_2_of_3,
    bench_signing_3_of_5,
    bench_signing_5_of_7,
    bench_individual_signature,
    bench_signature_aggregation,
    bench_threshold_ecdh,
    bench_threshold_ecdh_various_configs,
    bench_verifying_key_derivation,
);
criterion_main!(benches);
