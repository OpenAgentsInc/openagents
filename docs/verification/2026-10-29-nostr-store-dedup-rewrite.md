# Unverified Nostr frame deduplication primitive

This is a partial building block for #f539, not a verified store or admission
API. `ns_dedup_rewrite` accepts a bounded frame, candidate bytes and
caller-supplied per-record match flags. The flags are meaningful only if a
private owner has independently verified and protected the underlying
originals and correctly derived those matches from the candidate's verified
identity. This primitive does not verify an event, compute an ID, establish
ownership, produce an authorization token or expose a production C entry.

Within its live, immutable and physically disjoint views and truthful overlap
premises, it checks framing and workspace bounds, chooses the lexicographically
lowest exact byte representation among matching records and the candidate,
and retains every unrelated original in order. Existing duplicates for other
IDs are not compacted. The count and byte limits are preflighted before the
first output write, and any refusal leaves that output untouched. The
`duplicates` result counts matching old records. A later owner must provide
actual signature verification, identity matching, protected state, atomic
publication and fencing. Keeping a deletion original's bytes does not itself
implement NIP-09 authorization, tombstones, expiration or rollback recovery.

## Evidence

On the recorded frozen compiler, 24/24 Coax checks passed and the full
reference mutation gate killed 347/347 mutants with 1,603 checks, zero skipped
and no equivalence waivers. The initial mutation run found a survivor in an
unrelated-record multi-byte copy; the test was strengthened to check the full
bytes and the complete gate was rerun. An independent reviewer found no
concrete defect under the stated immutable/disjoint and caller-derived match
premises; that review is not store, security, deletion or persistence approval.
The reviewed source SHA-256 is
`31db25170a296ff4f6556b4512ecc7508245b60686958d757ac0e57edc14f43e`.

Thirteen independent framing cases pass in both checked and release modes on
native and Node/WASI WebAssembly, with ASan/UBSan in both modes. Cases cover
512 matching records, multi-byte unrelated records, too-small output,
malformed framing, flags-length mismatch, empty/oversized candidate, overlap,
and exact 1 MiB payload-quota growth refusal and shrinking replacement. The
Wasm test uses the web build's 1 MiB stack; it is not live-browser integration.
Fixture, compiler, source and target hashes are in
`data/2026-10-29-nostr-dedup-rewrite.json`.

Reproduce from the repository root with the matching verified compiler,
Clang/WASI, Node and sanitizers:

```sh
coax/bin/coax test coax/lib/nostr/store_dedup_rewrite.coax --run-steps 250000000
coax/bin/coax mutate coax/lib/nostr/store_dedup_rewrite.coax --run-steps 250000000 --jobs 2 --mutation-reference --mutation-stats
COAX_TEST_COMPILER=/path/to/verified/coax python3 coax/tests/nostr/store_dedup_rewrite_native.py
```

The separate verified-incoming stage remains local work and has not passed its
full mutation gate. Its preserved checkpoint and the original completed shard
records are described by the signed #f539 progress note. #f539, #8eb5 and
#966b remain open for their respective owner, authorized deletion and trusted
rollback-resistant recovery requirements.
