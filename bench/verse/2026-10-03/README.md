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

## Column navigation capture

`original-navigation.mp4` contains ten seconds of native rendered frames from
`verse_play --navigation-demo`. The fixture moves the adventurer to (13, 0, -13)
and a cultist to (17, 0, -13), on opposite sides of a real chamber column. It
postpones new hostile casts for the recording so the movement remains visible;
it does not change the normal encounter's cast timing. The cultist routes around
the column toward a clear firing position. The adjacent JSON records its actual
final position and the unchanged player health. The image is the renderer's
final frame. No grading or chat-input automation is applied.

#10430 adds bounded static navigation. #10431 adds target visibility, delayed
bow/cast checks, directional Thunderwave filtering, and hostile impact cover.
Validation for these slices: two physics navigation tests, 39 focused chamber
tests, 16 gameplay adapter tests, both native example builds, and the native
navigation recording. Full projectile CCD and explosion-radius occlusion remain
separate work.
