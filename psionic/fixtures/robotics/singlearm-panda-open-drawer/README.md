# NVIDIA SingleArm: a bounded original-data fixture

Source: NVIDIA, **PhysicalAI-Robotics-Manipulation-SingleArm**, revision
`73c00c18e7ca108e91846294b20a1d7cd5de9340`, subset `panda-open-drawer`.
NVIDIA's [pinned dataset card](https://huggingface.co/datasets/nvidia/PhysicalAI-Robotics-Manipulation-SingleArm/blob/73c00c18e7ca108e91846294b20a1d7cd5de9340/README.md)
identifies CC BY 4.0. These original files and the derived selection are
attributed to NVIDIA under [CC BY 4.0](https://creativecommons.org/licenses/by/4.0/).
The source describes automatic motion planning in Isaac Sim with a Franka
Panda, rather than recordings from a physical robot.

| Retained original | Bytes | SHA-256 |
| --- | --- | --- |
| [info.json](https://huggingface.co/datasets/nvidia/PhysicalAI-Robotics-Manipulation-SingleArm/resolve/73c00c18e7ca108e91846294b20a1d7cd5de9340/panda-open-drawer/meta/info.json) | 5,319 | `1602a33e424eb300d969efd7655c9d7a990ec3aec2526e5e0af652ea7589a756` |
| [tasks.jsonl](https://huggingface.co/datasets/nvidia/PhysicalAI-Robotics-Manipulation-SingleArm/resolve/73c00c18e7ca108e91846294b20a1d7cd5de9340/panda-open-drawer/meta/tasks.jsonl) | 47 | `f30d82aed0cd39a64b6d44e474b83f2649859213d1daadbee360db47014842d4` |
| [episode_000000.parquet](https://huggingface.co/datasets/nvidia/PhysicalAI-Robotics-Manipulation-SingleArm/resolve/73c00c18e7ca108e91846294b20a1d7cd5de9340/panda-open-drawer/data/chunk-000/episode_000000.parquet) | 28,043 | `90f5658a7af7d9df68746ec0726c290f218b279790fb5ad5026519a0c9a69c89` |

`samples.json` is an inert selection of frames **0, 1, 2, 3 and 132** from
that original episode, preserving exact binary32 representations as unsigned
integer bit patterns. Its SHA-256 is
`8c226f1892e074b7b450e71a0b227f61b900f789dc673aea4903b9ce326f7cd8`.
It contains 170 F32 values: five timestamps, 125 state values and 40 actions.
The final selected frame is separated by a gap; this is not one contiguous
five-step rollout. No interpolation, normalization, unit conversion or invented
training row was applied.

The independent development export used existing PyArrow 24.0.0 to decode the
original file and `struct.pack('<f')`/`struct.unpack('<I')` to capture each F32
bit pattern. The JSON has compact separators, listed key order and one trailing
LF. [The independent oracle](../../../tests/robotics/singlearm_reference.py)
re-reads the original Parquet, verifies its pinned hashes and feature names,
and compares every selected bit. No third-party source script was executed.
The [C golden tables](../../../tests/robotics/singlearm-goldens.h) were generated
directly from the original Parquet independently of `samples.json`; the optional
reference oracle checks those tables against the original as well.
PyArrow is a development reference dependency only; the ordinary Coax fixture
checks read `samples.json` and require no Parquet reader or network access.

## What the exact files establish

- `info.json`: LeRobot v2.1, robot type `panda`, 30 Hz, 1,273 source episodes,
  154,256 source frames, 25 F32 state channels and eight F32 action channels.
- Zero-based state positions 3–6 name quaternion components in **x, y, z, w** order;
  positions 11–17 name seven joint positions. The action names are
  `panda_joint1_delta_pos` through `panda_joint7_delta_pos`, then `gripper`.
- The original episode has 133 rows, frame/index 0–132, episode/task zero.
  The final stored F32 timestamp is `4.400000095367432`; adjacent timestamps
  approximate `1/30` within binary32 rounding.

The pinned card says 81 state dimensions for this subset; the actual metadata
and Parquet both say **25**. This fixture follows the artifact and records the
disagreement. The card also describes depth imagery, while its metadata mixes
one-channel feature shapes with three-channel video descriptors and
`video.is_depth_map=false`; no image/depth decode is claimed here.

Neither these metadata files nor the selected rows establish exact controller
scaling, SI units, the frame of every quantity, or the physical meaning of each
gripper sign. Field names alone do not qualify a hardware command. This fixture
does not establish a Panda simulator, GR00T support, policy task success, or
permission to control a real robot. The chapter's owned learning simulation is
a separate, explicitly defined system.
