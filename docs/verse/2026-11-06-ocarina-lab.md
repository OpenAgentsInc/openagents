# Ocarina Lab

An unlisted owner-local character lab at `/worlds/ocarina-lab?pack`. It reuses
Aethyr's paid fixed steps, walk trace, follow camera, collision and touch sticks,
and Animation Lab's inspection conventions. No Nintendo payload is in Git.

The native source import, texture layout/channel conversion, material alias
resolution, pack construction, admission, animation and scene logic are Coax.
There is no Python or Blender dependency in this build/playback path. Standard
ASTC blocks are decoded by ARM astcenc 5.7.1; standard BC blocks by the pinned
MIT/Unlicense bcdec header and its small reference runner. These SDKs do not own
BFRES/BNTX structure, Switch tiling, material selection or gameplay.

## Source profile

The source model has 110,515 vertices, 72,778 triangles, 309 bones, eight skin
influences, UV0/UV1, tangents and colors. Eight original clips retain translation,
quaternion rotation, nonuniform scale and per-clip Maya SSC flags. Sampling uses
an explicit 30 source frames/second assumption, not a measured game clock.
See `aethyr/tools/ocarina-source-format.txt` for the binary schema and source
references. The complete private pack is 49,448,496 bytes with SHA-256
`fad738db36516efd185a81b11ce00538096d4cbb4a7b824fc8d2d02d3fb585c7`.

`ExternalBinaryString.bfres` resolves material name hashes. `UnifyTex.Product.100.byml`
resolves ten missing references to nine shared texture payloads, including the
adult hair *textures* used by the child model. Original child geometry is kept.
The 69 decoded PNGs supply 60 unique runtime layers and 25 material records.

Hair uses source albedo/normal/roughness/AO on UV0 and coverage
`albedo.alpha(UV0) * Gn3.red(UV1) >= 0.5`. Gn0 is a specular LUT, not a replacement
base color. BNTX block-height and component selectors are preserved; normal XY
comes from semantic RG after selection. The old flat white hair and scrambled
coverage came from omitted aliases, UV1, and vertical GOB block height.

The lighting is an Aethyr material translation. Skin scattering, anisotropic hair
response and layered eyes are approximations; the outer corneal/eye-shadow meshes
are suppressed in this opaque inspection renderer. This is not a reproduction of
the game's full shader graph, physics or hair/cloth simulation. The forest is
original procedural geometry, with the same placements used for collision.

## Rebuild locally

Keep the decompressed original files in an owner-private directory, called `$src`
below. The local prepared copy is under
`~/.openagents/verse/private-build/ocarina/source`; generated streams and textures
are siblings. No NUP download is required to rebuild from that prepared source.

```sh
src="$HOME/.openagents/verse/private-build/ocarina/source"
out="$HOME/.openagents/verse/private-build/ocarina/streams"
tex="$HOME/.openagents/verse/private-build/ocarina/textures"
coax/web/tool.sh aethyr/tools/ocarina-source-pack.coax \
  "$src/Player_Child.Player_Child.bfres" \
  "$src/Player_Child_01_Animation.anim.bfres" \
  "$src/Player_Child_02_Animation.anim.bfres" \
  "$src/Player_Child_03_Animation.anim.bfres" \
  "$src/Player_Child_04_Animation.anim.bfres" \
  "$src/Player_Child.anim.bfres" "$out"
cc -O2 aethyr/tools/ocarina-bc-reference.c -lm -o /tmp/ocarina-bc-reference
coax/web/tool.sh aethyr/tools/ocarina-texture.coax \
  "$src" "$tex" /path/to/astcenc /tmp/ocarina-bc-reference
coax/web/tool.sh aethyr/tools/ocarina-material-pack.coax \
  "$src/Player_Child.Player_Child.bfres" "$src/ExternalBinaryString.bfres" \
  "$src/UnifyTex.Product.100.byml" "$tex" "$out"
coax/web/tool.sh aethyr/tools/ocarina-bundle.coax "$out" "$tex" \
  "$HOME/.openagents/verse/private-build/web/ocarina-lab.pack"
coax/web/wasm.sh apps/openagents-web/ocarinalab.coax \
  apps/openagents-web/ocarinalab-face.c apps/openagents-web/build/ocarinalab.wasm
coax/web/tool.sh coax/web/wgsl-module.coax apps/openagents-web/gpu/ocarinalab.coax \
  apps/openagents-web/ocarinalab.wgsl apps/openagents-web/gpu/ocarinalab.module
```

Run the existing watching dev server and open `/worlds/ocarina-lab?pack`. The
`?pack` opt-in is required by the existing local private-pack host. A static
public build has no source assets. A changed source pack requires an intentional
SHA/length update in the Coax admission policy, never silent fallback.

## Verification

The importer passes 70 checks. Rig admission kills all 250 root mutants; pack
identity kills all 22; renderer adoption kills all 58. Texture and material tools
pass their 63 and 69 loaded checks. Nonuniform scale and zero-scale SSC regression
checks pass, including a case which failed with reciprocal parent-scale chaining.
The runtime budget covers desktop, phone and high-DPR frames under 1.2 GiB,
including all 60 mipmapped 1536-square layers and retained source bytes.

`coax/web/ocarina-check.coax` runs muted, headless Chrome against the actual
private source pack. `--verify` checks ordinary lowercase walking, running,
jumping, backflip, pause, all eight clips, frame stepping, all four material views,
blur and touch movement. It captures actual GPU readback as PNG; macOS headless
Chrome's compositor screenshot was black despite valid WebGPU output.

The final 390×844 browser run passed all 32 checks, including forced GPU loss,
recovery in 720 ms, identical complete state digest, and zero private-pack network
requests during recovery. The normal render had no shader errors. Its declared
resident-byte measurement was 1,052,146,064 bytes, below the 1.2 GiB ceiling.
The target Wasm is 68,563 bytes. Reassembling from the durable private streams
reproduced the complete pack SHA above. These are local validation results, not
an original-game visual parity or frame-rate qualification.

```sh
coax/web/tool.sh coax/web/ocarina-check.coax \
  'http://127.0.0.1:8837/worlds/ocarina-lab?pack' /tmp/ocarina.png --verify --recover
```
