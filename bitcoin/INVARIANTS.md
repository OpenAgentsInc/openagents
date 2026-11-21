# Invariants — bitcoin/INVARIANTS.md

> Scope: Bitcoin protocol, explorer, P2P, chainstate, script, PSBT (11 invariants). Indexed from root [INVARIANTS.md](../INVARIANTS.md).

## The native explorer publishes only a complete durable cursor

`addrindex-private.coax` prepares an exclusive workspace without copying its
whole arena. A refusal poisons that workspace: discard and reopen it; never
serve it or append its batch. Successful preparation becomes queryable only
after the complete state frame is appended and synced. The public staged
transition's disjoint-workspace contract remains unchanged. All 76 private
transition mutants are killed, with no equivalents marked.

`kvstream.coax` and `bitcoin/platform/kvstream-file.coax` replay bounded
frames rather than a whole file. Sequence exhaustion refuses without
wrapping. Native journal segments retain KV v1 headers/frames and enforce
continuity between files. Only the final segment can have a recoverable
tail. A partial new segment header can be rebuilt only when it is an exact
canonical prefix and no later segment exists. A reader never repairs a
writer file. The exclusive node owns recovery and the optional overflow
directory; duplicate segment names across directories refuse. Placement of
new segments follows available storage, while existing segment names locate
stable bytes. Segment targets may be exceeded by one atomic frame.

The API binds the admitted in-memory cursor and authenticates raw blocks
including positional XOR. Header updates retain the node's own admitted
archive, so a competing feed cannot erase the old branch. The native
fixture journey covers both reorgs, cross-directory journal recovery and
one open headless browser; the full signet reference comparison is a
separate acceptance gate, qualified at frozen height 329,338 in
`docs/verification/bitcoin-explorer-reference-2026-11-03.md`.
A listening importer retains its private arena at
bounded checkpoints and serves HTTP 503 until the preferred known chain is
complete. Only a local continuation file resumes the next slice. A header
feed changed during initial import must still be observed afterward.

Restart replay has the same bounded continuation contract. It advances only
at complete frame boundaries, retains its private table and sequence, and
never serves that prefix as ready data. Journal names must form a contiguous
inventory across owned directories; unavailable archive targets and missing
interior segments refuse before a partial prefix can become the final state.

## P2P frames honor the protocol limit independently of SHA workspace limits

`coax/lib/bitcoin/p2p.coax` reads and writes full v1 messages through
4,000,000 payload bytes. Checksums reuse chapter 34's bounded streaming
SHA-256d updates; chapter 5's one-shot bound stays intact. Endian fields
address exactly their own bytes even inside a larger caller workspace.
Magic, length, checksum and command refusals keep Core's order. Address and
inventory writers preflight their exact serialized capacity. Malformed
payloads never become admitted list results.

`coax/tests/bitcoin/p2p_checks.coax`, the 718-mutant root gate and the
independent `p2p-large.py` oracle cover these boundaries, exact wire bytes,
fragmentation and corruption in debug/release and under ASan/UBSan.
`bitcoin/examples/live/sync-node.coax` uses this shared reader directly;
it has no second checksum/admission path.

## Explorer responses describe one committed snapshot

`coax/lib/bitcoin/explorer-{request,rpc,store,body,json}.coax` and
`bitcoin/examples/live/explorer-api.coax` expose read-only HTTP v1. Every
successful data object identifies the synchronized light/transaction/address
cursor and its pow-only profile. Readers never repair writer files or return
partial histories as complete. Raw blocks and transactions must match their
indexed framing, header, transaction boundary, txid and activated witness
commitment before serialization. A changed snapshot invalidates related
client views, including same-height reorgs.

Request framing refuses ambiguous lengths and duplicate host fields. Valid
JSON-RPC notifications produce no JSON response. Storage and capacity failures
are explicit; error responses reveal no file paths. The source projection is
`proto/openagents/bitcoin/explorer/v1alpha1/explorer.proto`; the envelope is
`docs/protocol/bitcoin-explorer-v1.md`. Chapter 90, root mutation gates and
`coax/tests/bitcoin/explorer.py` bind this boundary to independent raw-block
replay, both reorg directions and real durable files under ASan/UBSan.

`bitcoin/explorer/model.coax` owns client response tickets and exact snapshot
admission. Old replies cannot replace newer data. A failed query, changed
network or tip hash clears ready data until a complete page is admitted;
engine replacement reissues canceled effects. The Coax canvas displays the
network/profile and copies only public identifiers. `explorer-assets.coax`
serves an exact static allowlist, never a user-selected filesystem path.
`coax/tests/bitcoin/explorer-browser.py` checks actual browser input, links,
spent states, balances through both reorgs and failure/recovery against the
real writer, with independent raw-block replay as its oracle.

Height lookup seeks to the `J` block-receipt range and ends at its boundary,
using the exact end iterator of either table representation. Receipt and
active-membership checks remain mandatory. The explorer-store root gate
kills all 128 mutants; its checks cover packed and indexed representations.

The separate Lab projection (`bitcoin/lab/`) retains that snapshot boundary in
both Grid and ordinary table views. Public `lab1/` selections are closed,
bounded and bound to source/network/cursor/exact occurrence; a stale cursor
never selects a replacement active transaction. The browser fragment and native
selection file use the same parser. Tables virtualize complete public text;
24/32/16-element pages never imply whole-chain completeness. Fixture detach/attach
geometry starts after complete admission and never interpolates amounts. Evidence:
[Bitcoin Lab #12804](docs/verification/bitcoin-lab-12804.md).

## Address histories and spend links share the light chain's cursor

`coax/lib/bitcoin/addrindex.coax`, `addrindex-stage.coax` and
`addrindex-store.coax` own schema 1:
`E` is the synchronized transaction-index cursor, `F` binds full scripts,
outpoints and creation blocks, and `G` binds a spend to its outpoint and
original creation height. One batch contains coins, inverses, transaction
positions, address occurrences and all cursors. Only the light chain's
active block membership determines confirmed history. Archived records
survive disconnect and replay must reproduce them exactly. Queries compare
full scripts, deduplicate transaction occurrences, distinguish spent,
unspent and historical replacement, and refuse incomplete workspaces or
stale cursors. Balances include immature outputs and confer no money
authority. This remains the proof-of-work-only explorer profile. Evidence:
chapter 125 and `coax/tests/bitcoin/addrindex.py`'s independent raw-block
replay, native/file journeys and atomic-frame crash cuts.

## Transaction positions and light chainstate publish one cursor

`coax/lib/bitcoin/txindex.coax`, `txindex-store.coax` and `txindex-files.coax`
own transaction-index schema 1 (`I`, `J`, `T`) alongside the unchanged light
schema. Index receipts and full-txid/block-hash occurrences share the coins,
undo and final tip's atomic batch. The index cursor must match the light
cursor before reads or transitions. Archival entries survive disconnects;
active membership wins before first-indexed sequence, and replay preserves
original immutable positions and sequences. Genesis is excluded. Raw lookup
checks framing, header identity, a complete transaction and its full txid;
witness-only integrity relies on the admitted immutable block body. The
file adapter syncs new block bytes before the KV commit and acknowledges
only afterward. Per-file recovery never discards a committed position and
checks referenced header identity before trimming an unreferenced torn tail.
This remains the explorer's proof-of-work-only profile. Evidence: chapter
91's partial transaction profile and the independent native/file/sanitizer
oracle in `coax/tests/bitcoin/txindex.py`.

## Validated Bitcoin state commits coins, undo and one tip

`coax/lib/bitcoin/chainstate.coax`, `chainstate-store.coax` and `blockundo.coax`
own validated schema 1. `V` initializes only from the empty set at the actual
network genesis and refuses `L` metadata. The complete block validator runs
before replay constructs coins, full input metadata and ordered inverses.
`O` chunks retain exact Core CBlockUndo bytes; `H` binds their size and
parent-bound checksum plus every inverse. Coins, chunks, inverses and tip
share one KV batch. A refusal poisons private scratch and has no publishable
batch. Disconnect verifies the complete receipt and every afterimage before
restoring old bytes, including BIP30-overwritten coinbases. Legacy undo
metadata is not guessed or imported. All input views are immutable/disjoint.

The caller publishes only after durable append. The teaching file tool owns
its file exclusively and commits each block separately; later failures retain
already committed earlier blocks. It repairs only a checked torn tail. The
connector does not move the header-index tip or raise validity flags, migrate
light snapshots, implement reorganization/cache policy or grant money
admission. Evidence: chapter 58, its authority mutation receipts and the
independent live Core, historical, exact-undo, crash-cut, native file and
sanitizer comparisons. Historical mainnet fixtures supply an archival parent
and touched coins, not a global mainnet UTXO snapshot or a sync from genesis.

## Validated Bitcoin reorganizations publish only a durable cursor

`coax/lib/bitcoin/chainreorg.coax` alternates disjoint private validated coin
views while detaching to the fork and connecting the preferred target path.
`chainbatch.coax` joins checked unsealed KV bodies before one final frame.
Any refusal publishes neither coins nor an active index; scratch is discarded.
Only after append and sync may the caller publish the replacement snapshot
and bind the index to its exact validated network/height/hash. Complete
transaction-data receipts and immutable archived bodies are prerequisites.
Bounded preferred prefixes are permitted; they do not declare sync complete.

Completed script checks may advance validity caches during preparation.
Only stable contextual consensus verdicts mark the block and known descendants
failed. Framing, Merkle, witness mutation, signet preflight, local context,
damaged undo and capacity failures never authorize cached invalidity. Archive
data/undo ownership flags remain the adapter's responsibility. Recovery trusts
only the checked durable `V` cursor, never a light or unauthenticated imported
snapshot. Chapter 60's models, zero-survivor mutation receipts, native/Core
full-set MuHash comparisons, fork reversals and exhaustive frame cuts qualify
this boundary; they do not grant money admission or prove mainnet sync.

## A Bitcoin write-back cache stays private until one synced flush

`coax/lib/bitcoin/kvwriteback.coax` keeps only final differences from one
immutable checked base. Actual base absence determines whether create/spend
can disappear; spending, re-adding and spending an existing base coin retains
its deletion. Empty values remain distinct from absence. All workspaces are
disjoint, and failed staging changes neither published base nor delta.

`coinscache.coax` accepts only complete successful validated connector/reorg
bodies prepared against its exact private view. It binds the durable `V`
cursor, refuses light/foreign profiles and accounts its state plus both delta
arenas and tagged-record workspace against the configured cache limit.
Database, validation and frame workspaces have separate explicit bounds.
Dirty entries never evade the limit through eviction.

A flush freezes edits, coalesces coins/inverses/undo/receipts/tip into one KV
frame and returns a generation. Only append plus sync permits acknowledgement,
base publication and active-index publication. A stale acknowledgement cannot
clear a newer flush. Abort is allowed only before any append; ambiguous I/O
requires closing and checked recovery. Unflushed private work is replayed from
immutable durable block archives against the recovered cursor. Chapter 63's
authority receipts, full Core coin/MuHash journeys, restart/replay, physical
cuts and real Coax file adapter qualify this bounded native profile. They do
not qualify a mainnet sync, snapshot import or money admission.

## Bitcoin block validation changes only private coins

`coax/lib/bitcoin/validation.coax` requires chapter 59's admitted header tree
and an exact parent UTXO view. It runs every script with the block's consensus
flags, keeps all input scripts stable through each transaction, checks BIP-30
against the pre-block view and makes earlier outputs available to later
transactions. Signet's challenge precedes the transaction Merkle-root check.
Any refusal poisons scratch: discard it. Success publishes no durable tip,
undo, validity level or profile upgrade. The existing light state remains
proof-of-work-only; validated publication belongs to chapter 58.
Chapter 57's rule table, 13 historical fixtures, 35 live Core regtest verdicts,
four testnet4/signet comparisons, native/sanitizer coin comparisons and all
425 root mutations qualify this boundary. Independently supplied historical
parent context is not evidence of an own-node sync or money admission.

## Bitcoin block script rules keep their hash and ancestry context

`coax/lib/bitcoin/scriptflags.coax` selects pinned Core's block script mask:
P2SH, witness and Taproot form the base, full-hash exceptions trim it, and
buried deployment flags are added afterward at the checked block's height.
Policy flags are not added. Its BIP-30 gate skips duplicate-output lookups
only for the exact historical repeat pairs or a supplied known BIP-34
ancestor, and resumes checking at 1,983,702. Missing ancestry keeps checking;
malformed extents, networks and heights refuse. Parameters and ancestor
identity remain the validated block index caller's responsibility. These
queries do not validate a block, permit a UTXO overwrite or replace Core's
required verdict for money. Evidence: chapter 55, root-only mutation checks
and `coax/tests/bitcoin/scriptflags.py`'s independent pinned-source model
against native debug/release selectors and mainnet headers 0–2,016.

## Bitcoin file framing never grants chain validity

`coax/lib/bitcoin/blockfiles.coax` owns Core-compatible block/undo framing,
uint32 file extents, position-dependent XOR and the previous-block-bound
undo checksum. Refused writes and reads leave output bytes unchanged; an
undo body publishes only after its full checksum matches. Callers provide
live disjoint views and exclusive file ownership. A framing scan identifies
physical truncation or entirely zero preallocation, never authenticates block
contents or undo bodies, and refuses malformed nonzero framing without
authorizing truncation. The Coax file tool publishes an append position only
after storage sync and truncates only a reported recoverable tail. It assumes
no concurrent writer, rename, unlink or key change. Chapter 57 validates
block bodies against admitted headers and exact parent coins. Chapter 58
constructs undo and commits it with coins and tip; this framing layer grants
none of that validity itself. Evidence:
chapter 61, its mutation checks and `coax/tests/bitcoin/blockfiles.py` against
Python, captured Core records and real files under ASan/UBSan.

