//! Benchmarks for NIP-44 encryption and decryption operations
//!
//! Run with: cargo bench -p nostr --bench nip44_crypto

use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};
use nostr::{decrypt_v2 as decrypt, encrypt_v2 as encrypt};
use nostr::{generate_secret_key, get_public_key};

/// Generate test message of given size
fn generate_message(size: usize) -> String {
    "a".repeat(size)
}

fn bench_encrypt_various_sizes(c: &mut Criterion) {
    let mut group = c.benchmark_group("nip44_encrypt");

    let sender_sk = generate_secret_key();
    let recipient_sk = generate_secret_key();
    let recipient_pk = get_public_key(&recipient_sk).unwrap();

    for size in [100, 1_000, 10_000, 100_000].iter() {
        let message = generate_message(*size);
        group.throughput(Throughput::Bytes(*size as u64));

        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, _| {
            b.iter(|| {
                encrypt(
                    black_box(&sender_sk),
                    black_box(&recipient_pk),
                    black_box(&message),
                )
                .expect("Encryption failed")
            });
        });
    }

    group.finish();
}

fn bench_decrypt_various_sizes(c: &mut Criterion) {
    let mut group = c.benchmark_group("nip44_decrypt");

    let sender_sk = generate_secret_key();
    let recipient_sk = generate_secret_key();
    let recipient_pk = get_public_key(&recipient_sk).unwrap();
    let sender_pk = get_public_key(&sender_sk).unwrap();

    for size in [100, 1_000, 10_000, 100_000].iter() {
        let message = generate_message(*size);
        let encrypted = encrypt(&sender_sk, &recipient_pk, &message).expect("Encryption failed");
        group.throughput(Throughput::Bytes(*size as u64));

        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, _| {
            b.iter(|| {
                decrypt(
                    black_box(&recipient_sk),
                    black_box(&sender_pk),
                    black_box(&encrypted),
                )
                .expect("Decryption failed")
            });
        });
    }

    group.finish();
}

fn bench_encrypt_decrypt_roundtrip(c: &mut Criterion) {
    let mut group = c.benchmark_group("nip44_roundtrip");

    let sender_sk = generate_secret_key();
    let recipient_sk = generate_secret_key();
    let recipient_pk = get_public_key(&recipient_sk).unwrap();
    let sender_pk = get_public_key(&sender_sk).unwrap();

    for size in [100, 1_000, 10_000].iter() {
        let message = generate_message(*size);
        group.throughput(Throughput::Bytes(*size as u64));

        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, _| {
            b.iter(|| {
                let encrypted = encrypt(
                    black_box(&sender_sk),
                    black_box(&recipient_pk),
                    black_box(&message),
                )
                .expect("Encryption failed");

                decrypt(
                    black_box(&recipient_sk),
                    black_box(&sender_pk),
                    black_box(&encrypted),
                )
                .expect("Decryption failed")
            });
        });
    }

    group.finish();
}

fn bench_key_generation(c: &mut Criterion) {
    c.bench_function("generate_secret_key", |b| {
        b.iter(|| generate_secret_key());
    });
}

fn bench_public_key_derivation(c: &mut Criterion) {
    let secret_key = generate_secret_key();

    c.bench_function("get_public_key", |b| {
        b.iter(|| get_public_key(black_box(&secret_key)));
    });
}

fn bench_encrypt_empty_message(c: &mut Criterion) {
    let sender_sk = generate_secret_key();
    let recipient_sk = generate_secret_key();
    let recipient_pk = get_public_key(&recipient_sk).unwrap();

    c.bench_function("encrypt_empty_message", |b| {
        b.iter(|| {
            encrypt(
                black_box(&sender_sk),
                black_box(&recipient_pk),
                black_box(""),
            )
            .expect("Encryption failed")
        });
    });
}

fn bench_encrypt_unicode(c: &mut Criterion) {
    let sender_sk = generate_secret_key();
    let recipient_sk = generate_secret_key();
    let recipient_pk = get_public_key(&recipient_sk).unwrap();

    // Unicode message with emojis and various scripts
    let message = "Hello 世界 🌍 Привет мир مرحبا بالعالم";

    c.bench_function("encrypt_unicode", |b| {
        b.iter(|| {
            encrypt(
                black_box(&sender_sk),
                black_box(&recipient_pk),
                black_box(message),
            )
            .expect("Encryption failed")
        });
    });
}

fn bench_multiple_recipients(c: &mut Criterion) {
    let mut group = c.benchmark_group("nip44_multiple_recipients");

    let sender_sk = generate_secret_key();
    let message = generate_message(1_000);

    for num_recipients in [1, 5, 10, 20].iter() {
        let recipients: Vec<_> = (0..*num_recipients)
            .map(|_| {
                let sk = generate_secret_key();
                get_public_key(&sk).unwrap()
            })
            .collect();

        group.bench_with_input(
            BenchmarkId::from_parameter(num_recipients),
            num_recipients,
            |b, _| {
                b.iter(|| {
                    for recipient_pk in &recipients {
                        encrypt(
                            black_box(&sender_sk),
                            black_box(recipient_pk),
                            black_box(&message),
                        )
                        .expect("Encryption failed");
                    }
                });
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_encrypt_various_sizes,
    bench_decrypt_various_sizes,
    bench_encrypt_decrypt_roundtrip,
    bench_key_generation,
    bench_public_key_derivation,
    bench_encrypt_empty_message,
    bench_encrypt_unicode,
    bench_multiple_recipients,
);
criterion_main!(benches);
