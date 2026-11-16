# Microduck cinematic take two

[Watch take two](https://storage.googleapis.com/openagentsgemini-native-world-assets/objects/b2366524bb0ddc249bc4938046ff160b3e320b3d44eae3c3511c3164d4fa6444).
The silent H.264 / yuv420p MP4 contains 1,260 frames at 1920×1080 and 60 fps,
21.000 seconds, 9,223,496 bytes. The entire public download was compared
with the local MP4 and matched. [Media inspection](media.json),
[artifact and source hashes](record.json).

A second actual-render take adds a continuous full-circle orbit, a rising
camera over the exploded assembly and a gradual dolly toward reassembly.
The orbit settles before the same authored walk, followed by a gentle
tracking drift. It reuses the shared orbit camera, easing, assembly palette,
film clock and figure gait. Take one remains available unchanged.

The 21-second choreography:

- 0–1.2 s: establish the assembled model.
- 1.2–5.2 s: ease apart as the camera orbits and rises.
- 5.2–8.2 s: hold the exploded pieces for changing-angle inspection.
- 8.2–11.8 s: reassemble while the camera descends and moves closer.
- 11.8–13 s: hold the exact source stand and settle the orbit.
- 13–21 s: authored walking animation with gentle camera tracking.

The viewer's Animation → **Cinematic assembly + walk** starts this take.
Its phase captions use the first take's choreography coordinates; the
outer film clock supplies the retiming and camera motion. Backgrounding
holds that clock. No camera or explosion value becomes servo experiment
state, and neither take demonstrates trained locomotion or physical control.

The recording uses the actual Coax Native Web viewer on WebGPU / Apple
Metal, muted and headless, through the shared `clip` recorder. Source assets
and attribution remain Pollen Robotics + NVIDIA DLI, CC BY-NC-SA 4.0.

Actual frame extracts: [exploded orbit](exploded.png),
[restored stand](restored.png), [walking](walking.png).

## Verification and recording

The targeted viewer suite passes, including continuous camera values across
every phase boundary and all 1,260 sample times, bounded lens distance,
complete reassembly before walking and the unchanged first-take checks.
The new Wasm is 108,399 bytes, under its existing 112,000-byte limit.
Tools and layer checks pass without allowlist additions.

[verify.coax](verify.coax) inspects the actual completed recording page
through the shared resident browser. It checks 70 rendered parts, the final
phase and actual walk translation, all 14 source joint limits, then restarts
the take and compares every restored stand angle's exact word. Hiding holds
both camera and clock; resume discards its first delta. The original take
remains selectable. [Observed results](qa.json). These are web observations;
this recording does not claim a new native/TestFlight rollout.

Use the shared recorder after serving this built viewer. Wait for the real
scene with a wall-time deadline: the model download is asynchronous, so
stepping the synthetic clip clock alone does not establish readiness.

```sh
coax/web/tool.sh clip \
  'http://127.0.0.1:9136/demos/microduck/?take=cinematic-2' \
  /Users/christopherdavid/Downloads/Microduck-cinematic-take-2-2026-11-15.mp4 \
  --size 1920x1080 --seconds 21 --fps 60 --frames build/cinematic-frames --keep \
  --require '(async()=>{const until=performance.now()+45000;while(performance.now()<until){await coaxialClipStep(1,1e6);if(coaxial.sceneReady&&coaxial.drawStats().draws===70){document.querySelector("[aria-label=\"Viewer menu\"]").click();const action=document.querySelector("[data-key=\"4002\"]");if(!action)return false;action.click();return coaxial.drive.stateSize()===80536&&coaxial.drive.peek(80529,1)==="01"&&coaxial.drive.peek(74720,1)==="01";}await new Promise(r=>setTimeout(r,50));}return false;})()' \
  --eval 'document.querySelector(".coax-view-menu").style.opacity="0"'
```
