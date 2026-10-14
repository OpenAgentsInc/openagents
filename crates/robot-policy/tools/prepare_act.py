#!/usr/bin/env python3
"""Bind offline ACT training evidence to an exact saved Robot Lab scene.

This prepares checked data files only. Rust loads and runs the resulting
checkpoint; neither this helper nor the product executes downloaded code.
"""
import argparse
import hashlib
import json
from pathlib import Path

parser = argparse.ArgumentParser()
parser.add_argument("--artifacts", type=Path, required=True)
parser.add_argument("--scene-bundle", type=Path, required=True)
args = parser.parse_args()
root = args.artifacts.resolve()
assert not any((parent/".git").exists() for parent in (root, *root.parents))
training = json.loads((root/"so101-state-imitation.json").read_text())
bundle = json.loads(args.scene_bundle.read_text())
assert training["architecture"]["cameras"] == 0
robot = bundle["robot"]
description = robot["source"]["files"][robot["description"]]["sha256"]
definitions = {joint["name"]: joint for joint in robot["joints"] if joint["kind"] != "fixed"}
joints = []
for feature, unit in zip(training["features"], training["units"], strict=True):
    name = feature.removesuffix(".pos")
    limits = definitions[name]["limits"]
    assert unit in ("radians", "meters")
    joints.append({"feature":feature,"joint":name,"unit":{"unit":unit},"offset":0.0,"direction":1.0,"lower":limits["lower"],"upper":limits["upper"]})
mapping = {"schema":"robot-joint-mapping.v1","robot_type":training["robot_type"],"robot_version":description,"joints":joints}
task = {"version":{"name":"cube-placement-v1","sha256":hashlib.sha256(b"cube-placement-v1").hexdigest()},"object":"cube","target_min":[0.3,0.75,-0.15],"target_max":[0.4,0.85,-0.05],"stable_seconds":0.2,"max_linear_speed":0.01,"max_angular_speed":0.01}
normalization = {"state":training["state_normalization"],"environment":training["environment_normalization"],"action":training["action_normalization"],"cameras":{}}
(root/"normalization.json").write_text(json.dumps(normalization,indent=2)+"\n")
files = {}
for name in (training["weights"]["path"], "normalization.json", "so101-state-imitation.json"):
    data = (root/name).read_bytes()
    files[name] = {"sha256":hashlib.sha256(data).hexdigest(),"bytes":len(data)}
assert files[training["weights"]["path"]]["sha256"] == training["weights"]["sha256"]
pin = {"schema":"robot-policy.v1","family":"act","source":{"repository":"https://github.com/huggingface/lerobot","revision":training["reference_revision"],"license":"Apache-2.0","purpose":training["qualification"],"training_evidence":{"dataset_manifest":training["training_dataset_manifest_sha256"],"generator_sha256":hashlib.sha256(Path(__file__).with_name("act_reference.py").read_bytes()).hexdigest(),"steps":training["steps"],"seed":training["seed"],"frames":training["frames"],"training_normalized_mse":training["training_normalized_mse"],"training_action_rmse":training["training_action_rmse"],"source":training["training_source"]}},"architecture":training["architecture"],"binding":{"robot_type":training["robot_type"],"robot_description_sha256":description,"task":task["version"],"task_definition_sha256":hashlib.sha256(json.dumps(task,separators=(",",":")).encode()).hexdigest(),"instruction":"Move the cube into the target","state":mapping,"actions":mapping,"environment":[{"object":"cube","axis":axis} for axis in ("x","y","z")],"cameras":[],"timestep_seconds":1/240,"control_every_steps":8,"action_steps":4},"weights":training["weights"]["path"],"normalization":"normalization.json","files":files}
(root/"policy.json").write_text(json.dumps(pin,indent=2)+"\n")
print(hashlib.sha256((root/"policy.json").read_bytes()).hexdigest())
