# CAD Chat Session Lifecycle

This document defines CAD session binding behavior for Autopilot chat threads.

## Ownership

- App orchestration lives in `apps/autopilot-desktop`.
- Typed intent/schema/dispatch primitives stay in `crates/cad`.

## Flow

1. User submits chat message.
2. CAD adapter attempts chat-to-intent translation.
3. If an intent is produced:
   - Resolve or create deterministic CAD session for the thread.
   - Bind thread -> session in `CadDemoPaneState`.
   - Dispatch intent through typed CAD dispatcher.
   - Record revision/action and activity feed row.
4. Follow-up intents on same thread reuse the same session.

## Deterministic Session IDs

- Session IDs are derived from thread IDs with stable normalization.
- Same thread always maps to same CAD session unless state reset.

## Failure Behavior

- Ambiguous CAD-like prompts set CAD recovery guidance (`last_error` + recovery text).
- Non-CAD prompts do not force CAD session creation.

## Follow-Up Interaction Golden

Golden fixture:

- `apps/autopilot-desktop/tests/goldens/cad_followup_parameter_edit_interaction.json`

Covered scripted path:

1. chat prompt translation (`Select rack_outer_face`)
2. selection action (`SelectTimelineRow`)
3. typed dimension edit (`StartDimensionEdit` + char input + commit)
4. rebuild receipt commit
5. warnings/analysis refresh

Run:

```bash
cargo test -p autopilot-desktop follow_up_parameter_edit_interaction_matches_golden_receipts --quiet
```

Regenerate fixture intentionally:

```bash
CAD_UPDATE_GOLDENS=1 cargo test -p autopilot-desktop follow_up_parameter_edit_interaction_matches_golden_receipts --quiet
```
