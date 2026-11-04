# Bitcoin Lab

Issue [#12802](https://github.com/OpenAgentsInc/openagents/issues/12802),
step 01 of [#12801](https://github.com/OpenAgentsInc/openagents/issues/12801).
Direct route: **`/worlds/bitcoin-lab`**. Module and save-slot identity:
`bitcoin-lab`. The world is **`listed:false`**. Finishing this work does not
authorize listing it.

This first slice is a **FIXTURE**, with seven stations and all 125 Bitcoin
book chapters. It has no node connection, chain cursor, transaction graph,
amounts, addresses or validation verdicts. Every chapter's visual exercise
is explicitly unavailable. The existing explorer remains pow-only. No book
chapter is promoted by this scene.

## Walk and select

Spawn is `(x=0,z=-7)`, facing the Chain plinth at `(0,0)`. The linked blocks
are decorative fixture geometry. Plinth outlines mark the flat walking
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
technology. The Markdown twin contains the same table.

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
picking and focus. The GPU module adapts the existing splash/Filament shader
to the HUD's twelve-float vertices; geometry and projected label placement
are Coax. There are no downloaded assets or external Bitcoin libraries.

Mutable realm state, including the entire walking session and UI, lives in
the host-owned `BitcoinLab` region. The C face only exposes scalar effects
and derived accessibility transport bytes. The optional
`coaxial_scene_ready(state)` export lets a stroke world signal that its world
geometry was written. The host waits for that frame's GPU submission before
removing the loader. Readback uses that same frame texture and releases its
temporary GPU buffer.

## Bounds and discovery

- 7 stations, 125 chapter records, at most 20 UI nodes and 4 pointers.
- Frame budget: 8 fixed steps, 70,000 vertices, 2 draws, 5,000,000 arena bytes.
  Physical vertex capacity: 72,000 twelve-float vertices; 88 uniform floats.
- Qualified state: 134,592 bytes; arena: 3,590,944 bytes. The compiled native
  sweep of all chapters on three layouts peaks at 11,616 vertices, with no
  budget refusals. The Wasm release budget is 220,000 bytes.
- No data requests, refresh loop, graph expansion or live-source selection
  exist in this slice. Source/exit navigation is the only network effect
  requested by the realm.

The registry filters normal Worlds, homepage/Start galaxy selection and
galaxy picking; `?realms=all` remains deliberate diagnostics. CLI discovery
does not acquire this static fixture. Generated unlisted pages have
`noindex, nofollow`. Deployment calls `discovery-realms.coax` to exclude
every unlisted realm directory, including child pages, from `sitemap.xml`
and `llms-full.txt`; listed:false alone previously did not do that. This is
discovery exclusion, not authentication. Direct HTML and Markdown still ship.

## Qualification and next slice

See [the recorded Linux checks and screenshots](../verification/bitcoin-lab-12802.md).
The next ordered issue is
[#12803](https://github.com/OpenAgentsInc/openagents/issues/12803): the
versioned same-origin snapshot adapter and atomic connection of validated
V state to indexing. It must preserve published explorer/mempool contracts,
invalidate related views on every source/network/profile/tip change, and
never relabel a pow-only L cursor as validated. Keep the realm unlisted.
