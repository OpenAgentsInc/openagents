# The Whispering Spring

An album you discover by spending time in a place: `/whispering-spring` on gamebuilder.org.
One fixed, composed shot of a low-poly Texas Hill Country cedar grove. An obsidian-black quail
goes about its day; the grove answers when you touch something or listen closely, and each
discovery reveals a track.

- **Listen to the water**: touch the pool once; it answers in three phrases, the third with a
  curling plume of champagne-gold particles. Then "Play full track".
- **Stack creek stones**: pick, turn and set four stones; each sounds a note of a chord, so the
  finished cairn plays a voicing you made. Assisted placement gives the same reward.
- **Hang a prayer ribbon**: fabric, colour, charm, a word, a branch; it catches the wind.
- **Album journal**: found tracks, a lyric, an alternate take and a voice note (with transcript);
  play, keep or download them.

Plan, layout, timeline, audio manifest and save format: [PLAN.md](PLAN.md).

## Files

| File | What it is |
|------|------------|
| `world-spring.coax` | the world: camera, picking, the spring, cairn, ribbon, saves, events to the page, tests |
| `quail.coax` | the quail's state machine and its faceted mesh |
| `grove.coax` | the grove's layout and static mesh |
| `spring-tune.coax` | every tunable: quail timing, particles, bloom, audio levels |
| `spring-layout.coax` | the frame block shared with the shaders |
| `gpu/` | shaders (the engine's stylized shading; the live grove reuses Fated Dragon's HDR bloom path) |
| `whispering-spring-face.c` | the scalar Wasm face for `coax/web/host.js` |
| `index.html`, `glue.js` | the page: landing, panels, journal, settings, text-first grove |
| `audio.js`, `audio-manifest.json` | sound: recordings when present, synthesized placeholders until then |

```
coax/bin/coax test gamebuilder/worlds/whispering-spring/world-spring.coax --library
gamebuilder/scripts/web.sh          # http://127.0.0.1:8810/whispering-spring
```

## Credits and licences

- Grove, quail, stones, ribbon and gold: procedural geometry written for this world (no models).
- Shading: the Metaverse Engine's stylized shaders (this repository).
- Sound: every sound is synthesized in `audio.js`; no samples. All album audio, lyrics, titles and
  the voice-note transcript are placeholders until the artist supplies them; each will be
  credited here and in `audio-manifest.json` as it arrives.
- Fonts: the visitor's own system fonts (nothing is downloaded).
