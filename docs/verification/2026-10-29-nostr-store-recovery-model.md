# Nostr store recovery: unconnected policy model

This is a partial result for `nostr-store-trusted-recovery` (#966b). The Coax
model is **not connected** to the Nostr event store, storage pipe, an
application, or a checkpoint provider. It does not read a checkpoint, compute
or authenticate a digest, verify events, create a writer fence, publish a
snapshot, or establish crash durability. No production recovery, authority,
or rollback-resistance approval follows. The existing owner must continue to
return `STORE_UNAVAILABLE` for restore unless an actual trusted recovery
implementation is separately established.

## What the model checks

A checkpoint has a 32-byte store identifier, a nonzero schema version, a
nonzero 64-bit generation and a 32-byte digest. The model compares every byte
of the identity and digest and refuses a mismatched schema or generation. A
candidate must preserve identity and schema and advance exactly once without
wrapping. Schema migration is intentionally not implemented: it requires its
own reviewed transition binding both formats and preserving deletion effects.

The abstract attempt can only progress through prepared, candidate durable,
checkpoint CAS submitted, exact candidate checkpoint observed and privately
staged. A read showing the old checkpoint while a CAS may still be in flight is
inconclusive and cannot authorize publication. An unrelated checkpoint fails
closed. The final predicate also requires an observation exactly matching the
candidate. Private staging is not externally visible installation.

These are pure value comparisons and ordering rules. The caller supplies the
observations: neither a plausible checkpoint record nor a positive predicate
establishes authenticity, currency, hash correctness, valid Nostr events,
physical memory lifetime or a writer lock. No caller-controlled Boolean is
considered evidence of a real fence. A directly fabricated state is not an
admission token. Do not expose the test adapter or use the model alone to
restore or publish.

## Required integration contract

A real implementation must establish and independently test all of these:

1. **Separate trusted state.** Provision a per-store identity and a real
   rollback-resistant checkpoint source. A hash, signature, generation, or
   checkpoint stored only alongside a replaceable snapshot cannot establish
   freshness or completeness. Bind the identity, schema, exact digest and
   nonwrapping generation in the trusted record, and specify who can create,
   replace, reset, delete or roll it back. Lost keys, provider rollback,
   administrator restore, migration and device replacement are explicit trust
   and availability decisions.
2. **Durable immutable candidate first.** Compute the reviewed domain-separated
   digest over exactly the retained bytes and metadata. The complete candidate
   must be durably written, immutable and retrievable before atomically
   advancing the checkpoint. The write response must be known durable under
   the actual backend's failure model. If a request times out or cancellation
   races commit, retain both candidate and prior data and resolve the unknown
   outcome against the trusted source.
3. **Real writer fence.** A cross-process, cross-device fence or equivalent
   atomic protocol must cover the authoritative linearizable checkpoint read,
   the moment a verified snapshot becomes externally visible, and the ACK,
   without a gap. A second read by itself is insufficient: another writer can
   advance the checkpoint after it. Staging stays private until the current
   checkpoint is checked under that real fence. The model cannot prove any
   fence exists or is honored.
4. **Trusted CAS and semantic update.** A conditional checkpoint advance is
   atomic and linearizable, comparing the entire expected identity/schema/
   generation/digest; concurrency losers cannot publish or ACK their stale
   work. Every accepted update must derive from the committed state and
   preserve the required authenticated originals and deletion effects. A
   higher generation alone does not prove semantic inclusion or tombstone
   preservation.
5. **Recovery fails closed.** Obtain a fresh authenticated checkpoint under
   the real fence, read the exact immutable candidate and recompute its digest,
   reverify every original and reconstruct indexes internally. Missing,
   unavailable, stale, corrupt, truncated, conflicting, uncertain or
   unsupported records do not replace a prior active frame and must not fall
   back to an older validly signed snapshot. An unavailable current snapshot
   can make recovery unavailable; durability depends on the provider and
   retention guarantees, not on the comparison model.
6. **Check the actual provider.** Test provisioning and permissions, exact
   commit/read semantics, uncertain outcomes, real concurrent writers,
   crash/restart at every transition, deleted or restored checkpoint data,
   truncation, migration, quota and deletion non-resurrection. Distinguish a
   mock from real provider evidence and record its security and target premises.

## Existing storage is not that provider

The shipped browser byte pipe, `coax/web/STORAGE.md`, offers independent
IndexedDB `put`, `get` and `delete`; it does not implement a trusted checkpoint,
a compare-and-swap, a cross-process writer fence or a rollback-resistant
record. Its own contract documents eviction, user deletion and uncertain
outcomes after cancellation or timeout. The host currently uses the default
IndexedDB durability. The [IndexedDB specification](https://www.w3.org/TR/IndexedDB-3/)
describes transaction durability as a user-agent hint; that is not an
independent monotonic witness. In addition, the byte-pipe record limit is
1,048,576 bytes, whereas the prototype's complete NSLG frame may exceed that
once record framing is included. A safe chunked format and complete-manifest
binding would require separate design and tests.

Apple's [Keychain accessibility documentation](https://developer.apple.com/documentation/Security/restricting-keychain-item-accessibility)
says a `ThisDeviceOnly` item can be restored to the same device. The documented
accessibility and device binding do not by themselves establish a monotonic
checkpoint or cross-process fencing. The [Secure Enclave key documentation](https://developer.apple.com/documentation/Security/protecting-keys-with-the-secure-enclave)
describes protected keys and operations, not a general app-accessible
rollback-resistant counter. Neither is claimed here as an implemented provider.

A service backed by a transactional database is a possible future provider,
but its client authentication, access control, rollback/restore authority,
durability, cross-device fence and failure behavior must be designed and
validated. For example, [Firestore documents serializable transactions](https://docs.cloud.google.com/firestore/native/docs/transaction-data-contention);
serializability alone does not prove that an operator cannot restore, reset or
replace the checkpoint, nor that a local snapshot is durable.

## Scoped checks

The checked-in test-only adapter and C fixture execute a finite, controlled
mock of immutable snapshots and an atomic monotonic checkpoint. They test
identity, schema, generation and digest mismatches, exhaustion without wrap,
invalid state-machine edges, stale and forged candidates, interruption points,
an old value while a CAS may be outstanding, conflict and a newer checkpoint
before final publication. The mock assumes the fence and provider properties;
it does not implement a real fence, concurrency primitive, persistent store,
event verifier or production recovery API.

Commands run on the source and compiler identities below:

```sh
coax/bin/coax test coax/tests/nostr/recovery_policy.coax
coax/bin/coax mutate coax/tests/nostr/recovery_policy.coax --mutation-reference
python3 coax/tests/nostr/recovery_policy.py
```

Seven Coax checks passed; full reference mutation killed 50/50 with no
waivers. The finite fixture reported 10,501 assertions for each checked and
release native ASan/UBSan and Wasm artifact. Generated C also compiled for
an iOS Simulator target; no iOS execution or physical-device test was done.
The test matrix is finite and does not establish actual power-loss behavior.
Python is test tooling only and is not required by the compiler bootstrap or
runtime.

- Compiler SHA-256: `9206daaec6f14afbdb1deee17e46d14efb7494aeb0b72fe71137a95f4e52c104`.
- Model SHA-256: `faa1b9b66f1e463af0251444bd1657eb31b4ed3a39dfbe02f1a0336465f28957`.
- Root wrapper SHA-256: `351c7bbab072d70d3161ff62b539c9928ff583031e2c13f6ae348561face038d`.
- Test adapter SHA-256: `b2b51539a325c85799f16ff106972222fa5e4410d6ec20d1ba1ac033d265595c`.
- C fixture SHA-256: `a875481aa3c671c9dfa754921849d05d965bad5c22ece0fa33a6057f197f18d6`.
- Test driver SHA-256: `e742f23856619752b75b1f1ab06b9e5ed9abb7f4fec9c7f96eff88b244d66075`.
- Coax test log SHA-256: `808aba3cde7f3c94d6a52c5247a3a119bc8224c9c3c6ca4583fbc0804a1b7e90`.
- Full mutation log SHA-256: `832ec1ade68ca44732a1c1d104d56737d7dc76ee150c1245254d5f8d3f981f51`.
- Cross-target log SHA-256: `a9f23786fcc13513f60c94aae17216d6a5c476ea24637df29b0843feaeae9920`.

An independent read-only review of the prior model identified that another
writer could advance a checkpoint after an observation and before installation.
The model was changed to require a matching final observation and private
staging; review also identified that the real fence must precede external
visibility and remain held through ACK. Neither review reran tests or approved
a provider, target timing, durability or production recovery.
A final independent read-only delta review covered model
`faa1b9b66f1e463af0251444bd1657eb31b4ed3a39dfbe02f1a0336465f28957`,
adapter `b2b51539a325c85799f16ff106972222fa5e4410d6ec20d1ba1ac033d265595c`
and fixture `a875481aa3c671c9dfa754921849d05d965bad5c22ece0fa33a6057f197f18d6`.
It found no new concrete issue in the abstract private-staging and final
checkpoint comparison under the stated independently enforced fence and host
premises. The reviewer did not rerun tests or approve a real provider, event
recovery, target behavior or production deployment.
