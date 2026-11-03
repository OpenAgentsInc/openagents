# Ice Cream Man's camera on aethyr/follow.coax: before and after (2026-11-02, #12609)

The same replayed round, drawn before and after Ice Cream Man's boom camera
moved onto `aethyr/follow.coax`.

- **Round:** `/worlds/ice-cream-man#seed=4&bot` (seed 4, the bot playing).
  The bot's input comes from the rules alone, so both builds play the same
  round: the timer and the keys match in every pair.
- **Frames:** host.js clip mode (`globalThis.coaxialClip = true`): each
  frame is one fixed 1/30 s of game time, stepped with
  `coaxialClipStep(k, 30)`. Shots at frames 30, 60, 90, 150, 210, 300 and
  450, at 1280x800.
- **Browser:** headless, muted Chromium 152 (ungoogled-chromium from Nix), WebGPU on
  SwiftShader: `--headless=new --mute-audio --enable-unsafe-webgpu
  --use-angle=swiftshader --use-webgpu-adapter=swiftshader
  --enable-unsafe-swiftshader --use-vulkan=swiftshader --enable-features=Vulkan`.
  No window opened.
- **Builds:** `COAXIAL_RENDER=auto coax/web/wasm.sh
  apps/openagents-web/icecream.coax apps/openagents-web/icecream-face.c
  apps/openagents-web/build/icecream.wasm verse/icecream/scene.coax`, served
  by `coax/web/serve.sh apps/openagents-web --build true` with the
  ice-cream-man render profile from `aethyr/render/build.coax`. Before:
  e45c8bd588 (sha256 of the wasm d8a586fe...). After: this change
  (2eef7b5c...).
- **Reproducible:** the before frames were taken twice and were the same,
  byte for byte, all 7 of them.

`before/` and `after/` are the full frames. `side-by-side/` shows each pair
at half size, before on the left.

## What changed in the look

- Where a wall or piece of furniture is behind the kid, the old boom pulled in
  short of it with a 0.2 m sphere plus an 0.08 m margin. The follow keeps
  0.2 m, and never comes closer than 0.35 m (`FOLLOW_CLOSEST`). So in tight
  rooms and doorways the camera is a little further back and the kid looks
  smaller (frames 150, 210, 300).
- The follow's target trails the kid on a 0.1 s spring, so while walking
  the kid sits a little ahead of centre.
- After being pulled in, the camera backs out faster: 18% each 1/60 s
  (`FOLLOW_EASE`), where the boom backed out at 3 per second.
- Under the ceiling the old boom tilted flatter. The follow pulls in instead.
  The starting pitch is now 0.247 rad (it was 0.35), so a run starts with the
  same eye height of 2.3 m, 4.5 m back.

## Why not coax/web/tool.sh

On coderos-4080, `CDP_WEBGPU=swiftshader coax/web/tool.sh shot` and `clip`
lose the GPU device four times and stop ("the GPU was lost 4 times in a
minute"). Chromium's log says it can't create the WebGPU swap chain's shared
image. Adding `--use-vulkan=swiftshader --enable-features=Vulkan` fixes
that, and `coax/lib/net/cdp.coax` doesn't pass those flags. These frames
came from a scratch CDP driver with the same flags as clip mode plus those
two. The driver wasn't committed.
