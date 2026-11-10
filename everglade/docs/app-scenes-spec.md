# Worlds in the Everglade apps (spec)

Written 2026-11-10. Status: **the list and two worlds are built; two worlds
are specified, not built.**

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
| Everglade | The main flow, as the web has it: make a character, then walk into the glade | No. A card says where it runs on the web |
| The Golemancers' Workshop | The temple and the unfinished golem | Yes |
| The Archery Range | Shoot for points on foot, from horseback and dragonback ([spec](archery-range-spec.md)) | Yes |
| Everglade Experimental | Classes, monsters, a horse and a red dragon | No. A card says where it runs on the web |

Nothing stands in for a world that isn't here: there is no web view, as the
[desktop spec](desktop-app-spec.md) decided.

## 2. How the list works

[`apps/everglade-desktop/scenes.coax`](../../apps/everglade-desktop/scenes.coax)
is the one face every host calls. It holds each world's state, sends each
frame, key, press and stick to the world that is open, and draws what they
share.

- The app opens on the list, over the temple seen from a slowly circling
  camera.
- Click or tap a row to open that world. On a keyboard, 1 to 4.
- Every world has a **Worlds** button under its title. Escape does the same.
- Leaving a world lets go of every key and finger, and the world keeps its
  state: come back to the range and your tally is still there.
- `--scene N` opens the Mac and iOS apps on world N.

## 3. How a host draws a world

A host asks `eg_place` which place to draw behind the open world: the temple
or the range. Both are built from Aethyr's architecture kit and drawn by
shaders emitted from Coax, with the same buffers and bindings, so each host
draws either with one piece of code. A touch host asks `eg_claims` before it
treats a touch as the movement stick, so the list and every button work
under the left thumb.

## 4. Bringing the web's worlds into the apps

The main flow is the web's `everglade2` app
(`apps/openagents-web/everglade2.coax`, with Character Create inside it).
Experimental is the web's `fivelab` app
(`apps/openagents-web/fivelab.coax`). Neither can be opened by a native host
today. This is the work, in order:

1. **Shaders for Metal and Vulkan.** Both worlds' shaders are emitted only
   as WGSL (`gpu/everglade2.coax`, `gpu/fivelab.coax`,
   `gpu/charactercreator.coax`). Each needs a render carrier and a catalog
   profile (`aethyr/render/carriers/`, `aethyr/render/catalog.coax`), as
   `everglade5e` has.
2. **Faces at host contract v1.** `everglade2-face.c` and `fivelab-face.c`
   are older than [`aethyr/host/README.md`](../../aethyr/host/README.md):
   they declare no contract version and lack `focus`, `outbox_capacity` and
   `media_capacity`.
3. **A native build of each.** `verse/native/build.sh REALM OUT` already
   compiles any web realm and its face to C. It needs step 1's carrier.
4. **Native versions of what the browser gives:** the socket, HTTP and the
   clock (`coaxial_socket_open`, `coaxial_http`, `coaxial_now_ms`), and the
   saved character (`coaxial_store`).
5. **The realm host inside these apps.** `apps/aethyrforge-desktop/`
   (`MetalRealm.m`) already runs Everglade 5e on Metal through that
   contract, and `verse/native/realm-host.c` does on Vulkan. The Everglade
   apps link those and open a realm when its row is chosen.
6. **Packs.** Experimental needs its own pack (`glade.pack`) in the app.

Step 1 is the large one. Steps 2 to 4 are small. After step 5 the two cards
become worlds with no change to the list.

## 5. Adding a world

1. Write the scene in Coax with the calls `range.coax` has: a frame, a key,
   a pointer, a stick, where you stand and the view.
2. Give it a number, a name and a line in `scenes.coax`, and send its calls.
3. If it has its own place, build it from the architecture kit with the
   temple's exports, emit its three shader pairs, and add it to `eg_place`.
4. Add its files to the three build scripts.
