# Microduck parade take three

[Watch the 28-second film](https://storage.googleapis.com/openagentsgemini-native-world-assets/objects/0894c0ab89eb0e42627fc3f5cbd83e2b24fea2fff93f83d799b3450283c61a36).
The silent H.264 MP4 is 1920×1080, 60 fps, 1,680 frames and
13,147,024 bytes. [record.json](record.json) pins the video, source, Wasm
and original model pack; [media.json](media.json) is the independent frame
count. The complete public download matches the local bytes ([receipt](public.json)).

The cinematic assembly orbit and restored stand lead into a close walking
shot. From 18–24 seconds, the camera pulls back and turns to the side,
revealing the lead Microduck and ten followers in a single file. The wide
shot continues to 28 seconds.

All eleven use the same admitted source geometry and mechanical pose palette,
rendered through hardware instancing. The shared assembly draw map keeps
part identity separate from copy identity. The fixed 0.34 m spacing spans
3.4 m between the lead and last follower. Their authored gait is synchronized;
this is an illustrative animation, without trained locomotion or actuation.
The frame clock, camera, easing, pose admission and source geometry are reused.

The first two takes remain selectable. The new menu action is
Animation → **Assembly + eleven-duck parade**. The recording is the actual
Coax Native Web viewer on WebGPU / Apple Metal, headless and silent.
Pollen Robotics + NVIDIA DLI / CC BY-NC-SA 4.0 attribution remains on screen.

## Verification

The viewer and GPU suites passed on the remote host. They check the
continuous bounded camera timeline, fixed spacing, all eleven shader
instances' part/normal/material identity, one-model behavior outside the
parade, and existing source-pose and lifecycle rules. The selected Wasm
build passed its 112,000-byte budget at 109,325 bytes. Tools and layer
checks passed without allowlist additions.

The [actual-Wasm drive](verify.coax) ran against the completed recording
page ([results](qa.json)): 70 part draws, eleven instances per part,
26,294,466 submitted indices, complete reveal, 4.8 m camera distance,
and every animated joint within source limits. It also checked the exact
restored stand, background clock hold, skipped resume delta, both earlier
takes, and Reset returning to one model. This records the web viewer;
native hardware and TestFlight distribution are not claimed for this take.

Inspected frame extracts:

- [Exploded, 6.5 s](exploded.png)
- [Restored, 12 s](reassembled.png)
- [Reveal, 22 s](reveal.png)
- [Wide parade, 26 s](parade.png)

## Reproduce

Use the repository's shared muted headless browser against the selected
viewer build. Wait for the source asset to finish uploading to the GPU
before starting the clock. The frozen state layout is asserted here so
a changed build refuses to record silently with the wrong menu/timeline.

```sh
coax/web/tool.sh clip \
  'http://127.0.0.1:9136/demos/microduck/?take=parade-3' \
  build/Microduck-parade-take-3.mp4 \
  --size 1920x1080 --seconds 28 --fps 60 \
  --frames build/parade-frames --keep --times \
  --require '(async()=>{const until=performance.now()+45000;while(performance.now()<until){await coaxialClipStep(1,1e6);if(coaxial.sceneReady&&coaxial.drawStats().draws===70){document.querySelector("[aria-label=\"Viewer menu\"]").click();const action=document.querySelector("[data-key=\"4003\"]");if(!action)return false;action.click();return coaxial.drive.stateSize()===80552&&coaxial.drive.peek(80530,1)==="01"&&coaxial.drive.peek(74720,1)==="01";}await new Promise(r=>setTimeout(r,50));}return false;})()' \
  --eval 'document.querySelector(".coax-view-menu").style.opacity="0"'
```

The recording took 87 seconds for the 1,680 rendered frames, followed by
4.4 seconds of encoding. No model asset was duplicated or downloaded for
each follower, and no native capability or protocol shape was added.
