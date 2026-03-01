---
name: autopilot-cad-builder
description: Deterministic CAD build orchestration for Autopilot Chat using openagents CAD and pane tools.
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

## Minimal Build Sequence

1. `openagents.pane.open` for CAD.
2. `openagents.pane.focus` for CAD.
3. `openagents.cad.intent` with typed intent payload.
4. `openagents.pane.action` with `snapshot` (or equivalent status action).
5. Repeat 3-4 until target shape is complete.
6. Return final summary tied to CAD snapshot state.

## Safety Rules

- Do not invent unsupported CAD intents.
- Do not claim completion without reading a CAD snapshot/checkpoint.
- Do not use non-`openagents.*` tools for CAD pane mutation.
