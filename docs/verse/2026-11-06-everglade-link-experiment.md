# Link in the Everglade raid

The unlisted owner-local experiment now shares Ocarina Lab's locomotion profile
and character material evaluator. Open
`/experiments/link-raid/index.html?pack=everglade-link&raid=preview&levels=off&offline=worker`.
The matching local room worker is required: public rooms have not been deployed
with this experimental input extension.

## Controls and rollback

WASD runs; Shift walks; Space jumps. Shift+R rolls, Shift+B backflips, and Shift+L
switches between Link and the ordinary character. Camera controls, the spell bar,
targeting and Meteor Swarm keep their existing paths. Chat, modal and busy/held
rules still apply. Down/death/shapechange presentation retains the existing
fallback. Remote players retain their normal avatars.

Toggle off or use the normal Everglade URL for rollback; no saved character
migration is required. Extracted assets are private and are not deployed or
committed.

## Shared movement, prediction and room authority

`verse/ocarina/locomotion.coax` is the common profile used by Ocarina Lab and the
raid's shared `everglade-step.coax` motor. Walk is 0.64 m/s, run is
2.2500002 m/s (the measured source carrier endpoints), and roll is 5.625 m/s.
Acceleration is 12, turn rate 10 and ride height 0.35 m. Normal gravity is 16;
active jump gravity is 8.8 with launch `sqrt(2 * 0.7 * 8.8)`. Backflip gravity is
12.8, launch `sqrt(2 * 0.35 * 12.8)`, and horizontal launch is 6.8 m/s opposite
the captured facing. The backflip retains facing and stops planar velocity on
landing. Roll heading is captured too. Collision uses the Lab capsule profile:
0.23 m radius, 1.17 m height, 0.35 m step, 0.115 m maximum travel, three passes.
Everglade supplies its own actual floors, walls, bodies, spell conditions and
external forces.

The versioned `J1` input carries the profile, action, active jump gravity and
captured heading through input history, server stepping and reconciliation.
There is no local-only impulse to be undone by ordinary server movement.
The original `I` command is unchanged. See
[the extension contract](../protocol/everglade-link-input-v1.md).

This establishes parity with **our Ocarina Lab port**, not proof that every
native game's controller equation has been recovered. Native targeting,
invulnerability and a complete native action state graph are not claimed.
Spell casting uses Everglade's authoritative systems and animations remain
source locomotion clips; no native Link spell-cast animation was imported.

## Original rig and materials

The source is the same owner-local BFRES/PNG extraction as Ocarina Lab.
OCL2 retains 309 joints, inverse binds, all eight influences, 30-fps sampled
translation/quaternion/scale tracks and Maya SSC. Slot 7 is source file 03's
`Rolling_Short`; BackFlip and normal movement tracks remain original.
Root X/Z is removed to avoid duplicate movement. Carrier Y is removed during
physical jumps/backflips, matching the Lab. Rolls hold their final pose once.

`apps/openagents-web/gpu/ocarina-material.coax` is the shared character material
evaluator used by both scenes: original albedo, UV1 hair coverage, iris/white/eye
mask layers, cloth UV transforms, tangent-space normals, roughness, AO, alpha
cutoffs and the Lab skin-wrap/GGX response. Gradients are calculated before
material branches for valid WebGPU execution. The private pack retains all 60
original decoded PNGs, 25 material records, original vertex attributes and
indices; no vertex-colour baking remains. The host allocates 1536-pixel texture
layers from the Coax scalar extent declaration, matching the Lab's sampler.
Smaller/non-square images undergo the same host resize/mip path as the Lab.

Scene sun, shadow visibility, atmosphere and final tone mapping remain the raid's.
Sharing character materials does not imply the two worlds have identical scene
lighting. GPU skinning reads the original 16-bit indices, including joints above
255, and skins the original tangents as well as positions/normals.

## Rebuild the private pack

All extracted payloads stay under `~/.openagents/verse/private-build/`; none are
committed or included in a public deployment. First run the existing Ocarina Lab
source/texture extraction documented in `2026-11-06-ocarina-lab.md`. Then:

```sh
src="$HOME/.openagents/verse/private-build/ocarina/source"
raid="$HOME/.openagents/verse/private-build/everglade-link/streams"
coax/web/tool.sh verse/tools/ocarina-source-pack.coax \
  "$src/Player_Child.Player_Child.bfres" \
  "$src/Player_Child_01_Animation.anim.bfres" \
  "$src/Player_Child_02_Animation.anim.bfres" \
  "$src/Player_Child_03_Animation.anim.bfres" \
  "$src/Player_Child_04_Animation.anim.bfres" \
  "$src/Player_Child.anim.bfres" "$raid" --raid
coax/web/tool.sh verse/tools/everglade-link-pack.coax \
  /path/to/everglade.pack "$raid" \
  "$HOME/.openagents/verse/private-build/ocarina/streams" \
  "$HOME/.openagents/verse/private-build/ocarina/textures" \
  "$HOME/.openagents/verse/private-build/web/everglade-link.pack"
```

The combined pack uses [VPK2](../protocol/asset-pack-v2.md): the VPK1 record layout and checks with a separately
identified maximum of 128 entries rather than 48. VPK1's original maximum stays
48. This does not increase the separate 64-slot AssetRegistry limit: oversized
registry batches still refuse atomically. The streaming cap stays 64 MiB.
The combined pack has 101 entries (38 base and 60 Link texture layers), retains
base licenses and declares the extracted source as owner-local proprietary L6.

Catalogue records `Y link-materials-v1 1536 25 60`, `Z <base64 OCL2>` and
`X link-v2 FIRST_INDEX INDEX_COUNT HEADER_WORD_BASE` identify the exact material
and geometry profile. Old baked packs cannot silently select the new shader.
The geometry header points to original vertices and remapped material records;
base texture numbers stay unchanged. Water remains the last blended draw.

The admitted pack is 61,179,532 bytes; SHA-256
`720ec4652d51ee67a15294957f17d5c714c2b8c125ce7c79145a1f0bb174d57b`.
The decoded rig is 10,343,136 bytes. No payload is in Git.

## Verification

Focused checks cover shared source speeds, jump height, backflip launch/facing,
landing, action cancellation, input serialization and room J1 dispatch. Flat
motor integration peaks between 0.66 and 0.71 m for a 0.7 m jump. Admission
mutation runs killed 25/25 J1, 576/576 pack and 155/155 streaming mutants, with
zero survivors. Existing source rig admission remains in force.

The browser check uses actual DevTools keys and mouse clicks, a muted headless
Metal WebGPU browser, the live local room worker and a server-returned Meteor
Swarm effect. It checks original roll/backflip clips, toggle reversal and the
1536-pixel material allocation. The measured live run returned squared-speed
probes 506 (2.25 m/s) and 40 (0.64 m/s), a 670 mm jump, 328 reconciliation
replays and a maximum correction of 80 mm² (about 9 mm). The room-delivered
played-effect count increased from 0 to 1 for Meteor Swarm. Reproduce with:

```sh
coax/web/tool.sh coax/web/everglade-link-check.coax \
  'http://127.0.0.1:8841/experiments/link-raid/index.html?pack=everglade-link&raid=preview&levels=off&offline=worker' \
  "$HOME/.openagents/verse/private-build/link-raid-check.png"
```

Captures containing extracted assets remain private. The three Everglade shader
outputs and Ocarina Lab output must be regenerated from their own Coax modules.
The unrelated whole-site Valley World `vw_bg_fs` return-type failure is outside
this change.
