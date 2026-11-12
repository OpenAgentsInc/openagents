# Replace an Everglade gameplay body during development

`fast-edit.coax` replaces `door_ease(t: f32) -> f32` in a running local
native room and its Wasm page. The procedure controls door rotation and
collision capsules. This explicit development experiment addresses #12960;
general procedure replacement and state migration remain roadmap R26–R27.

The native room retains its server, world and seats. The page retains its
engine instance, state and GPU resources. A replacement owns no persistent
data and receives no host pointers. The native host publishes between complete
Coax calls; the page publishes between animation frames. An active Wasm room
worker refuses replacement.

## Prepare a session

Run these commands from an isolated repository worktree. Use the qualified
`coax/bin/coax`. Resolve the Wasm toolchain before starting the session.
On coderos-4080, run builds at nice 15 and keep each command under 15 minutes.

```sh
session="$PWD/apps/openagents-web/build/fast-edit"
coax/web/tool.sh fast-edit prepare "$session"
COAX_FAST_EDIT="$session" coax/web/room/build.sh "$session/room"
COAX_FAST_EDIT="$session" COAXIAL_MODE=dev \
  coax/web/wasm.sh apps/openagents-web/everglade5e.coax \
  apps/openagents-web/everglade5e-face.c \
  apps/openagents-web/build/everglade5e.wasm \
  everglade/shared/everglade-step.coax verse/grid.coax \
  verse/everglade/river.coax verse/everglade/guard-guide-policy.coax \
  apps/openagents-web/everglade5e-rules.coax
```

Preparation seals the compiler, toolchain paths and versions, checking
profile, complete Coax load graphs, browser adapters, native faces and catalog
inputs. Every byte outside the selected body must remain unchanged. Caches
hold ordinary checked C; instrumentation changes only private generated copies.
Object keys include their resulting bytes and the development identity.

The prepared private source tree supplies both initial builds and subsequent
background checks. An active session cannot be resealed. Start a new session
when its context changes.

## Measure an edit

```sh
coax/web/tool.sh fast-edit-bench "$session" --port 9060 --keep-edit
```

The driver starts its own loopback room, page server and muted headless Chrome.
Before measuring, it walks the connected seat from the gate to the inn using
the ordinary gamepad interface. The native room then evaluates nearby door
colliders on ordinary movement steps.
It makes ten distinct edits to the real gameplay source, compiles each closed
body for native and Wasm in parallel, and observes both running hosts. The
clock starts before the file write. Each observation requires the new revision,
the expected rule value, ordinary gameplay calls after publication and a later
page frame. Engine identity, state address and seat must remain unchanged.

`measurement-N.json` records each sample, including admission, Coax compilation,
native compilation, Wasm compilation and publication times. `benchmark.json`
records all ten measurements and their median. Without `--keep-edit`, the
driver restores the source after stopping its hosts.

An instrumented loopback page polls `/build/fast-edit/latest.json`. Set the
`fast-edit` query parameter to use another same-origin endpoint. Ordinary
builds have no replacement exports and do not poll it.

## Apply an edit and verify it

Keep the fixed `k := clamp(t, 0, 1);` preamble and change its return expression.
The accepted expression has scalar arithmetic and the existing scalar
intrinsics only. The candidate copies the exact original `lo`, `hi` and
`clamp` helpers, then passes ordinary Coax checking before either C compiler
runs.

```sh
coax/web/tool.sh fast-edit apply "$session"
coax/web/tool.sh fast-edit gate "$session"
coax/web/tool.sh fast-edit commit "$session" -m "Describe the gameplay change"
```

`apply` publishes first and starts full room/page checks in a separate process.
The existing compiler cache reuses unchanged passing verdicts. The verifier
coalesces newer edits while checking an immutable candidate snapshot. Each
root has a 780-second deadline. A missing, stale, failed or timed-out proof
blocks the guarded commit command. The working tree must also match the index;
a verified working copy cannot authorize different staged bytes.

The page displays failed background checks in its session. For a long checker,
split the negative test into two bounded commands:

```sh
coax/web/tool.sh fast-edit-bench "$session" --stage-bad-check
# After verification.json reports failed:
coax/web/tool.sh fast-edit-bench "$session" --observe-bad-check
```

Staging installs a body that fails the existing door check and leaves that
source in place. Observation requires the completed failure for the exact
same source, checks the visible session message, attempts a guarded dry-run
commit and verifies that HEAD stays unchanged. It does not rerun the checker.
Inspect `bad-check.json`, `verification.json` and the per-revision check logs.

Unsupported expressions, changed signatures, layouts, helpers or other inputs
return status 2 and retain the old running body. Use the ordinary full build
and host restart path for these edits. The tool does not signal a host it does
not own. Release builds use their existing checks and link path.

## Compare with a full build

After the latest candidate passes `gate`, its private source tree holds that
exact candidate. Set `COAX_FAST_EDIT_ORACLE=1` alongside `COAX_FAST_EDIT` to
compile and link that whole tree without the dispatch boundary. Disable the
Wasm artifact cache with `COAXIAL_WASM_CACHE=''` for a clean artifact build.
This mode supports the ordinary release optimization profile for budget checks.
Run the room suite and `everglade-drift --offline native` against the replaced
and full-build artifacts. The default instrumented endpoint lets the ordinary
drift pages consume the same revision as the native room.

Stop the session's background workers when finished:

```sh
coax/web/tool.sh fast-edit stop "$session"
```

The filesystem lock is process-owned and nonblocking. A dead verifier cannot
retain it. A refusal preserves the old callable; the next valid revision can
publish. Persistent-state or shader migration is outside this scalar profile.
