# Original ritual capture

`original-combat.mp4` contains 51.933 seconds of native Verse GPU frames at
1280×720 and 30 frames per second, encoded with H.264. No grading or chat-input
automation is applied. `original-ritual.png` is the eight-second frame extracted
from that recording; `original-combat.png` is the renderer's final frame.

The procedural `verse-original-ritual-v1` pack generates all models, animation
keys, weapons, textures, particles, and UI graphics in Rust. The font is bundled
Fira Mono under its retained SIL Open Font License. The capture reads no WoW
asset pack or installation. Source and launch instructions are in
[`assets/verse/original`](../../../assets/verse/original/README.md).

The recording starts with programmed ritual dialogue and a cinematic camera.
At 20 seconds, it hands off to the third-person tactical controller. The adjacent
JSON records all ten abilities, 32 enemy casts, 54 absorbed shield damage, ten
cultists defeated, and the adventurer's defeat at 46.9 simulated seconds.
Claude finishes at 299,877 of 300,000 HP. The capture ends five seconds later.

This capture is refreshed after engine-core extraction, shared primitive
collision, and third-person camera clearance (#10427–#10429). The original scene
uses the headless `verse-engine` contracts and owned `physics` box queries.

Validation: Physics suite (one existing oracle ignore), six engine tests,
16 gameplay adapter tests, 33 focused chamber tests, both native example builds,
formatting, and diff checks. The asset test verifies generated file digests and
animation; the timeline test verifies the full ability kit and actual combat
outcomes. Collision tests cover tunneling, wall sliding, corners, teleport
arrival effects, and obstructed/unobstructed camera views. This fixture does not
validate multiplayer, saves, or capsule/mesh character collision.
