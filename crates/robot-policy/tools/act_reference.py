#!/usr/bin/env python3
"""Generate ACT conformance evidence and a simulated-state imitation checkpoint.

Uses LeRobot 0.3.3, reference b883328e6c95681ca90a18b102e4ae5e1f91e2bf.
Weights stay in the explicitly chosen artifact folder, outside git. This is
offline training/acceptance tooling, not a Robot Lab service.
"""
import argparse
import hashlib
import importlib.metadata
import json
from pathlib import Path

parser = argparse.ArgumentParser()
parser.add_argument("--artifacts", type=Path, required=True)
parser.add_argument("--fixtures", type=Path, required=True)
parser.add_argument("--dataset", type=Path)
args = parser.parse_args()
assert args.artifacts.is_absolute()
args.artifacts.mkdir(parents=True, exist_ok=True)
assert not any((parent / ".git").exists() for parent in (args.artifacts, *args.artifacts.parents))
args.fixtures.mkdir(parents=True, exist_ok=True)
assert importlib.metadata.version("lerobot") == "0.3.3"

import numpy as np
import torch
import torch.nn.functional as F
import pyarrow.parquet as pq
from PIL import Image
from safetensors.torch import save_file
from torchvision.ops.misc import FrozenBatchNorm2d
from lerobot.policies.act.modeling_act import ACT
from lerobot.policies.act.configuration_act import ACTConfig
from lerobot.configs.types import FeatureType, PolicyFeature

torch.set_num_threads(2)
torch.use_deterministic_algorithms(True)
REVISION = "b883328e6c95681ca90a18b102e4ae5e1f91e2bf"


def configuration(vision=False, pre_norm=False, activation="relu"):
    inputs = {"observation.state": PolicyFeature(FeatureType.STATE, [6])}
    if vision:
        inputs["observation.images.front"] = PolicyFeature(FeatureType.VISUAL, [3, 64, 96])
    else:
        inputs["observation.environment_state"] = PolicyFeature(FeatureType.ENV, [3])
    return ACTConfig(device="cpu", use_vae=False, dim_model=32, n_heads=4,
        dim_feedforward=64, n_encoder_layers=2, n_decoder_layers=2, latent_dim=8,
        chunk_size=4, n_action_steps=4, dropout=0.0, pre_norm=pre_norm,
        feedforward_activation=activation, pretrained_backbone_weights=None,
        input_features=inputs, output_features={"action": PolicyFeature(FeatureType.ACTION, [6])})


def architecture(config):
    return {"dim_model": config.dim_model, "n_heads": config.n_heads,
        "dim_feedforward": config.dim_feedforward, "n_encoder_layers": config.n_encoder_layers,
        "n_decoder_layers": config.n_decoder_layers, "latent_dim": config.latent_dim,
        "chunk_size": config.chunk_size, "state_dim": 6,
        "environment_dim": 0 if config.image_features else 3, "action_dim": 6,
        "cameras": len(config.image_features), "pre_norm": config.pre_norm,
        "activation": config.feedforward_activation}


def save_weights(name, model):
    path = args.artifacts / f"{name}.safetensors"
    save_file({f"model.{key}": value.detach().contiguous() for key, value in model.state_dict().items()}, path)
    return {"path": path.name, "sha256": hashlib.sha256(path.read_bytes()).hexdigest(), "bytes": path.stat().st_size}


def norm(tensor, specification):
    if specification["mode"] == "identity":
        return tensor
    mean = torch.tensor(specification["mean"])
    std = torch.tensor(specification["std"])
    return (tensor-mean)/(std+1e-8)


state_raw = torch.tensor([[0.0, 0.1, -0.2, 0.3, -0.4, 0.5]], dtype=torch.float32)
state_norm = {"mode": "mean_std", "mean": [0.01, -0.02, 0.03, -0.04, 0.05, -0.06], "std": [0.2, 0.3, 0.4, 0.5, 0.6, 0.7]}
state = norm(state_raw, state_norm)
environment = torch.tensor([[0.22, 0.775, -0.1]], dtype=torch.float32)
y, x = np.mgrid[:96, :144]
rgb = np.stack(((x*3+y*2)%256, (x+y*5)%256, (x*7+y)%256), axis=-1).astype(np.uint8)
image_path = args.fixtures / "camera-pattern.png"
Image.fromarray(rgb).save(image_path)
image_raw = torch.from_numpy(rgb.copy()).permute(2, 0, 1).float().div(255).unsqueeze(0)
image_resized = F.interpolate(image_raw, size=(64, 96), mode="bilinear", align_corners=False)
image_norm = {"mode": "mean_std", "mean": [0.2, 0.3, 0.4], "std": [0.4, 0.5, 0.6]}
image = (image_resized-torch.tensor(image_norm["mean"]).reshape(1,3,1,1))/(torch.tensor(image_norm["std"]).reshape(1,3,1,1)+1e-8)
cases = []
for name, vision, pre_norm, activation in [("state-postnorm", False, False, "relu"), ("state-prenorm", False, True, "gelu"), ("vision-postnorm", True, False, "relu")]:
    torch.manual_seed(41)
    config = configuration(vision, pre_norm, activation)
    model = ACT(config).eval()
    if vision:
        for module in model.modules():
            if isinstance(module, FrozenBatchNorm2d):
                module.running_mean.copy_(torch.rand_like(module.running_mean)*0.4-0.2)
                module.running_var.copy_(torch.rand_like(module.running_var)+0.5)
                module.weight.copy_(torch.rand_like(module.weight)*0.4+0.8)
                module.bias.copy_(torch.rand_like(module.bias)*0.2-0.1)
    batch = {"observation.state": state}
    if vision:
        batch["observation.images"] = [image]
    else:
        batch["observation.environment_state"] = environment
    with torch.inference_mode():
        actions = model(batch)[0]
    cases.append({"name": name, "architecture": architecture(config), "weights": save_weights(name, model),
        "raw_state": state_raw.flatten().tolist(), "state_normalization": state_norm,
        "normalized_state": state.flatten().tolist(), "environment": [] if vision else environment.flatten().tolist(),
        "image": {"path": image_path.name, "sha256": hashlib.sha256(image_path.read_bytes()).hexdigest(), "source": [144,96], "target": [96,64], "normalization": image_norm} if vision else None,
        "normalized_image": image.flatten().tolist() if vision else [], "actions": actions.flatten().tolist()})
report = {"schema": "robot-act-reference.v1", "reference_revision": REVISION,
    "lerobot": importlib.metadata.version("lerobot"), "torch": importlib.metadata.version("torch"),
    "torchvision": importlib.metadata.version("torchvision"), "seed": 41,
    "purpose": "numerical-conformance", "action_absolute_tolerance": 0.0002,
    "preprocessing_absolute_tolerance": 0.000002, "cases": cases}
(args.fixtures / "act-reference.json").write_text(json.dumps(report, indent=2)+"\n")

if args.dataset:
    export = json.loads((args.dataset / "export.json").read_text())
    rows = pq.read_table(args.dataset / "data/chunk-000/episode_000000.parquet").to_pylist()
    stats = json.loads((args.dataset / "meta/stats.json").read_text())
    states = torch.tensor([row["observation.state"] for row in rows], dtype=torch.float32)
    actions = torch.tensor([row["action"] for row in rows], dtype=torch.float32)
    state_spec = {"mode":"mean_std","mean":stats["observation.state"]["mean"],"std":stats["observation.state"]["std"]}
    action_spec = {"mode":"mean_std","mean":stats["action"]["mean"],"std":stats["action"]["std"]}
    state_input = norm(states, state_spec)
    action_targets = norm(actions, action_spec)
    chunk = torch.stack([action_targets[[min(i+j,len(rows)-1) for j in range(4)]] for i in range(len(rows))])
    torch.manual_seed(73)
    config = configuration()
    model = ACT(config)
    optimizer = torch.optim.AdamW(model.parameters(), lr=0.002, weight_decay=0.0001)
    batch = {"observation.state":state_input,"observation.environment_state":environment.repeat(len(rows),1)}
    for step in range(400):
        optimizer.zero_grad(set_to_none=True)
        prediction = model(batch)[0]
        loss = F.mse_loss(prediction, chunk)
        loss.backward()
        torch.nn.utils.clip_grad_norm_(model.parameters(), 1.0)
        optimizer.step()
    model.eval()
    with torch.inference_mode():
        predictions = model(batch)[0]
        raw_predictions = predictions * torch.tensor(action_spec["std"]) + torch.tensor(action_spec["mean"])
    artifact = {"schema":"robot-act-training.v1","reference_revision":REVISION,"architecture":architecture(config),
        "weights":save_weights("so101-state-imitation",model),"state_normalization":state_spec,"action_normalization":action_spec,
        "environment_normalization":{"mode":"identity"},"environment_object":"cube","environment_axes":"y_up",
        "robot_type":export["spec"]["robot_type"],"features":export["spec"]["joints"],"units":export["spec"]["units"],
        "training_dataset_manifest_sha256":hashlib.sha256((args.dataset/"export.json").read_bytes()).hexdigest(),
        "training_source":export["spec"]["source"],"steps":400,"seed":73,"frames":len(rows),
        "training_normalized_mse":float(loss.detach()),"training_action_rmse":float(F.mse_loss(raw_predictions[:,0],actions).sqrt()),
        "qualification":"joint-trajectory imitation; cube-placement success has not been established",
        "golden_raw_state":states[8].tolist(),"golden_environment":environment.flatten().tolist(),
        "golden_normalized_actions":predictions[8].flatten().tolist(),"golden_actions":raw_predictions[8].flatten().tolist()}
    (args.artifacts/"so101-state-imitation.json").write_text(json.dumps(artifact,indent=2)+"\n")
    print(json.dumps({key:artifact[key] for key in ["weights","frames","training_normalized_mse","training_action_rmse","qualification"]},indent=2))
print(f"Wrote {len(cases)} reference cases and their pinned weights")
