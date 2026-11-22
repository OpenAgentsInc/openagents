# The Whispering Spring

Obsidian Quail's album as a place: `/whispering-spring` on gamebuilder.org. One fixed, composed shot of
a low-poly Texas Hill Country grove at dusk. An obsidian quail, who stands for the musician, keeps the
grove: she greets visitors, speaks prayers, and leads them to three small rituals. Each ritual opens a
page of the musician's and the album's lore. Everyone who visits sees the same prayer tree and can talk
in a live grove chat. "Listen to the Album" plays the music on its own.

- **Listen to the water**: touch the pool; it answers in three phrases, the last with a curling plume
  of champagne gold. A lore page opens.
- **Stack creek stones**: click a stone in the pile to pick it up, carry it over the cairn and click to
  let it go. It falls, and settles with a wobble, or tips off and rolls back to the pile (faux physics).
  It is meant to be easy: a stone comes up the right way round and is drawn to the stack's middle.
  The wheel or Q / E turns it. Four stones sound a chord; assisted placement always holds. A lore page opens.
- **Decorate**: a basket of six things (lantern, candles, bluebonnets, crystal, sage bundle, offering
  bowl) to set about the grove, move or clear; up to sixteen, kept in the save.
- **Hang a prayer ribbon** on one of six limbs: fabric, colour, charm, an intention (shared only if
  the visitor chooses). It joins everyone's ribbons on the tree. A lore page opens.
- **Greet the quail**: she turns to you and says a prayer in a cozy-game babble voice.
- **Album journal**: the lore pages found, to read, keep or download.

Plan, layout, timeline, rooms and save format: [PLAN.md](PLAN.md).

## Files

| File | What it is |
|------|------------|
| `world-spring.coax` | the world: camera, picking, the spring, cairn, ribbons (own and shared), the spout's water, fireflies, saves, the modelled pieces' loading, events to the page, tests |
| `quail.coax` | the quail's state machine (with its speak state) and its faceted mesh |
| `grove.coax` | the grove's layout, its hand-built mesh, the six prayer limbs, and where each modelled piece is placed |
| `spring-tune.coax` | every tunable: quail timing, particles, bloom, audio levels |
| `spring-layout.coax` | the frame block shared with the shaders |
| `gpu/` | shaders (the engine's stylized shading; the live grove reuses Fated Dragon's HDR bloom path) |
| `whispering-spring-face.c` | the scalar Wasm face for `coax/web/host.js` |
| `models/*.glb` | the modelled pieces from the Obsidian Quail grove asset kit: earthen bank, cedar (its crowns also top the three prayer-limb trunks), limestone, exposed roots, and the pool with its spill sheet, stream and sill. The grove is laid out around the kit's scene, placed at `WS_KIT` |
| `index.html`, `glue.js` | the page: title page, in-grove frame, panels, journal, settings, text-first grove, speech bubble, chat |
| `room.js` | the shared grove over the OpenAgents API's rooms: guest key, chat, everyone's ribbons |
| `audio.js`, `audio-manifest.json` | sound and words: lore pages, the quail's lines, recordings when present, synthesized placeholders until then |

`gamebuilder/tools/grove-bake.coax` bakes `models/*.glb` into `assets/grove.wsg` (one binary of coloured
triangles, 16-bit fixed point; about 18 KB). `web/build.sh` rebakes it when a model or the baker
changes. The world loads it as its one asset; until it arrives, or if it never does, the hand-built
bank, trees, rocks and roots stand in. To swap or add a model: replace the GLB (flat base colours,
root nodes with translation, rotation and scale; the baker refuses textures and nested nodes), then
adjust its placements in `grove.coax` `WS_PLACE_AT`.

```
coax/bin/coax test gamebuilder/worlds/whispering-spring/world-spring.coax --library
coax/bin/coax test gamebuilder/tools/grove-bake.coax
gamebuilder/scripts/web.sh          # http://127.0.0.1:8810/whispering-spring
```

Chat and shared ribbons need the OpenAgents API behind `/api/playtest/` (on gamebuilder.org:
`web-deploy.sh domain` routes those paths to `oa-api`). Without it the grove still plays; the chat
says it can't be reached and only the visitor's own ribbon hangs.

## Credits and licences

- Earthen bank, cedar, limestone, exposed roots: the Obsidian Quail grove asset kit, supplied by the
  project's owner for this world.
- Everything else in the grove, the quail and the effects: procedural geometry written for this world.
- Shading: the Metaverse Engine's stylized shaders (this repository).
- Sound: synthesized in `audio.js`; no samples. Lore pages, the quail's lines, album audio and titles
  are placeholders until the artist supplies hers, and will be credited here as they arrive.
- Fonts: the visitor's own system fonts (nothing is downloaded).
