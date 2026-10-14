# Robot Lab documents

`robot-lab` implements the document and storage foundation in
[`docs/lab/`](../../docs/lab/README.md) ([#11603](https://github.com/OpenAgentsInc/openagents/issues/11603)).
The renderer, dataset reader, simulation runner, and chat mounting are separate
delivery slices.

Pass an absolute private file-store directory and the existing project's name
to `Store::open`. The directory must be outside a Git checkout. The store never
chooses your home directory or opens a network connection.

`Document` covers `robot.v1`, `robot-scene.v1`, `robot-experiment.v1`, and
`robot-episode.v1`. Rust checks dimensions, link trees, inertia, joint limits,
camera settings, rates, seeds, and resource bounds. Saving also checks that
referenced versions and files exist and that controllers match the scene.
The [JSON schema](schemas/documents.v1.json) describes the serialized types;
Rust performs the numerical and cross-document checks.

Stream files through `put_asset` with a byte limit. `save` creates a version
with its parent, author, summary, timestamp, and content hash. An edit names
the exact latest version; simultaneous edits cannot overwrite each other.
Versions are immutable. Notes name their subject and optional episode step.
Chat selections persist independently of edits, and experiments keep their
original scene versions.

`Operation` exposes opening, editing, posing, moving an object, comparison,
history, notes, selection, and episode inspection to callers. Tests use private
temporary stores and need no robot hardware or model weights.
