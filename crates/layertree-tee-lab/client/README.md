# LayerTree attestation client

Research-only client for the Google-backed TDX route selected in #11787.
It has its own workspace and stays outside the default build and signer
image. It verifies the pinned Google PKI chain, production/debug policy,
freshness, configurable TCB evaluation/date/build floors, exact image,
key and tree-parameter hash, signed release/head, Rekor inclusion and
checkpoint, rebuild statements, and the expected tree script and amount.
The caller supplies output bytes from its validated signet chain reader;
the pure verifier does not establish that supplied bytes are on chain.

`Binding::Live` requires three signed nonces in order: the report-data
hash, the client's fresh 32-byte challenge, and SHA-256 of the certificate
used on its TLS connection. `Binding::Recorded` is explicitly for historical
capture qualification. A captured single-nonce token cannot establish a
live channel. Witness state advances only after the entire check succeeds.

The TCB claims are Google's signed evaluation data, not a direct Intel
collateral check. Google defines `UpToDate` relative to its firmware rollout,
and says it does not guarantee fleet agreement with Intel's real-time
reference values. A policy can raise the minimum evaluation number,
attester date, and Confidential Space build independently. See
[Google's token claim definitions](https://docs.cloud.google.com/confidential-computing/confidential-space/docs/reference/token-claims).

```sh
cargo test --locked --manifest-path crates/layertree-tee-lab/client/Cargo.toml
# The bounded token parser and policy check are also a cargo-fuzz target.
cd crates/layertree-tee-lab
cargo +nightly-2026-08-21 fuzz run gcp_token_verify -- -runs=1000 -max_len=32768
```

Integration tests include the real five-member signet creation output,
confirmed at height 326595, and its three-nonce Google token, TLS peer
certificate, signed release, Rekor proof, and two rebuild statements.
The live client verified the output through its separate, fully synced
Bitcoin Core node. Offline tests replay that capture at its recorded time;
they do not establish a current chain view. Negative cases substitute the
image, enclave key, tree ID, delay, challenge, certificate, output amount
and script, TCB floor, time, and token signature. See the
[signet verification report](../../../docs/research/layertree/2026-10-16-signet-verification.md).

Public record files use the closed, unmerged [inline-file proposal
#345](https://github.com/nostr-protocol/nips/pull/345) at commit
`9ce5ab56710ee1686b8c7a6c793e7d402326229a`: base64 file event `1064` and
header `1065`, fetched by exact event ID. This is a research transport;
we do not claim official NIP-95 or relay feature support. Files contain only
`openagents.layertree.mirror.v1`, the tree ID, and the public record/unroll
bytes. Headers bind the file ID, MIME type, byte count, and SHA-256, with
`t=layertree:<tree-id>` and `r=<on-chain record hash>` for discovery.
Clients verify both event signatures, expected author/tree/hash, and the
file hash and bounds before passing bytes to the chain-anchored follower.
Publication requires acknowledgement and retrieval on a new connection.
It uses a separate research Nostr key and publishes no wallet credentials.


`export-signet PUBLIC_CAPTURES RELEASE OUTPUT.jsonl [NEGATIVE_VIEW.jsonl|--halt]`
rechecks the selected public captures at their recorded attestation times
and reads each actual transaction and confirmation through the separate
signet Core node (`LAYERTREE_CHAIN_COOKIE`). It exports only confirmed
normal operations, measured fees and sizes, and exact exit payouts. The
separate optional negative view changes the expected tree ID and checks
that the real captured token is refused; it never adds that refusal to
normal service history. Each output has a source list separating
confirmation height from the server's authorization tip. The exporter
reads no private test keys and sends no transaction.


`--halt` additionally checks the recorded guardian HALT, exact `Halted`
refusal, and every member's saved escape-package signatures. A waiting
view requires an unspent confirmed tree and the actual `non-BIP68-final`
rejection; it records the observed tip and first valid block without
claiming payouts. A paid view requires the original parent and fee child
to confirm, checks their complete bytes, CSV timing, independently computed
fees, the anchor spend, and every member's exact exit output. The source
file labels this as guardian HALT, rather than physical VM deletion.
