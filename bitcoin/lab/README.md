# Bitcoin Lab sources

The unlisted `/worlds/bitcoin-lab` renders actual regtest blocks from the
retained Bitcoin Core 32.1 / Electrs recording in
`bitcoin/fixtures/explorer-core-reorg`. This source is **recorded**, not live.
Its observation time is unknown. No Core or Electrs process is needed to
rebuild it or run the realm. No book chapter gains a visual-exercise status.

Rebuild into a **new** directory, check it independently, then freeze the
acknowledged facts into the realm:

```sh
coax/web/tool.sh bitcoin/lab/record.coax /tmp/bitcoin-recording
python3 bitcoin/lab/oracle.py /tmp/bitcoin-recording
coax/web/tool.sh bitcoin/lab/recover.coax /tmp/bitcoin-recording/state.kv /tmp/bitcoin-cut.kv
coax/web/tool.sh bitcoin/lab/project.coax /tmp/bitcoin-recording apps/openagents-web/bitcoin-lab-data.coax
coax/web/tool.sh bitcoin-lab-map
bun coax/web/realms.ts
coax/web/wasm.sh apps/openagents-web/bitcoin-lab.coax apps/openagents-web/bitcoin-lab-face.c apps/openagents-web/build/bitcoin-lab.wasm
```

`record.coax` starts with empty validated state at the actual genesis. It
admits raw headers/bodies, executes the complete block validator and stages
coins, undo, transaction/address archives and V/X metadata together. A private
CoinsCache flush is appended and fsynced before its base, cache generation
and active index are published. The three durable states are:

| Snapshot | Height | Detach | Attach | Retained blocks |
| --- | --- | --- | --- | --- |
| A | 102 | 0 | 102 | 103 |
| B | 103 | 2 | 3 | 106 |
| C | 104 | 3 | 4 | 108 |

Schema-2 metadata (`X`, byte 2) is 48 bytes: schema, network, height,
next archive sequence (four little-endian u32s), then the committed hash.
Only validated V origins enter these writers. L/I/E origins are refused;
there is no conversion or migration from the old pow-only explorer. A
disjoint disposable query view shortens checked H receipts for the existing
read-only codecs. That view contains foreign compatibility metadata, so the
validator refuses it. It is never persisted, cached or used to label a source.

The recorder's JSON is an internal build artifact. `project.coax` writes
immutable typed Coax arrays for the offline Recording source. The realm
changes all facts together when A/B/C changes. The independent
Python oracle parses raw transactions/blocks, computes txids and weight,
replays active UTXOs, and checks output/spend states and retained Core/Electrs
answers. `recover.coax` writes and reloads physical journal cuts and checks
that V, X, coins and indexes recover at the same complete frame.

This is a deliberately bounded source: 128 headers/blocks, 16 transactions
per block and 16 inputs/outputs per transaction; all excesses refuse the
recording. The realm has 103/106/108 retained blocks and shows a bounded
height window plus stale blocks, with explicit omitted counts.

The implemented alpha adapter contract is in
[`docs/protocol/bitcoin-lab-v1.md`](../../docs/protocol/bitcoin-lab-v1.md).

The native app defaults to **Fixture API**, served by its owned native Coax
process. **Recording** works offline without that process. **Own Node** is
explicitly selected and unavailable until configured. Its metadata comes from
one complete committed journal: L/I/E stays pow-only; acknowledged V/X stays
validated-v1. The reader never repairs or migrates state. No Bitcoin Core,
Electrs, Python, browser or public explorer is needed at runtime. The service
does not start a node or perform administrative RPC.

Build and run the standalone loopback service from this checkout:

```sh
coax/web/tool.sh bitcoin/lab/build.coax
bitcoin/lab/build/native/bitcoin-lab --port 8835
# Optional configured node source (absolute operator-owned paths):
bitcoin/lab/build/native/bitcoin-lab --port 8835 --state /path/state.kv --blocks /path/blocks --chain regtest
```

`--blocks` accepts the block directory or its single `blk00000.dat` file.
This first reader supports unmasked block files. Pruned/missing bytes and
local capacity failures refuse the whole view. A snapshot is read afresh for
each request; a file publication during a response returns 409. Source,
profile, network, cursor and tip changes clear related amounts together.
The client owns two requests, unique monotonic tickets, cancellation and
500 ms–30 s transport backoff; replies stop at 2 MiB.

Windows page 24 blocks including genesis; transaction pages contain at most
32 occurrences; graph pages contain at most 16 inputs and outputs. `< PAGE`
and `PAGE >` expose the rest with explicit totals/omitted counts. FIND accepts
public identifiers. HISTORY and OUTPUTS query an exact script, with no wallet
ownership inference. Fixture history retains stale occurrences; node history
uses the active index. Unknown fees, parent receipts, cumulative work and
observation/background time stay unknown. The ordinary explorer/table, exact
selection links, large-page navigation and admitted replay are described below.

The Linux installer packages `bitcoin-lab-api` and `bitcoin-lab-fetch` beside
`aethyrforge-native` in `~/.openagents/bin`. Optional node configuration for
the existing user service belongs in `~/.openagents/aethyrforge/lab.env`:

```ini
BITCOIN_LAB_STATE=/absolute/path/state.kv
BITCOIN_LAB_BLOCKS=/absolute/path/blocks
BITCOIN_LAB_CHAIN=regtest
# Optional startup selection; selecting Own Node in the app also works.
METAVERSE_ENGINE_LAB_SOURCE=own-node
```

No configuration file is created automatically. `METAVERSE_ENGINE_LAB_SOURCE=recording`
selects the offline fallback. Restart the user's existing app service after
changing its environment file. The build does not change shared model files,
provider credentials or the standalone `openagents` command.

Browser checks use the same API through `coax serve --api 8835`. Qualification
and its exact source commit are recorded in
[`docs/verification/bitcoin-lab-12803.md`](../../docs/verification/bitcoin-lab-12803.md).
No mainnet validation or chapter promotion follows from these regtest checks.

## Explorer, exact selections and replay

EXPLORER opens the ordinary table inside the same native app or browser realm;
GRID returns to the spatial view with the same admitted object. BACK restores
a bounded breadcrumb, including page and selected outpoint. BLOCK shows full
header metadata; TXS/DETAILS returns to transaction rows; INPUTS, OUTPUTS and
SCRIPT expose the selected transaction. HISTORY matches the complete script,
not a person or ownership cluster. UTXOS separates unspent/immature coins from
historical/unspendable output occurrences on each page. Integer balances include
immature outputs and confer no spending authority.

PAGE traverses bounded HTTP pages (24 blocks, 32 transactions, 16 links).
ROWS traverses wrapped text, with two overlapping lines so a touch link is never
permanently split across pages. Full hashes and scripts are readable there;
the Grid retains relative weight tiles with a declared 12% floor. Fee colors
use sat/vbyte bands only when the fee is known; unknown fees are gray.

A/B/C scrubs acknowledged fixture states, PLAY/PAUSE advances every six seconds,
and STEP advances once. Active/stale lanes animate only after a complete window
has been admitted; amounts never interpolate. MOTION toggles reduced motion.
STATUS includes complete cumulative-work values for both replay tips. Exact
selected occurrences remain stale when detached; missing objects and mismatched
saved cursors are explicitly unavailable. Keyboard: T table/Grid, Backspace back,
B block, P playback, V reduced motion, PageUp/PageDown text rows, Tab/Enter pick.

LINK writes a closed public `lab1/` selection token. The browser stores it in the
URL fragment. The native app copies it through `wl-copy` and atomically saves it
at `~/.openagents/aethyrforge/selection.txt`, then restores it on startup unless
`METAVERSE_ENGINE_LAB_SOURCE` explicitly selects a source. Tests override that path
with `METAVERSE_ENGINE_LAB_SELECTION_FILE`; tokens cannot select hosts or filesystem
paths. Cursor changes refuse restoration rather than changing occurrence.

The large-page qualification fixture is generated by
`coax/web/tool.sh bitcoin/lab/stress-record.coax NEW_DIRECTORY`: it extends C
with a mined height-105 block, 72 fully validated transactions and a 40-input,
40-output transaction. This is synthetic public regtest qualification material,
separate from retained Core/Electrs A/B/C, and never a production node claim.
`stress-oracle.py` independently checks its raw hashes, Merkle root, PoW,
amounts, spend links and script balance. `scene-check.coax PORT` drives the real
API through all pages and graph/table layouts under native sanitizers.
