# The Whispering Spring — first release plan

An album you discover by spending time in a place. Standalone GameBuilder world at
`/whispering-spring` (`gamebuilder/worlds/whispering-spring/`), built like Fated Dragon's: its own
Wasm module, shaders and page, so no shared HUD, character or default world changes.

## The fixed shot

Metres, +y up, the camera looks toward -z. One composed shot: the camera aims at
(0, 0.6, -0.6) from up and behind (pitch about -16°, 50° tall field of view). A screen
narrower than 16:9 dollies the camera straight back until the whole safe box still fits, so
a phone in portrait sees the same grove, only smaller (`ws_camera`).

| Thing                 | Where (x, z)               | Notes |
|-----------------------|----------------------------|-------|
| Cut bank              | arc at z ≈ -3.9, x -9..9   | dark umber, 3.2 m tall, cedar roots hang over its face |
| Pool                  | centre (0.3, -1.6), 1.75 × 1.15 m | blue-green; the warm patch of light lands here |
| Limestone spill lip   | (2.0, -1.1)                | water spills over it into the brook |
| Brook                 | (2.2, -1.1) → (7, 3.5)     | runs off frame to the right |
| Creek-stone pile      | (-2.95, 1.35)              | left foreground |
| Cairn slab            | (-2.35, 0.45)              | the stack goes here |
| Ribbon cedar + branch | trunk (-4.2, -2.6), branch tip (-2.15, 2.35 high, -1.8) | 3 tie points |
| Quail's range         | x -1.4..2.8, z 0.0..1.9    | foreground strip; nothing stands in it or between it and the camera |

The quail's clue spots are inside its range: by the stones (-1.3, 0.9), under the branch
(-1.2, 0.05) and at the pool's front edge (0.3, 0.05). Tests check that the quail never leaves
the range, that the range and every interaction spot project inside the picture at 16:9 and in
phone portrait, and that no scenery stands between the camera and any of them.

## The quail

A small state machine (`quail.coax`): forage → scurry → scan → peck → call → attend-to-clue →
alert → resume. Its attention is the clue system: it scratches by the stones, pauses under the
ribbon branch and lifts its head at the pool, for whichever discovery is still hidden. Each clue
is also sent to the page as text (the Discoveries list), and a soft gold glint marks the spot,
so no clue is sound-only or colour-only.

## The spring's three phrases (seconds after one touch)

| t      | What happens |
|--------|--------------|
| 0.0    | Phrase 1: one ripple ring; the quail goes still (alert) until about 3 s |
| 3.2    | Phrase 2: two more rings; the spillway's broken gold glints brighten |
| 6.8    | Phrase 3, the signature: a curling plume of champagne-gold particles rises for 6 s with the isolated piano/vocal phrase |
| 13.5   | Settles: the track is found, and "Play full track" is offered |

Reduced motion: the plume rises straight and slowly, fewer particles, one ripple only.
Reduced bloom: the gold stays under HDR 1.1, so the post pass barely blooms it.

## Audio manifest

`audio-manifest.json` lists tracks, fragments (the spring's phrases), stems (the cairn's
stone notes), extras (lyric, alternate take, voice note with transcript) and ambience. Every
entry has `file: null` for now and a `placeholder` recipe that `audio.js` synthesizes with
WebAudio; setting `file` to a URL swaps in the artist's recording with no code change.
(The engine's Coax audio reactor, `coax/web/audio-host.js`, wants pinned content-addressed
assets; it is the place for the real files once they exist.)

## Save format

Progress is one line of text the world keeps through the host's keep action (action 4:
IndexedDB `coaxial/kept`, key `whispering-spring`, localStorage if IndexedDB is refused):

    WS1;f=<found bits>;x=<saved extras bits>;c=<stone>:<roll>,...;r=<fabric>,<colour>,<charm>,<branch>,<word>

`WS1` is the version, so an account can later take the same text over and migrate it. Settings
(sound, volumes, captions, reduced motion and bloom, text-first) are per-viewer and live in
localStorage `whispering-spring.settings.v1`.

## Later phases (designed for, not built)

What OpenAgents already has: Nostr sign-in (NIP-07/NIP-98) and the browser playtest player;
app byte storage (`coax/web/STORAGE.md`) and per-slot keeps; WebSocket rooms
(`room-worker.js`) for chat; Lightning tips to a realm's claimed maker
(`docs/metaverse/makers.md`, WebLN in `host.js`). Accounts take over the `WS1` text; a shared
ribbon tree and listening parties sit on rooms with moderation; tips map onto makers. Nothing
involving real payments is built until the artist picks prices and providers.
