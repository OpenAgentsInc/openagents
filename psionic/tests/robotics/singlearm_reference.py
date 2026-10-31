#!/usr/bin/env python3
"""Firm reason: 2 (independent oracle). Read original Parquet with existing
PyArrow and compare an inert Coax fixture bit-for-bit. Development only;
neither PyArrow nor this reader is part of Psionic's runtime/ordinary checks.
"""
import argparse
import hashlib
import json
import math
from pathlib import Path
import re
import struct


REVISION = "73c00c18e7ca108e91846294b20a1d7cd5de9340"
PARQUET_SHA = "90f5658a7af7d9df68746ec0726c290f218b279790fb5ad5026519a0c9a69c89"
PINNED = {
    "episode_000000.parquet": PARQUET_SHA,
    "info.json": "1602a33e424eb300d969efd7655c9d7a990ec3aec2526e5e0af652ea7589a756",
    "tasks.jsonl": "f30d82aed0cd39a64b6d44e474b83f2649859213d1daadbee360db47014842d4",
}
FRAMES = [0, 1, 2, 3, 132]
STATE_NAMES = [
    "eef_pos_x", "eef_pos_y", "eef_pos_z", "eef_quat_x", "eef_quat_y",
    "eef_quat_z", "eef_quat_w", "panda_finger_joint1_pos",
    "panda_finger_joint2_pos", "panda_finger_joint1_vel", "panda_finger_joint2_vel",
] + [f"panda_joint{i}_pos" for i in range(1, 8)] + [
    f"panda_joint{i}_vel" for i in range(1, 8)
]
ACTION_NAMES = [f"panda_joint{i}_delta_pos" for i in range(1, 8)] + ["gripper"]


def require(condition, message):
    if not condition:
        raise SystemExit("SingleArm reference: " + message)


def f32_bits(value):
    require(math.isfinite(value), "nonfinite original numeric value")
    return struct.unpack("<I", struct.pack("<f", value))[0]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("fixture", nargs="?", type=Path, default=(
        Path(__file__).resolve().parents[2] / "fixtures/robotics/singlearm-panda-open-drawer"
    ))
    args = parser.parse_args()
    for filename, wanted in PINNED.items():
        require(hashlib.sha256((args.fixture / filename).read_bytes()).hexdigest() == wanted,
                "original hash mismatch: " + filename)

    info = json.loads((args.fixture / "info.json").read_text())
    require(info["codebase_version"] == "v2.1" and info["robot_type"] == "panda"
            and info["fps"] == 30 and info["total_episodes"] == 1273
            and info["total_frames"] == 154256, "source metadata identity")
    for key, count, names in [("action", 8, ACTION_NAMES), ("observation.state", 25, STATE_NAMES)]:
        feature = info["features"][key]
        require(feature["dtype"] == "float32" and feature["shape"] == [count]
                and feature["names"] == names, "source feature profile: " + key)
    tasks = [json.loads(line) for line in (args.fixture / "tasks.jsonl").read_text().splitlines()]
    require(tasks == [{"task_index": 0, "task": "panda-open-drawer"}], "source task")

    try:
        import pyarrow
        import pyarrow.parquet as pq
    except ImportError:
        raise SystemExit("SingleArm reference: use an existing PyArrow environment; no install is performed")
    table = pq.read_table(args.fixture / "episode_000000.parquet")
    require(table.num_rows == 133 and table.column_names == [
        "action", "observation.state", "timestamp", "frame_index", "episode_index", "index", "task_index"
    ], "original episode shape")
    require(pyarrow.types.is_fixed_size_list(table.schema.field("action").type)
            and table.schema.field("action").type.list_size == 8
            and pyarrow.types.is_float32(table.schema.field("action").type.value_type), "original action dtype")
    require(pyarrow.types.is_fixed_size_list(table.schema.field("observation.state").type)
            and table.schema.field("observation.state").type.list_size == 25
            and pyarrow.types.is_float32(table.schema.field("observation.state").type.value_type)
            and pyarrow.types.is_float32(table.schema.field("timestamp").type), "original state/timestamp dtype")
    original = table.to_pylist()
    for frame, row in enumerate(original):
        require(row["frame_index"] == row["index"] == frame and row["episode_index"] == row["task_index"] == 0,
                "original episode/frame identity")
        require(abs(row["timestamp"] - frame / 30) < 0.000001, "original 30Hz timestamp")

    expected = {
        "schema": "psionic-singlearm-f32-v1", "source_revision": REVISION,
        "parquet_sha256": PARQUET_SHA, "episode": 0, "fps": 30, "source_frames": 133,
        "rows": [{
            "frame": frame, "timestamp_bits": f32_bits(original[frame]["timestamp"]),
            "state_bits": [f32_bits(x) for x in original[frame]["observation.state"]],
            "action_bits": [f32_bits(x) for x in original[frame]["action"]],
        } for frame in FRAMES],
    }
    actual = json.loads((args.fixture / "samples.json").read_text())
    require(actual == expected, "export differs from original Parquet F32 bits or profile")
    # Python booleans compare equal to integers; the inert schema requires integers.
    for row in actual["rows"]:
        values = [row["timestamp_bits"], *row["state_bits"], *row["action_bits"]]
        require(type(row["frame"]) is int and all(type(x) is int and 0 <= x <= 0xFFFFFFFF for x in values),
                "export integer encoding")
    require(all(type(actual[key]) is int for key in ["episode", "fps", "source_frames"]), "export scalar encoding")
    header = Path(__file__).with_name("singlearm-goldens.h").read_text()
    wanted_arrays = {
        "frame": FRAMES,
        "timestamp_bits": [row["timestamp_bits"] for row in expected["rows"]],
        "state_bits": [value for row in expected["rows"] for value in row["state_bits"]],
        "action_bits": [value for row in expected["rows"] for value in row["action_bits"]],
    }
    for suffix, values in wanted_arrays.items():
        match = re.search(r"sa_reference_" + suffix + r"(?:\[\d+\])+ = \{(.*?)\};", header, re.S)
        require(match is not None, "missing independent C table: " + suffix)
        got = [int(number, 16) for number in re.findall(r"UINT32_C\(0x([0-9a-f]{8})\)", match.group(1))]
        require(got == values, "independent C table differs from original Parquet: " + suffix)
    print(f"SingleArm reference: PyArrow {pyarrow.__version__}, 133 original rows, 5 selected frames, 170 F32 values exact in JSON and C tables")


if __name__ == "__main__":
    main()
