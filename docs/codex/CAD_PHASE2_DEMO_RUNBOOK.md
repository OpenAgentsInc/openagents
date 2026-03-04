# CAD Phase-2 Robotic Hand Demo Runbook

## Purpose

This runbook is the operator guide for the Phase-2 robotic hand progression demo. It is designed to be executable without tribal knowledge and to map each step to deterministic checkpoints.

## Preconditions

- CAD release gates pass: `scripts/cad/release-gate-checklist.sh`.
- Phase-2 harness goldens are current: `cargo test -p autopilot-desktop cad_chat_build_e2e_harness_phase2 -- --nocapture`.
- Desktop app is running on the target build, with Autopilot Chat + CAD pane visible.

## Phase-2 Sequence (Six Steps)

1. Baseline gripper
Prompt: `Create a basic 2-jaw robotic gripper with a base plate, two parallel fingers, and mounting holes for a servo motor. Make it 3D-printable and parametric for easy scaling.`
Manual tweaks: set `variant.baseline` material to `steel-1018`, cycle to `variant.wide-jaw`, set material to `al-5052-h32`.
Camera moves: fit view, orbit ~20 degrees, quick zoom in/out once.
Checkpoint: `design_profile=parallel_jaw_gripper`, single layout shows only active variant, quad layout shows `baseline/wide-jaw/long-reach/stiff-finger`.
Harness reference: `apps/autopilot-desktop/tests/scripts/cad_chat_build_e2e_phase2_gripper_script.json`.

2. Underactuated gripper
Prompt: `Modify the gripper to be underactuated with compliant flexure joints and a single servo drive; use 3 compliant joints and 1.3mm flexure thickness.`
Manual tweaks: set `variant.baseline` material to `ti-6al-4v`, cycle to `variant.wide-jaw`, set material to `al-5052-h32`.
Camera moves: orbit ~15 degrees, pause on flexure joints.
Checkpoint: `design_profile=parallel_jaw_gripper_underactuated`, quad layout still shows gripper variant set.
Harness reference: `apps/autopilot-desktop/tests/scripts/cad_chat_build_e2e_phase2_underactuated_script.json`.

3. Three-finger + thumb
Prompt: `Evolve the gripper into a 3-finger hand with an opposable thumb, tendon-driven for dexterity. Add cable routing channels and tripod grasp pose.`
Manual tweaks: set `variant.baseline` material to `steel-1018`, cycle to `variant.pinch`, set material to `al-5052-h32`.
Camera moves: orbit ~25 degrees, hold on thumb alignment.
Checkpoint: `design_profile=three_finger_thumb`, quad layout shows `baseline/pinch/tripod/wide-thumb`.
Harness reference: `apps/autopilot-desktop/tests/scripts/cad_chat_build_e2e_phase2_three_finger_script.json`.

4. Motor integration
Prompt: `Add servo motors to each finger joint, including wiring paths and gearbox housings. Optimize for compact layout and low-cost 3D printing.`
Manual tweaks: set `variant.baseline` material to `steel-1018`, cycle to `variant.pinch`, set material to `ti-6al-4v`.
Camera moves: orbit ~25 degrees, pause on servo housings and gearbox blocks.
Checkpoint: `design_profile=three_finger_thumb`, verify servo housings and wiring channels are visible.
Harness reference: `apps/autopilot-desktop/tests/scripts/cad_chat_build_e2e_phase2_motors_script.json`.

5. Sensors + electronics mounts
Prompt: `Incorporate force sensors on fingertips, proximity sensors, and a control board mount. Ensure the design is modular for easy upgrades.`
Manual tweaks: set `variant.baseline` material to `steel-1018`, cycle to `variant.pinch`, set material to `al-5052-h32`.
Camera moves: close-up on sensor pads, then pull back to show control board mount.
Checkpoint: `design_profile=three_finger_thumb`, sensor pads and control board mount are visible in the timeline.
Harness reference: `apps/autopilot-desktop/tests/scripts/cad_chat_build_e2e_phase2_sensors_script.json`.

6. Full humanoid hand
Prompt: `Generate a complete 5-finger humanoid robotic hand with all motors, tendons, sensors, electronics, and mounting arm interface.`
Manual tweaks: set `variant.baseline` material to `steel-1018`, cycle to `variant.precision`, set material to `ti-6al-4v`.
Camera moves: slow orbit ~30 degrees, pause on full finger spread, quick top-down pass.
Checkpoint: `design_profile=humanoid_hand_v1`, quad layout shows `baseline/precision/power/wide-spread`.
Harness reference: `apps/autopilot-desktop/tests/scripts/cad_chat_build_e2e_phase2_full_hand_script.json`.

## Short Hook Clip (15-20s)

- Start on the final humanoid hand pose (step 6).
- Quick cut to steps 1, 3, and 4 (1-2s each).
- End on a slow orbit of the full hand with motors visible.

## Deep-Dive Clips (30-45s each)

- Clip A: Steps 1-2 (gripper to underactuation), focus on joint compliance.
- Clip B: Steps 3-4 (three-finger to motors), focus on tendon routing and servo housings.
- Clip C: Steps 5-6 (sensors to full hand), focus on sensor pads and full assembly silhouette.

## Operator Checklist

- Confirm phase-2 harness passes on current build.
- Use the exact prompts above; avoid paraphrasing during recording.
- Capture at least one single-layout and one quad-layout shot per step.
- Verify checkpoint expectations before recording each segment.
- Archive raw clips with date/time and step number.

## Runbook Artifacts

- `apps/autopilot-desktop/tests/scripts/cad_chat_build_e2e_phase2_*`
- `apps/autopilot-desktop/tests/goldens/cad_chat_build_e2e_phase2_*`
