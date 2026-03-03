---
name: autopilot-cad-builder
description: Deterministic CAD build orchestration for Autopilot Chat using openagents CAD and pane tools, including week-1 gripper flow.
metadata:
  oa:
    project: openagents
    identifier: autopilot-cad-builder
    version: "0.1.0"
    expires_at_unix: 1798761600
    capabilities:
      - codex:tool-call
      - cad:orchestration
      - cad:intent-control
      - desktop:pane-control
---

# Autopilot CAD Builder

Use this skill for CAD design turns from the main `Autopilot Chat` pane.

## Objective

- Keep the user in chat while building CAD in realtime.
- Use structured CAD mutations only.
- Make progress deterministic and inspectable.
- Keep snapshot truth aligned with what is visibly rendered.

## Required Tools

Use only:

- `openagents.pane.list`
- `openagents.pane.open`
- `openagents.pane.focus`
- `openagents.pane.action`
- `openagents.cad.intent`
- `openagents.cad.action`

## Operating Contract

1. Ensure CAD pane is open/focused before CAD mutations.
2. Prefer `intent_json` with typed payloads over free-form prompt edits.
3. After each mutating intent, checkpoint with snapshot/status action.
4. Keep tool sequences short and deterministic.
5. If intent parse fails, retry once with stricter `intent_json`.
6. If CAD mutation fails, return concise user-facing remediation.
7. Never claim a 2x2 grid is visible unless snapshot truth confirms it.

## Week-1 Canonical Prompt

Use this exact natural-language prompt when user intent is week-1 gripper build:

`Create a basic 2-jaw robotic gripper with a base plate, two parallel fingers, and mounting holes for a servo motor. Make it 3D-printable and parametric for easy scaling.`

## Preferred `intent_json` For Week-1

Use strict typed payloads:

```json
{
  "intent": "CreateParallelJawGripperSpec",
  "jaw_open_mm": 42.0,
  "finger_length_mm": 65.0,
  "finger_thickness_mm": 8.0,
  "base_width_mm": 78.0,
  "base_depth_mm": 52.0,
  "base_thickness_mm": 8.0,
  "servo_mount_hole_diameter_mm": 2.9,
  "print_fit_mm": 0.15,
  "print_clearance_mm": 0.35
}
```

For week-1 variants, require deterministic IDs in stable order:

- `variant.baseline`
- `variant.wide-jaw`
- `variant.long-reach`
- `variant.stiff-finger`

Set material per active variant with explicit `SetMaterial` calls and verify `variant_materials` map after each assignment.

## Week-1 Build Sequence

1. `openagents.pane.open` for CAD.
2. `openagents.pane.focus` for CAD.
3. `openagents.cad.intent` with strict `CreateParallelJawGripperSpec` `intent_json`.
4. `openagents.cad.intent` for `GenerateVariants` with `count=4`.
5. Cycle active variant and call `SetMaterial` for each target variant.
6. Capture snapshot truth in single layout.
7. Toggle viewport layout (`toggle_viewport_layout` or `toggle_layout`) to quad.
8. Capture snapshot truth again and confirm all variants are visible.
9. Return final summary tied to checkpoint fields, not assumptions.

## Snapshot Truth Contract

Before asserting layout/visibility, verify:

- `design_profile == "parallel_jaw_gripper"` for week-1 path.
- `viewport_layout` is `single` or `quad`.
- `visible_variant_ids` matches current rendered layout.
- `all_variants_visible` is `false` in single and `true` in quad.
- `variant_materials` map includes all four week-1 variant IDs.

## Safety Rules

- Do not invent unsupported CAD intents.
- Do not claim completion without reading a CAD snapshot/checkpoint.
- Do not claim "all four variants are showing" unless `all_variants_visible=true` and `visible_variant_ids` includes all four IDs.
- Do not use non-`openagents.*` tools for CAD pane mutation.
