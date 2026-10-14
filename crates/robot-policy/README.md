# Robot policies

`robot-policy` runs ACT in Rust with Candle 0.11 on CPU. The port follows
[LeRobot 0.3.3](https://github.com/huggingface/lerobot/tree/b883328e6c95681ca90a18b102e4ae5e1f91e2bf)
(Apache-2.0): a ResNet-18 feature-map backbone, frozen batch normalization,
two-dimensional image positions, pre-norm or post-norm transformer layers,
ReLU or GELU, a zero inference latent, and joint-position action chunks.

The checked cases cover both transformer normalization styles and image
inference. Their largest Rust/reference action difference is 4.77e-7 with a
2e-4 absolute tolerance. State normalization and RGB bilinear resizing use
LeRobot's `1e-8` denominator adjustment and `align_corners=false`; preprocessing
has a 2e-6 tolerance. Weights remain outside git. Original camera-pattern pixels
and reference outputs are in `fixtures/`.

A private `policy.json` pins the weights, architecture, normalization,
robot-description hash, named joint units and limits, task definition, fixed
instruction, environment fields, cameras, control interval, and used chunk
length. The loader verifies every declared file. It reads data and runs Rust;
it neither downloads nor executes model repository code. Supported checkpoints
use one observation, robot state, up to four cameras or mapped environment
positions, and a fixed task. Temporal ensembling, other visual backbones,
dilated ResNet blocks, and language-conditioned ACT variants need separate
qualification before support.

`NativeController` checks its experiment and scene before starting. It reads
only saved camera images, preserves observation-step identities and chunk
expiry, and maps decoded actions within the robot's declared limits. One worker
runs inference. A timeout returns without waiting for the compute operation;
the controller then closes its queue, preventing a late result from entering
another observation. Simulation time stays paused during the call.

Generate evidence with the offline Python tools:

```sh
python tools/act_reference.py --artifacts /private/act-weights \
  --fixtures fixtures --dataset /private/recorded-dataset
python tools/prepare_act.py --artifacts /private/act-weights \
  --scene-bundle /private/saved-scene-bundle.json
ROBOT_ACT_ARTIFACTS=/private/act-weights cargo test -p robot-policy \
  -- --include-ignored
```

The reference environment uses LeRobot 0.3.3, PyTorch 2.7.1 CPU, and
torchvision 0.22.1. The first compatible checkpoint learns the recorded
SO-101 joint trajectory from 30 simulated frames. Its training error does not
establish cube-placement performance. `robot-workbench`'s `act_run` example
qualifies the native controller and saves its physical episode for replay;
Gym records task outcomes separately.

The native walkthrough on `coderos-4080` saved 241 physical steps and eight
inference calls, with a largest inference time of 38 ms and no joint-limit
violations. It checked pause, single-step, reset with the previous recording
preserved, and action origin/expiry. Saved cameras accompany the episode.
Run `cargo run -p robot-workbench --example act_run -- STORE PROJECT
SCENE@VERSION JOB LIBRARY LIBRARY_SHA POLICY_FOLDER POLICY_SHA` with explicit
private paths and the reviewed policy manifest hash.
