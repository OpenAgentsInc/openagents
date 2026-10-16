# layertree-tee-lab

A research sandbox. It holds LayerTree's off-chain tree and transition
rules, enforced by a modelled TEE co-signer on today's Bitcoin, with a
pre-signed, timelocked unroll that needs no enclave. The spec is
[`docs/research/layertree/2026-10-15-mainnet-tee-spec.md`](../../docs/research/layertree/2026-10-15-mainnet-tee-spec.md).

- **Outside the default build.** The crate has its own `[workspace]`, and
  the root `Cargo.toml` lists it under `exclude`. Nothing ships in a Coder
  binary.
- **No upstream code.** It is written from LayerTree spec v0.15.2, read at
  `lightninglabs/layertree` commit `f6c4d29`, because the repository has no
  licence. One opt-in test reads upstream's published vectors from a local
  reference clone at test time.
- **Signet and regtest research only.** The model and tests use test keys. Opt-in chain and mirror clients connect where the harness points them. The attestation probe generates a key inside Confidential Space and exposes only its public key and token; it signs no Bitcoin transaction.

```sh
cargo test --manifest-path crates/layertree-tee-lab/Cargo.toml
# the opt-in cross-check against upstream's vectors:
LAYERTREE_VECTORS=~/work/projects/repos/layertree/spec/test-vectors.json \
  cargo test --manifest-path crates/layertree-tee-lab/Cargo.toml -- --ignored
```

| Module | What |
| --- | --- |
| `tree` | leaf, Merkle-sum nodes, dense tree, proofs, vouchers, path maintenance |
| `codec` | byte decoders: `dec64`, `Leaf::parse`, `Record::decode`/`encode` (the bytes the anchored hash covers), the unroll package wire form |
| `output` | the tree output: `Q + H_state(r)·G` key path, the unroll tapleaf, `TREE_SPK_CCV` |
| `predicate` | pure edit accounting, voucher terms, and exact output checks shared with Kani |
| `rules` | the Bitcoin and BIP-340 adapter, including Merkle witnesses and one `Reject` per rule |
| `enclave` | the in-process co-signer: its key never leaves, and it signs only what passes; two-phase signing (`prepare`, `finish`), forced inclusion of exit requests, mirror receipts before any signature, and the guardian's `HALT` |
| `mirror` | record mirrors: the `Mirror` trait, an in-memory mirror, and the `m`-of-`M` receipt policy |
| `unroll` | the delayed exit with deleted intermediate keys, checked with public data only |
| `attest` | a fake hardware report and the light client's attestation check |
| `light` | a member following the tree from chain data and anchored records |
| `follow` | that member on a real chain: BIP-158 filters find the spend, a record source (directory or HTTP mirror) supplies the record, accepted only if it hashes to the `OP_RETURN`; alarms `RecordNotAnchored`, `RecordMissing`, `UnrollNotHeld` and the rule-breaking ones |

`tests/lean_mirrors.rs` has property tests named after upstream's Lean
theorems (`ExitK_no_inflation`, `Steps_sum`, `Reachable_ledger`, `payable`,
`exit_available`, `voucher_no_replay`, `path_maintenance` and others).

## Bounded proofs

`src/proofs.rs` (#11783) checks the monetary functions that `predicate`
uses before `rules` signs a Bitcoin transaction. Kani 0.68.0 verifies:

| Harness | Scope |
| --- | --- |
| `ExitK_no_inflation` | Every valid leaf amount, arbitrary `u64` tree balance and proposed amounts, arbitrary 34-byte payout scripts; includes terminal exits |
| `exit_available` | Exact amount and all 32 bytes of the exit key |
| `Steps_sum_Reachable_ledger` | Up to three accepted edits, with arbitrary `u64` balances and old/new amounts |
| `voucher_no_replay` | Every valid amount and `u32` sequence; a partial claim advances the sequence, a full claim removes the source, and the final sequence refuses |
| `checked_addition_never_wraps` | Every pair of `u64` Merkle-node sums |
| `script_equality_checks_every_byte` | Arbitrary 34-byte scripts and every byte position |

The proof boundary excludes tree lookup, Merkle witness construction,
Bitcoin script construction, and real cryptography. Hash results are
unconstrained, and key and voucher validity are symbolic; the arithmetic
proofs need no hash-injectivity assumption. The existing property tests
still check tree roots, paths, and complete histories. The Bitcoin adapter
replays the edits and refuses a resulting root sum that differs from the
proved accounting. These are scoped proofs, not a proof of the entire signer.

```sh
cargo install --locked kani-verifier --version 0.68.0
cargo kani setup
scripts/layertree-kani.sh
scripts/layertree-kani-mutation.sh
# equivalent proof command:
cargo kani --manifest-path crates/layertree-tee-lab/Cargo.toml --lib -Z stubbing
```

The mutation command copies the crate into the scratch directory, raises an
exit payout by one satoshi, and requires the named no-inflation assertion
to fail. It leaves the checked-out source unchanged. A manual Cloud Build
job is scoped to these proofs and the mutation check in
`deploy/layertree/kani-cloudbuild.yaml`; no GitHub automation is used.

## Fuzzing

Everything the enclave and light clients read from outside has a decoder
that returns a typed `codec::DecodeError` and never panics, and each is the
exact inverse of its encoder. `fuzz/` (its own workspace, nightly
`cargo-fuzz`) has seven targets: `leaf_roundtrip`, `enc64_roundtrip`,
`record_decode`, `unroll_package_decode` (roundtrips assert
`encode(decode(x)) == x`, so the encodings are injective), and
`snp_report_parse` and `tdx_quote_parse`, which fuzz the SEV-SNP report and
TDX quote v4 parse layers in `crates/att-platforms` (#11745) rather than a
second copy. `gcp_token_verify` fuzzes the bounded Google token verifier
and both recorded and live-binding policies in the [research client](client/README.md).
Seeds in `fuzz/corpus/*/seed-*` come from lab data and the
synthetic att-platforms vectors (test keys only; regenerate with
`LAYERTREE_WRITE_SEEDS=1` and `ATT_FUZZ_SEEDS=<corpus dir>`).

```sh
cargo install cargo-fuzz
cd crates/layertree-tee-lab
cargo +nightly fuzz build
cargo +nightly fuzz run record_decode -- -max_total_time=1800
# likewise leaf_roundtrip, enc64_roundtrip, unroll_package_decode,
# snp_report_parse, tdx_quote_parse, gcp_token_verify
```

A crash becomes a regression test in `tests/codec.rs` (or att-platforms'
tests for the report parsers).

## Regtest: the same transitions on a real node

`scripts/layertree-regtest.sh` (#11781) starts Bitcoin Core 30 on regtest
in Docker (pinned image digest; TRUC, pay-to-anchor and ephemeral-dust
relay), runs the ignored harness in `tests/regtest.rs` (feature `regtest`,
no extra dependencies; plain JSON-RPC over HTTP), and removes the container
at the end. It uses the host's cargo, or builds in a pinned Rust image when
the machine has only Docker. It exits non-zero on any failure.

```sh
scripts/layertree-regtest.sh            # logs go to a fresh scratch dir
LAYERTREE_REGTEST_ROOT=/some/scratch LAYERTREE_WRITE_EVENTS=1 scripts/layertree-regtest.sh
```

For a flat tree (radix 128) and a two-level one (radix 4) it:

1. builds the creation transaction, has every member check the genesis
   unroll, then broadcasts it;
2. runs join, update, claim, exit and a refresh heartbeat through
   `Enclave::authorize`, each with a fee input and change, mined in its own
   block; after each, the tree output on chain equals `TreeState::txout`,
   the old one is spent, the exit payout is exact, every member re-checks
   the new unroll, and a light client follows the spend;
3. drops the enclave, submits the unroll with a CPFP child on its anchor
   one block early (refused: `non-BIP68-final`), then one block later, and
   checks it is mined exactly Δ blocks after the tree output; deeper levels
   follow, one CPFP package per transaction;
4. checks every leaf's exit key holds exactly its amount, and that the
   `layertree.events.v1` log replays. `fixtures/events/regtest.jsonl` and
   `regtest-two-level.jsonl` are logs from such runs.

A third test (#11782) runs three `follow::Follower`s (two on a directory,
one on an HTTP mirror stub) through 20 honest transitions, checking their
paths against the full tree after every block, then a withheld record
(`RecordMissing` after 3 blocks), a forged one (`RecordNotAnchored`), a
withheld unroll (`UnrollNotHeld`) and a mined theft by a broken enclave
(`OutputMismatch` for every member in that block). Its checked-in
`fixtures/events/regtest-light.jsonl` also records the mined theft block and
`observed_output_amount` (output 0 as read from Core, separate from the
signed record's claimed sum) for
the [Verse captures](../../docs/research/layertree/captures/README.md). The node runs with
`-blockfilterindex=1`. `tests/follow.rs` runs the same follower against an
in-memory chain with real BIP-158 filters.

### Measured on regtest (#11785)

Bitcoin Core 30.2, 2026-10-15, Δ = 144, radix 128, 1,000 sat leaves, every
unroll transaction and CPFP child broadcast with `submitpackage` (package
feerates 1, 10 and 50 sat/vB in turn) and mined
(`unroll_timing_sizes_and_heartbeat_fees_measured`; the numbers also go
into the run log as `measured` lines).

| Leaves at unroll | Levels | Transactions per level | Largest transaction per level (vB) | Blocks from the last tree output to the root's block | Then blocks until every leaf is paid |
| --- | --- | --- | --- | --- | --- |
| 128 | 1 | 1 | 5,604 (128 payouts + anchor) | 144 (= Δ) | 0 |
| 16,384 | 2 | 1, 128 | 5,604 (128 groups); 5,585 (128 payouts) | 144 | 1 |
| 16,385 | 3 | 1, 2, 128 | 186 (2 groups); 5,585; 5,585 | 144 | 2 |

- The root is refused one block early (`non-BIP68-final`), accepted
  the block after, and mined exactly Δ blocks after the tree output.
  TRUC lets an unconfirmed parent have one child, which the CPFP takes,
  so each further level waits one block: Δ + (levels − 1) blocks in all.
- A full level is 5,604 vB at the root (script-path input) and 5,585 vB
  below it (key-path input), about 43.7 vB per leaf, well inside TRUC's
  10,000 vB; `tests/unroll.rs` fails if it ever is not.
- The CPFP child (anchor plus one key-path coin, one output) is 153 vB.
  Carrying a full level costs 5,757 sat at 1 sat/vB, 57,570 sat at 10 and
  287,850 sat at 50; the 186 vB root of a three-level unroll costs 339,
  3,390 and 16,950 sat.
- A `Refresh` heartbeat is 255 vB and a key-path exit 298 vB, both with
  one fee input and a change output (the bare tree spend is less).

| Δ (blocks) | Heartbeats a week (every Δ/2) | 1 sat/vB | 10 sat/vB | 50 sat/vB |
| --- | --- | --- | --- | --- |
| 144 | 14 | 3,570 sat | 35,700 sat | 178,500 sat |
| 288 | 7 | 1,785 sat | 17,850 sat | 89,250 sat |
| 1,008 | 2 | 510 sat | 5,100 sat | 25,500 sat |

`cargo test` without `--features regtest` needs no Docker.

## Run log: `layertree.events.v1`

One JSON Lines file per run, written by the lab and the regtest/signet
harnesses, read back by light clients, and replayed by the Verse scenes
(`events` module, behind the `events` feature so the predicate stays
dependency-light). Every line starts with `seq` (contiguous from 0),
`height` (a block height, or null off chain), `network` (`lab`, `regtest`
or `signet`, the same on every line) and `kind`. Hashes and keys are
lowercase hex; outpoints are `txid:vout`.

`events::read` replays a log and refuses an unknown `kind` or field, a
`seq` gap, a second network, a transition whose old leaves, paths or roots
differ from the replayed tree, a transition that creates or destroys value
for its kind (only `join` adds, only `exit`/`evict` remove one leaf), and a
payout no exit or unroll owes. At the end, value in the tree plus value
owed plus value paid must equal everything that entered the tree.
`events::Writer` builds logs from lab types. Fixtures under
`fixtures/events/` (`happy.jsonl`, `theft-caught.jsonl`, `unroll.jsonl`)
hold test keys only; `LAYERTREE_WRITE_EVENTS=1` regenerates them.

One example per kind (keys and hashes shortened):

```text
{"seq":0,"height":100,"network":"regtest","kind":"tree_open","tree_id":"e236…8c82","depth":8,"dust_min":330,"unroll_delay":144,"enclave_key":"3733…3bd6","outpoint":"1bd1…f741:0","leaves":[{"pos":0,"leaf":{"amount":10000,"exit_key":"9799…e660","owner_key":"5624…7c0e","evictor_key":"ab07…6c18","evict_after":1000,"nonce":0,"seq":0}}],"root":{"hash":"0c1d…77aa","sum":10000}}
{"seq":1,"height":null,"network":"regtest","kind":"attestation","platform":"sev-snp","measurement":"0707…0707","log_entry":{"measurement":"0707…0707","source_commit":"abc123"},"result":"attested"}
{"seq":2,"height":101,"network":"regtest","kind":"transition","op":"exit","prev":"1bd1…f741:0","changes":[{"pos":1,"old":{"amount":20000,"exit_key":"95dc…9e19","owner_key":"f40c…9bba","evictor_key":"ab07…6c18","evict_after":1000,"nonce":1,"seq":0},"new":null,"path":[{"right":true,"sibling":{"hash":"ed3a…","sum":10000}}]}],"old_root":{"hash":"0c1d…","sum":210000},"new_root":{"hash":"fae8…7315","sum":190000},"txid":"5a1e…","record_hash":"9b2c…","next_unroll_key":"77f0…"}
{"seq":3,"height":101,"network":"regtest","kind":"payout","exit_key":"95dc…9e19","amount":20000}
{"seq":4,"height":null,"network":"regtest","kind":"rejected","op":"evict","reject":"Tree(PositionEmpty)"}
{"seq":5,"height":101,"network":"regtest","kind":"light_client","member":4,"result":"ok"}
{"seq":6,"height":102,"network":"regtest","kind":"block","spent":["1bd1…f741:0"]}
{"seq":7,"height":103,"network":"regtest","kind":"enclave_down"}
{"seq":8,"height":247,"network":"regtest","kind":"unroll_broadcast","level":0,"txid":"c0de…","cpfp_fee":1000}
{"seq":9,"height":248,"network":"regtest","kind":"unroll_confirmed","level":0,"txid":"c0de…"}
{"seq":10,"height":248,"network":"regtest","kind":"measured","what":"unroll_tx_vsize_max","value":5604,"unit":"vB","level":0}
```

`light_client.result` is `ok` or a `LightError` (`OutputMismatch`,
`RecordNotAnchored`, …); `attestation.result` is `attested` or an
`AttestError`; `rejected.reject` names the `Reject` variant.

## Reproduced signet image

The attestation probe has a source build with pinned Rust, Debian, and
BuildKit images. It runs no package-manager install during the build.
The script uses a persistent BuildKit cache, fixed paths, a committed
source tree, and that commit's timestamp. Buildx 0.31.1 was tested against
Docker 29 locally and Docker 20 on the GCP builder.

```sh
ref=2bc0a8f11e79da4aaf3b70e19f43152a9b429108
out="$(openagents scratch)/layertree-rebuild"
scripts/layertree/build-image.sh "$ref" "$out"
cat "$out/build.json"
sha256sum "$out/image.oci.tar"
```

Run this on two separate machines with separate caches. The October 16
local and GCP builds both produced image
`sha256:97ebee658897e3e655c18e01c043b6ab9394ff63ab06b056aea6a6db0127671f`
and byte-identical OCI archives, SHA-256
`bee4ba8b56274b8798efea44b13bbe03b56b19aceef26ca99cc5308e8a2ea62a`.
Both machines were operated by the same researcher; this is not an
independent operator audit.

[Public release evidence](../../docs/research/layertree/evidence/reproduced-image-2026-10-16.tar.gz)
contains the NIP-ATT release and head, two separately signed machine
rebuild statements, Rekor entry **3275843513** with its signed checkpoint
and inclusion proof, and the real TDX token naming the image. The
research publisher is
`fc4d6cd08891e19627bca90ae667097d261b8290e54ada0bb7e959f06a390f8d`;
it admits only workload `layertree-signet-probe`. It is separate from the
production attestation publisher. The signed release's source commit and
recipe hash identify exactly what to rebuild.

From the checkout root, extract the archive into your scratch directory
and verify it without contacting a relay or log:

```sh
repo=$PWD
evidence="$(openagents scratch)/layertree-release-evidence"
mkdir -p "$evidence"
tar -xzf docs/research/layertree/evidence/reproduced-image-2026-10-16.tar.gz -C "$evidence"
cargo run --locked -p att-release -- verify \
  --release "$evidence/release.json" --head "$evidence/head.json" \
  --publisher fc4d6cd08891e19627bca90ae667097d261b8290e54ada0bb7e959f06a390f8d \
  --workload layertree-signet-probe --now 1792137325 \
  --rekor "$evidence/rekor-entry.json" \
  --log-key "$repo/crates/att-release/vectors/rekor.sigstore.dev.pub" \
  --witness "$evidence/witness.json" \
  --rebuild "$evidence/local-rebuild.json" --rebuild "$evidence/gcp-rebuild.json" --min-rebuilds 2
cargo test -p att-release --test layertree_signet
cargo test -p oa-att --test layertree_probe
```

The fixed time tests the recorded release policy after its 600-second
notice; the token fixtures use their own recorded validity windows. They
do not bypass freshness checks for a running service.

To publish another research image, set `LAYERTREE_IMAGE_TAG` to a tag in
the `layertree-research/attestation-probe` registry before running the build
script. `att-release`'s `layertree_release` example prepares the release's
canonical body and checks its real Rekor proof before signing the release
and head. Its arguments accept private key *files*, never secret values;
keep those files outside the repository. The existing `att-release rebuild`
command signs each builder's matching result. Publication to Rekor is a
separate explicit step using the prepared public body and a detached
signature. The checked-in example neither publishes nor activates a
payment service.

The bounded hardware check uses the resulting immutable digest:

```sh
export LAYERTREE_ISSUE=11791
export LAYERTREE_PROBE_IMAGE=us-central1-docker.pkg.dev/openagentsgemini/layertree-research/attestation-probe@sha256:97ebee658897e3e655c18e01c043b6ab9394ff63ab06b056aea6a6db0127671f
scripts/layertree/attestation-probe.sh create tdx-space
scripts/layertree/collect-probe.py oa-layertree-tdx-space-probe "$out/probe"
scripts/layertree/attestation-probe.sh delete tdx-space
```

Wait for the workload to finish before collecting. Verify the token's
chain, audience, validity, image digest, and recomputed key/parameter hash;
`collect-probe.py` only extracts the public evidence. The October 16 check
passed and its VM was deleted. Key sealing and the real signing service
remain #11790 and #11789. The [route decision](../../docs/research/layertree/2026-10-16-attestation-routes.md)
states the Google trust boundary and the unqualified SNP alternative.

## Key generation and restart recovery on TDX

The `gcp-sealing` feature generates `Q` inside the guest and seals it
without exporting its plaintext. AES-256-GCM encrypts the enclave key
locally with a fresh random envelope key. Cloud KMS wraps that envelope
key under the key for this research release, `enclave-envelope-v2`. The
sealed record binds the public key, every report-data parameter, and the
unroll radix. A restored key starts with no tree state; the service must
reload the chain, mirrors, exit requests, and HALT before accepting work.

Cloud KMS admits only the approved image's federated identity. The
workload service account has no direct decrypt grant. The identity pool
requires production Confidential Space, TDX, debug disabled since boot,
and our project. This route trusts Google attestation, KMS, and the IAM
administrators who can change its policy. A service upgrade needs a new
wrapping key and policy; it does not gain the previous image's key.

The October 16 hardware check used source
`a4fb41ec1f4e2f3ba8a281ad1cb9881c872d7632` and image
`sha256:79bf22d374f8c33a58f4c126ce36da9f729501d881ac80b48707d007653bf100`.
Two build machines produced that digest. Two separate production-mode
TDX boots reported the same public key:

```text
a90d75983b148a0d5f684e6bcc018bd55a95237854ca0c8ab77ec4df57438c0d
```

The first boot generated it; the second recovered it from the encrypted
record. Both Google tokens pass Rust verification and independently
recomputed parameter binding. Both workloads ran a synthetic refresh
through the real predicate and reported zero ephemeral keys afterward.
A derivative image with only a changed label was refused by the key
service when it tried to recover the same record. These are test keys;
no tree on signet or mainnet uses them.

[Public key-recovery evidence](../../docs/research/layertree/evidence/key-recovery-2026-10-16.tar.gz)
contains both boots, the refusal, KMS and identity policies, independent
build records, and release evidence. Rekor entry **3276247676** logs the
release `4963b7fcc2180c2a1afa4fe28fd5a0ced13b6527a7ad75c8ea0eb72b98274552`
for workload `layertree-signet-key-probe`; `att-release verify` checks its
proof and both rebuild statements offline. Use the research publisher
above and the archive's head and release, with recorded time `1792139163`.

An initial prototype's JSON-formatted environment allowlist prevented
its second workload from launching. The corrected image uses the
comma-separated launch-policy format. That failure is retained, its
unfunded wrapping key is disabled, and all five key-experiment VMs were
deleted. Estimated probe compute is below $1 at the issue's approximate
rate, excluding the builder, storage, logs, registry, and KMS; this is not
a billing export.

The rollback check uses a separate Bitcoin Core 30.2 regtest node. It
recovers the key, deliberately supplies old public tree state, and signs
a different refresh with its own unspent fee coin. Core accepts the
transaction before the tree input is spent. After another valid refresh
confirms, Core rejects it as `missing-inputs` while its fee coin remains
unspent. Bitcoin's spent output prevents the stale state from taking
effect. The local envelope in this unit test checks rollback separately
from the real KMS and hardware checks above.

```sh
cargo test --locked --manifest-path crates/layertree-tee-lab/Cargo.toml --features gcp-sealing
LAYERTREE_ROLLBACK_ROOT="$(openagents scratch)/layertree-key-rollback" \
  scripts/layertree/sealing-rollback.sh
cargo test -p oa-att --test layertree_probe
```

To reproduce the hardware check, run
`LAYERTREE_EXAMPLE=key_lifecycle_probe scripts/layertree/build-image.sh a4fb41ec1f4e2f3ba8a281ad1cb9881c872d7632 "$out/keys"`, then run
`scripts/layertree/sealing-policy.sh setup`, and admit the matching digest
with `sealing-policy.sh admit sha256:...`. Launch and collect the first
probe as above with `LAYERTREE_ISSUE=11790`. Save its `sealed` JSON field
to a scratch file, delete the VM, then set `LAYERTREE_SEALED_BLOB_FILE`
to that file before launching the second VM with the same image. Delete
that VM after collection. A different-image check uses the same encrypted
record and a derivative image that has no KMS admission. Never add that
image to the key policy. The saved tokens, key equality, and refusal must
all be checked before calling recovery successful.

This probe accepts no external signing requests. The actual signet
service, current-chain restoration, attested client channel, and public
tree runs remain #11789, #11792, and #11793.

## Signet service

The separate [TDX service](server/README.md) validates signet with Core inside
the guest and uses the existing predicate, mirror receipts, and pre-signed
exits. Its [client](client/README.md) verifies a fresh challenge bound to the
actual TLS certificate before submitting an operator-signed request. The
research CLI keeps its private test keys outside the repository. A service
restart creates a fresh key; the old tree exits through its saved unroll.
## Live followers (#11794)

The opt-in `layertree-follow` program reads signet or regtest only. It has
no wallet or signing endpoint. A bootstrap configuration supplies the public
tree-creation event, operator and unroll public keys, and the initial signed
unroll package. The program recomputes the root, checks the member's path and
exact escape payout, and compares the creation output with Core's confirmed
block. Hardware-attestation verification remains the separate P1 verifier.

Records can come from directories, HTTPS mirrors (loopback HTTP for tests),
or signed public Nostr notes. The research mirror profile uses regular
NIP-01 kind `1`, not NIP-78's owner-only app data. Its exact tags are
`["t", "layertree-mirror-v1"]` and `["d", FILE_NAME]`. The signed content is
`{"schema":"layertree.mirror.v1","name":FILE_NAME,"bytes":HEX_BYTES}`.
File names are the existing `record-HASH.bin` and `unroll-TXID-VOUT.bin`.
Readers pin the mirror publisher and check the event ID, signature, kind,
tags and body before decoding. A publisher identifies the source; Bitcoin's
record hash and the member's checked unroll determine what it accepts.

The live signet controller in `client/` uses the experimental inline-file
proposal, kinds `1064` and `1065`, described in its README. Set
`"inline_file": true` and `"tree_id": TREE_ID_HEX` on that relay entry to
read its existing publications. Set `"auth_env": ENVIRONMENT_VARIABLE_NAME`
when the relay requires NIP-42; that variable holds a dedicated test reader
key. The follower verifies both signatures, header/file binding, SHA-256,
size, tree ID, and record hash. It fetches each blob by the signed header's
exact event ID. Its inline-file limit is 32 KiB, matching the controller.

The transport bounds a query to ten seconds, 32 data messages, and
400,000 decoded bytes per mirror file. Records and checked unrolls are
cached locally. A valid first source avoids waiting on unavailable backup
mirrors. Public relay reads need no private key. These notes contain public
experiment data; use a dedicated mirror identity for the test.

Each process owns a separate state directory. Its observation log and
webhook outbox survive restarts; webhook receivers deduplicate by the
`Idempotency-Key` header, which is also the alarm's `id`. A successful HTTP
response is recorded before removing that alarm from the outbox. Failed
notifications remain queued and do not stop chain checks. Source read
failures leave the follower unchanged, and a changed chain triggers replay
from the checked creation output. A late record causes its block and later
blocks to be scanned again. Held unroll transactions are followed through
the member's exact payout without treating that exit as a missing record.

Build and test the standalone program:

```sh
cargo build --locked --manifest-path crates/layertree-tee-lab/Cargo.toml --features follow-live --bin layertree-follow
LAYERTREE_REGTEST_ROOT="$(openagents scratch)/follower-process-check" \
  LAYERTREE_REGTEST_FEATURES=regtest,follow-relay-tests \
  LAYERTREE_REGTEST_FILTER=independent_follower_processes_detect_theft_and_notify \
  scripts/layertree-regtest.sh
```

A configuration has this shape. Set `LAYERTREE_RPC_USER`,
`LAYERTREE_RPC_PASSWORD`, and the selected webhook environment variable
outside the file. Paths resolve relative to the configuration file. Use
three different members and state directories. Set up a local Core with
`blockfilterindex=1`; forward remote private HTTP endpoints to loopback,
or serve them through HTTPS.

```json
{
  "schema": "layertree.follower.v1",
  "network": "signet",
  "rpc": "http://127.0.0.1:38332",
  "tree_open": "creation.jsonl",
  "operator_key": "OPERATOR_PUBLIC_KEY_HEX",
  "unroll_key": "INITIAL_UNROLL_PUBLIC_KEY_HEX",
  "unroll": "initial-unroll.bin",
  "member": 0,
  "sources": {
    "directories": [],
    "https": ["https://MIRROR_HOST/public-tree"],
    "relays": [{"url": "wss://RELAY_HOST", "publisher": "MIRROR_PUBLIC_KEY_HEX"}]
  },
  "state": "member-0",
  "webhook_env": "LAYERTREE_TEST_ALARM_WEBHOOK",
  "poll_seconds": 10,
  "missing_after": 3
}
```

Core selects and validates the signet chain. The follower checks header
continuity, fetched block hashes and Merkle roots, records, member paths,
and payouts; it does not implement independent chain selection or signet
consensus. Independent processes are permitted by #11794; this launcher
uses three processes on one machine. It does not make three independent
chain sources out of a shared Core.

Run one check with `layertree-follow --config FILE --once`, or use
`scripts/layertree/followers.py run --binary PATH --until UNIX_SECONDS
--output NEW_LAUNCH_DIRECTORY MEMBER_0.json MEMBER_2.json MEMBER_5.json`.
The launcher records the executable hash, configuration hashes, PIDs,
outputs, and exit codes. It stays in the foreground and stops its own
children on interruption. Choose an explicit deadline that covers the
public tree's full week. Exit `2` means a stopped tree; `3` means queued
notifications remain. Restart with the same state directory to retry
undelivered alarms. A receiver must retain each JSON body and deduplicate
by its ID. Never point this experiment at the owner's production account.

Publish the public mirror files with the separate mirror identity:

```sh
layertree-follow publish --relay wss://RELAY_HOST --directory PUBLIC_MIRROR_DIRECTORY \
  --receipts MIRROR_RECEIPTS_FILE --secret-env LAYERTREE_TEST_MIRROR_SECRET
```

Run this after each confirmed transition. The publisher checks file names,
record hashes, and package encodings before publishing. It saves exact
signed events before sending, records acknowledgments per relay, and
retries the same event after interruption. It does not publish arbitrary
files or the enclave key.

The acceptance helper takes a JSON file with schema
`layertree.followers.evidence.v1`, an `operator` event-log path,
`honest` (three state-directory paths), `attacks` (keys `theft`, `withheld`,
and `mismatch`, each with three dedicated test-tree state directories),
and `notifications` (the receiver's JSONL alarm bodies). Paths resolve
relative to that file. Run `scripts/layertree/followers.py check --evidence
FILE --output SUMMARY.json`. It requires a common 604,800-second signet
interval, checks no more than two minutes apart, all confirmed transitions,
held unrolls, and all three staged alarms delivered to the receiver.
Theft detection uses the actual detection tip, at most one block after
mining. Short runs, regtest logs, gaps, and unacknowledged alarms fail.
The summary hashes every input; it checks evidence consistency and cannot
replace checking the public transactions and hardware reports.

Use `followers.py merge --operator FILE --states STATE_0 STATE_2 STATE_5
--output NEW_FILE.jsonl` for Verse. It preserves operator event order,
adds follower observations, assigns contiguous sequence numbers, and saves
input hashes beside the output. The raw journals retain notification IDs,
detection tips, and times. The week-long signet run and staged signet
attacks remain required before #11794 can close.

To start from the attestation client's public capture, use the confirmed
genesis `state-after.json`, its confirmation height, and a JSON file with
only the `sources` object's contents:

```sh
layertree-follow bootstrap --snapshot GENESIS_CAPTURE/state-after.json \
  --sources sources.json --output NEW_MEMBER_DIRECTORY --network signet \
  --rpc http://127.0.0.1:38332 --height CONFIRMATION_HEIGHT --member MEMBER_POSITION \
  --webhook-env LAYERTREE_TEST_ALARM_WEBHOOK
layertree-follow --config NEW_MEMBER_DIRECTORY/follower.json --once
```

The converter checks the tree, output bytes, and member's exact unroll
payout before writing public bootstrap files. The subsequent follower
check compares that output with Core. First qualify the capture through
the attestation client; this converter does not verify hardware reports.

`scripts/layertree/build-follower.sh REF OUTPUT_DIRECTORY` builds a native
Linux reader using the same pinned Rust compiler image as the research
service. Set `CARGO_TARGET_DIR` to your existing external target directory.
The output records the source commit, compiler image, input hashes, and
binary hash. The isolated build parent preserves the workspace's package
and lint settings and includes the Nostr schemas required by its library.

For an isolated operator test receiver, run
`scripts/layertree/alarm-receiver.py --directory NEW_RECEIVER_DIRECTORY
--until UNIX_SECONDS`. It binds loopback on an available port, writes the
URL to `receiver.json`, and syncs each alarm to `received.jsonl` before
responding. It rejects mainnet observations and conflicting duplicate IDs.
Restarting with the same directory keeps its port and receipt IDs. Inspect
its `receiver.log` or query its loopback endpoint through SSH forwarding.
Use this test receiver independently of the owner's production account.

Set the evidence plan's `normal_until` to the end of normal operation.
The week check uses statuses before that time, so later legitimate unroll
payouts do not invalidate the normal-operation evidence. A check may be one
block behind a tip that advanced during its RPC calls; longer lag fails.

The measured signet service's mirrors use plain HTTP inside its GCP VPC.
List those numeric RFC 1918 URLs under `sources.private_http` to select
that transport explicitly. This option carries only public record/package
bytes, and their hashes and payout checks still determine acceptance.
It accepts no URL credentials, public IPs, or hostnames. RPC credentials
continue to require HTTPS or loopback.
