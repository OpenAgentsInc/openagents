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
- **No network, no real keys, no money.**

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
| `rules` | the predicate the enclave enforces (spec 7.1 to 7.5), with one `Reject` per rule |
| `enclave` | the in-process co-signer: its key never leaves, and it signs only what passes |
| `unroll` | the delayed exit with deleted intermediate keys, checked with public data only |
| `attest` | a fake hardware report and the light client's attestation check |
| `light` | a member following the tree from chain data and anchored records |

`tests/lean_mirrors.rs` has property tests named after upstream's Lean
theorems (`ExitK_no_inflation`, `Steps_sum`, `Reachable_ledger`, `payable`,
`exit_available`, `voucher_no_replay`, `path_maintenance` and others).

## Fuzzing

Everything the enclave and light clients read from outside has a decoder
that returns a typed `codec::DecodeError` and never panics, and each is the
exact inverse of its encoder. `fuzz/` (its own workspace, nightly
`cargo-fuzz`) has six targets: `leaf_roundtrip`, `enc64_roundtrip`,
`record_decode`, `unroll_package_decode` (roundtrips assert
`encode(decode(x)) == x`, so the encodings are injective), and
`snp_report_parse` and `tdx_quote_parse`, which fuzz the SEV-SNP report and
TDX quote v4 parse layers in `crates/att-platforms` (#11745) rather than a
second copy. Seeds in `fuzz/corpus/*/seed-*` come from lab data and the
synthetic att-platforms vectors (test keys only; regenerate with
`LAYERTREE_WRITE_SEEDS=1` and `ATT_FUZZ_SEEDS=<corpus dir>`).

```sh
cargo install cargo-fuzz
cd crates/layertree-tee-lab
cargo +nightly fuzz build
cargo +nightly fuzz run record_decode -- -max_total_time=1800
# likewise leaf_roundtrip, enc64_roundtrip, unroll_package_decode,
# snp_report_parse, tdx_quote_parse
```

A crash becomes a regression test in `tests/codec.rs` (or att-platforms'
tests for the report parsers).

No `bitcoind` was available when this was written. The regtest harness is
the next step of P0 (see the spec, section 11).

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
```

`light_client.result` is `ok` or a `LightError` (`OutputMismatch`,
`RecordNotAnchored`, …); `attestation.result` is `attested` or an
`AttestError`; `rejected.reject` names the `Reject` variant.
