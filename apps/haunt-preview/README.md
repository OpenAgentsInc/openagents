# Haunted House creator preview

A local browser editor with direct WebGPU, Effect, the real Kiln WASI/OPFS
workspace, and the Rust Jev SDK. Scene controls apply typed commands immediately.
Natural language takes one bounded decision request, then uses those same commands.

## Creator flow

| State | What you see | Next action |
| --- | --- | --- |
| Open | A running house and placement controls | Walk, or add one pumpkin |
| Select and change | A pumpkin selector, X/Z/size fields, and lantern colors | Add, move, resize, remove, or change light |
| Describe | A short request such as “Add a big pumpkin to my left” | See the result, keep walking, or cancel |
| Invalid or unclear | A short explanation; the working house continues | Correct the coordinates, use a control, or describe one clearer change |
| Changed | The same walk, with Undo and Redo | Keep the change or undo it |
| Saved | “Saved in Kiln” after the OPFS write | Reopen the scene or continue editing |
| Publish later | Local editing only | Public publishing is phase 4; no inactive publish control is shown |

Shader source editing is under **Advanced**. Ordinary scene edits do not invoke
Kiln's general coding loop, rewrite source, or compile a shader. Tint changes
update a uniform; object changes update geometry. New behaviors or shader programs
belong in an explicit authoring workflow, rather than the placement interaction.

## Run

From the repository root:

```sh
export CARGO_TARGET_DIR=~/work/openagents-target-1
. scripts/dev/cargo-build-env.sh
cargo build -p kiln --bin haunt-director
export KILN_OUT="$(openagents scratch)/haunt-kiln-site"
KILN_TRANSCRIPT=0 KILN_DEV=0 scripts/build-kiln.sh
# Load the existing server-only TypeSafe credential, or set TYPESAFE_API_KEY.
set -a; . ~/work/.secrets/typesafe.env; set +a
cd apps/haunt-preview
npm ci
npm run dev
```

Open the `127.0.0.1` URL Vite prints in current Chrome or Edge with WebGPU and
OffscreenCanvas. The Vite adapter starts `haunt-director` on a free loopback port
and stops it with the dev server. `HAUNT_DIRECTOR_BIN` can name another build.
Keep `KILN_OUT` set for `npm run dev` and `npm run build`.

The API key stays in server process environments; only the Rust SDK sends it. Do not put it in a `VITE_*`
variable. Direct controls and shader editing work without a decision credential;
text requests then report that the service is unavailable. Static `dist/` serves
the local controls; natural language requires this dev adapter or an authenticated
host integration. This change does not deploy a public decision endpoint.

## Decision path

- One Jev SDK call asks four independent Choice questions together: action,
  placement, size, and light color. Every question includes an unsupported option.
- Only the arguments used by the selected action contribute to its confidence
  floor (0.75 for this prototype). Uncertain requests make no change. This policy
  is exercised on a small retained example set, not broadly calibrated.
- Code reads exact X/Z coordinates and `scale=` from source spans. Jev never
  invents numerical values. Relative positions use the current player pose when
  the result is applied. Existing objects use the selected or explicitly named ID.
- A result applies only to the project revision and selection it was requested
  against. A newer manual edit cancels a pending decision. No delayed decision
  may overwrite it.
- SDK budget: 2.2 seconds, zero automatic retries. UI budget: 2.7 seconds. The
  renderer and direct controls keep working while the request is pending.
- The local adapter and native endpoint check the editor origin, bound request
  sizes, and expose fixed questions rather than arbitrary prompts, tools, or models.

This follows TypeSafe's [function-calling pattern](https://docs.typesafe.ai/cookbooks/function_calling):
code owns the functions, the decision chooses closed-set values, and software
handles exact arguments and execution.

## Project and command contract

- `haunt/scene.json` is a schema-1 project with a stable project ID, monotonically
  increasing revision, material asset path, tint enum, and up to 64 stable pumpkin
  IDs with X/Z/scale. `readProject` copies only saved fields and refuses unsupported
  versions, duplicate IDs, invalid assets, out-of-room coordinates, and invalid sizes.
- GPU handles, Effect values, player pose, model responses, and undo history are
  outside saved project data. Inspection returns copies.
- A transaction carries its ID, project ID, base revision, and an allowed operation.
  Add, move, resize, remove, tint, replace, undo, and redo share `SceneHistory`.
  Batches contain at most eight scene edits and validate atomically.
- The worker validates the targeted preview/token and command, prepares the change,
  and commits at a frame boundary. Failed commands preserve the project, history,
  player, shader, and device. IDs survive undo/redo; revision numbers never rewind.
- Undo/redo retain 32 recent changes; replay IDs retain 128 entries. Older replays
  also fail their base revision. The preview holds one in-flight scene change.
- Manual controls, Jev decisions, and real Kiln `write` edits share this command
  path. A whole-scene Kiln write must use the next revision. Accepted scenes save
  through the actual WASI `write` tool into OPFS. Workspace saves serialize with
  shader saves. Local scene reopen is supported; cross-tab collaboration is not.
- `hauntPreview.snapshot()`, `.command(operation)`, `.capture()`, and `.save(wgsl)`
  provide bounded inspection, transactions, PNG capture, and asset-draft validation.
  Capture runs in the worker with one encoding in flight. No evaluate-JavaScript
  command or account key is sent to that worker.
- A future session adapter must use existing Loro rooms on `coder-sync` and their
  grants. It carries project/transaction/base-revision identity, admits changes
  through the host, and forwards accepted data to this preview. Local undo is not
  a second collaboration authority, identity service, or storage backend.

## Check

```sh
npm run check
npm test
npm run build
HAUNT_URL=http://127.0.0.1:PORT/ \
  HAUNT_EVIDENCE="$(openagents scratch)/haunt-commands-check" \
  openagents browser run -- npm run verify:browser
```

The browser checks cover shader failures and recovery, 30 shader edits, manual
scene edits, 30 scene edits, real Jev decisions, interleaved Kiln/manual edits,
undo/redo, stale/canceled requests, timeout, OPFS reopen, origin checks, capture,
and teardown. `HAUNT_DECISIONS=0` skips only live inference; mocked delay checks
still run and are recorded separately. That option does not establish live acceptance.

See the [retained scene/decision results](../../docs/verse/verification/2026-10-22-haunted-house-commands/README.md).
Shader timing ends after first frame submission; screenshot timing is a conservative
observation bound. Neither is a GPU presentation timestamp. Resource counters cover
the demo's objects, not total driver allocations. The earlier Pylon source-patch
record remains historical evidence, rather than the current interaction.

Scene Undo currently covers objects and light color. Raw WGSL uses the Phase 0
draft/validation path and **Original**; integrating asset-draft undo into the
shared transaction history remains in #12502.

Device reconstruction (#12503), full game parity, shared projects, and public
publishing remain follow-ups. The scoped browser-language exception is in
[the agent contract](../../AGENTS.md).
