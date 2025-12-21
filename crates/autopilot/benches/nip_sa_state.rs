//! Performance benchmarks for NIP-SA state encryption operations

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId, Throughput};
use serde_json::json;
use nostr::{encrypt_v2, decrypt_v2};
use rand::Rng;

/// Generate test state data of specified size
fn generate_state_data(size_kb: usize) -> String {
    let size_bytes = size_kb * 1024;
    let content = "x".repeat(size_bytes);
    json!({
        "goals": vec!["goal1", "goal2", "goal3"],
        "memory": {
            "context": content,
            "facts": vec!["fact1", "fact2"]
        },
        "wallet": {
            "balance_sats": 100000,
            "address": "spark1test"
        }
    }).to_string()
}

/// Generate random keys for encryption
fn generate_test_keys() -> ([u8; 32], [u8; 32]) {
    let mut rng = rand::rng();
    let mut secret_key = [0u8; 32];
    let mut recipient_pubkey = [0u8; 32];
    rng.fill(&mut secret_key);
    rng.fill(&mut recipient_pubkey);
    (secret_key, recipient_pubkey)
}

fn bench_nip44_encryption(c: &mut Criterion) {
    let mut group = c.benchmark_group("nip44_encryption");

    for size_kb in [1, 10, 100].iter() {
        group.throughput(Throughput::Bytes((*size_kb * 1024) as u64));

        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}KB", size_kb)),
            size_kb,
            |b, &size| {
                let state_data = generate_state_data(size);
                let (secret_key, recipient_pubkey) = generate_test_keys();

                b.iter(|| {
                    let encrypted = encrypt_v2(
                        black_box(&secret_key),
                        black_box(&recipient_pubkey),
                        black_box(&state_data),
                    );
                    black_box(encrypted)
                });
            },
        );
    }

    group.finish();
}

fn bench_nip44_decryption(c: &mut Criterion) {
    let mut group = c.benchmark_group("nip44_decryption");

    for size_kb in [1, 10, 100].iter() {
        group.throughput(Throughput::Bytes((*size_kb * 1024) as u64));

        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}KB", size_kb)),
            size_kb,
            |b, &size| {
                let state_data = generate_state_data(size);
                let (secret_key, recipient_pubkey) = generate_test_keys();

                // Pre-encrypt for decryption benchmark
                let encrypted = encrypt_v2(
                    &secret_key,
                    &recipient_pubkey,
                    &state_data,
                ).expect("encryption failed");

                b.iter(|| {
                    let decrypted = decrypt_v2(
                        black_box(&secret_key),
                        black_box(&recipient_pubkey),
                        black_box(&encrypted),
                    );
                    black_box(decrypted)
                });
            },
        );
    }

    group.finish();
}

fn bench_state_serialization(c: &mut Criterion) {
    let mut group = c.benchmark_group("state_serialization");

    for size_kb in [1, 10, 100].iter() {
        group.throughput(Throughput::Bytes((*size_kb * 1024) as u64));

        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}KB", size_kb)),
            size_kb,
            |b, &size| {
                let state_data = generate_state_data(size);

                b.iter(|| {
                    let serialized = serde_json::to_string(black_box(&state_data));
                    black_box(serialized)
                });
            },
        );
    }

    group.finish();
}

fn bench_state_deserialization(c: &mut Criterion) {
    let mut group = c.benchmark_group("state_deserialization");

    for size_kb in [1, 10, 100].iter() {
        group.throughput(Throughput::Bytes((*size_kb * 1024) as u64));

        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}KB", size_kb)),
            size_kb,
            |b, &size| {
                let state_data = generate_state_data(size);
                let serialized = serde_json::to_string(&state_data).unwrap();

                b.iter(|| {
                    let deserialized: Result<serde_json::Value, _> = serde_json::from_str(black_box(&serialized));
                    black_box(deserialized)
                });
            },
        );
    }

    group.finish();
}

fn bench_full_encrypt_cycle(c: &mut Criterion) {
    let mut group = c.benchmark_group("full_encrypt_cycle");

    for size_kb in [1, 10, 100].iter() {
        group.throughput(Throughput::Bytes((*size_kb * 1024) as u64));

        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}KB", size_kb)),
            size_kb,
            |b, &size| {
                let state_data = generate_state_data(size);
                let (secret_key, recipient_pubkey) = generate_test_keys();

                b.iter(|| {
                    // Full cycle: serialize → encrypt
                    let serialized = serde_json::to_string(black_box(&state_data)).unwrap();
                    let encrypted = encrypt_v2(
                        black_box(&secret_key),
                        black_box(&recipient_pubkey),
                        black_box(&serialized),
                    );
                    black_box(encrypted)
                });
            },
        );
    }

    group.finish();
}

fn bench_full_decrypt_cycle(c: &mut Criterion) {
    let mut group = c.benchmark_group("full_decrypt_cycle");

    for size_kb in [1, 10, 100].iter() {
        group.throughput(Throughput::Bytes((*size_kb * 1024) as u64));

        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}KB", size_kb)),
            size_kb,
            |b, &size| {
                let state_data = generate_state_data(size);
                let (secret_key, recipient_pubkey) = generate_test_keys();

                // Pre-encrypt
                let serialized = serde_json::to_string(&state_data).unwrap();
                let encrypted = encrypt_v2(
                    &secret_key,
                    &recipient_pubkey,
                    &serialized,
                ).expect("encryption failed");

                b.iter(|| {
                    // Full cycle: decrypt → deserialize
                    let decrypted = decrypt_v2(
                        black_box(&secret_key),
                        black_box(&recipient_pubkey),
                        black_box(&encrypted),
                    ).unwrap();
                    let deserialized: Result<serde_json::Value, _> = serde_json::from_str(black_box(&decrypted));
                    black_box(deserialized)
                });
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_nip44_encryption,
    bench_nip44_decryption,
    bench_state_serialization,
    bench_state_deserialization,
    bench_full_encrypt_cycle,
    bench_full_decrypt_cycle
);
criterion_main!(benches);
