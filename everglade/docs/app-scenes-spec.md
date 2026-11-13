# Worlds in the Everglade apps (spec)

Written 2026-11-10. Status: **the list and all four worlds are built on the
Mac (Everglade itself, the Workshop, the Archery Range and Everglade
Experimental), with the realms' sound, and on iOS, where they ran in the
simulator; Android has the Workshop.**

The Everglade apps are an experiment beside the main experience, which is
the web app. They are where the next work, managing agents from inside a
world, is being set up. They open on a list of worlds and you hop between
them.

## Contents

1. [The four worlds](#1-the-four-worlds)
2. [How the list works](#2-how-the-list-works)
3. [How a host draws a world](#3-how-a-host-draws-a-world)
4. [Bringing the web's worlds into the apps](#4-bringing-the-webs-worlds-into-the-apps)
5. [Adding a world](#5-adding-a-world)

## 1. The four worlds

| World | What it is | In the apps today |
| --- | --- | --- |
| Everglade | The main flow, as the web has it: make a character, then walk into the glade | On the Mac, yes: the web's realm runs natively ([section 4](#4-bringing-the-webs-worlds-into-the-apps)). On iOS too ([how](../../apps/everglade-mobile/README.md#the-webs-realms-on-ios)). On Android a card says where it runs |
| The Golemancers' Workshop | The temple and the unfinished golem | Yes |
| The Archery Range | Shoot for points on foot, from horseback and dragonback ([spec](archery-range-spec.md)) | On the Mac, yes: the web's realm runs natively ([section 4](#4-bringing-the-webs-worlds-into-the-apps)). On iOS too ([how](../../apps/everglade-mobile/README.md#the-webs-realms-on-ios)). On Android a card says where it runs: `openagents.com/worlds/archery-range` |
| Everglade Experimental | Classes, monsters, a horse and a red dragon | On the Mac, yes: the web's realm runs natively ([section 4](#4-bringing-the-webs-worlds-into-the-apps)). On iOS too ([how](../../apps/everglade-mobile/README.md#the-webs-realms-on-ios)). On Android a card says where it runs |

Nothing stands in for a world that isn't here: there is no web view, as the
[desktop spec](desktop-app-spec.md) decided, and no scene written for the apps
alone. The first Archery Range was such a scene, with its own bow, aim and
mounts. It is deleted: a world in the apps is the web's realm, running on the
game's own systems, as Everglade does on the Mac.

## 2. How the list works

[`apps/everglade-desktop/scenes.coax`](../../apps/everglade-desktop/scenes.coax)
is the one face every host calls. It holds each world's state, sends each
frame, key, press and stick to the world that is open, and draws what they
share.

- The app opens on the list, over the temple seen from a slowly circling
  camera.
- Click or tap a row to open that world. On a keyboard, 1 to 4.
- Every world has a **Worlds** button under its title. Escape does the same.
- In Everglade, the Archery Range and Experimental the keys are the realm's, Escape too. The Worlds button sits
  at the middle of the top edge, between the realm's own panels, and the
  Mac's menu has **Worlds** (Command 0).
- Leaving a world lets go of every key and finger, and the world keeps its
  state.
- `--scene N` opens the Mac and iOS apps on world N.

## 3. How a host draws a world

A host asks `eg_place` which place to draw behind the open world. Today that
is always the temple, built from the OpenAgents Metaverse Engine's architecture kit and drawn by
shaders emitted from Coax. A touch host asks `eg_claims` before it
treats a touch as the movement stick, so the list and every button work
under the left thumb.

## 4. Bringing the web's worlds into the apps

The main flow is the web's `everglade2` app
(`everglade/client/client.coax`, with Character Create inside it).
Experimental is the web's `fivelab` app (`apps/openagents-web/old/experimental/fivelab.coax`),
and the Archery Range is the web's `archery-range` app
(`apps/openagents-web/archery-range.coax`). Both hold an Everglade zone
(`everglade/zone.coax`).

### Everglade on the Mac (built 2026-11-10)

The Everglade row opens the realm itself: Character Create, then the run
into the glade and the path beyond, with the web's HUD, class bar and
pictures. It is the same Coax and the same shaders as the web, not a copy.

- **The page host.** [`metaverse-engine/host/apple/`](../../metaverse-engine/host/apple/page-host.h)
  is the native twin of the browser host's own passes (`coax/web/host.js`):
  it loads a realm's face, hands it the pack, the messages, the keys and the
  frames, uploads what the face hands back, and draws the shadow tiles, the
  scene, the particles, bloom, post and the HUD with Metal. It is engine
  code, not the app's: any realm that draws through the page's passes can
  open in it, on the Mac and on iOS.
- **The realm.** `metaverse/native/build.sh everglade2 OUT` compiles the realm
  and its face to C, as it does any web realm. The app builds that into a
  library the host loads.
- **The shaders.** `coax metal` emits each pair of `everglade/client/gpu/everglade2.coax` and
  `everglade/client/gpu/everglade2-fx.coax` for Metal at build time, with the list of what
  each reads. The host binds every resource by its name.
- **The face.** `everglade2-face.c` takes addresses of the host's width and
  has what host contract v1 asks of a face: `focus`, `outbox_capacity`,
  `media_capacity` and the sound bank's allocations. A native build
  declares version 1. The browser's module does not yet (see below).
- **The browser's services.** The socket and HTTP are the Mac's own
  (URLSession), to `https://openagents.com`: the app joins the site's room
  (`wss://openagents.com/api/room/`) as a guest's browser does. The clock is
  the machine's. What a browser keeps in its storage (your character, your
  guest key, your place, your settings) is kept as files under
  `~/.openagents/everglade/realm/`, readable only by you.
- **In the app.** `scenes.coax` knows a host can run a realm
  (`sc_host_runs`): the row then opens the world, and the Coax draws only
  the Worlds button over it.

### Experimental on the Mac (built 2026-11-10)

The Experimental row opens the web's `fivelab` realm on its Experimental
page (`/worlds/everglade-experimental`): the glade, the village and the
castle, the classes and levels, the bar, the monsters, the Longbow (hold the
right mouse button to draw, let go to loose), and the horse and the red
dragon (E to mount). The page host opens it with no fork: it is the same
host Everglade uses.

- **The face.** `fivelab-face.c` got what `everglade2-face.c` got: addresses
  of the host's width, `focus`, `outbox_capacity`, `media_capacity`, and
  version 1 in a native build.
- **The shaders.** The Experimental page draws with Character Create's passes
  (the site's `data-wgsl="/charactercreator.wgsl"`), so the app emits
  `everglade/scene/gpu/charactercreator.coax`'s pairs and `gpu/fivelab-fx.coax`'s particles.
- **The pack.** The app carries the site's own `glade.pack.gz` (92 MB). The
  page host inflates a gzipped pack as it opens, as the browser host does.
- **One host, two realms.** The Mac app keeps a page for each realm world.
  A realm out of sight draws nothing and keeps its state. Both keep what a
  browser would in the same folder, so your character is the same in each.
- **No room.** Experimental has no room on the web, so it joins none here.
  Everglade is the world that joins the site's room.
- **In a shot.** `--scene 4` opens it. `--do` gained `down:BUTTON` and
  `up:BUTTON`, so a shot can hold a draw, turn the aim and loose:
  `look;down:2;turn:229,-143;wait:0.9;up:2`.

Checked on 2026-11-10 from native shots: the glade and its HUD, a Ranger
picked from the classes, a held draw that looses an arrow which hits an Imp
for 7, the horse mounted and ridden, and the dragon mounted and flown over
the village.

### The Archery Range on the Mac (built 2026-11-10)

The Archery Range row opens the web's `archery-range` realm on its page
(`/worlds/archery-range`, its own save slot): the shooting line, the five
butts, the Longbow, the tally at the top left, the horse and the dragon.

- **The row.** The table of realms is
  [`apps/everglade-desktop/Sources/Realms.swift`](../../apps/everglade-desktop/Sources/Realms.swift),
  shared with the iOS host's build: world, module, page, realm, slot and
  pack. `scenes.coax` takes `sc_host_runs` for the range (world 3).
- **Its own library.** The range is its own module (`archery-range`: an
  Everglade zone with the range on it). The app builds `libarchery-range.dylib`
  and its shaders beside fivelab's; the pack is Experimental's, one copy.
- **One page of a module at a time.** A module's library loads once in a
  process, and the face keeps some of a page's state beside the state it is
  handed (your body, the agent driver), so two pages of one module can't be
  open together (`Realms.sharing`). The range and Experimental are two modules
  now, so both stay open.
- **In a shot.** `--scene 3`.

### Sound in the page host (built 2026-11-10)

The realms' sound plays through the Apple audio host the Metaverse Engine already
plays with, not a second one:

- **The Coax.** `metaverse-engine/host/native-audio.coax` (the PCM interpreter
  `metaverse-engine/audio/host-pcm.coax`, the session, the bank, the memory tickets,
  the WAV and Opus plans and the Everglade PCM assets), compiled to C into
  the app.
- **The C wrappers.** `metaverse-engine/host/apple/audio-host.h` and `audio-host.c`,
  moved out of the Metaverse Engine's `MetalRealm.m`, which now includes them. The
  page host answers the four calls a realm host owes the audio host: the
  sound bank, its storage, the sound packets (`mf_sound_commands`: each
  packet read once after every handoff, refused whole when it is outside the
  face's allocation) and the spatial probe (none).
- **The Swift.** `metaverse-engine/host/apple/PageAudio.swift` hands a page to
  the Metaverse Engine's `EditorMetalAudio` (`Sources/MetalAudio.swift`): the bank the
  realm renders (Experimental's synthesized sounds, the bow's among them),
  the site's sound files and PCM companions (`/realms/everglade/sfx/`,
  `pcm-v1/`, copied into the app), the first key or press as the gesture,
  and a covered page's clock stopped.
- **In a shot** the device is never opened. The interpreter renders in the
  shot's time, stderr says what it did (packets, voices, the names played,
  the loudest sample), and `--audio OUT.wav` writes it.

What the realms don't do yet:
- **Your web character.** The app has no log in, so it starts from its own
  kept character, not the one your browser has.
- **The catalog renderer.** The host draws with the page's own passes, as
  the web's Everglade II does today. It is not a profile in the Metaverse Engine's render
  catalog (`metaverse-engine/render/catalog.coax`), and the shared transport and
  lifecycle authorities (`metaverse-engine/host/transport.coax`, `lifecycle.coax`)
  are not in its path. In the browser, declaring contract v1 also needs the
  shared adapter a page gets with its render carrier, so the web module
  declares no version until Everglade II has a carrier.
- **Leaving for another page.** A portal to another realm, the clipboard
  and the address bar have no effect. The address the web gives the
  dungeon has no row of its own in the app.
- **Instance culling on the GPU.** The host draws every piece the page
  picked, as the browser does without its cull module.
- **Math mode.** The shaders are compiled in Metal's relaxed math mode, not
  the safe mode the host contract names for catalog renderers. The safe mode
  took about 45% more GPU time a frame here. Change one flag in
  `apps/everglade-desktop/build.sh` to compare.

### What is left, in order

1. **The phones.** iOS runs the three realms through the same page host,
   table of rows and audio host (built 2026-11-10; what is the phone's own,
   and what the realms don't do on a phone yet, is in
   [the mobile README](../../apps/everglade-mobile/README.md#the-webs-realms-on-ios)).
   The web's realm needs a thumb stick on the range's and Experimental's
   pages before you can walk there by touch. Android needs a twin of the
   host on Vulkan (`metaverse/native/realm-host.c` is the start).
2. **A carrier and catalog profile** for both, when the web's Everglade II
   moves to the shared renderer. The page host then gives way to the realm
   host (`apps/metaverse-engine-desktop/MetalRealm.m`).

## 5. Adding a world

1. Build the world as a web realm from the engine's and the game's shared
   modules, and bring it here through the page host. Until then, give its row
   an address in `sc_address`. A scene of the apps' own (the Workshop is the
   one) has these calls: a frame, a key, a pointer, a stick, where you stand
   and the view.
2. Give it a number, a name and a line in `scenes.coax`, and send its calls.
3. If it has its own place, build it from the architecture kit with the
   temple's exports, emit its three shader pairs, and add it to `eg_place`.
4. Add its files to the three build scripts.
