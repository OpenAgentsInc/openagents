# OpenAgents Desktop Full Auto: implementation and proof deep dive

- Date: 2026-07-16
- Class: current-status
- Status: code-landed and module-proven; not distributed, live-proven, or owner-accepted
- Source snapshot: `8d4bc1d3fa393c124743c4ef77d07903b48bc9d8`
- Implementation commit: `d480f779aa037516042b57ccd474a4d3fed6f04b`
- Issues: [#8852](https://github.com/OpenAgentsInc/openagents/issues/8852),
  [#8853](https://github.com/OpenAgentsInc/openagents/issues/8853) (both closed)
- Owner: OpenAgents Desktop Full Auto
- Dispatch: no; this is a point-in-time analysis, not a work queue
- Final disposition: retain-hardening-baseline; release/live/owner proof remains open

## Executive finding

Full Auto now has a real restart-survival mechanism in source. The renderer no
longer owns an in-memory continuation loop. Electron main persists a per-thread
enabled bit and continuation count, reopens them after a process restart, waits
for interrupted-turn recovery, and runs the same reconciliation function used
after a normal Full Auto turn completes. That is a material improvement over
the initial implementation in #8852.

The honest status is narrower than “Full Auto survives restarts” as a complete
product claim:

- **Landed:** yes. The #8853 implementation is an ancestor of current
  `origin/main`, and both implementation issues are closed.
- **Module-proven:** yes. Registry persistence, clean-restart reconciliation,
  in-flight exclusion, durable toggle-off, and the 20-continuation cap have
  automated coverage.
- **Electron-wiring-proven for Full Auto:** no. The test named
  `full-auto-restart.e2e.test.ts` opens the same JSON file twice in one test
  process. It does not launch, quit, and relaunch Electron.
- **Distributed:** no evidence. No Desktop release tag contains the
  implementation commit. The checked-in package is `0.1.0-rc.17`; the latest
  GitHub Desktop release at this snapshot is `0.1.0-rc.12`.
- **Live-proven and owner-accepted:** no. The adopted ProductSpec itself leaves
  “toggle, send, quit, relaunch, observe resume” as an owner follow-up.
- **Roadmap-complete autonomous next-turn work:** no. The implementation has a
  durable enabled registry, but not the WorkContext binding, idempotent
  outbox/lease, serialized dispatcher, or exactly-once proof required by
  [`MASTER_ROADMAP.md`](./MASTER_ROADMAP.md) invariant 24.

The feature should therefore be described as **restart-persistent intent with
module-level resume proof**, not yet release-grade, exactly-once unattended
execution.

## What the user gets

The current interaction is intentionally small:

1. The React composer renders one `Full Auto` toggle, off by default.
2. Turning it on for an existing thread immediately sends a typed IPC request
   to main, which persists `enabled: true` for that thread.
3. The user still sends the first message manually.
4. A Full Auto Codex turn receives an instruction prefix telling Codex to read
   the repository README, docs, and open issues, choose one concrete useful
   action, do it, and stop.
5. The turn uses `approvalPolicy: "never"`. The sandbox remains the Desktop
   Codex lane's existing `danger-full-access` posture.
6. After a successful flagged turn, main starts another flagged turn with the
   synthetic message “Continue Full Auto: look at this repository …”.
7. Turning the toggle off persists `enabled: false`. It does not cancel the
   current turn; it is intended to prevent the next continuation.
8. After 20 automatic continuation dispatches, the next reconciliation turns
   Full Auto off and appends a system note explaining the cap.

This is Codex-lane behavior only. It does not create a second permission model,
special review screen, commit authority, merge authority, push authority, or
public-claim authority. It does, however, run with the same high-trust local
Codex execution profile as an ordinary owner-authorized Desktop turn, while
removing mid-turn approval pauses. That makes the stop-control and work-context
truth gaps in this audit safety-relevant, not merely cosmetic.

## Architecture and control flow

```text
React composer
  -> DesktopFullAutoToggled
  -> preload-validated full-auto:set IPC
  -> main-owned full-auto/registry.json

manual or automatic Codex turn succeeds
  -> local turn journal becomes terminal
  -> fire-and-forget runFullAutoReconciliation()
  -> read enabled threads + nonterminal turn refs
  -> increment durable continuation count
  -> dispatchCodexLocalTurn(..., sender = null)
  -> completed thread refresh over existing recovery-update channel

app launch
  -> reopen registry and local-turn journal
  -> reconcile interrupted turns
  -> run the same runFullAutoReconciliation()
```

### Renderer responsibility

[`react-composer.tsx`](../../apps/openagents-desktop/src/renderer/react-composer.tsx)
renders the toggle and reports `DesktopFullAutoToggled`.
[`shell.ts`](../../apps/openagents-desktop/src/renderer/shell.ts) keeps a
renderer-local boolean, persists a toggle through `fullAutoHost.set`, and sends
`fullAuto: true` with the user's first turn. The old renderer `while` loop is
gone; one user submit produces one renderer send.

For a brand-new conversation, the user may turn Full Auto on before a thread
identifier exists. Once Send creates the thread, `shell.ts` persists the
enabled state against the new identifier before dispatching the turn.

### IPC boundary

[`codex-local-contract.ts`](../../apps/openagents-desktop/src/codex-local-contract.ts)
defines bounded set/get request schemas. The thread ref is non-empty and at
most 120 characters; set also carries a boolean. The preload validates before
invoking main, and main decodes again before mutating the registry.

The get channel is exposed all the way to the renderer host, but current
renderer code never calls `fullAutoHost.get`. This matters after restart and
on thread switches; see Finding 1.

### Durable registry

[`full-auto-registry.ts`](../../apps/openagents-desktop/src/full-auto-registry.ts)
stores this v1 record under Electron `userData/full-auto/registry.json`:

```text
{ threadRef, enabled, continuationCount, updatedAt }
```

Important storage behavior:

- the directory is created with mode `0700` and the JSON file with `0600` on
  non-Windows hosts;
- writes go to `registry.json.pending` and rename over the target;
- every write schema-validates records, sorts by `updatedAt`, and retains at
  most 128 records;
- disabling a record resets its counter to zero;
- an absent thread is disabled by default; and
- malformed existing JSON throws `FullAutoRegistryError("invalid_registry")`
  while main is initializing. There is no quarantine/recovery path.

The file is durable local state, but it is not a durable dispatch outbox. It
does not store a pending continuation identity, attempt, lease, workspace,
account, model, reasoning effort, or expected terminal predecessor.

### Reconciliation decision

[`full-auto-reconcile.ts`](../../apps/openagents-desktop/src/full-auto-reconcile.ts)
is shared by the live-completion and startup paths. Each call:

1. snapshots the set of threads with nonterminal local-turn journal rows;
2. iterates every enabled registry record;
3. skips a thread present in that one nonterminal snapshot;
4. increments its continuation count durably;
5. disables it and reports the cap when the count becomes 21; otherwise
6. awaits a caller-provided dispatch and reports successful thread refs.

This centralizes the decision, but it is not reentrancy-safe or exactly-once.
There is no lock, in-progress registry state, idempotency key, or lease around
steps 1–6.

### Main-process wiring

[`main.ts`](../../apps/openagents-desktop/src/main.ts) opens the registry beside
the local-turn journal. A renderer-initiated send and main-initiated
continuation both call `dispatchCodexLocalTurn`, so transcript persistence,
turn-journal recording, runtime execution, usage recording, and terminal
handling share one implementation.

The two reconcile triggers are:

- after any successful request carrying `fullAuto: true`; and
- once at startup after `localTurnRecovery` resolves.

Automatic turns are dispatched with a new random turn ref, only the thread ref,
the synthetic continuation message, and `fullAuto: true`. `sender` is `null`,
so live turn events are not sent to a renderer. After the entire turn finishes,
main broadcasts the updated thread through
`DesktopLocalTurnRecoveryUpdateChannel`, which the renderer already consumes.

### Runtime behavior

[`codex-local-runtime.ts`](../../apps/openagents-desktop/src/codex-local-runtime.ts)
prefixes the Full Auto instruction and changes approval policy from
`on-request` to `never`. Both the control-plane gate and app-server request see
the `danger-full-access` sandbox mode.

Continuation dispatch does not preserve the initiating turn's explicit target,
model, or reasoning effort. With those fields absent, it uses the Codex lane's
defaults and normal account discovery/rotation. Provider thread continuity is
reused only when the selected account matches the runtime's remembered
thread/account binding.

## Restart semantics, precisely

### Clean completion before quit

If the enabled bit reached disk and no local turn remains nonterminal, a new
app process reopens the registry and dispatches a continuation after local-turn
recovery settles. This is the path the new automated test proves at module
level.

### Quit during a provider turn

The Full Auto reconciler does not itself resume or classify the interrupted
provider turn. It waits for the existing local-turn recovery promise. During an
isolated reconcile call, a supplied nonterminal thread ref suppresses dispatch.
After recovery makes the journal terminal, startup reconciliation may begin the
next Full Auto turn.

### Toggle off during a turn

The renderer writes `enabled: false` immediately. The in-flight provider turn
continues. Its later success may call reconciliation, but that reconciliation
should see no enabled record and avoid the next turn. This behavior depends on
the user-visible toggle representing the same durable thread record, which is
not true after restart today.

### Cap across restarts

The continuation count lives in the registry, so reopening the file does not
reset the cap. Reconciliation increments before dispatch. Counts 1 through 20
are dispatch-eligible; count 21 disables the record and produces no dispatch.

## Status by proof rung

| Rung                                   | Status at snapshot | Evidence and limit                                                                                                                            |
| -------------------------------------- | ------------------ | --------------------------------------------------------------------------------------------------------------------------------------------- |
| Intent/spec                            | Present            | ProductSpec revision 2 and generated AssuranceSpec describe main-owned restart persistence.                                                   |
| Code landed                            | Green              | `d480f779aa` is on `origin/main`; #8852 and #8853 are closed.                                                                                 |
| Focused unit/integration tests         | Green              | UI toggle, request flag, prompt/approval behavior, persistence calls, registry reopen, in-flight exclusion, durable off, and cap are covered. |
| Current Desktop typecheck/tests        | Green              | Independent rerun described below: 164 files and 1,525 tests passed; 39 skipped.                                                              |
| General Electron build/smoke           | Green              | Production build, standard headless smoke, and React headless smoke passed on the same Desktop source. Neither smoke activates Full Auto.     |
| Full Auto two-process Electron restart | Missing            | No script or test launches Runtime A and Runtime B for Full Auto. The similarly named test uses two registry objects in one process.          |
| Packaged-app restart observation       | Missing            | ProductSpec calls this a recommended owner follow-up.                                                                                         |
| Distributed artifact                   | Not established    | No Desktop tag contains `d480f779aa`; latest published Desktop release was rc.12 at snapshot.                                                 |
| Live real-repository dogfood           | Not established    | No retained receipt shows toggle -> real turn -> quit -> relaunch -> resumed work against the same repo.                                      |
| Owner acceptance                       | Open               | ProductSpec retains an owner review gate. Closing #8853 is not the same evidence rung.                                                        |

## How it is tested

### Tests directly about Full Auto

- [`react-composer.test.tsx`](../../apps/openagents-desktop/src/renderer/react-composer.test.tsx)
  proves one off-by-default toggle, `aria-pressed`, and intent reporting.
- [`codex-local-runtime.test.ts`](../../apps/openagents-desktop/src/codex-local-runtime.test.ts)
  proves Full Auto uses `approvalPolicy: "never"` and the instruction prefix,
  while an ordinary turn remains `on-request` and unprefixed.
- [`shell.test.ts`](../../apps/openagents-desktop/src/renderer/shell.test.ts)
  proves the renderer sends one flagged turn and does not loop, persists toggle
  transitions for an existing thread, persists an enabled new thread after it
  receives an id, and sends no flag when off.
- [`full-auto-restart.e2e.test.ts`](../../apps/openagents-desktop/tests/full-auto-restart.e2e.test.ts)
  proves four plain-module cases: enabled file reopened and dispatched, an
  explicitly nonterminal thread skipped, an off record remains stopped, and a
  count at 20 disables on the next reconcile.

Despite its filename, the last test does **not** spawn Electron or two OS
processes. “Runtime A” and “Runtime B” are two calls to `openFullAutoRegistry`
against one temporary file within one test process. This is valuable durable
module proof, but it does not exercise:

- Electron boot and quit ordering;
- IPC set/get registration and delivery;
- renderer hydration after relaunch;
- real local-turn recovery followed by Full Auto reconciliation;
- a real Codex app-server turn;
- workspace restoration and binding;
- background result delivery to an open window; or
- packaged application behavior.

The analogy to local-turn restart proof is incomplete. Local-turn recovery has
both a plain-module `local-turn-restart.e2e.test.ts` and
[`local-turn-restart-smoke.ts`](../../apps/openagents-desktop/scripts/local-turn-restart-smoke.ts),
which actually spawns Electron twice against the same temporary user-data
directory. Full Auto has no equivalent two-process smoke.

### Independent verification during this audit

The audit reran the current Desktop source at `47c4b58855`; the only change
between that commit and the document snapshot `8d4bc1d3fa` is outside
`apps/openagents-desktop` and the Full Auto specs.

```text
apps/openagents-desktop/node_modules/.bin/tsc \
  -p apps/openagents-desktop/tsconfig.json --noEmit
  -> PASS

node_modules/.bin/vp test --run --max-concurrency 1 --root . \
  apps/openagents-desktop/tests/full-auto-restart.e2e.test.ts \
  apps/openagents-desktop/src/codex-local-runtime.test.ts \
  apps/openagents-desktop/src/renderer/shell.test.ts \
  apps/openagents-desktop/src/renderer/react-composer.test.tsx
  -> 4 files passed; 156 tests passed; 11 skipped

node_modules/.bin/vp test --run --max-concurrency 1 --root . \
  apps/openagents-desktop
  -> 164 files passed; 1,525 tests passed; 39 skipped

node --import tsx apps/openagents-desktop/scripts/build.ts
  -> PASS

OPENAGENTS_DESKTOP_SMOKE=1 electron apps/openagents-desktop
  -> standard smoke OK; lifecycle teardown OK

OPENAGENTS_DESKTOP_SMOKE=1 \
OPENAGENTS_DESKTOP_SMOKE_REACT=1 electron apps/openagents-desktop
  -> React smoke OK; lifecycle teardown OK
```

The earlier #8853 receipt recorded 1,521 passing tests, 39 skipped tests, and
one unrelated failure. That receipt was honest for its commit, but it is not
the current suite result: subsequent main changes repaired the failure, and
the independent rerun is fully green. Neither the earlier nor current general
smoke exercises Full Auto.

### Audit-only adversarial probe

An uncommitted Node probe called `reconcileFullAutoThreads` twice concurrently
against one enabled registry while holding both dispatch promises open. Before
either dispatch completed, the probe observed:

```json
{ "dispatchesBeforeRelease": 2, "continuationCount": 2 }
```

A second probe returned `{ ok: false }` from dispatch and observed:

```json
{ "failedDispatchEnabled": true, "failedDispatchCount": 1, "onDispatchFailedCalls": 0 }
```

These results follow directly from the current plain-module API and identify
missing test cases. They are not retained release tests and should not be
treated as a permanent receipt until converted into regression tests.

## Material findings

### 1. High — the visible stop control can disagree with durable execution

The renderer starts with `fullAuto: false`. Main exposes a get IPC, boot wraps
it in `fullAutoHost.get`, and comments say it seeds the toggle from durable
truth. No code calls that get method.

After a restart, main can resume an enabled thread while the composer displays
Full Auto as off and labels the next click “Turn on Full Auto.” Clicking once
therefore persists `true`, not `false`; a user trying to stop unexpected
background work must click twice. Thread switching has the same class of stale
state because the single renderer boolean is not hydrated from the selected
thread's registry record.

This exceeds the ProductSpec's deliberate cut of “no automatic resync on every
arbitrary thread switch.” A cut may permit coarse presentation, but the only
stop control cannot honestly display off while main continues high-trust work.

### 2. High — restart intent is not bound to a repository or WorkContext

The registry stores only `threadRef`, `enabled`, count, and timestamp. A
Desktop thread may contain a `cwd`, but Full Auto reconciliation does not read
or validate it. `codexLocal` obtains its workspace from the currently selected
global Desktop workspace, falling back to the app launch directory.

Consequently, a thread enabled in repository A can later resume against
repository B if the selected workspace changes before reconciliation or
restart. Startup scans every enabled registry row, so old enabled threads are
not fenced to the repository in which the owner granted Full Auto.

Durability without authority binding is unsafe. The next-turn record must bind
at least thread, admitted WorkContext/repository identity, expected predecessor,
and execution profile, then fail closed if that context cannot be restored
exactly.

### 3. High — reconciliation is not serialized or idempotent

`runFullAutoReconciliation()` is fire-and-forget after each successful Full
Auto turn and is also invoked from startup. The decision function snapshots
nonterminal refs once, increments state, and dispatches without a per-thread
lock or durable pending identity. Two overlapping calls can both see the same
thread as idle and both dispatch it.

The local-turn journal does not prevent this. Each automatic dispatch creates
a fresh random turn ref, and journal acceptance rejects duplicate keys, not a
second nonterminal turn with a different key on the same thread. The Codex
runtime's `activeTurnByThread` map also overwrites the active entry when a
second turn starts, weakening interrupt/steer ownership.

This is the exact gap between a persistent flag and the master roadmap's
required idempotent outbox/lease. The #8853 issue plan's phrase
“idempotency-guarded against the turn journal” is not supported by current
code.

### 4. High — background work is not represented as in flight in the UI

Main-originated turns use `sender: null`; their live events are not delivered
to the renderer. The renderer remains non-pending until the final thread
refresh. During that interval the user cannot see streaming progress or use
the normal turn control against the actual background turn, and the composer
can submit another manual turn on the same thread.

The ProductSpec explicitly cuts live token streaming, so missing deltas alone
are not a spec violation. The concurrency and stop-control consequences are
still a release blocker: a coarse typed “Full Auto turn running” projection,
with a working interrupt/stop action and manual-send exclusion or queueing,
does not require token streaming.

### 5. Medium — a normal dispatch failure silently stalls an enabled loop

The registry count increments before dispatch. `reconcileFullAutoThreads`
only calls `onDispatchFailed` when dispatch throws. Main's adapter normally
returns `{ ok: false }` for runtime, account, policy, workspace, or transcript
failures. That result is not added to the success list, but it also does not
invoke the failure callback, append a visible note, retry, disable, or schedule
another reconcile.

The record remains enabled, the counter is consumed, and the loop is dormant
until some unrelated future trigger or app restart. “Enabled” therefore does
not mean progressing, stopped, or visibly failed.

### 6. Medium — automatic continuations do not preserve the initiating profile

The synthetic request omits target account, selected model, reasoning effort,
images, explicit context attachments, and extension selection. Defaults and
normal account rotation apply. This may be an acceptable product decision, but
it is not stated as such and is not surfaced to the user. It also makes a
restart continuation less deterministic than the initiating turn.

The durable record should either bind the intended profile or state and test
which fields deliberately reset.

### 7. Medium — the cap API and implementation semantics have drifted

The registry documents `continuationCount` as consecutive automatic turns
“since the last manual send or reset” and exposes `resetContinuation`. No
production caller invokes that reset. Toggling off resets the counter; a manual
send while the toggle remains on does not.

The ProductSpec says the cap resets after an “intervening manual stop,” so the
current acceptance wording can be read as satisfied. The source contract is
still misleading and the dead reset method indicates an unresolved semantic
choice that needs an explicit test.

### 8. Medium — documentation and claims disagree with source

The adopted ProductSpec revision 2 correctly describes main ownership, but
several other committed authorities or near-authorities remain stale:

- [`GUARANTEES.md`](../../apps/openagents-desktop/GUARANTEES.md) says the loop
  is renderer-owned, switching threads stops it, and it does not survive a
  restart.
- The historical
  [`docs/fable` design](../fable/2026-07-15-full-auto-repo-intent-to-dispatch-loop.product-spec.md)
  still says the shipped loop is renderer-owned and restart-fragile.
- `codex-local-runtime.ts` comments still say repetition comes from the
  renderer resubmitting.
- ProductSpec/boot comments say get IPC reflects durable truth on boot, but no
  renderer code calls get.
- #8853 is closed against an acceptance bar that explicitly asked to quit and
  relaunch the app, while the closeout says the actual packaged-app restart
  observation was not performed.

These contradictions make it easy for future work to preserve the wrong
behavior or promote the wrong proof rung.

### 9. Medium — success metrics are declared but not instrumented

The ProductSpec names adoption, observed continuation, and restart-survival
metrics with consented public-safe local counters as their source. The metric
identifiers appear only in the ProductSpec and generated AssuranceSpec; there
is no implementation that records or reports them. The first-week success
targets are therefore currently unmeasurable.

### 10. Low — invalid registry state can block application initialization

Opening the registry synchronously decodes the entire file. Invalid JSON or a
schema mismatch throws before Full Auto IPC and most main startup work. There
is no bounded quarantine, migration, disable-all fallback, or owner-visible
diagnostic. A non-critical automation preference should fail closed without
making the Desktop shell unavailable.

## What the current tests do not falsify

The green suite can coexist with all of the material findings because it does
not currently test:

- renderer hydration from `fullAuto:get` at initial mount or thread switch;
- one truthful click stopping a loop after restart;
- workspace A remaining bound after workspace B becomes selected;
- overlapping reconcile invocations;
- two nonterminal random turn refs on one thread;
- `{ ok: false }` dispatch behavior;
- background pending/interrupt/manual-send exclusion;
- configuration continuity across automatic turns;
- corrupt registry recovery;
- a native question arriving on a background (`sender: null`) turn;
- eviction of a still-enabled record once the registry's bounded record limit
  is reached;
- a two-process Electron Full Auto restart; or
- a packaged real-Codex restart on a real repository.

The generated AssuranceSpec mirrors the ProductSpec's criterion claims. It is
not an independent runtime oracle for the missing cases above.

## Recommended hardening order

1. **Bind authority and make dispatch exactly-once.** Replace the enabled-only
   decision with a durable per-thread next-turn record containing WorkContext,
   expected predecessor, execution profile, attempt/idempotency identity, and
   lease state. Serialize reconciliation per thread. Recovery must replay or
   close the same intent, never mint a second random intent because a scan ran
   twice.
2. **Make the stop control truthful.** Hydrate from main before presenting a
   thread as off, resync on selected-thread changes, and project main-owned
   running/stopping/failed/capped state. One click labeled Stop must durably
   stop the next turn.
3. **Fence background and manual turns.** Expose a coarse main-owned in-flight
   state and working interrupt even if token streaming remains cut. Queue or
   reject a manual send while a Full Auto turn owns the thread.
4. **Define failure policy.** Treat both thrown errors and `{ ok: false }` as
   typed outcomes. Persist a visible stopped/blocked/retry state with bounded
   backoff; do not leave `enabled: true` dormant and unexplained.
5. **Freeze profile continuity.** Decide and test whether account, model,
   reasoning effort, extensions, and attachments persist or reset. Bind every
   field that affects authority or reproducibility.
6. **Add the missing proof ladder.** Add a Full Auto counterpart to
   `local-turn-restart-smoke.ts` that launches Electron twice against one
   temporary user-data directory, then run a packaged-app real-repository
   owner smoke with a retained public-safe receipt.
7. **Reconcile contracts and metrics.** Update `GUARANTEES.md`, stale source
   comments, and the historical design disposition; add real consented metric
   instrumentation or remove unmeasurable success claims.

The first three items should land before calling Full Auto release-ready. The
two-process smoke should be a merge gate for any renewed “survives restart”
claim; the packaged real-repository observation is the gate for live and owner
acceptance.

## Bottom line

#8853 fixed the original architectural mistake: a renderer loop cannot own
restart-survivable autonomous intent. The new registry and shared reconcile
function are a useful foundation, and current source is type-clean, test-green,
and generally smoke-green.

What exists today is still a foundation. The durable record is not bound to
the repository it can modify, reconciliation is not exactly-once, the visible
toggle can lie after restart, background work is not presented as in flight,
and the only Full Auto restart test never restarts Electron. No artifact
containing the change has been identified as distributed, and no live owner
restart observation is retained.

Final status: **landed and module-proven; hardening and real restart proof
required before release/live/owner-accepted claims.**

## Addendum — fable, 2026-07-16

Two additional gaps, traced directly in
[`codex-local-runtime.ts`](../../apps/openagents-desktop/src/codex-local-runtime.ts)
and [`full-auto-registry.ts`](../../apps/openagents-desktop/src/full-auto-registry.ts),
plus one concrete interim mitigation for Finding 2 that does not require the
full hardening order to land first.

### 11. High — a native question during a background turn can hang that thread's loop forever, invisibly

`item/tool/requestUserInput` (`codex-local-runtime.ts:548`) is a distinct
app-server request from `item/commandExecution/requestApproval` and
`item/fileChange/requestApproval`. `approvalPolicy: "never"` only reaches the
latter two; it does not touch native question requests at all. A question is
resolved only by an explicit `answerQuestion` call (driven by renderer UI) or
by `interrupt(turnRef)`, which auto-denies every pending question for that
turn (`codex-local-runtime.ts:1290-1299`).

A main-initiated Full Auto continuation dispatches with `sender: null`
(`main.ts`'s `runFullAutoReconciliation` wiring) and its turn ref never
reaches any renderer. Nothing holds a reference capable of calling
`interrupt` on it. If the model emits `item/tool/requestUserInput` during
that turn — the Full Auto instruction asks it not to, but does not and
cannot enforce this at the protocol layer — the turn's JSON-RPC round trip
blocks indefinitely: `pendingQuestions` holds the resolver, nobody ever
calls `accept`/`deny`, and the turn never reaches a terminal local-turn
journal state.

The consequence compounds silently: that thread reads as permanently
nonterminal, so every later `reconcileFullAutoThreads` call skips it as "in
flight" — no cap increment, no disabled note, no visible error, just a
quietly frozen Full Auto loop for that thread. Only a full app restart
unsticks it, because `local-turn-recovery.ts`'s existing restart reconciliation
treats any nonterminal turn as interrupted regardless of why it never
finished.

Minimal mitigation, scoped smaller than the full next-turn/lease redesign in
Recommendation 1: when a turn has `fullAuto: true` and `sender === null`,
answer `item/tool/requestUserInput` automatically with an empty/synthetic
response (or deny) at the point it is received, the same way `interrupt`
already auto-denies — do not leave it pending for a listener that structurally
cannot exist.

### 12. Medium — the registry's bounded record limit can silently evict a still-enabled thread during ordinary operation

`openFullAutoRegistry`'s `persist()` (`full-auto-registry.ts`) sorts all
records by `updatedAt` and slices to `FULL_AUTO_RECORD_LIMIT` (128) on every
write, with no distinction between enabled and disabled records. A thread
enabled once and left alone — no further sends, no toggle — ages toward the
tail of that sort. Once 128 other records (enabled or not) have been touched
more recently, the next write silently drops it. It will not resume on
restart, and nothing records that it was dropped.

This is distinct from Finding 10 (a corrupt file blocking initialization
entirely): this is quiet data loss during normal, healthy operation, and it
scales with how many distinct threads/repos an owner has ever toggled Full
Auto on, not with anything going wrong. Minimal mitigation: exclude
`enabled: true` records from the eviction slice (bound only the disabled
tail), or raise the limit and emit an owner-visible warning before it binds.

### A concrete interim mitigation for Finding 2

The full fix — a durable next-turn record binding thread, admitted
WorkContext/repository identity, expected predecessor, and execution profile
— is real work and correctly sequenced first in the hardening order. Until
it lands, a small additive change closes the most dangerous instance of the
gap (resuming against the wrong repository) without redesigning dispatch:
record the granted workspace `cwd` (or a digest of it) in each
`FullAutoRecord` at the moment `CodexLocalFullAutoSetChannel` sets
`enabled: true`, and have the dispatch wiring refuse to start a continuation
— disabling the record rather than silently redirecting it — if the
currently resolved Desktop workspace does not match what was granted. This
does not make dispatch exactly-once or serialized; it only stops a stale
enabled thread from ever executing against a repository the owner did not
grant it against.

## Addendum 2 — fable, 2026-07-16: agent-triggerable Full Auto and remaining full-feature scope

The document above is thorough on restart mechanics but says nothing about
who or what is allowed to start, stop, and inspect Full Auto in the first
place. Today the answer is: only a human, only by clicking the composer
toggle inside the running Desktop app. That is too narrow a control surface
for the product this is becoming. The owner should be able to direct an
agent — in a chat session, this one included — to start Full Auto against a
granted repository, watch it work, and stop it, with the actual Desktop UI
showing what is happening while it happens. None of that exists yet.

### 13. High — no programmatic control surface exists

Every path into `full-auto-registry.ts` and `full-auto-reconcile.ts` today
runs through `CodexLocalFullAutoSetChannel`/`CodexLocalFullAutoGetChannel`,
which are `contextBridge`-exposed to one specific renderer window over
Electron IPC (`preload.cts`). There is no HTTP endpoint, no CLI, no MCP
server, and no other process-external entry point. An agent running in a
terminal — even one running on the exact same machine as Desktop, which is
the ordinary case for "the owner directs me in this chat" — has no way to
enable Full Auto, check its status, or force an immediate continuation. The
only thing an agent can do today is ask the owner to click the toggle
themselves.

### Recommended architecture: one OpenAPI spec, not three hand-written surfaces

The right shape for this is not "pick MCP or CLI or API" as three
competing options — it is one typed description generating all three, so
they cannot drift out of sync with each other or with the implementation.
This is standard, well-established guidance among people who build MCP
servers for a living (a recent public framing of it, credited to Rhys, put
it plainly: the best way for a company doing this is an OpenAPI spec that
automatically gets converted into an SDK, an MCP server, and a CLI — write
the operations once, generate the rest). It is also **already this repo's
own convention**, not a new idea being imported: the public
`openagents.com` API already does exactly this triad —
[`openagents-openapi-routes.ts`](../../apps/openagents.com/workers/api/src/openagents-openapi-routes.ts)
serves the OpenAPI document, [`omni-api-sdk-seed.ts`](../../apps/openagents.com/workers/api/src/omni-api-sdk-seed.ts)
describes the generated-SDK surface, and
[`public-agent-mcp-discovery.ts`](../../apps/openagents.com/workers/api/src/public-agent-mcp-discovery.ts)
publishes the matching public MCP manifest. Full Auto's control surface
should follow that exact convention rather than inventing a fourth pattern.

Recommended in two phases, because the honest cheap thing to build now is
not the same as the eventual right thing:

**Phase 1 — local, same-machine, the actually-easy version for now.** Desktop
main hosts a loopback-only (`127.0.0.1`), opt-in-only HTTP server describing
a small OpenAPI surface: `GET /v1/full-auto` (list), `GET
/v1/full-auto/{threadRef}` (status), `POST .../enable`, `POST .../disable`,
`POST .../continue-now` (force an immediate reconciliation attempt instead of
waiting for the next natural trigger), and `GET .../turns` (bounded recent
turn/outcome history). Gate it behind an opt-in flag and a scoped, locally
minted bearer credential, exactly the way the existing Harness MCP pilot
already does for Pylon (`apps/pylon/src/harness-mcp-server.ts`, flag
`OPENAGENTS_PYLON_CODEX_HARNESS_MCP_PILOT`, scopes minted via
`@openagentsinc/environment-auth`) — do not invent a new auth pattern for
this. A thin MCP server and a thin CLI are generated (or, if generation
tooling for this pair isn't wired up yet, hand-written as a deliberately
thin pass-through) from that one spec. This is what lets an agent on the
owner's own machine — this session, right now, is exactly that case — call
`full_auto.enable`/`full_auto.status`/`full_auto.continue_now` directly
during a chat.

**Phase 2 — cross-machine, later.** Extend the already-live
`openagents.com` OpenAPI/Omni-SDK/public-MCP triad with Full Auto routes
that relay through Khala Sync to a specific owner's running Desktop
instance, mirroring the existing Khala → Pylon → Codex delegation runbook's
cross-machine dispatch pattern (root workspace `CLAUDE.md`, `docs/khala/`).
That is what lets an agent running anywhere — not only one on the owner's
own machine — direct the owner's Desktop app. It is real, separate work
(a command channel into an already-running Desktop process that today only
listens on local IPC) and should stay explicitly out of scope until Phase 1
and the hardening items above it are done.

### This surface must not widen the authority gaps already found above

An externally-triggerable enable endpoint is a materially larger attack
surface than a UI button a human has to physically click, so the mitigations
already described in this document stop being optional the moment this
control surface exists:

- An `enable` call must name the repository/workspace it expects; the
  server must refuse (not silently redirect) if that does not match the
  thread's actual granted workspace — this is the Finding 2 interim
  mitigation above, promoted from "worth doing" to "required before this
  ships."
- Programmatic `enable` must never itself grant a new, previously-ungranted
  repository — granting stays a human/UI action; the API can only turn Full
  Auto on for a thread whose workspace is already granted.
- Every programmatic call (enable, disable, continue-now) must produce a
  durable, distinctly-attributed receipt so the owner can later tell, per
  thread, whether a given run was started by their own click or by an agent
  acting on their instruction, and by which agent/session.
- `continue-now` must go through the exact same `reconcileFullAutoThreads`
  path as every other trigger (Finding 3's non-idempotency applies to it
  identically) — it is a new trigger point, not a new dispatch mechanism.

### This capability makes Finding 4 a hard blocking dependency, not a nice-to-have

The owner's stated requirement — start it via an agent, then watch it happen
in the actual Desktop UI — cannot be satisfied while Finding 4 stands as
written. If the UI shows nothing until a background turn fully completes,
an owner who just asked an agent to kick off Full Auto and switches to the
Desktop window sees no confirmation that anything is happening at all. At
minimum, a coarse per-thread state (idle / turn running / turn completed /
turn failed / cap reached) must render in the composer and thread list
before this feature is usable the way it was asked for. This does not
require the token-streaming Finding 4 already deliberately leaves cut — it
requires only that "something is running right now" become a real,
type-checked, rendered fact instead of silence until the end.

### Other under-specified dimensions of the full feature

Named here so they are not lost, not designed in depth:

- **Cost/token visibility.** Nothing today surfaces how much an unattended
  Full Auto run has spent. An owner (or an agent managing it on their
  behalf) directing potentially dozens of unattended continuations has no
  running total to watch or cap against, separate from the 20-turn count
  cap.
- **Pause vs. stop.** Today there is only enabled/disabled plus the hard
  cap. There is no "pause without losing the continuation count" or
  "pause for N minutes" concept — every stop is the same as never having
  started that run again from the counter's perspective.
- **Concurrent/multi-repo Full Auto.** Explicitly cut in the ProductSpec,
  but "start Full Auto on repo A and repo B, let me watch both" is an
  obviously natural agent-driven request once programmatic control exists.
  Worth tracking as a likely near-term ask even though it stays out of
  scope today.
- **Outcome-quality evaluation.** Everything in this document and its
  ProductSpec is about mechanism safety — does it survive a restart, does
  it stop when told to, does it stay in the granted repo. Nothing evaluates
  whether the "one concrete useful thing" Codex actually picked, turn after
  turn, was good work. That is a distinct, unaddressed product question.
- **Attention/notification model.** Full Auto runs while the owner is not
  looking, by design. Beyond an in-app system note on a thread the owner
  may not have open, there is no OS-level notification or digest for
  "something worth your attention happened while you were away."

### Addition to the recommended hardening order

Item 8 (fable addition, ordered after the original items 1-4): **build the
Phase 1 local control surface only after dispatch is exactly-once and the
stop control is truthful.** Exposing `enable`/`continue-now` to programmatic
callers before those land does not just carry over the existing correctness
and safety gaps from Findings 2, 3, and 4 — it multiplies them, because a
script or agent can now trigger the double-dispatch and stale-workspace
failure modes far faster and more repeatably than a human clicking a toggle
ever would.
