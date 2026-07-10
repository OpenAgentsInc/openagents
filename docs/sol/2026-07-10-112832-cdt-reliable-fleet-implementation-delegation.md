# Reliable fleet software implementation delegation

- Created: 2026-07-10 11:28:32 CDT (`2026-07-10T16:28:32Z`)
- Repository snapshot: `19ebe9741f23cfd58e4a1be47c6c08476f742709`
- Audience: bounded, high-throughput coding agent with limited architectural
  judgment
- Authority: [`MASTER_ROADMAP.md`](./MASTER_ROADMAP.md), Revision 24
- Coordination: [`CLAIM_PROTOCOL.md`](./CLAIM_PROTOCOL.md)
- Status: executable delegation packet; not a replacement for the roadmap,
  live issue state, or an active claim

## Mission

Rapidly close the active OpenAgents reliability program in dependency order:

1. make Desktop and mobile truthfully green;
2. establish one persona-neutral authenticated identity/session contract;
3. connect both clients to the same Khala Sync conversation and Fleet state;
4. expose real fleet start, inspection, steering, approval, pause/resume/stop,
   outcomes, and receipts on both clients;
5. prove offline/reconnect/restart and duplicate-suppression behavior;
6. finish the practical OpenCode-parity Desktop workbench and purposeful mobile
   supervision client;
7. package, release, dogfood, and close with exact receipts.

The agent implements small, already-decided leaves. It does **not** invent a
new architecture, relax authority, infer completion from UI text, or revive
paused Sarah/persona/A/V/presentation scope.

## Read this before touching code

At this timestamp, GitHub reports **15** open `roadmap:sol` issues. Revision 24
still records 16 because #8652 closed after that snapshot. #8652 is closed;
do not reopen or continue portal scope. Live issue labels and bodies still use
older Sarah-first/P1 wording. Revision 24 controls priority and product shape
until those records are reconciled.

Several lanes are concurrently owned:

- #8597 has an active claim removing the mobile Sarah/persona/demo path and
  making the native composer drive the persona-neutral Khala surface. Do not
  touch its listed mobile files until `CLAIM-RELEASE` lands or the claim is
  properly audited under the 90-minute-plus-process rule.
- #8640 has coordinator-owned type-boundary/scanner/acceptance work and an
  owner-gated Codex account reconnect. Do not take it, modify its hot files, or
  attempt credential work.
- #8574 has a continuing Desktop execution-lane claim. Its icon leaf landed at
  `19ebe9741f`; the broader claim must be clarified before another agent edits
  Desktop or shared Effect Native renderer paths.

This is a timestamped warning, not permanent state. Refresh issue comments and
`origin/main` every time a leaf is selected.

## Non-negotiable operating rules

1. Work on `main`. Start from clean, current `origin/main`.
2. Never stash, reset, restore, checkout away, or overwrite another agent's
   changes. If the checkout is dirty with unrelated work, stop and ask the
   coordinator for a clean worktree.
3. Claim one leaf, not an epic. Name exact paths, hot files, hot contracts, and
   verification before editing.
4. Stage only the claimed files. Make one coherent commit, rebase safely on
   current `origin/main`, push `main`, then post `CLAIM-RELEASE`.
5. Do not change schemas, migrations, generated clients, route tables,
   authority policy, public promise versions, package keys, lockfiles, shared
   catalogs, or credential handling unless the claim names one integration
   owner and an already-approved exact design.
6. Never add ad hoc keyword matching. Use the central typed intent/command
   registry and typed semantic selection.
7. Never make a renderer authoritative. Desktop/mobile request and present;
   Khala Sync distributes state; server/Pylon authorities decide claims,
   attempts, approvals, commands, and terminal outcomes.
8. Never display a local draft, optimistic mutation, transport timeout, staged
   Pylon brief, transcript sentence, or fixture as an accepted FleetRun/action.
9. Never print, copy, commit, fixture, or comment credentials, private paths,
   raw prompts, raw shell output, or private repository contents.
10. Do not use `--no-verify`, weaken tests, add casts around a contract error,
    broaden IPC/process/filesystem authority, or hide a failing state.
11. Do not deploy, upload builds, change live issue state, or mutate external
    systems unless the dispatch explicitly authorizes that external action.
12. If the same approach fails twice, or 15 minutes of inspection does not
    reveal a bounded causal change, post a blocker receipt and pull the next
    ready leaf. Do not wander.

## Required preflight for every leaf

```bash
git status --short --branch
git fetch origin main
git pull --ff-only origin main
gh issue view <ISSUE> --repo OpenAgentsInc/openagents --comments
git log -5 --oneline --decorate
```

Continue only when all are true:

- the checkout is clean and on `main`;
- the issue is still open and active under Revision 24;
- no live claim overlaps files or contracts;
- the leaf has a falsifiable acceptance test;
- required upstream contract decisions already exist.

Post the exact `CLAIM` template from `CLAIM_PROTOCOL.md` before mutation. A
general “working on desktop/mobile” comment is not enough.

## Live issue disposition and what this agent may do

| Issue | Revision 24 disposition | Delegation rule |
| --- | --- | --- |
| #8566 | P0 parent | Do not claim the epic. Reconcile its stale body/labels when authorized; close only after R0–R7 and child exits reconcile. |
| #8574 | P0 Desktop | Primary implementation lane. Take one D0–D6 leaf only after the active claim releases or explicitly hands off disjoint paths. |
| #8597 | P0 mobile | Primary implementation lane. Current claim is active; wait for release, then take one R0/R1/R2/R6 leaf. |
| #8638 | P0 Fleet substrate parent | Do not rebuild Fleet or claim the epic. Integrate existing contracts; close only after #8640, R3/R7, and accepted follow-on disposition. |
| #8640 | P0 live proof | Coordinator/owner-gated. Do not take credentials, scanner, type-gate, deploy, or live burn work. Consume its receipts after release. |
| #8547 | P1 follow-on | Do not start before local R7 unless the owner reprioritizes. Firecracker/grant/custody work requires a senior integration owner. |
| #8636 | P1 follow-on | Do not start before local R7. Never design target fallback or grant authority without an approved contract. |
| #8634 | maintenance/deferred | Only production/security/API/receipt work required by R0–R7. No broad route conversion. |
| #8635 | maintenance/deferred | Only production integrity or R0–R7 dependency repairs. No Forum expansion. |
| #8595 | paused | No implementation. Do not close merely because it is paused. |
| #8610 | paused | No Sarah/avatar/opener/voice/video/presentation work. |
| #8642 | privacy tripwire | Start only for a real correction/deletion/privacy incident or explicit post-R7 activation. Requires an authority/schema owner. |
| #8643 | paused | No roles, named colleagues, or relationship-mode expansion. |
| #8646 | paused | No glass/Sarah-in-app presentation work. Preserve shipped compatibility only. |
| #8650 | paused except blocker | Touch only when an exact lowering defect blocks accessibility, correctness, platform support, or R0–R7. |

Paused issues are not an invitation to manufacture closure commits. They stay
untouched until an owner disposition says implement, supersede, or close.

## Dependency queue

| Order | Gate | Main outcome | Parallel work allowed |
| --- | --- | --- | --- |
| 0 | Claim and truth | Current issue bodies/labels/claims; clean baselines | Documentation and test-only inventory |
| 1 | R0 | Both clients green; honest local/fixture/unconfigured states | Bounded Desktop workbench leaves with disjoint claims |
| 2 | R1–R2 | Shared identity/session and Khala Sync continuity | Pure codecs, client adapters, read-only views, fault fixtures |
| 3 | R3 | Real Fleet start/control/outcome on both clients | #8640 live proof when owner/coordinator is ready |
| 4 | R4 | Offline/restart/replay/refetch convergence | Desktop D2/D3 and mobile read-only supervision |
| 5 | R5–R6 | Complete Desktop workbench and mobile supervision | Packaging prep and diagnostics |
| 6 | R7 | Signed/installable releases and sustained dogfood receipt | None that changes the acceptance baseline |
| 7 | Follow-ons | #8547/#8636, then explicitly reactivated work | Only after R7 or owner reprioritization |

Never skip a gate because later UI work looks easier. A later leaf may run in
parallel only if it does not consume an unfrozen earlier contract and its files
and semantics are disjoint.

## First fast leaves

These are deliberately small. Recheck claims before using them.

### FAST-1 — clear the Desktop `Compose` typecheck drift

Issue: #8574, D0/R0.

Current reproducible failure:

```text
apps/openagents.com/packages/effect-native-render-dom/src/index.ts:1949
"Compose" is not accepted by @openagentsinc/ui/icon
```

The Effect Native semantic catalog includes `Compose`, while the OpenAI Apps
SDK icon source exposes the supported `ChatCompose` name. After the #8574
claim explicitly releases or hands off this shared hot contract:

1. add an exact renderer-owned alias from semantic `Compose` to supported
   asset `ChatCompose` before calling `openAiIconSvg`;
2. add a focused DOM renderer/oracle regression proving every closed
   `IconName` resolves and `Compose` renders through the alias;
3. do not cast the union, delete `Compose`, loosen `IconName`, add a second
   registry, or alter app code to hide the mismatch.

Expected paths:

- `apps/openagents.com/packages/effect-native-render-dom/src/index.ts`
- the existing render-dom icon test/oracle file only

Verification:

```bash
bun run --cwd apps/openagents.com/packages/effect-native-render-dom typecheck
bun test apps/openagents.com/packages/effect-native-render-dom/tests/index.test.ts
bun run --cwd apps/openagents-desktop typecheck
bun test apps/openagents-desktop
git diff --check
```

### FAST-2 — reconcile the live P0 issue instructions

Issues: #8566, #8574, #8597, #8638, #8640.

This is ready only when external GitHub mutation is explicitly authorized.
Update checked-in issue sources first, then live bodies/labels, so they say:

- reliable Desktop/mobile fleet software is P0;
- Sarah is a compatibility adapter, not the required front door;
- A/V/persona/presentation work is paused;
- R0–R7 are the acceptance order;
- #8652 is closed and not part of the open count.

Do not rewrite historical receipts or remove still-supported routes. Verify
checked-in/live body equality after GitHub newline normalization. This leaf
does not close any product issue.

Expected source files:

- `docs/sol/issues/app-epic.md`
- `docs/sol/issues/app-desktop.md`
- `docs/sol/issues/app-mobile.md`
- `docs/sol/issues/fc-epic.md`
- `docs/sol/issues/fc-5-dogfood.md`

### FAST-3 — record the R0 baseline after active app claims release

Issues: #8574 and #8597.

Run, without modifying source first:

```bash
bun run --cwd apps/openagents-desktop typecheck
bun test apps/openagents-desktop
bun run --cwd apps/openagents-desktop build
OPENAGENTS_DESKTOP_SMOKE=1 bun run --cwd apps/openagents-desktop smoke
bun run --cwd apps/openagents-mobile typecheck
bun run --cwd apps/openagents-mobile test
```

Classify every failure as:

- regression on current `main`;
- known owner/device/release gate;
- missing authority/Sync feature;
- fixture-only evidence;
- stale documentation.

Fix only a bounded regression with an exact test. Do not turn a missing R1–R6
feature into a fake R0 success. Update `docs/terra/CURRENT_STATE.md` only with
proof produced in the same leaf.

## R1–R2 contract freeze: senior decision required

The lower-reasoning agent must not invent the cross-device authority contract.
Before code delegation, a senior integration owner must publish one bounded
contract naming all of the following:

- canonical package and schema version;
- authenticated owner/org/device/session scope;
- conversation, project/session, FleetRun, work-unit, attempt, account/worker,
  approval, command, outcome, and receipt entity names/keys;
- bootstrap/connect/log/push routes and token refresh/revocation behavior;
- cursor/version/tombstone/gap and `must_refetch` semantics;
- mutation expected-version or commutative behavior, idempotency key, and
  accepted/rejected/failed/unknown-pending-reconcile outcomes;
- which fields are public-safe, owner-private, host-only, or never projected;
- whether Desktop/mobile persistence is SQLite, app document storage, secure
  storage, or a bounded adapter over the existing client;
- migration, compatibility window, rollback, and integration owner.

Reuse `@openagentsinc/khala-sync`, `@openagentsinc/khala-sync-client`,
`@openagentsinc/khala-sync-server`, and `@openagentsinc/khala-fleet-intents`.
Do not create a new sync engine or app-local fleet schema.

Once frozen, delegate these leaves separately.

### SYNC-1 — codecs and fixtures

- Implement only the approved codecs/decoders and safe projection fixtures.
- Add malformed, excess, private-field, wrong-scope, wrong-version, duplicate,
  and tombstone tests.
- Keep production mutators/routes/migrations out of this leaf unless explicitly
  assigned to the integration owner.

Verification:

```bash
bun run --cwd packages/khala-sync typecheck
bun run --cwd packages/khala-sync test
bun run --cwd packages/khala-fleet-intents typecheck
bun run --cwd packages/khala-fleet-intents test
```

### SYNC-2 — Desktop read-only adapter

- Subscribe through the approved Khala Sync client.
- Map typed session phases into explicit bootstrapping/catching-up/live/stale/
  must-refetch/denied/unavailable views.
- Render real conversation/Fleet refs and versions; no controls yet.
- Keep tokens, raw events, paths, and generic network/IPC access out of the
  renderer.

### SYNC-3 — mobile read-only adapter

- Use the same schemas and outcome grammar as Desktop.
- Build the neutral activity/fleet home: recent work, sync health, attention,
  active runs, outcomes, and handoff.
- Do not restore Sarah/demo modes, invent a backing worker/model, or add raw
  filesystem/editor/terminal capability.

### SYNC-4 — real cross-client continuity fixture

Extend the existing
`packages/khala-sync-client/src/cross-app-compose-turn.test.ts` pattern rather
than creating another harness. Prove Desktop-to-mobile and mobile-to-Desktop
projection for approved conversation and Fleet entities with matching refs,
versions, cursor, tombstones, and restart state.

Verification for SYNC-2 through SYNC-4:

```bash
bun run --cwd packages/khala-sync-client typecheck
bun run --cwd packages/khala-sync-client test
bun run --cwd apps/openagents-desktop typecheck
bun test apps/openagents-desktop
bun run --cwd apps/openagents-mobile typecheck
bun run --cwd apps/openagents-mobile test
```

## R3 fleet-control leaves

Start only after R1–R2 read-only continuity is green and the senior owner has
frozen mutation outcomes.

### FLEET-1 — authoritative start

- Replace Desktop's local `stageFleet` presentation with an authenticated,
  server-authoritative start adapter.
- Return a durable `runRef` plus typed accepted/rejected/failed/unknown result.
- Keep local Pylon staging available only as clearly labeled diagnostic
  substrate until retired by proof.
- Mobile receives the same start contract only if R6 product design explicitly
  includes start; otherwise it supervises runs started elsewhere.

### FLEET-2 — run and work detail

- Render plan, work unit, claim, assignment, attempt, worker/account readiness,
  verification, exact-or-`not_measured` usage, and closeout from authoritative
  projections.
- Preserve private-field redaction and explicit unavailable states.

### FLEET-3 — controls

- Add steer, approve/reject, pause/resume/drain/stop using the shared typed
  intent/command IDs.
- Show pending as pending. A timeout becomes unknown-pending-reconcile, never
  success and never an automatic unsafe replay.
- Reconcile the durable command outcome before changing the authoritative UI.

### FLEET-4 — two-client receipt

- Exercise one real Codex+Claude run after #8640 releases its accepted runtime
  receipt.
- Use controls from both clients and prove one durable outcome per command,
  zero duplicate claims, and matching closeout/receipt refs.
- Live execution, credentials, and deployment remain coordinator/owner work;
  this agent may build the clients and deterministic acceptance harness.

## R4 fault and recovery queue

The existing Khala Sync client already tests offline queueing, reconnect,
duplicate/out-of-order delivery, durable cursors, `must_refetch`, access
revocation, and cross-app compose turns. Read those tests before adding any.
Add only missing cases required by the frozen R1–R3 contract:

1. lost command acknowledgement with later durable reconciliation;
2. device process restart with pending mutation and no double execution;
3. cursor retention-window gap causing explicit `must_refetch`;
4. stale lease/attempt outcome that cannot overwrite the current generation;
5. duplicate and out-of-order command outcomes;
6. schema-version compatibility and rollback;
7. background/foreground mobile reconnect;
8. server restart during active Desktop/mobile handoff.

Prefer test-only leaves first. If a deterministic test exposes a production
bug, post the counterexample and extend the claim before changing production
code. Never change the expected result to fit current behavior.

Relevant existing files:

- `packages/khala-sync-client/src/session.test.ts`
- `packages/khala-sync-client/src/session-cvr.test.ts`
- `packages/khala-sync-client/src/transport.test.ts`
- `packages/khala-sync-client/src/sqlite-store.test.ts`
- `packages/khala-sync-client/src/overlay.property.test.ts`
- `packages/khala-sync-client/src/cross-app-compose-turn.test.ts`

## R5 Desktop workbench queue

These app-local leaves may run in parallel with Sync only after #8574 claim
coordination proves paths and contracts are disjoint.

### DESKTOP-1 — bounded edit/save

Extend the existing workspace contract/service. Require a user-selected root,
normalized relative path, traversal/symlink escape rejection, bounded byte
size, explicit dirty/conflict/reload result, and atomic save behavior. Add
fixed IPC channels with sender/origin validation. Never expose general
filesystem APIs to the renderer.

Expected files:

- `apps/openagents-desktop/src/workspace-contract.ts`
- `apps/openagents-desktop/src/workspace-service.ts`
- `apps/openagents-desktop/src/preload.cts`
- `apps/openagents-desktop/src/main.ts`
- focused workspace and Electron-boundary tests

### DESKTOP-2 — typed Git status/diff/review

Start read-only. Return bounded typed status/diff data from the selected root;
reject binary/excess/private output; show unavailable honestly. Review comments
and revert are later leaves. Never pass through arbitrary Git argv or raw shell
output.

### DESKTOP-3 — command registry and palette

Register existing navigation/session/workbench/Fleet intents under stable
command IDs, then add palette and conflict-safe keybindings. One command calls
the same typed handler as direct UI. No keyword intent routing and no duplicate
action path.

### DESKTOP-4 — bounded terminal (senior spec first)

Do not invent this seam. Require an approved contract for workspace root,
shell allowlist/config, environment redaction, PTY lifecycle, resize/input,
output bounds, reconnect, teardown, and renderer projection. After approval,
implement one exact host service and adversarial tests; no generic process or
shell IPC.

### DESKTOP-5 — runtime/settings and productization

Split into separate claims: provider/model catalog, MCP state/auth, permission
policy, diagnostics/export, identity freeze, fuses, signing/notarization,
updates, rollback, and clean-machine smoke. Owner identifiers, credentials,
signing, notarization, and release publication are owner-gated; never guess.

Every Desktop leaf runs:

```bash
bun run --cwd apps/openagents-desktop typecheck
bun test apps/openagents-desktop
bun run --cwd apps/openagents-desktop build
OPENAGENTS_DESKTOP_SMOKE=1 bun run --cwd apps/openagents-desktop smoke
```

## R6 mobile supervision queue

Begin only after the active #8597 Sarah-removal claim releases and rebase onto
its landed state.

1. **MOBILE-1:** neutral activity/fleet home with exact loading, offline,
   reconnecting, stale, must-refetch, denied, empty, and failed states.
2. **MOBILE-2:** run/work/attempt detail and worker/account readiness from
   authoritative Sync projections.
3. **MOBILE-3:** attention/approval queue and the approved subset of typed
   controls, sharing command IDs/outcomes with Desktop.
4. **MOBILE-4:** outcome/receipt inspection and deep-link handoff to Desktop.
5. **MOBILE-5:** background/foreground, restart, offline queue, notification,
   accessibility, and physical-device fault receipts.
6. **MOBILE-6:** Android build/install proof and cross-platform migration/
   rollback. Do not use EAS; follow the app's owned local release lane.

Do not add a mobile editor, Git client, terminal, raw device filesystem,
credential browser, Sarah/persona surface, voice/video, Minerals/demo pricing,
or presentation-only polish. Unsupported deep work hands off to Desktop.

Every non-native mobile leaf runs:

```bash
bun run --cwd apps/openagents-mobile typecheck
bun run --cwd apps/openagents-mobile test
```

Native/prebuild/archive/device commands run only when the leaf changes native
code or release identity and the dispatch explicitly authorizes them.

## R7 and issue closure

Code-landed is not issue-closed. Every closure comment must report the narrowest
true rung for each acceptance item:

1. code-landed;
2. fixture-proven;
3. deployed/distributed;
4. live-proven;
5. owner-accepted;
6. closed and reconciled.

Use this closeout shape:

```text
CLAIM-RELEASE
landed: <main SHA>
scope: <exact leaf>
verification:
- <command>: <exact result>
- <command>: <exact result>
proof rung: <narrowest true rung>
authority/security exercised: <specific boundary>
legacy/duplicate path removed: <path or none>
residual: <specific remaining work or none>
next ready leaf: <leaf id or blocked reason>
```

An issue closes only when its complete exit is met, residuals have owners or
are explicitly removed from scope by the owner, docs and live issue state agree,
and duplicate/legacy paths are deleted or demonstrably unable to ship. Parent
epics #8566 and #8638 close last, after child receipts and R7 reconciliation.

## Copy/paste task prompt for the coding agent

```text
Implement only <LEAF-ID> under openagents/docs/sol/
2026-07-10-112832-cdt-reliable-fleet-implementation-delegation.md.

Issue: #<NUMBER>
Base: current clean origin/main
Allowed paths: <EXACT PATHS>
Hot files/contracts: <EXACT LIST OR NONE>
Acceptance: <ONE FALSIFIABLE OUTCOME>
Verification: <EXACT COMMANDS>

Before editing, read the root AGENTS.md/INVARIANTS.md, the repo claim protocol,
Revision 24, the live issue body/comments, and the named source/tests. Post an
exact CLAIM. Do not touch overlapping claims, paused scope, schemas/migrations,
authority, credentials, lockfiles, shared catalogs, or external systems unless
listed above. Preserve unrelated work. If blocked or uncertain after two
attempts/15 minutes, post a bounded blocker receipt and take the next READY
leaf. On success, stage only scoped files, commit, rebase safely, push main,
and post CLAIM-RELEASE with exact results and residuals.
```

## Recommended next dispatch at this timestamp

1. Refresh the three active claim states (#8574, #8597, #8640).
2. If #8574 hands off the shared renderer contract, run FAST-1 immediately.
3. If #8597 releases, rebase and run FAST-3 mobile verification before adding
   any Sync/Fleet surface.
4. If GitHub metadata mutation is authorized and code claims remain occupied,
   run FAST-2 without touching active implementation paths.
5. The senior integration owner must then freeze R1–R2. As soon as it lands,
   dispatch SYNC-1, SYNC-2, and SYNC-3 as separate, file-disjoint claims and
   integrate them serially through SYNC-4.

The fastest route is not maximum parallel edits. It is a frozen shared contract,
small disjoint leaves, immediate receipts, and zero rework from invented
authority or competing local state.
