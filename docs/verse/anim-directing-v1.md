# Animation directions

A7 (#12972) extends the Decisions director to admitted Animation Lab projects.
The editor, request/reply admission, checks and session history live in
`metaverse-engine/anim/direction*.coax`. Scene loading, sampling and checking also live
under `metaverse-engine/anim/`; the former realm paths are compatibility loaders.

`openagents labs anim edit FILE "direction" [--origin URL]` sends a bounded
inventory and the current bundle SHA-256 to `/api/playtest/labs/anim/direct`.
The existing director transport supplies NIP-98 authentication, replay checks,
quotas, leases and provider deadlines. No keyword parser selects operations.
The response schema requires one operation and a confidence score for every
field. The weakest score must exceed 850/1000. These are model-reported
confidence scores, not calibrated probabilities. Unmatched and ambiguous
responses leave the scene unchanged.

The catalogue supports layer speed, layer timing, path key replacement,
look-at, hand-to, hold, compatible stock clip replacement, static pose keys,
shot insertion, removal, shot retiming and a cut at an authored effect's start.
Targets and indices must exist in the admitted inventory. A static key must
have identical samples and no events or root motion. Look-at and hand-to
targets are sampled at the beginning of the window and held in actor-local
space; this profile does not add live target bindings. Compound requests are
unmatched: each accepted response changes exactly one JSON component.

Every result is re-admitted as a complete project, then checked using the same
scene report as `labs anim check`: continuity, repeatability, face ranges,
canonical elbow/knee limits, root capsules, authored foot contacts and hero
head framing. A failure returns its check name and publishes nothing. These
are the A5 screening bounds, not a general physics proof. Legacy cut formats
remain readable but directing requires `anim.cut.v1`.

The CLI requires canonical private storage under `~/.openagents/`. It keeps
the source untouched and writes `FILE.anim-history/`: immutable ALP snapshots,
`anim.version.v1` receipts and an atomically replaced `head`. A version ID is
the SHA-256 of its receipt, which binds the content hash, parent, heard words
and complete typed proposal. Different direction receipts can retain the same
content without overwriting history. A directory lock and compare-before-
publish reject changes to the resolved current bundle. Reading a corrupt receipt or snapshot
fails rather than falling back to the source.

`labs anim diff FILE` prints the complete changed JSON component and parent
and current version IDs. `labs anim undo FILE` atomically restores the parent;
fork snapshots remain. Check and play resolve the current head automatically.

In `#scene` and `#cut`, typed message 701 and explicitly activated voice use
the same request and edit path. The voice, undo, diff and cancel controls also
accept V/U/D/Escape (ordinary lowercase keys work). Voice uses the existing
final-transcript bridge; interim and repeated final transcripts cannot create
extra versions. Cancellation revokes pending responses. The browser retains
16 session versions, with at most 32 KiB per changed JSON component; reaching
capacity refuses the edit. Its diff copies full JSON and its undo restores the
exact preceding bundle. Browser history is session-local; persistent file
versions are provided by the CLI.

Private qualification uses the Guardian study with a fifth, original static
wand pose. Licensed body data and all requests, captures and version files stay
outside Git. See the [verification record](../verification/animation-lab-a7-2026-11-07/README.md).
