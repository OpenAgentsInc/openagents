# Verified agent edit and replay workflow

Status: browser prototype for [issue #5](https://github.com/OpenAgentsInc/aethyrforge/issues/5).
The same tools operate on the owned courtyard and Medieval Town. The initial
task adds and moves a barrel, detects rejected edits and shader failures, repairs
the change, plays the key-and-exit quest, and reproduces the accepted actions in
a fresh process.

## Tool contract

The trusted editor exposes `window.aethyrforge.agent`, an `AgentClient` using
`aethyrforge.agent.v1`. It provides `discover`, `inspect`, `execute`, `result`,
`capture`, `exportLog`, `replay`, and `close`. It is an adapter over the existing
editor/preview bridge, not another world owner or an untrusted-code sandbox.
Existing origin checks, player permissions, Rust validation, shader preparation,
and command/history behavior still apply.

Inspection returns a source identity, authored revision, runtime generation,
kernel tick, working shader identity, player, input, camera, game progress, and
bounded objects. Authored objects have stable IDs and optional runtime handles.
The cursor pages authored objects; a remaining-budget sample of read-only source
objects and its total count do not imply that every imported object is editable.
The document identity hashes semantic state without revision or runtime handles.

Actions use explicit prop IDs and positions, transforms, removal, history,
bounded input, camera poses, shaders, and game interactions. Random placement
and free text are excluded from portable logs. The worker checks the expected
source, revision, generation, and shader before routing an action. Rust checks
the transaction revision again before commit. Input lasts at most one second
and releases in `finally`; cancel invalidates pending agent work and clears input.

Receipts identify the request and actual result. They distinguish rejection,
activation, and saved edits; an activation receipt does not claim a displayed
frame. Kernel preparation, renderer preparation, commit, activation, receipt,
and capture/readback spans remain separate. A timed-out client retains an
uncertain action and requires outcome recovery before another write or export.
The live receipt lookup is bounded and session-local. Portable logs are explicit
exports, not a claim of server-side durable command delivery.

## Captures and independent checks

Agent capture requires an expected context and explicit dimensions, camera,
time, and exposure. The worker freezes simulation admission, owns a temporary
render size, and waits for a new submitted frame and GPU readback. The renderer
records the camera actually passed to rendering, not a copy of requested fields.
The envelope binds session, request, sequence, context, submitted frame numbers,
settings, tightly packed RGBA bytes, and SHA-256. Cleanup restores live rendering.
The existing raw capture API remains available to other tools.

`agent-oracle.ts` checks context, camera direction, image hash, freshness,
nonblack/variable pixels, analytically projected landmark regions, causal edit
regions, authored object poses, and submitted geometry. Its quest expectations
come from a fixed phase table rather than calling the game implementation.
The owned fixture's boxes and material colors are checked against committed
source data. A before/after edit must change pixels where its independently
projected geometry moves; a newly labeled old image fails.

Negative controls include black and uniform images, missing geometry, wrong
camera metadata, wrong-camera pixels, and stale pixels with fresh metadata.
The controls recompute hashes, so a passing checksum cannot hide the defect.
No screenshot regeneration changes the independent geometry or phase rules.
Town checks establish causal edits and game behavior, not gallery visual parity.

Submitted geometry counters cover indexed source, authored, and room meshes in
the main HDR pass. Instance counts count primitive instances, so an object with
multiple material primitives contributes more than one. They exclude sky,
character, water, debris, effects, depth, and shadow passes. Use the image and
semantic checks together with these partial counters.

## Restart and replay

A bounded run log records actions, before/after contexts and document identities,
receipts, diagnostics, and elapsed times. Replay checks the initial source,
document, and working shader before its first write, skips confirmed failures,
and verifies the accepted semantic document and shader after every action.
Unrecorded edits or an incompatible starting world cause rejection.

Town draft storage preserves authored objects, rooms, and wall removals. Live
quest progress and temporary shader/environment edits do not all persist there.
Replay reconstructs recorded changes explicitly; it does not claim that a
browser restart preserves every live runtime value. Wall-clock input is observed
behavior, not cross-device deterministic physics. Explicit camera setup for
interaction tests is labeled and does not establish a continuous walking route.

Closing the adapter cancels its requests and input while preserving a shared
editor session. Closing the project through `EditorService.close()` unloads the
preview. The verification runner additionally closes its browser context and
servers, releasing the resources it owns.

## Reproduce

Run `npm test`, `npm run check`, and `npx vite build`.
Run `tools/verify-agent.mjs` through `openagents browser run`, with an external
`AF_CAPTURE_DIR` and `AF_VERIFY_WORLD=starter` or `medieval-town`. The private Town
needs `AETHYRFORGE_PRIVATE_ASSETS`. Logs and licensed captures remain outside Git.

The runner records and replays in separate Node processes, separate Vite server
pairs, fresh browser contexts, and fresh preview workers. It explicitly retains
the same physical Chrome process. It reports completion, attempt, edit, shader,
capture, and replay spans. This scripted tool exercise uses no model inference;
its runtime inference count and cost are zero. Engineering-agent token usage is
outside that measurement.

## Recorded result

The [scrubbed evidence](https://github.com/OpenAgentsInc/aethyrforge/blob/71c19bfe8b5de79811de5ca518e783876650d42f/experiments/kaigen/results/issue-5.json) records eleven
checks while editing and playing, two in a fresh replay process, and exact final
authored state, quest, player, shader, and fixed-image equality in both worlds.
The complete scripted recording/replay takes about 4.2 seconds for the owned
starter and 10.8 seconds for the frozen Druid Town on this Mac. These are local
task measurements, including startup, not general performance guarantees.

Both worlds reject stale contexts, invalid placement, and failed WGSL without
losing the working state. Rehashed missing geometry, black, wrong-camera, and
stale images fail independent checks. No preview workers remain after close.
The report retains browser warnings and records zero runtime model calls.
Type checking, 626 TypeScript tests, the production build, and specification
checks pass. Imported-object sample truncation is tested separately from complete
authored-object pagination; it cannot hide missing authored objects.
