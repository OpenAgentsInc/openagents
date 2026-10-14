#!/usr/bin/env python3
"""Check a Rust export with LeRobot 0.3.3's loader and statistics functions.

Reference tag v0.3.3 is b883328e6c95681ca90a18b102e4ae5e1f91e2bf.
This is offline acceptance tooling; Robot Lab's runtime is Rust.
"""
import argparse
import hashlib
import importlib.metadata
import json
import os
from pathlib import Path

parser = argparse.ArgumentParser()
parser.add_argument("dataset", type=Path)
parser.add_argument("--scratch", type=Path, required=True)
parser.add_argument("--output", type=Path, required=True)
args = parser.parse_args()
assert args.scratch.is_absolute()
args.scratch.mkdir(parents=True, exist_ok=True)
os.environ["HF_HOME"] = str(args.scratch / "hf")
os.environ["HF_DATASETS_CACHE"] = str(args.scratch / "datasets")
os.environ["HF_HUB_OFFLINE"] = "1"
os.environ["HF_DATASETS_OFFLINE"] = "1"
assert importlib.metadata.version("lerobot") == "0.3.3"

import numpy as np
import pyarrow.parquet as pq
from PIL import Image
from io import BytesIO
from lerobot.datasets.lerobot_dataset import LeRobotDataset
from lerobot.datasets.compute_stats import aggregate_stats, compute_episode_stats
from lerobot.datasets.utils import serialize_dict

root = args.dataset.resolve()
export = json.loads((root / "export.json").read_text())
for name, pin in export["files"].items():
    data = (root / name).read_bytes()
    assert len(data) == pin["bytes"] and hashlib.sha256(data).hexdigest() == pin["sha256"]
dataset = LeRobotDataset("local/robot-lab", root=root, video_backend="pyav")
stats = []
maximum_error = 0.0
count = 0
for episode in range(dataset.num_episodes):
    rows = pq.read_table(root / f"data/chunk-000/episode_{episode:06d}.parquet").to_pylist()
    raw = {}
    for key, feature in dataset.features.items():
        values = [row[key] for row in rows]
        if feature["dtype"] == "image":
            paths = []
            for frame, value in enumerate(values):
                path = args.scratch / f"episode-{episode}-{key}-{frame}.png"
                path.write_bytes(value["bytes"])
                paths.append(str(path))
                image = Image.open(BytesIO(value["bytes"]))
                assert list(image.size) == list(feature["shape"][1::-1])
            raw[key] = paths
        else:
            raw[key] = np.array(values, dtype=feature["dtype"])
    stats.append(compute_episode_stats(raw, dataset.features))
    for row in rows:
        actual = dataset[count]
        np.testing.assert_array_equal(actual["observation.state"].numpy(), row["observation.state"])
        np.testing.assert_array_equal(actual["action"].numpy(), row["action"])
        for camera in dataset.meta.camera_keys:
            assert tuple(actual[camera].shape) == (3, *dataset.features[camera]["shape"][:2])
        count += 1
expected = serialize_dict(aggregate_stats(stats))
for key, values in expected.items():
    for kind, numbers in values.items():
        a = np.array(numbers)
        b = np.array(dataset.meta.stats[key][kind])
        np.testing.assert_allclose(a, b, atol=2e-6, rtol=2e-6)
        maximum_error = max(maximum_error, float(np.max(np.abs(a-b))))
report = {"schema": "robot-export-reference.v1", "reference_revision": "b883328e6c95681ca90a18b102e4ae5e1f91e2bf", "lerobot": importlib.metadata.version("lerobot"), "torch": importlib.metadata.version("torch"), "pyarrow": importlib.metadata.version("pyarrow"), "episodes": dataset.num_episodes, "frames": count, "cameras": dataset.meta.camera_keys, "max_statistics_error": maximum_error, "tolerance": 2e-6, "dataset_manifest_sha256": hashlib.sha256((root/"export.json").read_bytes()).hexdigest()}
args.output.write_text(json.dumps(report, indent=2)+"\n")
print(json.dumps(report, indent=2))
