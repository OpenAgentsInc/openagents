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

    WS1;f=<found bits>;x=<saved extras bits>;c=<stone>:<roll>:<x cm>:<z cm>,...;d=<kind>:<x cm>:<z cm>,...;r=<fabric>,<colour>,<charm>,<branch>,<word>

(A cairn saved before the offsets, `c=<stone>:<roll>`, still loads.)

`WS1` is the version, so an account can later take the same text over and migrate it. Settings
(sound, volumes, captions, reduced motion and bloom, text-first) are per-viewer and live in
localStorage `whispering-spring.settings.v1`.

## Second pass (built)

- **Look:** dusk, toward the client's reference art. A low warm sun to the left of the grove, a
  banded umber cut bank with roots, twisted live-oak and cedar trunks framing the shot, low-poly
  boulders and agave clumps, and a brook that steps down from the pool toward the camera.
- **The quail stands for the musician.** It is upright and composed, with a slate breast, a white
  brow and necklace, and a teardrop plume. It scurries less and pauses more. Tap it, or use
  "Greet the quail", and it turns to you and speaks.
- **Speech:** a greeting on arrival, a prayer when greeted, a blessing for each ribbon, and a line
  once every song is found. The lines live in `audio-manifest.json` `voice` (placeholders for the
  musician to rewrite). They type into a bubble over the quail in a cozy-game babble voice
  (`audio.js` `speak`) and are announced to screen readers. The voice can be turned off in Settings.
- **Six prayer limbs**, six tie points each (36), named in the ribbon form (`grove.coax`
  `WS_LIMB_AT`). The camera's fit check includes every tie point and its hanging ribbon.
- **A shared grove:** every visitor sees the same ribbons and a live chat. Both use the OpenAgents
  API's moderated rooms (`coax/web/api/room.c`): a playtest guest key, a 500-character limit,
  5 lines per 10 s, the last 50 lines, mutes and bans, and admin removal of a line.
  - `whispering-spring` is the chat.
  - `whispering-spring-ribbons` holds one line per ribbon (`ribbon1|f,c,ch,limb,slot|word`).
    Every screen places them the same way (`room.js` `placeRibbons`): newest first, at most 36.
  - Words are shared only when the visitor leaves "Let other visitors read my words" ticked.
    Any visitor can hide a ribbon, or every word from a name, on their own device.
  - gamebuilder.org routes those paths to the `oa-api` service (`web-deploy.sh domain`).
- **The frame from the client's mockups:** a title page with Obsidian Quail across the bank,
  "Enter the Grove" as a ripple on the pool and "Listen to the Album" on a stone. In the grove,
  an Album Quest card, a small Album Journal with sealed songs, a callout pinned to the next spot,
  a sound toggle, and a Live Chat · Menu · Quiet View bar.

Not yet: a dedicated ribbon API with a report button (a ribbon line can only be removed by an admin
today), a display font for the title (the visitor's serif stands in), and painted or modelled
assets (see the client doc's asset prompts).

## Third pass (built)

- **Lore, not songs:** each ritual opens a page of the musician's and the album's lore
  (`audio-manifest.json` `lore.pages`, placeholders for the artist to write). The quest card counts
  lore pages, and the journal holds them to read, keep (the save's `x` bits) or download.
  "Listen to the Album" stays on the title page.
- **The spout:** a carved limestone mouth in the bank above the pool (`grove.coax` `WS_SPOUT`), its water
  drawn live as a glowing arc into the pool with a splash.
- **Camera:** low, tilted a little up (`WS_BACK` y below 0, `WS_AIM` raised). The fit check still holds the
  safe box and every tie point at every aspect ratio.
- **Edges:** four more trees and six bare framing limbs (`WS_FRAME_LIMB_AT`), all in the
  nothing-in-the-way test's props.
- **Dusk and glow:** the engine's painted sky (zenith lavender, horizon peach-rose, sunset glow), a low
  sun over 1 so lit surfaces bloom, 36 fireflies, glowing ribbons, stream and splash. With reduced
  bloom, everything stays under 1.1.
- **Dusk interface:** plum and violet panels, rose glow, gold lines (`index.html` `:root` tokens).
- **The asset kit's models:** `gamebuilder/tools/grove-bake.coax` bakes the GLBs into `assets/grove.wsg`;
  the world loads it as its asset and places the bank, cedars, limestone and roots (`WS_PLACE_AT`). The
  hand-built pieces stand in until it arrives. The kit's pool isn't placed, because its layout is for the
  kit's orthographic camera, not this shot.

## Later phases (designed for, not built)

What OpenAgents already has: Nostr sign-in (NIP-07/NIP-98) and the browser playtest player;
app byte storage (`coax/web/STORAGE.md`) and per-slot keeps; WebSocket rooms
(`room-worker.js`) for chat; Lightning tips to a realm's claimed maker
(`docs/metaverse/makers.md`, WebLN in `host.js`). Accounts take over the `WS1` text; a shared
ribbon tree and listening parties sit on rooms with moderation; tips map onto makers. Nothing
involving real payments is built until the artist picks prices and providers.

## Fourth pass: laid out around the asset kit (built)

The grove now follows the kit's Blender layout. The kit's scene sits at `WS_KIT` (3.2 m toward the camera,
0.55 m down), so its bank and pool line up as in the kit. The kit's pool, spill sheet, stream and sill are
placed; the ground is carved under the basin (`ws_basin_in`) and the stream (`WS_STREAM_HALF`), with a low
rim round both, and the stream is carried on to the frame's bottom. The quail's strip is the near bank
left of the stream (`WS_RANGE`), with the creek stones and cairn beyond it on the left. The three
prayer-limb trunks keep their limbs and wear the kit cedar's crowns. The camera stands at a fixed low
height, tilted up (`WS_TILT`), and moves straight back until the safe box fits; a phone held upright gets a
wider lens. The fit, nothing-in-the-way and long-visit tests all pass at 16:9, portrait and ultra-wide.

## Fifth pass: hands-on stones and decorating (built)

- **The creek stones moved to the front left**, near the camera (cairn at (-3.75, 3.9), pile round (-2.6, 4.5)),
  outside the quail's strip and its sight lines, and the stones are 1.7 times their old size and lighter in colour.
  A test keeps the pile and a full cairn in the picture at 16:9, phone portrait and ultra-wide.
- **Click to place, with faux physics** (`world-spring.coax` "the cairn"): a click on a pile stone lifts it
  (no panel). It hangs over the stack and follows the pointer (a ray to the stack's top plane). A click lets it go:
  it falls under gravity and either settles with a wobble, keeping where it landed, or tips off, tumbles to the
  ground and rolls back to the pile. A ring under it shows gold where it will hold and rose where it will fall,
  and the page's hint says the same. The wheel (`coaxial_wheel`) or Q / E turns it. Esc, or a click far away,
  puts it back.
- **Easy by design:** a stone comes up at its best angle. Within its support plus `WS_MAGNET` it is drawn to the
  middle, and it holds within `WS_HOLD_SHARE` of the support's half width. Shape (a round stone below, or a wider
  stone on top) only wobbles it, so no stack is a dead end. Only a stone turned well past its tolerance, or let go
  well off the stack, falls. Assisted placement always lands it centred. The panel and keys drive the same physics.
- **Decorating** (`WS_MSG_DECOR`, the Decorate button): a basket of six things. Pick one and it hangs under the
  pointer with a ring (gold where it can stand, rose in the water, the stream or on the cairn). A click sets it down,
  with a small drop and bounce. While decorating, a click on one already set picks it up to move it. Arrows and
  Enter work from the keyboard. Up to 16 are kept in the save. Lantern, candles and crystal glow (under 1.1 with
  reduced bloom).

## Sixth pass: phones (built)

- **An upright phone frames the grove closer** (aspect under WS_TALL_ASPECT): a 56° lens and a smaller first
  view (WS_SAFE_TALL: the quail's strip, the pool, the spout) instead of backing off until the whole grove fits.
  A sideways swipe with nothing in hand slides the camera along x (within WS_PAN). Touching a spot (a tap, the
  callout or a button) glides there, and finishing the cairn or tying a ribbon glides back. Tests check every spot
  and tie point is in the picture once glided to, and that a swipe touches nothing.
- **The page on a phone:** the title page stacks its parts with no overlap. The stones and decorating panels are
  short trays, with the basket in one row, so the ground stays free to tap. The callout carries an arrow toward a
  spot off the side of the screen, and the hints use tap wording. A one-time hint says a swipe looks around.
- Touch play: tap a pile stone, then tap where it should go (or drag it there); the same for decorations.
