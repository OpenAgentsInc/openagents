# Bitcoin explorer address index — 2026-11-03

Scope: epic #12704's address-index step, chapter 125. The index retains
exact-script output occurrences and creation-height-bound spend occurrences.
It shares the light chainstate and transaction index's atomic batch. This
is the proof-of-work-only explorer profile, without script validation.

Sources: `coax/lib/bitcoin/addrindex.coax`, `addrindex-stage.coax`, `addrindex-store.coax`, the shared
`bitcoin/platform/index-file.coax`, and its address/transaction CLI entries.

Verification commands (results recorded below as each completes):

```sh
coax/bin/coax mutate coax/lib/bitcoin/addrindex-store.coax --root-only --jobs 6
coax/bin/coax mutate coax/lib/bitcoin/addrindex-stage.coax --root-only --source-order --jobs 8
coax/bin/coax mutate coax/lib/bitcoin/addrindex.coax --root-only --jobs 6
python3 coax/tests/bitcoin/addrindex.py --coax coax/bin/coax
python3 coax/tests/bitcoin/addrindex.py --cli-tool ADDRESS_TOOL
python3 psionic/tests/book.py --book bitcoin
coax/web/tool.sh tools-check
```

The Python oracle parses raw transactions and replays coins/history/spends
independently. Its fixture starts with 100 mined blocks, then forks a mature
spend to P2WPKH, P2TR and P2PKH scripts. It includes two outputs to the same
address, zero value, same-block spending, the same txid mined on competing
branches, the return to the earlier branch, disconnect, and every byte cut
of a reorg's state frame. Failed queries publish no valid result; workspace
arrays are scratch on refusal.

The CLI requires exclusive file ownership and immutable admitted block
bodies. It supports one unmasked block file and bounded snapshots. Balances
include immature coinbase outputs; confirmed does not mean consensus
validated, and a balance is not spendable-wallet authority. Historic BIP30
replacement has a distinct output state. General node storage, file
rollover, the explorer API/front end and full signet service qualification
remain the subsequent epic steps.

Completed so far: storage/query mutation 241/241 killed; archive serialization 107/107 killed, zero survivors. Native debug and release each passed 630 complete script/address comparisons and 5,413 atomic reorg cuts. The normal CLI and ASan/UBSan CLI journeys passed; the latest module split is being requalified below. The full chapter registry passes (96 implemented, 3 partial, 26 planned). The full book checker stops on an existing unrelated `coax.wgsl.current` evidence mismatch for `verse/everglade/gpu.bindings`. Tools inventory reports six untouched files without reason headers: `coax/web/room/room-catalog.py`, `verse/tools/fivemmo_hotbar.py`, and `verse/tools/humans/{face_rig,metahuman,outfits,pack_human}.py`.

An unpartitioned transition mutation run was stopped before the 15-minute limit. Serialization now takes admitted bytes and explicit height/hash/parser workspaces in its own module, so its mutation gate no longer repeats unrelated header/script checks for each archive mutant. The public transition wrapper still retains its complete imported baseline. No mutants are marked equivalent.

Final results after the split:

- `addrindex-store.coax`: 241 killed, zero survivors.
- `addrindex-stage.coax`: 107 killed, zero survivors, including original
  witness transactions, multiple inputs, complete scripts and malformed
  original-coin receipt refusal.
- `addrindex.coax`: 32 killed, zero survivors. Total: 380 killed.
- Native debug and release: each 630 complete address comparisons and
  5,413 atomic reorg frame cuts passed on the final implementation.
- The final durable address CLI journey passed under ASan/UBSan. The
  subsequent API integration also passed with both the API and this writer
  sanitized, comparing 74 real HTTP/RPC responses against independent replay.
- All chapter registries passed. Tools inventory ran after staging the new
  oracle and still reports only the six pre-existing missing headers above.
- The release example matches its exact five-line golden transcript.
  The original transaction-index CLI also passed its full independent file
  journey after extraction into the shared adapter: active/stale/shared and
  witness lookup, both fork directions, CompactSize 253, orphan storage,
  torn recovery and corruption refusals. The unchanged 393-mutant txindex
  result remains the qualification recorded at `322e4f0188`.

Compiler: `coax0 0.48.0`, SHA-256
`cdaceb89e69c8d523e41242a940af34774e8b6b4f945998c2d3bf38486c357fb`.
Base commit: `54bc9468df`. The isolated worktree contains the verification
outputs; no tests or edits ran in the main checkout.
