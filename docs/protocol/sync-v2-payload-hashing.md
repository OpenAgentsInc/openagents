# Sync v2 Payload Hashing Contract

Date: 2026-02-25
Status: Active

## Purpose

Define deterministic payload hashing rules for `openagents.sync.v2` envelopes.

## Hashing Rules

1. Hash algorithm: SHA-256.
2. Hash input: canonical JSON string of the hash-critical payload object.
3. Canonicalization rules:
   - object keys sorted lexicographically,
   - no whitespace in serialized output,
   - arrays preserve order,
   - scalar values serialized using JSON canonical representation.
4. Hash output format: `sha256:<lowercase-hex-digest>`.

## Fixture Authority

- `docs/protocol/testdata/spacetime_payload_hash_vectors.v1.json`

Fixture vectors cover representative sync v2 envelope payload classes:

1. subscribe request
2. subscribe applied
3. transaction update
4. stale-cursor error with recovery metadata
5. heartbeat

## Verification

Canonical verification test:

```bash
cargo test -p openagents-proto sync_v2_hash_vectors_match_canonical_json_and_sha256 -- --nocapture
```

