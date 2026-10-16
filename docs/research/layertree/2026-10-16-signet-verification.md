# Real signet output verification (2026-10-16)

The #11792 client verified a real five-member tree output on signet through
the Google-backed TDX route selected in #11787. The creation transaction
`4b4a748ca63476360a02e3355c687a626d76fd61a08e7aa7b97ea483eb3aa8b6`,
output 0, confirmed at height **326595** in block
`000000093ddefcc966004b298a3dd193c1af5f8564d1cdbac80e3f252f7cb306`.
Its amount is 5,000 signet sats. No mainnet funds were used.

The live client read that output through a separate, fully synced Bitcoin
Core 30.2 node. It rebuilt the Taproot script from the attested enclave key,
the five leaves' root, the unroll key, and the 12-block delay, then compared
both script and amount. The signer reported zero remaining ephemeral keys.

## Evidence and trust

The production Confidential Space 260900 VM runs image
`sha256:9377f95b0562ac6b147c00b4465370c428ab681a4a776a6994c9110c53740a11`,
built from `59b89bc1cda6434e33c17a99674ac70976473c9c`. Its enclave key is
`9166f15159248064b551aafd2c062ca78472b28736c936c833e3a4bd788042ec`.
The Google PKI token binds three ordered nonces: the exact key and tree
parameters' report-data hash, a fresh caller challenge, and SHA-256 of the
actual TLS peer certificate. Only after checking these does the client
make requests with that certificate pinned.

The same verification checks the publisher's signed release and head,
Rekor entry **3276968029**, its signed checkpoint and inclusion proof,
the elapsed release notice, and two signed rebuild statements. The local
and GCP builds produced identical OCI archives, SHA-256
`370c78ba3be7e1159e3fc9310cdb44380368d53496ea102d3b74b6cc14498c80`.
These are separate machines under the same operator; they do not establish
independent operators.

This route trusts Google's attestation service and PKI. It does not claim
direct Intel collateral verification or a qualified SEV-SNP application
image. The configurable floor requires evaluation data number 19,
attester date 2025-05-14 or later, and Confidential Space build 260900 or
later. Google's signed `UpToDate` status describes its firmware rollout;
it is not a real-time Intel collateral query. See the
[route decision](2026-10-16-attestation-routes.md) for this distinction and
the separately tracked vendor-direct work.

## Reproduction and checks

The [public fixture directory](../../../crates/layertree-tee-lab/client/tests/fixtures/signet-genesis/)
contains the token, caller challenge, peer certificate, tree snapshot,
Core transaction and block-header reads, verified output, release proofs,
and a per-file SHA-256 manifest. It contains no private keys, credentials,
or wallet data. Offline replay uses the recorded capture time; it does not
claim the capture is fresh today or independently resync the chain.

```sh
cargo test --locked --manifest-path crates/layertree-tee-lab/client/Cargo.toml --lib --tests
```

All five tests passed on the GCP builder. The real-capture cases reject a
different image, enclave key, tree ID, delay, challenge, certificate, output
amount or script, stale time, a raised TCB floor, and a tampered signature.
An output failure leaves the Rekor witness unchanged. The token parser is
also exercised by the existing `gcp_token_verify` cargo-fuzz target; its
previous 1,000-run bounded check passed. This change adds capture tests and
does not change that parser.

Join, update, exit, refresh, the week of heartbeats, deliberate enclave
loss, and final public-package recovery remain #11789/#11793 measurements.
This first confirmed output does not complete those measurements.
