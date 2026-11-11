# Worlds in the Everglade apps (spec)

Written 2026-11-10. Status: **the list and two worlds are built on the Mac
(Everglade itself and the Workshop); the phones have the Workshop; the
Archery Range and Experimental are the web's realms and don't run in the
apps yet.**

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
| Everglade | The main flow, as the web has it: make a character, then walk into the glade | On the Mac, yes: the web's realm runs natively ([section 4](#4-bringing-the-webs-worlds-into-the-apps)). On the phones a card says where it runs |
| The Golemancers' Workshop | The temple and the unfinished golem | Yes |
| The Archery Range | Shoot for points on foot, from horseback and dragonback ([spec](archery-range-spec.md)) | No. A card says where it runs on the web: `openagents.com/worlds/archery-range` |
| Everglade Experimental | Classes, monsters, a horse and a red dragon | No. A card says where it runs on the web |

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
- In Everglade the keys are the realm's, Escape too. The Worlds button sits
  at the middle of the top edge, between the realm's own panels, and the
  Mac's menu has **Worlds** (Command 0).
- Leaving a world lets go of every key and finger, and the world keeps its
  state.
- `--scene N` opens the Mac and iOS apps on world N.

## 3. How a host draws a world

A host asks `eg_place` which place to draw behind the open world. Today that
is always the temple, built from Aethyr's architecture kit and drawn by
shaders emitted from Coax. A touch host asks `eg_claims` before it
treats a touch as the movement stick, so the list and every button work
under the left thumb.

## 4. Bringing the web's worlds into the apps

The main flow is the web's `everglade2` app
(`apps/openagents-web/everglade2.coax`, with Character Create inside it).
Experimental and the Archery Range are the web's `fivelab` app
(`apps/openagents-web/fivelab.coax`; the range is its `range-scene.coax`).

### Everglade on the Mac (built 2026-11-10)

The Everglade row opens the realm itself: Character Create, then the run
into the glade and the path beyond, with the web's HUD, class bar and
pictures. It is the same Coax and the same shaders as the web, not a copy.

- **The page host.** [`aethyr/host/apple/`](../../aethyr/host/apple/page-host.h)
  is the native twin of the browser host's own passes (`coax/web/host.js`):
  it loads a realm's face, hands it the pack, the messages, the keys and the
  frames, uploads what the face hands back, and draws the shadow tiles, the
  scene, the particles, bloom, post and the HUD with Metal. It is engine
  code, not the app's: any realm that draws through the page's passes can
  open in it, on the Mac and on iOS.
- **The realm.** `verse/native/build.sh everglade2 OUT` compiles the realm
  and its face to C, as it does any web realm. The app builds that into a
  library the host loads.
- **The shaders.** `coax metal` emits each pair of `gpu/everglade2.coax` and
  `gpu/everglade2-fx.coax` for Metal at build time, with the list of what
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

What it doesn't do yet:

- **Sound.** The realm's sounds are taken and not played. The host needs
  Aethyr's audio interpreter (`aethyr/audio/host-pcm.coax`), as Aethyrforge
  has.
- **Your web character.** The app has no log in, so it starts from its own
  kept character, not the one your browser has.
- **The catalog renderer.** The host draws with the page's own passes, as
  the web's Everglade II does today. It is not a profile in Aethyr's render
  catalog (`aethyr/render/catalog.coax`), and the shared transport and
  lifecycle authorities (`aethyr/host/transport.coax`, `lifecycle.coax`)
  are not in its path. In the browser, declaring contract v1 also needs the
  shared adapter a page gets with its render carrier, so the web module
  declares no version until Everglade II has a carrier.
- **Leaving for another page.** A portal to another realm, the clipboard
  and the address bar have no effect.
- **Instance culling on the GPU.** The host draws every piece the page
  picked, as the browser does without its cull module.
- **Math mode.** The shaders are compiled in Metal's relaxed math mode, not
  the safe mode the host contract names for catalog renderers. The safe mode
  took about 45% more GPU time a frame here. Change one flag in
  `apps/everglade-desktop/build.sh` to compare.

### What is left, in order

1. **Sound** in the page host.
2. **The phones.** iOS can link the same page host (it is Metal and
   Foundation only); its app needs the realm's library, shaders and pack in
   its bundle and its touches sent to the host. Android needs a twin of the
   host on Vulkan (`verse/native/realm-host.c` is the start).
3. **Experimental and the Archery Range.** They share one module, so one
   port brings both. `fivelab-face.c` needs the same changes
   `everglade2-face.c` got, its shaders emitted the same way
   (`gpu/fivelab.coax`), and its own pack (`glade.pack`) in the app. The
   page host then opens it with no change.
4. **A carrier and catalog profile** for both, when the web's Everglade II
   moves to the shared renderer. The page host then gives way to the realm
   host (`apps/aethyrforge-desktop/MetalRealm.m`).

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
