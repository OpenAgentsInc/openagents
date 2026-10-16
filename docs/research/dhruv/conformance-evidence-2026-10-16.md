# Protocol composition evidence, 2026-10-16

The [matrix](conformance-matrix.md) names exact tests for each supported role.
The composed runs use the existing Rust owners with isolated state and fake
funds. The sealed vault additionally ran once on a Google Confidential Space
TDX instance, with a synthetic file, a same-process client, and LocalKms.

## Funded labor through credit

`funded_labor.rs` follows the actual funded LAB ledger into a sent payout,
confirmed SOV income, an approved X402 top-up, durable account credit, and
metered use. A lost credit acknowledgment survives both treasury and credit
service restarts. Recovery uses the original hash: one invoice, one wallet
send, one income credit, and one usage charge. Wallet amounts stay in msat;
account credit and usage stay in USD micros.

This run exposed missing platform and payout fees reported as zero. SOV
now emits economic statement v2 with null unmeasured fees and partial
coverage for unknown fees or incomplete source pages. Legacy v1 remains
readable. No all-in compute, energy, or operating cost is claimed.

## Managed execution

The existing `session-host` composition uses the actual ENV allocator,
COORD authority, local resource, and SESS loop engine. Three focused runs
verify exact retries with one native turn, a lost native start across
restart with its original charge and unknown outcome, and revocation or
ENV drain blocking native continuations without another charge. These
are isolated fixtures, with no external engine or production account.

## Synthetic vault on real TDX

The `cloud_fixture` example ran from source
`b73efa585dcdafc9be0405981bcc9f1160b92e62` in image
`sha256:17ed47bc26e6f48edef97ea90b184223135dba885400d4451098d6ad5a20df2e`.
Its binary SHA-256 is
`ad3ed8f4ca15f193c6de7942ca19e8d14fa8d7bc5652e188e74ad63ee2c1339b`.
A fresh research publisher admitted the image after a real 600-second
notice; no production publisher or key was used.

The run verifies Google's PKI chain, the measured image, the release, the
signing key, and the HPKE session key before accepting a system certificate.
It seals a 20-byte synthetic file, opens it through a one-use lease, opens
the encrypted size-only reply, and checks the device copy. It refuses the
replayed request, a revoked lease, a signed replacement HPKE key absent from
the actual token, and expired evidence. The endpoint signs the completion,
including the exact object, request, and reply ciphertext hashes.

The integration review found that `oa-att::bind` previously refused every
HPKE-bearing endpoint. It now includes that key in the NIP-ATT binding. The
NIP-44 decision sender still refuses unsupported HPKE carriage.

Capture time: **2026-10-16 13:19:05 UTC**. Instance
`oa-vault-11749-tdx`, ID `8072841401835952884`, was deleted at
**2026-10-16 13:19:54 UTC**. Its private keys were not exported. The
[public capture](../../../crates/vault-sealed/tests/fixtures/confidential-space-vault-tdx-2026-10-16.json)
has SHA-256 `71d41e66f000af59daf94a9fca9bac553702327fe9c3651f336ca9a08ee0a067`.
`sealed_vault.rs` verifies that capture at its historical time, its system
certificate, and its signed completion. It rejects changed records,
certificates, ciphertexts, results, and expired evidence.

The client shared the workload's process and its key service was ephemeral
LocalKms. Remote-client delivery, production Cloud KMS policy, storage,
recovery after machine loss, independent rebuilding, and transparency-log
qualification remain separate. This run uses no real user file or money.
See [the remaining owner steps](../../../NEEDS_OWNER.md).

The [public evidence archive](evidence/protocol-composition-2026-10-16.tar.gz)
contains nine files: the capture, public release configuration, instance and
deletion records, exact source excerpts, build identities, and a manifest.
Its SHA-256 is
`efaf7d12d152f89b4001f41ab4bf3e1d1d2844ec2229bae3b6fdc6d03ac18f43`.
It includes no private key, credential, plaintext user share, or payment proof.

## Checks

`cargo test --locked -p dhruv-conformance -p vault-sealed` passes: eight
conformance tests and seven vault tests. The three focused `session-host`
compositions named in the matrix pass. The full affected `nostr`,
`sov-treasury`, and `oa-att` tests pass. Formatting passes for the changed
Rust crates. These checks require no production account or live payment.
