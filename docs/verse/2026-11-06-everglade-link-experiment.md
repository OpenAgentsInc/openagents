# Link in the Everglade raid

This owner-local, unlisted experiment replaces only your rendered avatar. Open
`/experiments/link-raid/index.html?pack=everglade-link&raid=preview&levels=off` on the local
Coax server. The normal Everglade page and its public pack remain available.

## Controls and rollback

- WASD and the existing camera/controller controls still work; Shift runs and Space jumps.
- Shift+R plays original `Rolling_Short` while sending forward/run input.
- Shift+B plays original `BackFlip` while sending backward/run and a brief jump input.
- Shift+L switches immediately between Link and the existing character. It cancels an action.
- Existing spell bar, targeting, casting and Meteor Swarm controls remain unchanged.
  `levels=off` enables the full spell bar in this local experiment.

The shortcuts yield to chat, character picker, conversations and settings. Held,
busy, prone, down and dead states keep the existing rules. Other players keep
the normal avatar. The replacement is local presentation, not a new network class.
For complete rollback, use the normal Everglade URL without the experimental
private pack; no saved character migration is needed.

## Source and fidelity

`aethyr/tools/ocarina-source-pack.coax` reads the same owner-local original BFRES
model and animation files used by Ocarina Lab. Its optional final `--raid` flag
exports OCL2: the original 309 joints, inverse binds, full eight influences,
30-fps sampled translation/quaternion/scale tracks and Maya SSC flags. Slot 7
is `Rolling_Short` from source animation file 03, replacing the facial inspection
clip in this explicitly distinct profile. OCL1's source provenance stays unchanged.

The raid adapter retains original Wait, Walk, Run, Dash, running jump/landing,
BackFlip and Rolling_Short poses, source scale/SSC composition, crossfades and
carrier X/Z removal. Source animation does not apply a second world translation.

This is **not the original game's complete controller**. Actions are ordinary
serialized Everglade movement inputs. Client prediction and the room therefore
apply the same collision, jump, speed, water and spell-condition rules. There are
no special dodge invulnerability frames, new action packets or source-accurate
roll/backflip trajectories. The source controller's state graph remains outside
this experiment. Backward input can turn Everglade's facing, which is not native
Link targeting behavior.

For the painterly renderer, the pack tool samples original material albedo PNGs
at original UVs into the dense source vertices. It retains the original geometry,
indices and skinning data. This is an albedo approximation, not Ocarina Lab's
full textured material rendering: fine texels, hair coverage, normal/roughness
maps and original shader responses are not reconstructed here. Spell effects,
targeting and damage use Everglade's existing systems; no native Link spell-cast
animation has been imported. Down/death presentation falls back to Everglade's
existing character so its rule state remains legible.

## Rebuild the private pack

All extracted payloads stay under `~/.openagents/verse/private-build/`; none are
committed or included in a public deployment. First run the existing Ocarina Lab
source/texture extraction documented in `2026-11-06-ocarina-lab.md`. Then:

```sh
src="$HOME/.openagents/verse/private-build/ocarina/source"
raid="$HOME/.openagents/verse/private-build/everglade-link/streams"
coax/web/tool.sh aethyr/tools/ocarina-source-pack.coax \
  "$src/Player_Child.Player_Child.bfres" \
  "$src/Player_Child_01_Animation.anim.bfres" \
  "$src/Player_Child_02_Animation.anim.bfres" \
  "$src/Player_Child_03_Animation.anim.bfres" \
  "$src/Player_Child_04_Animation.anim.bfres" \
  "$src/Player_Child.anim.bfres" "$raid" --raid
coax/web/tool.sh aethyr/tools/everglade-link-pack.coax \
  /path/to/everglade.pack "$raid" \
  "$HOME/.openagents/verse/private-build/ocarina/streams" \
  "$HOME/.openagents/verse/private-build/ocarina/textures" \
  "$HOME/.openagents/verse/private-build/web/everglade-link.pack"
```

The VPK1 pack preserves Everglade's original entries and appends Link vertices
and relative indices. An `X link FIRST_INDEX INDEX_COUNT WORD_BASE` catalogue
record identifies the extra draw. A preceding `Z` line contains the base64-encoded complete OCL2 rig, keeping
the catalogue valid text under the existing VPK1 admission gate. The tool
preserves texture/sound licenses, rewrites byte/budget metadata and runs the
complete pack reader before writing. The face owns separate bounded catalogue and rig
buffers; Coax copies and validates the rig before enabling the avatar. The water
draw remains last, preserving the host's alpha-blend ABI. GPU skinning reads all
eight original 16-bit joint indices, including joints above 255.

Rig admission mutation: 261 killed, zero survived. The unchanged valley admission
checks after extracting the shared binary-word codec: 138 killed, zero survived.
Focused checks cover reversible toggling, movement macro cancellation, source
profile provenance, and a roll ending once. Browser evidence is recorded after
the playable build rather than inferred from these checks.

The admitted local pack is 38,273,820 bytes; SHA-256
`8c3b3ca331bb6e6ddd1125e3cd4c3a76ca1e17026cbd5c2899695f022ec30534`.
The decoded OCL2 rig is 10,343,136 bytes. The private payload is not in Git.

The three changed Everglade shader outputs match their own Coax entry modules.
The broader `wgsl-module --site` run reaches an existing unrelated Valley World
error (`vw_bg_fs` returns `VcLit` where `V4` is required); this experiment does
not change that module.

## Playable verification

The 1440×900 headless browser used the hardware Apple Metal-3 adapter. Actual
DevTools key presses and mouse clicks exercised the ordinary browser input
handlers, with a live local room (state 2, seat 0):

- Source rig ready, avatar enabled, one Link draw: all 1.
- Shift+R: original clip 7 with forward input +1.
- Shift+B: original clip 6, airborne controller act 3, backward input −1.
- Shift+L: avatar disabled and its draw count 0; a second press restores it.
- Ordinary W moved the room-controlled body about 4.68 m in the measured interval.
- Hotbar slot 0 armed Meteor Swarm through its usual key and world click. The
  room-delivered played-effect count increased from 0 to 1, and the rendered
  meteor explosions were inspected. Link remained enabled.

Reproduce with:

```sh
coax/web/tool.sh coax/web/everglade-link-check.coax \
  'http://127.0.0.1:8841/experiments/link-raid/index.html?pack=everglade-link&raid=preview&levels=off' \
  "$HOME/.openagents/verse/private-build/link-raid-check.png"
```

The checker returns nonzero if an observed action, reversal, room movement or
server-returned spell effect fails. Captures containing extracted assets remain
private. Synthetic DOM key events did not exercise this host's input path;
the committed check uses real DevTools input instead.
