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

## Universal character capture

`universal-combat.mp4` records the licensed character scene through the native
GPU renderer. `universal-characters.png` shows all six selectable Standard
appearances under inspection lighting. The adjacent combat JSON records the
same ten-ability encounter: ten cultists defeated, 54 absorbed shield damage,
32 enemy casts, and the adventurer's defeat at 46.9 seconds. Claude retains
299,877 of 300,000 HP. Cinematic dialogue and camera handoff use programmed cues.

#10432 adds CC0 Quaternius base heads, fantasy outfits, rigged hair, glTF
rest-pose/inverse-bind skinning, and retargeted animation states. Cultists mix
male and female Ranger and Peasant outfits; Claude wears an enlarged red Ranger
outfit. Bow and death states are authored because the Standard animation library
has no dedicated clips for them. Source licenses, repairs, hashes, and all 43
available animation names live in `assets/verse/characters/quaternius/`.

Validation: seven engine tests, 40 focused imported-scene tests, the gameplay
adapter suite, formatting, three native example builds, source manifest digest
checks, and native lineup and battle captures. The character test exercises all
six rigs and every mapped state, finite skin palettes and bounds, head/outfit
composition, and named-clip loading. These captures use no video grading and no
Blizzard assets. Normal and roughness source images are retained but are not yet
used by the current material shader.

## Revised motion and Bestiary capture

#10433 replaces zombie backpedaling, carry locomotion, and throw-based spell
poses with authored limb motion. `improved-animations.mp4` shows 39 seconds of
idle, walk, run, backward movement, left/right strafe, guard, combat readiness,
spell windup/release, bow readiness/release, and fall/corpse poses across all six
humanoid appearances. Playback blends state transitions, plants and lifts feet,
and keeps ankle orientation level. Gait clocks advance from actual travel,
including collision stops and NPC movement speed.

`bestiary-combat.mp4` contains the full native cinematic battle with a six-meter
Puglin as Claude. Its receipt includes the local monster source digest and
rendered height. The Bestiary uses Quaternius Asset License v1.0; only code and
rendered product evidence are retained here, not the restricted monster assets.
The other retained character sources remain CC0. The monster has breathing motion in its native hunched
posture, while cultists and the player use the revised humanoid poses.

Validation: eight engine tests, 41 focused imported-scene tests, three native example builds,
formatting, native animation inspection, and a native battle capture. New checks
cover loop seams on all six rigs, backward/strafe state selection, motion clocks,
and stopped locomotion against collision. These authored clips are a first
rig-aware motion system; they do not add runtime terrain foot IK or a general
animation graph.
# Interactive combat

The normal `verse_play` launch starts the cinematic and then hands control to
the player in the full encounter. Press F1 to restart manual combat or F2 to
restart the controller-driven battle. Tab selects an enemy; the action bar and
number keys activate the ten abilities. Claude has 300,000 hit points, and
cultists respawn 60 seconds after death. Issue #10434 restores combat on normal
launch; explicit recording modes retain their existing fixtures.

## Owned world combat

Issue #10437 replaces the chamber's retained combat dependency with headless
`verse-world` authority. `owned-world-combat.mp4` records 51.933 seconds of
native 1280 × 720 frames at 30 FPS. The controller uses all ten abilities,
defeats nine cultists, absorbs 54 damage, and loses at 46.900 seconds. Claude
starts at 300,000 HP and survives at 299,871 HP. The JSON receipt records 1,558
authority ticks and 54 committed events. `owned-world-battle.png` is the
33-second frame; `owned-world-combat.png` is the final native frame.

The owned profile restores one mana per second, uses a visible 20-foot fireball
area, and sweeps projectiles against static cover. Checkpoint replay preserves
pending combat, control fences, defeat, events, and later respawns. Checks:
44 `verse-world` tests, 12 focused imported-desktop renderer tests, formatting,
both native example builds, dependency-tree inspection, and this native capture.
The local Bestiary license boundary remains unchanged. This proves owned local
combat; transactional saves, multiplayer, capsule/mesh movement, and tools remain
on the roadmap.
