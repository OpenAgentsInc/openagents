# Bitcoin explorer light connect/reorg verification, 2026-11-03

Base: `1bfc93819eb20e097d1d1729e31019a392f58de3`. All edits, builds and
checks ran in a separate worktree; the main checkout's existing changes
were left untouched. Host: Darwin arm64. Compiler: `coax0 0.48.0`, checked
cached build `c64ee8a16c2c`, installed by `coax/build.sh`.

Rebased onto `0951cb4beb` before publication. Upstream changes touched
unrelated Coder/gateway/Wayland code; the Bitcoin sources, crypto, tool/file
hosts and compiler inputs used here are unchanged. The source blobs below
still match, so their passing checks were retained.

Scope: [#12704](https://github.com/OpenAgentsInc/openagents/issues/12704)'s
light connect/reorg step, [chapter 58](../../bitcoin/book/ch058-connect-and-disconnect-a-block.md)
and [chapter 60](../../bitcoin/book/ch060-reorganizations-and-invalid-chains.md).
The qualified profile tracks outputs on admitted proof-of-work headers.
Full script/consensus validation, a running-Core reorg comparison, a public
node, live-network sync, Wasm and money decisions are outside this result.
Both chapters remain partly implemented.

| Check | Result |
| --- | --- |
| `coax mutate coax/lib/bitcoin/lightchain.coax --library --root-only --jobs 2 --shard I/4`, four shards and merge | All 567 baseline checks retained and passed; 292 mutants, 292 killed, 0 survived. Each shard completed below 15 minutes. |
| `coax mutate coax/lib/bitcoin/lightstore.coax --library --root-only` | 293 mutants, 293 killed, 0 survived. |
| `coax mutate coax/lib/bitcoin/lightpolicy.coax --library --root-only` | 46 mutants, 46 killed, 0 survived. |
| `coax mutate coax/lib/bitcoin/kvhash.coax --library --root-only` | 20 mutants, 20 killed, 0 survived. |
| `python3 -I coax/tests/bitcoin/lightchain.py --coax coax/bin/coax` | Debug and release UTXOs and MuHash match the independent model; all 2,713 physical reorg-frame cuts are atomic in each mode. |
| `coax run bitcoin/examples/light-chain.coax`, debug and release | Both exact stdout transcripts and exit statuses passed. |
| `python3 -I coax/tests/bitcoin/lightchain.py --cli-tool BUILT_TOOL` | Real-file init, connect, reorg, late refusal, disconnect, reopen, recovery and corruption refusal passed. A 1,375,532-byte batch reopens beyond 1 MiB. Exactly 4,000,000 weight units accepted; 4,000,004 refused with the log unchanged. |
| `python3 -I coax/tests/bitcoin/lightchain.py --sanitize-with coax/bin/coax` | The complete file-tool journey above passed under ASan/UBSan. |
| `python3 psionic/tests/book.py --book bitcoin` | 97 programs, 861 source-linked listings, registry/status consistency and local links passed; structure validation, not execution of every program. |
| `coax/web/tool.sh tools-check` | Ran; nine existing findings listed below. The new independent Python oracle was accepted. |
| `git diff --cached --check` | Passed. |

Mutation scopes retain every dependency baseline check. The transition
checks include exact independent coin-record digests, real mined header
admission, mature and intra-block spends, both fork directions, exact
script bytes, witness activation, stored block weight, late failure,
malformed receipts/bodies/inverses, damaged afterimages and capacity edges.
Narrow synthetic receipts isolate otherwise interdependent refusal gates;
the full fork trace always goes through actual header admission.

The independent Python oracle reproduces 100 mined regtest blocks and six
branch blocks. It supplies its own transaction parser, UTXO dictionary,
ChaCha20 and big-integer MuHash3072 calculation. It compares every coin's
bytes, metadata and MuHash against native Coax, independently checks the
ordered undo digest and expected record set, and appends/syncs/replays
actual files. Every physical cut of the reorg frame must expose exactly
the preceding snapshot or the complete new snapshot. No upstream scripts
or programs are executed.

The Coax file adapter admits headers with chapter 59, stages one complete
branch change, seals one chapter-62 frame and acknowledges only after
durable append. The independent file reader checks its actual log. The
adapter assumes exclusive state-file ownership and immutable inputs and
has explicit teaching capacities. Its output identifies `profile pow-only`;
it never raises header-index validation levels.

Source blobs for these results:

| Source | Git blob |
| --- | --- |
| `coax/lib/bitcoin/lightchain.coax` | `2656abb9b291fe731dd265ceda5c8a76bea269f3` |
| `coax/lib/bitcoin/lightpolicy.coax` | `91844c20213064c4305e8ffdc18b8849b58ee50b` |
| `coax/lib/bitcoin/lightstore.coax` | `ac5b45e1df92c97e5939fbd64fd71a863d64d366` |
| `coax/lib/bitcoin/kvhash.coax` | `8b06f2b56b83ce713f18a7b94470dc94bd63cbd9` |
| `coax/tests/bitcoin/lightchain.py` | `c70a3c7465746f9c12e074efae81b2d0b3e98122` |
| `bitcoin/examples/live/light-chain.coax` | `3700c2f95db7339e5e41e764b78e7e3b3ccafb49` |

Retain passing results when rebasing over unrelated changes; do not repeat
unchanged gates merely to confirm them.

## Existing tools-check findings

The check inspected 1,738 tracked tool files before rebase and 1,739 after,
including 35 upstream vector files. The same nine findings remained; these
paths are unchanged by this work:

- Missing reason headers: `coax/tests/vectors/check_pins.py`,
  `coax/tests/vectors/test_pins.py`, `verse/tools/fivemmo_hotbar.py`,
  `verse/tools/humans/face_rig.py`, `verse/tools/humans/metahuman.py`,
  `verse/tools/humans/outfits.py`, `verse/tools/humans/pack_checks.py`,
  `verse/tools/humans/pack_human.py`.
- Already has a reason but remains on the port list:
  `psionic/tests/decoder_oracle.py`.

The Python oracle added here carries `Firm reason: 2 (independent oracle)`.
No new host-language product logic or C glue was added.
