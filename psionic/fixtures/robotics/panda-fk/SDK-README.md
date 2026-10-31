# New forward-only MuJoCo reference

`sdk-profile.json` froze seven explicit seven-hinge configurations, body/site
order and absolute tolerance `2e-12` before capture. The tolerance is not
relative, bitwise or quaternion-sign-insensitive. Source quaternion signs are
preserved.

The collector uses the installed MuJoCo 3.3.1 C SDK directly. It imports no
LIBERO, robosuite or NVIDIA Python/repository code. For each case it calls
`mj_resetData`, changes only the seven named arm hinge positions, and calls
`mj_forward` once. It verifies time stays zero and those positions stay
unchanged. It calls no integration step, renderer, controller or learned model.
All other state begins at compiled SDK reset defaults, including `qpos0`; this
is not an HDF5 demonstration initial-state replay.

```shell
python3 psionic/tests/robotics/panda_fk_sdk_reference.py /tmp/panda-fk-sdk-new
```

The output directory must be new. The private installed SDK is discovered by
filesystem path, never by importing its Python package. On this macOS profile
the driver supplies `DYLD_LIBRARY_PATH` to the collector process so the wheel's
framework-style install name finds the pinned dylib. It does not change the
SDK, the executable or the owner's environment.

The checked-in inert numeric fixtures are:

- `sdk-cases.f64`: little-endian F64 `[7,7]` hinge angles in radians, 392 bytes.
- `sdk-poses.f64`: little-endian F64 `[7,204]`, 11,424 bytes. For each of twelve
  bodies: world XYZ, raw WXYZ quaternion, row-major 3×3 rotation. The tail is the
  grip site's world XYZ and row-major rotation. A site world quaternion is
  deliberately absent.
- `sdk-model.json`: observed compiled body parents and local transforms, joint
  axes/anchors/addresses/limits/reference positions, full `qpos0`, and local
  site transform. Values are copied from `mjModel`; no own normalization or
  forward-kinematics formula supplies this reference.
- `sdk-reference.json`: source/SDK/tool/input/output identities and the actual
  capture evidence.

The full original XML SHA256 is
`13e900d1086ecee8e16df2876104e6b6c95e06c46a14b5860ccebb16ee1af7e5`;
the retained rewritten XML is
`95a4049c1454ffd77b2185f0a21966832c5be55af67f64c2b110cc32e617d02f`.
Original scenes, meshes and textures remain private acquired data. The hashes
and compiled constants include inherited defaults for this one model; equality
of a robot XML subtree alone does not establish that another model compiles to
the same transforms. Other scenes/endpoints need their own explicit comparison.

The retained source is demo 0 of the
[pinned LIBERO dataset](https://huggingface.co/datasets/yifengzhu-hf/LIBERO-datasets/tree/f13aa24a3da8c43c7225569f28c562979fa0e35a).
Its card declares Apache-2.0. The existing acquisition record binds the whole
HDF5 SHA256, LIBERO and robosuite source/license identities (MIT), and MuJoCo
3.3.1 (Apache-2.0); see
[the acquisition record](../libero-panda/acquisition.json). This folder contains
small derived numeric observations, not those original binary assets or scene
XML. No training holdout, dynamics, finger, Jacobian, episode or hardware claim
follows from this reference.

The new collector compiled under fail-fast ASan/UBSan; the external SDK itself
is the original uninstrumented binary. The first launch stopped in dyld before
`main` because of that framework-style install name. Its failed record is
preserved. The successful followup reused the same compiled collector and
case bytes with the per-process library path; it did not repeat a successful
SDK forward run. The composed receipt keeps both phases rather than changing
the earlier failed observation into a pass.
