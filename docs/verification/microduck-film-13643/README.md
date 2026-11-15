# Microduck assembly film — #13643

[Watch the recorded video](https://storage.googleapis.com/openagentsgemini-native-world-assets/objects/48916df893418bda111d5e61561f6d1640c67ad8e2a490ded962e9c90ce24250).
The actual Coax Native Web viewer rendered all 1,080 frames on WebGPU
(Apple Metal 3). The silent H.264 MP4 is 18 seconds at 1920×1080 and 60 fps,
6,063,463 bytes. Its SHA-256 is
`48916df893418bda111d5e61561f6d1640c67ad8e2a490ded962e9c90ce24250`.
The public object was downloaded again and its complete bytes matched;
[public.json](public.json) and [record.json](record.json) retain the receipts.

The sequence holds the original assembly, eases apart from 1.2–4.2 seconds,
holds until 5.6 seconds, reassembles by 8.8 seconds, then holds the restored
stand until 10 seconds. The final eight seconds animate alternating legs.
The walking caption explicitly says **ILLUSTRATIVE ANIMATION**. This is
authored kinematic motion, with a visual foot-bounds anchor; it does not
demonstrate a trained locomotion policy, contact, balance or hardware control.

The reusable [presentation](../../../metaverse/microduck/presentation.coax)
uses shared component easing and the engine's figure gait, mapped to the
14 pinned mechanical joints. The existing assembly pose and palette still
admit all joint angles. No mesh, renderer, importer or animation runtime was
duplicated. The original model pack and source/license identities are in
the record. The film retains Pollen Robotics + NVIDIA DLI attribution and
the CC BY-NC-SA 4.0 license on screen.

Actual frame extracts:

- [Exploded, frame 240](exploded.png)
- [Restored stand, frame 570](reassembled.png)
- [Walking, frame 780](walking.png)

The viewer's **Animation → Play assembly + walk film** menu starts the same
sequence; **Pause animation** holds it and **Reset** restores the original
scene. Backgrounding holds the clock and resuming skips the first delta.

## Verification

The targeted viewer suite passed remotely, including the presentation's
phase/easing checks and existing pose, menu, servo and lifecycle checks.
The final Wasm measured 72,358 bytes and passed its 76,000-byte budget.
Tools, layer and wire checks passed with no allowlist additions.

A separate actual compiled-Wasm browser drive checked full explosion,
unchanged film time while hidden, all 14 angles bit-identical to source
stand before walking, time bounded at 18 seconds, and all animated angles
inside the source joint limits. The three keyframes above were inspected.
These are web render observations; a native hardware film test is not claimed.

Record through the repository's shared muted headless browser, after serving
the built viewer. The readiness guard waits for the real renderer, since
the loading card can disappear before its first ready frame.

```sh
coax/web/tool.sh clip \
  'http://127.0.0.1:9126/demos/microduck/?film=record-13643' \
  build/Microduck-assembly-walk-1080p60.mp4 \
  --size 1920x1080 --seconds 18 --fps 60 --frames build/film-frames --keep \
  --eval '(()=>{document.querySelector("[aria-label=\"Viewer menu\"]").click();document.querySelector("[data-key=\"4000\"]").click();document.querySelector(".coax-view-menu").style.opacity="0";return true;})()' \
  --require '(async()=>{for(let i=0;i<120;i++){if(coaxial.sceneReady&&coaxial.drawStats().draws===70)return true;await coaxialClipStep(1,1e6);await new Promise(requestAnimationFrame);}return false;})()'
```

The recording adds no native SDK capability or protocol shape. Saved-run
and replay work remains tracked separately in #13641 and #13642.
