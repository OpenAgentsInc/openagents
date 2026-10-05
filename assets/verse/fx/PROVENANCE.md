# Particle sprite sheets

Every sheet here is our own work: rendered by Blender 5.2.2 LTS, run headless
(`Blender -b --factory-startup --python SCRIPT -- OUT.png CELL`), with Cycles
on the CPU from procedural shaders in a script under `scripts/blender/fx/`.
No texture, model, or data from any game or other third party went into
them. Rebuild them all with `scripts/blender/build-fx.sh`; see
[the particle pipeline](../../../docs/verse/particles.md).

Each sheet is a 512-pixel square 8-bit RGBA PNG. Its RGB is premultiplied
linear color, sRGB-encoded, and its alpha is linear coverage. Cycles' path
tracing and denoising aren't bit-identical across versions and machines, so
compare rebuilds by eye, not by bytes.

| File | Script | Cells | Frames | Samples |
| --- | --- | --- | --- | --- |
| `fireball.png` | `scripts/blender/fx/fireball.py` | 4 × 4 of 128 px | 16 | 64, denoised |
| `smoke.png` | `scripts/blender/fx/smoke.py` | 4 × 4 of 128 px | 16 | 64, denoised |
| `sparks.png` | `scripts/blender/fx/sparks.py` | 2 × 2 of 256 px | 4: spark, flare, ring, dust | 16 for the glows, 64 and denoised for the dust |

The effects that use them are under `effects/`, one TOML file each.

License: the same as this repository.
