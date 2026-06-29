# @openagentsinc/input-bindings

Shared action catalog, default binding profile, profile decoding, display
labels, and conflict helpers for Autopilot Desktop and Verse input.

The package intentionally starts as a pure contract package. Runtime event
listeners, controller state, and Settings UI should consume these helpers rather
than re-declaring action IDs or key labels locally.

It also exports `createOpenAgentsKeyboardControls`, a small Drei-inspired
named-action keyboard state primitive. It accepts a scoped event source, maps
keyboard events to action IDs, tracks held state, supports subscriptions, and
can swap binding maps while clearing stale held keys.
