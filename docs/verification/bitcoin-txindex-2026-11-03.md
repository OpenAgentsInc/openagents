# Bitcoin chapter 91 transaction-index verification, 2026-11-03

Base: `65da61173ee7657667472880df2b396599fbed6b`, in a separate worktree.
The main checkout's pre-existing website edits were left untouched. Host:
Darwin arm64. Compiler: `coax0 0.48.0`, checked cached build `c64ee8a16c2c`
installed by `coax/build.sh`.

Rebased onto `1223b431aa` before publication, preserving the upstream Coax
pin-checker port and provenance-book updates. The Bitcoin implementation,
crypto, compiler and file/tool host inputs are unchanged; the source blobs
below still match. Their passing results were retained. The changed
provenance checks and book structure were checked after rebase.

Scope: [#12704](https://github.com/OpenAgentsInc/openagents/issues/12704)'s
transaction-index step and the transaction portion of
[chapter 91](../../bitcoin/book/ch091-indexes-transactions-filters-and-coin-statistics.md).
Chapter 91 remains partly implemented. Filters, historical coin statistics,
node index management, live-network/running-Core RPC comparison and Wasm
are not qualified. The underlying light connector remains `pow-only`.

| Check | Result |
| --- | --- |
| `coax mutate coax/lib/bitcoin/txindex-store.coax --library --root-only --jobs 4` | 226 mutants, 226 killed, 0 survived. |
| `coax mutate coax/lib/bitcoin/txindex-files.coax --library --root-only --jobs 4` | 55 mutants, 55 killed, 0 survived. |
| `coax mutate coax/lib/bitcoin/txindex.coax --library --root-only --jobs 4` | 578 baseline checks retained; 112 mutants, 112 killed, 0 survived. |
| `python3 -I coax/tests/bitcoin/txindex.py --coax coax/bin/coax` | Debug and release each pass 369 verified file lookups, shared txids, active/stale preference, witness transactions, CompactSize 253 and every one of 3,535 physical cuts of the reorg KV frame. |
| Oracle with `--cli-tool BUILT_TOOL` | Actual Coax tool's index/UTXO bytes, raw lookups, reorgs, orphan records, recovery and corruption refusals match the independent model. |
| Oracle with `--sanitize-with coax/bin/coax` | Complete file-tool trace passed under ASan/UBSan with errors fatal. |
| Debug/release `bitcoin/examples/tx-index.coax` transcript | Both exact stdout transcripts and exit statuses pass. |
| `python3 coax/tests/vectors/check_pins.py --write` | All pins agree: 349 manifest entries and 422 citations checked, including four new immutable Core source pins. |
| After rebase: `coax/web/tool.sh coax/tests/vectors/check-pins.coax --write` and the Python reference without `--write` | Both pass with the same 349 entries and 422 citations; the generated chapter index is unchanged. |
| `python3 psionic/tests/book.py --book bitcoin` | 98 programs, 861 source-linked listings, registry/status consistency and local links pass; structure validation, not execution of all programs. |
| `coax/web/tool.sh tools-check` | Ran before and after rebase. Upstream resolved four old findings; five pre-existing findings remain on paths untouched by this change, listed below. The new independent oracle was accepted. |
| `git diff --cached --check` | Passed. |

Mutation scopes retain dependency baseline checks. Focused cases check exact
init/source/batch capacities, every receipt field, malformed candidate records,
cursor mismatch, zero-sequence stale selection, active-before-sequence ordering,
replay without writes, index-write failures after successful coin preparation,
original file positions, every cut of a referenced block record, unreferenced
tails and XOR framing. Synthetic private-stage cases isolate serialization
and refusal gates; the public differential trace admits real mined headers.

The independent oracle has a witness-aware transaction reader, a dictionary
of archived occurrences and a separately reconstructed active path. It compares
every `I`, `J`, `T` and coin byte after each commit. Raw lookup reads actual
file spans, parses them and verifies the block header and full txid before
publication. Complete raw bytes, including witnesses, match the model. A txid
does not authenticate witness-only edits; stored bodies must remain immutable.

The fork trace mines the same transaction into two branches, moves to the later
branch and returns to the older active branch. Stale occurrences remain
queryable with zero confirmations, while active-only misses. Genesis and wtxid
queries miss. A 253-transaction block verifies the three-byte CompactSize count
and every following offset. Late failure preserves the live state and files.

The file adapter syncs block records before the one state commit. Recovery
tests include a state-frame cut after block append, a torn unreferenced block
tail, refusal to discard a committed block position, complete orphan records,
interior committed-frame checksum corruption, wrong networks, read-time header
and stripped-transaction corruption, and an unindexed light-only database.
Per-file audit checks framing and referenced headers; it does not revalidate
every stored block body on each query.

Core's `txindex.cpp`, `txindex.h`, `txindex_key.h` and `txindex_tests.cpp` are
pinned byte-for-byte at `ddea9196493d3b8fd841a36a6c53efd85dac7a05`, with SHA-256
and byte counts in the vector manifest. They supply the candidate-order,
stale-retention, replay and genesis conventions; our full-txid KV schema is
distinct from Core's hashed LevelDB format. Downloaded sources were read,
never executed. The only new Python tool is an independent oracle with its
required `Firm reason: 2` header; product and file-tool logic are Coax.

Passing prerequisite results remain those of `65da61173e` and its
[light-connect record](bitcoin-light-connect-2026-11-03.md), including its
651 killed mutants. Those unchanged modules were not rerun merely to confirm
them.

Source identities:

| Source | Git blob |
| --- | --- |
| `coax/lib/bitcoin/txindex.coax` | `271494499f8aa258653e47c66373b5fe33c4ec61` |
| `coax/lib/bitcoin/txindex-store.coax` | `9b2e681c7ad96632190b9f8e80428385ecae23d0` |
| `coax/lib/bitcoin/txindex-files.coax` | `494fc2e949dbad7d465ff9e33fe3611bfb6601d0` |
| `coax/tests/bitcoin/txindex.py` | `222510fd4104f72cd639cb0fd59692722f25c016` |
| `coax/tests/bitcoin/txindex-native.coax` | `2a30ac2b65f01a273f27e144ff053b630bae8364` |
| `bitcoin/examples/live/tx-index.coax` | `a4eaf62b8b6b259f837a8707a2f068b7fd7e2b17` |

## Existing tools-check findings

The initial check inspected 1,740 tracked tool files and reported nine
pre-existing findings. After rebase it inspected 1,742 files, including 35
upstream vector files, and reported five. The new Python oracle is accepted.
The remaining missing reason headers are on these untouched paths:

- `verse/tools/fivemmo_hotbar.py`
- `verse/tools/humans/face_rig.py`
- `verse/tools/humans/metahuman.py`
- `verse/tools/humans/outfits.py`
- `verse/tools/humans/pack_human.py`
