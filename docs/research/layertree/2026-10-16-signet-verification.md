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

The normal transitions are recorded below. The week of heartbeats,
deliberate enclave loss, and final public-package recovery remain #11793
measurements. A confirmed creation output alone does not complete them.

## Confirmed normal transitions

All #11789 transitions confirmed on the same real TDX-controlled signet
tree. The independent Core reader measured these transactions:

| Operation | Confirmed height | Transaction ID | Fee (sat) | Size (vB) |
| --- | ---: | --- | ---: | ---: |
| Creation | 326595 | `4b4a748ca63476360a02e3355c687a626d76fd61a08e7aa7b97ea483eb3aa8b6` | 500 | 154 |
| Join | 326596 | `0d0742fb32a532e651c7d1703ef2fd7c8704af74631763c27c4ea9affa721358` | 500 | 255 |
| Update | 326597 | `0537056d4e066216b13528849755e292b20226d5e993baa7554f45633c4bed9c` | 500 | 255 |
| Exit | 326598 | `5cb3ae0ed3ae6d3bb28b30498bd87c1fc1b453edb31bd584c4254c5aa8ad2d22` | 500 | 298 |
| Refresh | 326599 | `aa46acc116e881b1f1c0b80846a07d0d1469d85350fe9f712a21f254ca256aa5` | 500 | 255 |

The join added a sixth 1,000-sat member. Update rotated member 0's exit
key; exit paid member 1, leaving five members and 5,000 sats in the tree.
The client checked the confirmed output with a fresh three-nonce token and
pinned TLS after each operation. The enclave stored the public record and
pre-signed escape transaction with at least two of three receipt-signing
mirrors before completing each transition. The three mirror VMs run in
zones a/b/c under the same operator.

All five records were also published to `wss://relay.openagents.com` and
read back on new connections. The client checked the exact event IDs,
signatures, publisher, tree ID, file hash, and record hash. This uses the
closed, unmerged inline-file proposal described in the client README;
it is not advertised as official NIP-95 support.

The [public capture archive](evidence/signet-transitions-2026-10-16.tar.gz)
contains 110 selected evidence files plus a SHA-256 manifest. Archive
SHA-256: `5d1154c212946d3159dc12b5c110fe85224d7070394bc4ec51025189ef70534e`.
The separate [confirmation and fee measurements](evidence/signet-transitions-2026-10-16.json)
include actual Core block hashes, outputs, and input-minus-output fees.
Server event heights record the authorization tip; use these independent
confirmation measurements for mined heights.

Create, inspect, or delete the primary VM with
`scripts/layertree/signet-service.sh create|status|delete`. Creation requires
`LAYERTREE_SERVICE_IMAGE` to name the approved digest-pinned research image.
The `halt` slot has a separate VM and a 24-hour lifetime; it does not restart
the funded primary service.

The first measurement began at **2026-10-16 11:01:32 UTC**, height 326599,
after all five publications passed. The second heartbeat confirmed at 326612,
seven blocks after the previous one. That misses the six-block target while
remaining inside the twelve-block exit delay. The first window stays in the
history as interrupted; it does not qualify as a clean week.

The revised observer submits a refresh after three blocks, polls every fifteen
seconds, and reserves the earlier part of each interval for publication retries.
It began a new full week at **2026-10-16 13:34:53 UTC**; the earliest completion
is **2026-10-23 13:34:53 UTC**. It retains every late heartbeat, restarts an
interrupted measurement, and preserves a completed clean week through later
misses. Three isolated window-state tests pass. The enclave VM, Q, native
research binary, 500-sat refresh fee, and kill deadline are unchanged. See the
[measurement record](evidence/signet-heartbeat-window-2026-10-16.json).
A one-time Cloud Task will
delete the primary VM at **2026-10-24 09:28:00 UTC**, with a nine-day native
VM deadline as fallback. #11793 still requires the elapsed week, deletion
audit evidence, CSV maturity, exact payouts, and the separate HALT run.

The separate HALT run uses committed `server/halt-service.json`, with fresh
test keys and 20,000 signet sats. Its image
`sha256:33b4bda396fede6c1a3fe4e3c37c07967f2c6a8ee81088b238ba5fd1319ee436`
from `6fb4b4e71d4fb21d8bbd008ee5a4775cf0cbdea6` reproduced on both builders
(OCI archive SHA-256
`d74332efaaeb45fed4bb32c4d32ab59b634437df7ca3f1b7010f9ece48f32efe`).
Rekor entry is 3278267593. The verifier rejected the release before its
actual 10-minute notice elapsed. Its separate VM had a 24-hour native
deadline. `scripts/layertree/observe-halt.py` waits for sync, funding
confirmation, and release admission, then checks HALT signing refusal and
runs saved-package recovery through the separate Core node. The actual guardian HALT occurred at height **326605**: the attested state
reported `halted: true`, zero ephemeral keys, and the exact `Halted` refusal
for a subsequent refresh. Mirror publication received an ACK and passed
readback over a fresh connection.

Saved-package recovery is now complete. Core refused the original escape
with `non-BIP68-final` at tip 326614, one block before admission for the
first valid block, **326616**. The original escape transaction
`4cff145e6592bd1899f015b9187a6957541752a9c56eff1fc558a9d0c7fea747`
and its fee child both confirmed at **326616**. All five members received
exactly **1,000 sats** at their original exit scripts. The parent fee was
zero; the separate anchor child paid **934 sats**. No member amount funded
that fee.

The independent exporter checked the historical Google-backed TDX token,
release and rebuild records, every member's saved package signature, exact
consensus transaction bytes, CSV timing, child anchor input, fees, and
confirmed payout scripts and amounts. It rejected four altered captures:
wrong HALT refusal, delay, fee, and child transaction. See the
[negative checks](evidence/halt-paid-export-checks.json).

The [final public archive](evidence/signet-halt-paid-2026-10-16.tar.gz)
contains 62 files plus a SHA-256 manifest, including the public capture,
release proofs, raw Core transaction and header reads, confirmed unspent
member outputs, and exporter identity. Archive SHA-256:
`e6c903a792a2eaa445ae8b2d5c3f86a31312fa0f315f93e78e20a72e0fe3536e`.
The [14-event signet fixture](../../../crates/layertree-tee-lab/fixtures/events/signet-halt-paid.jsonl)
pays 5,000 sats; the earlier waiting capture remains available.

![Five exact payouts after the guardian HALT](captures/signet-halt-paid.jpg)

The 1920×1080 still was visually checked. The corresponding H.264 clip,
`~/work/film-out/verse/layertree-signet-halt-paid.mp4`, is eight seconds at
24 fps and CRF 12; `ffprobe` confirmed its dimensions and duration. Both a
[film shot](../../verse/film/verse-shots/layertree-signet-halt-paid.json)
and a [Video Lab scene](../../verse/film/layertree-signet-halt-paid-scene.json)
are committed. Re-render with `scripts/layertree-captures.py --signet-halt-paid`.

This measures guardian HALT followed by public-package recovery. VM cleanup
follows payout verification; it does not establish the primary tree's
scheduled physical-loss recovery or its required week of heartbeats.

After all payout and archive checks passed, GCP deleted only
`oa-layertree-halt-tdx` (instance ID `4848185565226450307`). A subsequent
instance inventory returned no matching VM. The primary service, mirrors,
and shared builder/Core remain running.
