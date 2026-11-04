# Bitcoin Lab

Block-view milestone for [#12803](https://github.com/OpenAgentsInc/openagents/issues/12803)
and [#12804](https://github.com/OpenAgentsInc/openagents/issues/12804), after
[#12802](https://github.com/OpenAgentsInc/openagents/issues/12802), in
[#12801](https://github.com/OpenAgentsInc/openagents/issues/12801).
Direct route: **`/worlds/bitcoin-lab`**. Module and save-slot identity:
`bitcoin-lab`. The world is **`listed:false`**. Finishing this work does not
authorize listing it.

This is a **recorded regtest / validated-v1** explorer with seven stations
and all 125 Bitcoin book chapters. The retained Core recording is rebuilt
from genesis through our complete validator, not relabeled from the old
pow-only index. Blocks, transactions, scripts, satoshi amounts, output states
and exact spend occurrences come from acknowledged commits. Observation time
was not retained and is explicitly unknown. It has no live connection, keys,
signing, custody or ownership inference. Every chapter's visual exercise
remains unavailable; this scene promotes no book chapter. The published
explorer keeps its separate pow-only contract.

Select a solid block slab, or use the block arrows (`[` / `]`). TXS opens its
transactions; T tiles show relative transaction weight with a 12% minimum
width, I tiles navigate exact input outpoints, and O tiles select outputs.
OUTPUT cycles outputs and SPEND follows the exact spending block/transaction.
FIND (`/`) accepts a height, block hash, txid, address or bounded script; it
selects a matching occurrence, preferring the active chain. It is not an
address ownership/history report. LINK puts phase/block/transaction in the
URL fragment. The snapshot tip hash and selected block hash remain distinct.

A/B/C select acknowledged states at heights 102/103/104. STEP (`R`) advances
and PLAY toggles six-second replay. B detaches two A blocks and attaches
three B blocks; C detaches three B blocks and attaches four A blocks. A
selected detached block remains stale rather than silently changing identity.
All shown facts change together; there are no asynchronous fixture reads.
Blue slabs are active, brown stale and orange selected. Each equal-sized
slab represents one block. Exact size, weight, chainwork, header time,
subsidy and known fees are inspector values. Window/retained/omitted counts
describe the bounded recording, not a claim of mainnet completeness.

## Walk and select

Spawn is `(x=0,z=-7)`, facing the Chain station at `(0,0)`. The slabs are
recorded blocks. Plinth outlines mark the flat walking
floor; they are not solid collision bodies.

| Station | Centre (x,z), metres |
| --- | --- |
| Chain | 0, 0 |
| Transactions / UTXOs | -8, 7 |
| Proof / Script | 8, 7 |
| Node | -12, 15 |
| Mempool / Mining | 12, 15 |
| Wallets | -8, 23 |
| Lightning | 8, 23 |

WASD walks; Shift walks slowly; Space jumps; left/right arrows turn the
follow camera. The left touch gesture walks and the right looks. A moving
thumb draws its origin and displacement. The station bar, visible world
labels, and keys 1–7 select the same station. N selects the next station.
Selecting any UI action releases walking; the header and chapter panel
also consume non-actionable touches. Tab/Enter and the host's screen-reader
mirror use the same Aethyr UI actions. Escape leaves focus, then exits to
Worlds. M or MAP toggles the chapter panel. Reduced motion preserves walking.

The chapter panel browses all 125 chapters, including other stations, with
Previous/Next or PageUp/PageDown. Station selection starts at that station's
first chapter. Source opens a real chapter file, or the exact criteria row
in `bitcoin/book/PLAN.md` for a chapter whose file does not exist yet.
The generated HTML contains the entire table inside its hidden `<main>`:
it is the text fallback without WebGPU and remains available to assistive
technology. It also includes every retained block, input outpoint, output,
full identifier, script and spend occurrence in all three snapshots. The
Markdown twin contains the chapter and complete block-summary tables.

## Data and shared engine

[`coax/web/bitcoin-lab-map.coax`](../../coax/web/bitcoin-lab-map.coax) reads
`bitcoin/book/chapters.json` and writes the compiled chapter table and the
marked Markdown/HTML fallback rows. Book status is copied exactly, separately
from visual-exercise availability. The primary station map is in
`map_station`; each chapter has one primary station, including cross-cutting
verification chapters at Node. The generated table is the complete mapping.

[`bitcoin-lab.coax`](../../apps/openagents-web/bitcoin-lab.coax) embeds the
existing `verse/gridwalk.coax` session: Aethyr's walker, 1/64-second clock,
input buffer, thumbs, follow camera and replay record. It does not run a
second movement loop. Aethyr's HUD stroke writer and UI tree supply drawing,
picking and focus. The GPU module uses the shared scene/depth pass for solid
procedural meshes and the existing Filament stroke pass for labels and HUD.
CPU Coax supplies homogeneous clip vertices from that same follow camera;
there is no private renderer. There are no downloaded assets or external
Bitcoin libraries. Store-only scenes bind after pipeline creation, including
when there are no fetched assets.

`bitcoin/lab/record.coax` initializes empty V state, admits retained raw
headers/bodies, and attaches transaction/address archives in the same batch
as coins, undo and the schema-2 X cursor. CoinsCache prepares each flush;
append+fsync precede base publication, cache acknowledgement and active-index
publication. A disposable V-to-query view supplies existing read-only index
codecs. The view cannot enter the validator and is never persisted. L-to-V
conversion is refused. `project.coax` freezes the acknowledged facts into
`bitcoin-lab-data.coax`; `oracle.py` independently checks raw bytes, txids,
weights, active ancestry, UTXOs and retained Core/Electrs answers.

Mutable realm state, including the entire walking session and UI, lives in
the host-owned `BitcoinLab` region. The C face only exposes scalar effects
and derived accessibility transport bytes. The optional
`coaxial_scene_ready(state)` export lets a stroke world signal that its world
geometry was written. The host waits for that frame's GPU submission before
removing the loader. Readback uses that same frame texture and releases its
temporary GPU buffer.

## Bounds and discovery

- 7 stations, 125 chapter records, at most 96 UI nodes and 4 pointers.
- Three immutable snapshots with 103/106/108 retained blocks respectively;
  at most 128 blocks per recorded state and 16 transactions/inputs/outputs per
  entity. Rendering shows a 6-block phone or 10-block desktop height window,
  plus archived stale blocks in that window, with explicit omitted counts.
- Frame budget: 8 fixed steps, 70,000 vertices, 3 draws, 5,000,000 arena bytes.
  Physical vertex capacity: 72,000 twelve-float vertices; 88 uniform floats.
  The scene has a separate 4,096-vertex mesh store, included in arena and
  vertex accounting. The Wasm release budget is 270,000 bytes, including the
  immutable block/transaction recording. Native and browser evidence is
  recorded below; live source selection remains future work.

The registry filters normal Worlds, homepage/Start galaxy selection and
galaxy picking; `?realms=all` remains deliberate diagnostics. CLI discovery
does not acquire this static fixture. Generated unlisted pages have
`noindex, nofollow`. Deployment calls `discovery-realms.coax` to exclude
every unlisted realm directory, including child pages, from `sitemap.xml`
and `llms-full.txt`; listed:false alone previously did not do that. This is
discovery exclusion, not authentication. Direct HTML and Markdown still ship.

## Qualification and remaining work

See [the recorded Linux checks and screenshots](../verification/bitcoin-lab-12802.md).
See also [the block-view evidence](../verification/bitcoin-lab-blocks.md).
#12803 and #12804 stay open: the alpha HTTP contract still needs a same-origin
allowlisted live adapter, bounded graph paging, source/ticket invalidation
and live-source recovery. Full script-history pages, the ordinary explorer
handoff, general expansion/cancellation and reorg animation/restore gates
remain. The immutable fixture viewer does not claim those criteria are done.
Keep the realm unlisted.
