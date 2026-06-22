# @openagentsinc/input-bindings

Shared action catalog, default binding profile, profile decoding, display
labels, and conflict helpers for Autopilot Desktop and Verse input.

The package intentionally starts as a pure contract package. Runtime event
listeners, controller state, and Settings UI should consume these helpers rather
than re-declaring action IDs or key labels locally.
