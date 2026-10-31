# Five new arm-and-finger SDK configurations

`sdk-profile.json` froze one seven-radian arm tuple, five two-metre slide tuples,
all names/order, original scene identities, raw quaternion-sign policy and the
absolute pose budget `2e-12` before capture. The five finger pairs are
`(0,0)`, `(.04,0)`, `(0,-.04)`, `(.04,-.04)`, `(.013,-.027)`.

The authored `panda_fingers_sdk_collect.c` uses the installed MuJoCo 3.3.1 C SDK.
For each case it resets model data, writes only the nine named qpos entries,
and calls `mj_forward` exactly once. It verifies time remains zero, selected
qpos bits are unchanged, and every qvel is zero. It calls no integration step,
renderer, controller or learned model, and imports no repository/Python code
from LIBERO, robosuite or NVIDIA.

```shell
python3 psionic/tests/robotics/panda_fingers_sdk_reference.py /tmp/panda-fingers-sdk-new
```

Output must be a new directory. The driver discovers the private installed C
SDK by filesystem path and supplies `DYLD_LIBRARY_PATH` only to its collector
process for the wheel's framework-style install name. Neither the SDK nor the
owner's environment is modified. The new collector passed fail-fast ASan/UBSan
with empty stderr; the SDK is the original uninstrumented binary.

The exact pose-fixture inventory bound by `sdk-reference.json` is:

- `sdk-profile.json`: frozen request and scope.
- `sdk-model.json`: sixteen named compiled body transforms and inertial facts,
  nine joint axes/anchors/types/addresses/references/limits/armatures, local
  grip-site transform, complete qpos0 and observed gravity.
- `sdk-cases.f64`: little-endian F64 `[5,9]`, 360 bytes; first seven radians,
  last two metres.
- `sdk-poses.f64`: little-endian F64 `[5,268]`, 10,720 bytes. Each of sixteen
  bodies contributes world XYZ3, raw WXYZ4 and row-major rotation9; the tail
  is the grip site's XYZ3 and rotation9. Site world quaternion is not captured.

The compiled slide qpos/dof addresses are 7/8; both references are zero and
both local axes are `(0,1,0)`. The moving left/right BODY fixed quaternions are
both positive-Z quarter turns, normalized by the SDK to
`(.7071067811865475,0,0,.7071067811865475)`. The right finger's separate visual/
collision GEOM rotation is not its BODY pose. Tip BODY quaternions are identity;
the separate tip inertial quaternions are `(0,0,0,1)`. These are distinct fields.

The full original XML SHA256 is
`13e900d1086ecee8e16df2876104e6b6c95e06c46a14b5860ccebb16ee1af7e5`,
and retained rewritten XML is
`95a4049c1454ffd77b2185f0a21966832c5be55af67f64c2b110cc32e617d02f`.
This one compiled model includes its inherited defaults. No another-scene or
another-model equality is inferred from subtree XML alone. Original XML and
binary meshes/textures remain private acquired data. Dataset/source/license
attribution remains in [the original acquisition record](../libero-panda/acquisition.json).
The dataset card and SDK declare Apache-2.0; LIBERO/robosuite source records
are separately pinned and MIT. These small derived numeric observations are
not a redistribution of original scene or binary asset payloads.

## Separately versioned future dynamics facts

During the SAME five forward calculations, `mj_fullM` expands the existing
qM once and the collector copies the named nine-DOF row-major 9×9 mass block
plus nine qfrc_bias entries. `sdk-dynamics.f64` is F64 `[5,90]`, 3,600 bytes:
81 mass entries then nine bias entries per configuration. No additional
forward or integration call occurs.

`sdk-dynamics.json` binds that file in a SEPARATE inventory/schema and declares
`qualified_owned_dynamics=false`. It is raw independent SDK data for future
work, not evidence that an owned mass, gravity or bias implementation exists.
All model velocities are zero by construction; this is not an equilibrium or
required-motor-torque claim. qfrc_bias is the SDK's generalized bias field,
not a complete passive/actuator/constraint-force result. The mixed coordinate
order is seven radians then two metres, so no single universal unit is assigned
to all mass-matrix entries here. Observed gravity is `(0,0,-9.81)`.

The model metadata preserves raw SDK mass, inertial position, inertial WXYZ
quaternion and principal-inertia fields for each body. No own normalization,
coordinate conversion, inertia algorithm or sign canonicalization supplies
these reference facts. Pose qualification and any future dynamics comparison
must retain their separate scopes.
