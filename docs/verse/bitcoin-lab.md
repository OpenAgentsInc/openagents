# Bitcoin Lab

Block-view milestone for [#12803](https://github.com/OpenAgentsInc/openagents/issues/12803)
and [#12804](https://github.com/OpenAgentsInc/openagents/issues/12804), after
[#12802](https://github.com/OpenAgentsInc/openagents/issues/12802), in
[#12801](https://github.com/OpenAgentsInc/openagents/issues/12801).
Direct route: **`/worlds/bitcoin-lab`**. Module and save-slot identity:
`bitcoin-lab`. The world is **`listed:false`**. Finishing this work does not
authorize listing it.

The default native source is **Fixture API / regtest / validated-v1**, rebuilt
from genesis through our full validator. Recording remains the offline fallback;
Own Node reads only explicitly configured committed journals and authenticated
block files, preserving their original validation profile. Every view carries
network, source/profile, exact snapshot identity and unknown observation time.
The seven stations and all 125 chapter statuses remain available in MAP; no
chapter or unavailable visual exercise is promoted.

Selectable blocks open bounded, weight-scaled transaction tiles. Exact input
outpoints lead to creating occurrences, outputs retain integer amounts,
scripts/addresses/status, and SPEND follows the exact spending occurrence.
FIND accepts height/hash/txid/address/exact script. HISTORY and UTXOS use the
complete script without inferring people, change or individual satoshi paths.

EXPLORER is the ordinary table inside the native app, over the same admitted
objects as the Grid. GRID and BACK preserve selection and bounded navigation;
BLOCK, TXS/DETAILS, INPUTS, OUTPUTS and SCRIPT expose full facts. PAGE moves
24/32/16-element HTTP pages; ROWS moves bounded wrapped text, including complete
hashes and scripts. LINK preserves exact source/network/cursor/occurrence and
page, using a browser fragment or the native public selection file/clipboard.
See [the source guide](../../bitcoin/lab/README.md) for commands and shortcuts.

A/B/C scrubs acknowledged fixture states, STEP advances once and PLAY/PAUSE
advances every six seconds. Active/stale lanes animate detach/attach only after
complete admission; integer amounts change atomically. STATUS compares full
cumulative work. MOTION (V) disables interpolation. A detached selected object
remains stale; absent objects and incompatible saved cursors are unavailable.
The published explorer v1 keeps its existing pow-only contract; the in-app
table uses the separate Lab projection.

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
mirror use the same Metaverse Engine UI actions. Escape leaves focus, then exits to
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
existing `metaverse/gridwalk.coax` session: the OpenAgents Metaverse Engine's walker, 1/64-second clock,
input buffer, thumbs, follow camera and replay record. It does not run a
second movement loop. The Metaverse Engine's HUD stroke writer and UI tree supply drawing,
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
  recorded below; explicit own-node selection is qualified in #12803.

The registry filters normal Worlds, homepage/Start galaxy selection and
galaxy picking; `?realms=all` remains deliberate diagnostics. CLI discovery
does not acquire this static fixture. Generated unlisted pages have
`noindex, nofollow`. Deployment calls `discovery-realms.coax` to exclude
every unlisted realm directory, including child pages, from `sitemap.xml`
and `llms-full.txt`; listed:false alone previously did not do that. This is
discovery exclusion, not authentication. Direct HTML and Markdown still ship.

## Qualification and scope

See [the foundation checks](../verification/bitcoin-lab-12802.md),
[recorded blocks](../verification/bitcoin-lab-blocks.md),
[own-node snapshots](../verification/bitcoin-lab-12803.md) and
[explorer/replay qualification](../verification/bitcoin-lab-12804.md).
The owner reduced the epic to original 02/30 and 03/30. Other open Bitcoin Lab
work is canceled as not planned; book chapter state is unchanged. Qualification
is Linux/native and muted headless desktop/phone, public regtest fixtures and
bounded readers. It does not claim mainnet, Mac or physical-phone validation.
The realm remains unlisted.
