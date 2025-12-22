# Deeper Findings (Pass 5)

## Medium
- D5-M-1 MockRelay binds to port 0 but returns a URL built from an unrelated counter, so `start()` produces a URL that doesn't match the actual listening port and clients cannot connect. Evidence: `crates/testing/src/mock_relay.rs:69`, `crates/testing/src/mock_relay.rs:82`.
- D5-M-2 Trajectory hash calculation concatenates event JSON strings without any delimiter/length prefix, which makes the hash sensitive to ambiguous concatenation and non-canonical JSON serialization. This weakens integrity checks across different producers. Evidence: `crates/nostr/core/src/nip_sa/trajectory.rs:176`, `crates/nostr/core/src/nip_sa/trajectory.rs:187`.

## Low
- D5-L-1 MockRelay shutdown is a no-op, leaving background servers running and risking port conflicts across tests. Evidence: `crates/testing/src/mock_relay.rs:153`, `crates/testing/src/mock_relay.rs:159`.
- D5-L-2 Test fixtures use placeholder key/address values (zero public key, fake npub/bitcoin address), which can hide encoding/validation bugs in tests that rely on them. Evidence: `crates/testing/src/fixtures.rs:7`, `crates/testing/src/fixtures.rs:17`, `crates/testing/src/fixtures.rs:34`.
- D5-L-3 The random-text test is intentionally nondeterministic (`assert_ne!` on random values) and can flake in CI. Evidence: `crates/testing/src/fixtures.rs:114`, `crates/testing/src/fixtures.rs:119`.
