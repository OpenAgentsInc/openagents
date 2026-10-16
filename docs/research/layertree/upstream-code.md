# LayerTree: the upstream code, package by package

A walkthrough of the Go packages roasbeef linked on 2026-10-15
([`tree`](https://github.com/lightninglabs/layertree/tree/main/tree),
[`script`](https://github.com/lightninglabs/layertree/tree/main/script),
[`txn`](https://github.com/lightninglabs/layertree/tree/main/txn)), plus
`msg`, written in our own words. We read them at commit
[`f6c4d29`](https://github.com/lightninglabs/layertree/tree/f6c4d296130fa39f32d383eb841c82f9ea575539)
(2026-10-15), from a read-only reference clone at
`~/work/projects/repos/layertree`. File links below are pinned to that
commit. The repository has **no LICENSE file**, so nothing here is copied
into our code. Our own Rust, written from the spec, is in
[`crates/layertree-tee-lab`](../../../crates/layertree-tee-lab).

The Go module is `github.com/roasbeef/layertree`. It is built on
`btcsuite/btcd`, `lnd/tlv` and `taproot-assets`. The package doc
([`doc.go`](https://github.com/lightninglabs/layertree/blob/f6c4d296130fa39f32d383eb841c82f9ea575539/doc.go))
and the README say the Python tools under `tools/` remain the executable
reference, because only they include a simulator for the proposed opcodes.
The Go code is checked byte for byte against vectors the Python emits
(`spec/golden.json`, `spec/test-vectors.json`). **Go can build and sign
every transition, but it cannot execute a covenant script.**

## `tree`: the data

Two files, about 600 lines.

[`tree/tree.go`](https://github.com/lightninglabs/layertree/blob/f6c4d296130fa39f32d383eb841c82f9ea575539/tree/tree.go)
holds everything off-chain:

- **Tags.** Every hash is a BIP-340 tagged hash with its own tag: leaf,
  node, state, voucher, voucher-hashlock, voucher-merge, exit, pos and
  tree-id. The separate tags are what stop one signed message being
  replayed as another kind. For example, an exit authorisation can never be
  redeemed as a voucher.
- **The leaf** is a fixed 116-byte record: `amount` (le64), `exit_key`,
  `owner_key`, `evictor_key`, `evict_after` (le32), `nonce` and `seq`.
  `Body()` is the leaf without its amount (108 bytes). Vouchers and exit
  authorisations sign the body, so an amount-only top-up by the operator
  does not void them. `Validate()` enforces `0 < amount < 2^51`, a lease
  given as a block height, and keys that are valid points.
- **Nodes** are `(hash, sum)` pairs. A parent hashes
  `left.hash || enc64(left.sum) || right.hash || enc64(right.sum)`, where
  `enc64(n) = le64(n) || 0x01`. That is the minimal BIP-441 encoding of
  `n + 2^64`, which lets a script pad a computed sum with one `OP_ADD`. The
  code notes that `Parent` adds unchecked, which is only for trees the
  holder built itself. Anything from a peer goes through `Fold`, which
  rejects a sum that wraps past 2^64 (`ErrSumOverflow`). A path that only
  balances modulo 2^64 is exactly what an attacker would try.
- **The tree** is a map from position to leaf, with fixed depth.
  `Subtree(h, i)` is computed lazily, and empty ranges become `E_h`.
  `Path` returns the sibling path bottom up, and `Fold` recomputes the root.
- **Two proof encodings** for the sparse profile: an empty element
  standing for an empty sibling (`CompressPath`, cheaper on chain), and an
  MS-SMT-style bitmap (`BitmapPath`, recommended for off-chain wire use).
- **Message hashes.** `VoucherMsg`, `VoucherHMsg`, `VoucherMMsg` and
  `ExitAuthMsg` bind `tree_id`, not the taptree root `m`. The reason is
  that `m` names a rule set two trees can share, while `tree_id` names one
  lineage and survives a `DETACH_h`.
- **Sparse positions.** `PosHash` is `H_pos(owner_key || nonce)`, and a
  leaf's position is the low `D` bits.

[`tree/taproot.go`](https://github.com/lightninglabs/layertree/blob/f6c4d296130fa39f32d383eb841c82f9ea575539/tree/taproot.go)
binds the root to an output:

- The **state key** is `NUMS + H_state(root)·G`, where NUMS is BIP-341's
  unspendable point. The code notes that this is "deliberately not lnd's
  TaprootNUMSKey".
- The **output key** is the ordinary BIP-341 tweak of the state key by the
  taptree root `m`, so the address commits to both the state and the rules.
- The **CCV tier** uses a different state key,
  `NUMS + sha256(x(NUMS) || root)·G`, which is BIP-443's data tweak. A CCV
  tree and a FULL tree with the same root therefore have different
  addresses, and the tier is fixed at creation.

**For our TEE spec,** this package is the part we reuse in substance. Our
Rust tree produces the same roots (the opt-in test in
`tests/upstream_vectors.rs` checks the empty nodes, leaf bytes and hashes,
the example root and `tree_id`). Our TEE output uses the same `H_state`
tweak, with the enclave key in place of NUMS.

## `script`: the rules, in four opcode tiers

About 6,000 lines. Every file is a token-level port of a Python generator,
and the comments say it "mirrors the Python token for token, including its
oddities". `asm.go` turns tokens into bytes, with two numeric modes: BIP-441
minimal unsigned (tapleaf `0xC2`) and `CScriptNum` (`0xC0`). It also parks
the opcodes that have no settled number. `OP_TX` sits at a placeholder,
because the `OP_TX` draft and the EC-ops draft both claim `0xbd`, and
BIP-443 and the EC-ops draft both claim `0xbb` (spec assumption A-4).

What every transition script does, in any tier:

1. **Bind the input.** It rebuilds the spent output's scriptPubKey from
   the witness `r` and `m` and compares it with the real input
   (`fullPrologue`: `OP_TX` reads the current input's scriptPubKey). This
   is what stops a spender substituting another rule set `m'` (spec 10.4).
2. **Fold both roots in one pass.** Each proof level takes `dir` and the
   sibling `sc` from the witness and folds the old and the new node up
   together (`fullLevelBody`). It checks that `sc` is 41 bytes, adds the
   sums with BIP-441 big numbers, pads each with `2^64 OP_ADD`, and hashes
   with the node tag's prefix.
3. **Check the old root and amount.** The folded old root must equal the
   bound `r`, and the folded old sum must equal the input amount.
4. **Authorise.** A transaction signature by the owner, the evictor (with
   `OP_CHECKLOCKTIMEVERIFY`) or the operator; an `OP_CHECKSIGFROMSTACK`
   voucher or exit authorisation; or nothing, for a voucher redemption.
5. **Check the outputs.** Output 0 must be the tree output for the new
   root with exactly the new sum. The only exception is a terminal
   transition whose new root is `E_D`. The payouts must be
   `OP_1 <exit_key>` with the amount the rule requires.

The tiers differ in how they do steps 1 and 5:

| File | Tier | How it binds outputs | Notes |
| --- | --- | --- | --- |
| [`full.go`](https://github.com/lightninglabs/layertree/blob/f6c4d296130fa39f32d383eb841c82f9ea575539/script/full.go) | FULL (`0xC2`) | recomputes `TREE_SPK(r, m)` with `OP_EC_POINT_MUL`/`ADD`/`X_COORD` (`fullTreeSPK`) and compares with `OP_TX`'s output fields | leaf as one 116-byte element, fields by `OP_SUBSTR`; every transition, cohort and receive script; `FullTreeOptions` compiles a tree's whole taptree |
| [`mid.go`](https://github.com/lightninglabs/layertree/blob/f6c4d296130fa39f32d383eb841c82f9ea575539/script/mid.go), [`mid_pending.go`](https://github.com/lightninglabs/layertree/blob/f6c4d296130fa39f32d383eb841c82f9ea575539/script/mid_pending.go) | MID (`0xC0`) | `OP_TWEAKADD` twice (`midTreeSpkMin`) | 4-byte `CScriptNum`, so a leaf is seven field elements; `PADZ` pads numbers and `NONNEG` blocks negative-number tricks (spec 10.6); trees capped at 2^31 sat |
| [`min.go`](https://github.com/lightninglabs/layertree/blob/f6c4d296130fa39f32d383eb841c82f9ea575539/script/min.go) | MIN (`0xC0`) | no `OP_TX`: the script assembles the BIP-341 signature message itself and checks a signature under the generator `G`, whose only valid `s` is `e + 1`; the spender grinds the fee input's `nSequence` until `e` ends in a zero byte (about 256 tries) | only `EXIT_1` and `SPLIT` are built; fixed transaction shape |
| [`ccv.go`](https://github.com/lightninglabs/layertree/blob/f6c4d296130fa39f32d383eb841c82f9ea575539/script/ccv.go) | CCV | one `OP_CHECKCONTRACTVERIFY` call binds the input to its own taptree (`CHECK_INPUT`, taptree `-1`), and others check output 0 (assign mode) and each payout (deduct mode); `OP_TX` only reads amounts | no `r` or `m` in the witness; the smallest scripts (`EXIT_1` 1,716 bytes at `D = 16` against FULL's 2,101) |
| [`chan.go`](https://github.com/lightninglabs/layertree/blob/f6c4d296130fa39f32d383eb841c82f9ea575539/script/chan.go) | channel leaves | separate outputs, not tree scripts | LN-Symmetry `REBIND_n`/`SETTLE_n` with `OP_TX` + CSFS, HTLC scripts, P2A anchors |
| [`taptree.go`](https://github.com/lightninglabs/layertree/blob/f6c4d296130fa39f32d383eb841c82f9ea575539/script/taptree.go) | all | the balanced taptree (adjacent leaves paired) whose root is `m` | |

Tests in this package compare every compiled script with `spec/golden.json`
(145 scripts). They do not execute the scripts. Execution happens in the
Python simulator.

## `txn`: building and signing transitions

About 3,400 lines.
[`txn/transitions.go`](https://github.com/lightninglabs/layertree/blob/f6c4d296130fa39f32d383eb841c82f9ea575539/txn/transitions.go)
and
[`txn/cohort.go`](https://github.com/lightninglabs/layertree/blob/f6c4d296130fa39f32d383eb841c82f9ea575539/txn/cohort.go)
have one builder per transition:

- single-leaf: `BuildExit`, `BuildJoin`, `BuildJoinK`, `BuildUpdate`,
  `BuildUpdateX`, the four `BuildClaim` forms and `BuildSplit`;
- cohort: `BuildEvictSubtree`, `BuildRewrite` and `BuildDetach`.

Each builder:

1. takes a `TreeState` (the off-chain tree, the option set and the
   outpoint), applies the rule to a copy of the tree, and refuses anything
   the script would refuse;
2. lays out the transaction: input 0 is the tree, output 0 is the new tree
   output, outputs 1 to K are the payouts, and anything else is free;
3. computes the BIP-341 script-path sighash for the revealed leaf
   ([`sign.go`](https://github.com/lightninglabs/layertree/blob/f6c4d296130fa39f32d383eb841c82f9ea575539/txn/sign.go)
   does this two ways, once through `txscript` and once by hand, and
   compares them);
4. fills the signature slots, then assembles the witness
   ([`witness.go`](https://github.com/lightninglabs/layertree/blob/f6c4d296130fa39f32d383eb841c82f9ea575539/txn/witness.go):
   the path from the top level down, `dir` as empty or `0x01`, then the
   leaf, signatures, flags, `r` and `m`) plus the control block
   ([`build.go`](https://github.com/lightninglabs/layertree/blob/f6c4d296130fa39f32d383eb841c82f9ea575539/txn/build.go));
5. returns the spend and the tree after it.

Other details:

- [`tier.go`](https://github.com/lightninglabs/layertree/blob/f6c4d296130fa39f32d383eb841c82f9ea575539/txn/tier.go)
  holds everything that differs per tier: the leaf version, numerics,
  witness encoding, whether the stack ends in `r m` or relies on CCV, and
  the output key. So each builder is written once.
- The tree input's `nSequence` is below `0xffffffff`, so lock time is
  enforced for evictions, and it opts into RBF (`TreeSequence` in
  `build.go`).
- [`pending.go`](https://github.com/lightninglabs/layertree/blob/f6c4d296130fa39f32d383eb841c82f9ea575539/txn/pending.go)
  builds the pending-leaf Lightning receive: merge claims and reclaims.
- [`min.go`](https://github.com/lightninglabs/layertree/blob/f6c4d296130fa39f32d383eb841c82f9ea575539/txn/min.go)
  does MIN's nSequence grinding.
- The README counts 427 end-to-end cases and 156 negative cases. Each is
  pinned to the simulator's sighash and replayed through the Python
  interpreter in CI (`tools/replay_txn_dump.py`).

**For our TEE spec,** the builders' "refuse before signing" checks are,
in effect, the rule predicate. Our enclave runs the same checks, but as
the authority rather than as a courtesy to the script.

## `msg`: the off-chain messages

About 1,700 lines of TLV encodings (`lnd/tlv`), plus tests:

- compressed proofs, in
  [`proof.go`](https://github.com/lightninglabs/layertree/blob/f6c4d296130fa39f32d383eb841c82f9ea575539/msg/proof.go);
- vouchers, in
  [`voucher.go`](https://github.com/lightninglabs/layertree/blob/f6c4d296130fa39f32d383eb841c82f9ea575539/msg/voucher.go);
- join offers, which a member verifies before funding
  (`JoinOffer.VerifyMember`), in
  [`join.go`](https://github.com/lightninglabs/layertree/blob/f6c4d296130fa39f32d383eb841c82f9ea575539/msg/join.go);
- the channel-leaf protocol (state proposals, signing rounds, HTLC add,
  fulfil and fail, splices), in
  [`chan.go`](https://github.com/lightninglabs/layertree/blob/f6c4d296130fa39f32d383eb841c82f9ea575539/msg/chan.go).

Message framing between parties is still an open item upstream.

## How a leaf exits, end to end

1. The member holds its leaf and its path. It keeps the path current from
   the witnesses of every spend (spec 9.7: only one sibling can change).
2. It builds `EXIT_1`: input 0 is the tree, output 0 is the tree minus its
   leaf, output 1 pays `OP_1 <exit_key>`, and a fee input pays the fee
   unless the exit takes it from the payout. It signs with `owner_key`.
3. The script binds `r` and `m` to the input, folds the leaf (old) and
   `E_0` (new) up the path, checks the old root and amount, checks the
   owner's signature, and checks output 0 and output 1.
4. The transaction is about 1,020 vB at `D = 16` (FULL). If a competing
   spend confirms first, the member rebuilds against the new root.

## How light clients follow the tree

Every spend's witness publishes the paths it used and the old and new
nodes. A client that knows the current `r` and `m` can compute the current
scriptPubKey and find the block that spends it with BIP-158 filters, which
include the scriptPubKeys of the outputs each block spends. It then
updates its path from the witness and moves on to the next scriptPubKey.
The paper (`paper/appendix-impl.tex`) says exactly this.
[`design/towers.md`](https://github.com/lightninglabs/layertree/blob/f6c4d296130fa39f32d383eb841c82f9ea575539/design/towers.md)
puts it the other way round: a light tower "cannot match a fixed
scriptPubKey", so it "follows the outpoint chain". Both are right. The
scriptPubKey changes every step, so a client can only watch the *current*
one, and a client that falls behind needs the outpoint chain from a
server. The one piece of data that is not on chain is a grafted member's
path inside its own subtree, which the operator sends at join time.

## The security model in one paragraph

The key path is NUMS, so nobody can spend the output without a script.
Every script needs a proof against the committed root plus the right
signature:

- the owner's, for anything that touches its leaf;
- the evictor's, after the lease, paying the leaf in full;
- the operator's, for adding value.

Conservation is a Lean theorem, not a trust assumption. All of this holds
only if each member recomputes `m` from the published parameters before
funding, because an extra hidden tapleaf would look identical on chain.
The bytecode is tested against the rules, not proved.
[analysis.md](analysis.md) covers the rest of the trust model.
