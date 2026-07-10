# Reliable fleet software implementation delegation

- Created: 2026-07-10 11:28:32 CDT (`2026-07-10T16:28:32Z`)
- Repository snapshot: `19ebe9741f23cfd58e4a1be47c6c08476f742709`
- Audience: bounded, high-throughput coding agent with limited architectural
  judgment
- Authority: [`MASTER_ROADMAP.md`](./MASTER_ROADMAP.md), Revision 25
- Mobile capability ledger:
  [`2026-07-10-khala-code-mvp-to-openagents-mobile-port-plan.md`](./2026-07-10-khala-code-mvp-to-openagents-mobile-port-plan.md)
- Coordination: [`CLAIM_PROTOCOL.md`](./CLAIM_PROTOCOL.md)
- Status: executable delegation packet; not a replacement for the roadmap,
  live issue state, or an active claim

## Execution progress

- 2026-07-10 11:38 CDT — FAST-1 claimed on #8574 after the prior Desktop
  execution claim released. The scope is limited to the DOM renderer's
  `Compose` → `ChatCompose` asset alias and its closed-icon regression; no
  catalog, schema, app, authority, or Fleet contract change is in scope.
- 2026-07-10 11:41 CDT — Renderer typecheck and focused regression passed.
  The Desktop suite exposed its former direct-call static assertion, so the
  claim was explicitly extended to update that guard to require the narrow
  alias before the shared catalog call. No production contract changed.
- 2026-07-10 11:42 CDT — FAST-1 verification is green: render-dom typecheck;
  four focused DOM tests; Desktop typecheck; 61 Desktop tests; and
  `git diff --check`. Preparing the scoped commit and release receipt.
- 2026-07-10 11:45 CDT — FAST-1 landed on `main` as `68dd9254db` and its
  `CLAIM-RELEASE` receipt was verified on #8574 (fixture-proven). Refresh of
  #8597 found an active 16:28 CDT mobile Sarah-removal/Khala-composer claim;
  FAST-3 and R6 remain unavailable until it explicitly releases. #8640 remains
  coordinator/owner-gated, including its Codex account reconnect and live proof.
- 2026-07-10 11:41 CDT — FAST-2 claimed across #8566, #8574, #8597, #8638, and
  #8640 in a clean worktree because the shared checkout contains unrelated Sol
  edits. The metadata-only leaf added the Revision 24 override to each checked-in
  issue source, then reconciled each live body and label without touching active
  implementation paths.
- 2026-07-10 11:42 CDT — FAST-2 reconciliation is verified: all five live
  bodies match their checked-in source after GitHub final-newline normalization;
  every issue now has `priority:P0`, while stale `priority:P1-parallel` and
  `area:sarah` labels were removed. `git diff --check` passes; preparing the
  scoped commit and release receipts.
- 2026-07-10 11:43 CDT — FAST-2 landed on `main` as `8b23f6f45d`; release
  receipts were posted to #8566, #8574, #8597, #8638, and #8640. The next
  implementation leaf was blocked by #8597's then-active mobile claim and the
  missing senior R1–R2 contract freeze; #8640 remained owner/coordinator-gated.
- 2026-07-10 11:43 CDT — Refresh confirmed #8597's current claim is only
  fifteen minutes old and therefore not stale under the claim protocol. No
  senior R1–R2 contract freeze was found, so SYNC-1 through SYNC-4, R3, and R4
  remain intentionally undispatched.
- 2026-07-10 11:53 CDT — Revision 25's mobile remote-workroom decision
  supersedes FAST-2's supervision-only wording and requires another source/live
  reconciliation.
- 2026-07-10 11:51 CDT — the #8597 mobile claim released at `e8bf6b8603` after
  build 116 removed Sarah/persona/demo/local-catalog state, made the native
  composer the sole Khala input, passed typecheck and 20 tests/69 expectations,
  archived/exported, and was accepted for App Store delivery. Processing/
  `VALID` and owner physical-device acceptance remain unproven. FAST-3 and a
  disjoint M0–M2 leaf may now be selected after refreshing current claims.
- 2026-07-10 11:47 CDT — FAST-3 ran from clean current `main`: Desktop
  typecheck, 61-test/330-expectation suite, build, and fixture-only Electron
  smoke all passed; mobile typecheck and 20-test/69-expectation suite passed.
  This is R0 evidence only, not proof of Sync/Fleet authority, build-116
  processing, or physical-device acceptance. A real current Desktop reconnect
  reached the OpenAI device-auth consent screen after honestly displaying five
  `credentials_revoked` accounts; it remains pending owner consent.
- 2026-07-10 11:55 CDT — The published senior R1–R2 contract enabled SYNC-1.
  A focused regression found that Effect Schema class decoders silently stripped
  an excess private request field. `decodePushRequest`,
  `decodeBootstrapRequest`, and `decodeCvrPullRequest` now reject excess
  top-level fields; the negative matrix covers excess private input, wrong
  scope, and invalid schema version. Khala Sync typecheck plus 175 tests and
  Fleet Intents typecheck plus 27 tests pass.
- 2026-07-10 11:58 CDT — Following transcript 248's predictable-software
  standard, Desktop now has a normal `verify` gate: enforced UX/e2e contracts,
  typecheck, bundle build, and real Electron smoke run together. Current main
  passes 70 Desktop tests/363 expectations, including the 256 MiB first-content
  budget and selected recent-Codex-history smoke receipt.
- 2026-07-10 12:05 CDT — M0 mobile truthfulness now consumes the frozen
  `ScopeSyncState` vocabulary. The neutral OpenAgents surface renders an
  explicit `Sync not configured` state until the later authenticated adapter
  exists; its regression oracle forbids fabricated FleetRun and repository
  content. Mobile typecheck and 21 tests/74 expectations pass.
- 2026-07-10 12:12 CDT — SYNC-4 now has a deterministic two-client fleet
  projection fixture: a server-originated `fleet_run` upsert is fanned out to
  desktop and mobile sessions, then a tombstone removes it from both views.
  The fixture proves both durable cursors reach version 2 and a fresh overlay
  over the desktop durable store retains that empty/tombstoned state. This is
  fixture proof over the real client/session/store seam, not a deployed
  server, physical-device, or live-account receipt. Khala Sync typecheck plus
  134 tests/12,572 expectations, Desktop typecheck plus 70 tests/363
  expectations, and mobile typecheck plus 21 tests/74 expectations pass.
- 2026-07-10 12:18 CDT — R4 now covers the mobile-equivalent lifecycle seam:
  background unsubscribe leaves the confirmed projection at its durable cursor,
  and foreground resubscribe catches up to the server without bootstrap or
  duplicate rows. This is a deterministic shared-client fixture, not proof of
  a native app lifecycle adapter or physical-device background execution.
  Khala Sync typecheck plus 135 tests/12,578 expectations and mobile
  typecheck plus 21 tests/74 expectations pass.
- 2026-07-10 12:24 CDT — M1 compatibility audit found a security-contract
  mismatch that must be resolved by Sol before a real mobile sign-in can land:
  `apps/openagents-mobile/app.json` registers `openagents://`, while the
  frozen server-side mobile OpenAuth PKCE redirect policy accepts only
  `khala://auth` for the mobile client. No client redirect, issuer allowlist,
  native manifest, or credential-storage behavior was invented or relaxed.
  The next M1 leaf is a senior-owned decision that names the canonical
  OpenAgents redirect/client registration and its migration/rollback posture;
  only then can secure-storage recovery and the authenticated Sync adapter be
  implemented honestly.
- 2026-07-10 12:34 CDT — DESKTOP-1 now completes the bounded existing-file
  save seam. Local filesystem authority starts only after a directory-picker
  selection; canonical-root and symlink escapes, binary/truncated files, and
  oversized edits fail closed. Saves are revision-bound, atomic within the
  selected directory, and return an explicit conflict that preserves the
  draft until the user chooses reload; the renderer receives only the fixed,
  schema-validated bridge. Desktop `verify` passes: 77 tests/399 expectations,
  bundle build, and Electron fixture smoke. This is local fixture proof, not
  authenticated Sync/workroom/writeback authority or a physical-device
  receipt.
- 2026-07-10 12:42 CDT — DESKTOP-2 now has a read-only typed Git review seam:
  fixed porcelain-status and selected-file diff arguments only, normalized
  change records, bounded output, and explicit unavailable results for binary,
  secret-shaped, escaped, or excessive output. The renderer receives no Git
  argv, process error, or generic shell surface, and has no review mutation
  controls. Desktop `verify` passes: 80 tests/414 expectations, bundle build,
  and Electron fixture smoke. Review comments, revert, remote writeback, and
  a physical visual receipt remain separate work.
- 2026-07-10 12:50 CDT — DESKTOP-3 now has a closed command registry and
  renderer-only palette. Each palette entry dispatches the same existing typed
  intent as its direct UI control; it carries no callback, shell command,
  host capability, or user-supplied route. Cmd/Ctrl+K opens it only when the
  browser event is otherwise unhandled and focus is not editable. Desktop
  `verify` passes: 82 tests/426 expectations, bundle build, and Electron
  fixture smoke. Visual/manual accessibility receipt and later command search
  remain separate work.

## Mission

Rapidly close the active OpenAgents reliability program in dependency order:

1. make Desktop and mobile truthfully green;
2. establish one persona-neutral authenticated identity/session contract;
3. connect both clients to the same Khala Sync conversation and Fleet state;
4. expose real fleet start, inspection, steering, approval, pause/resume/stop,
   outcomes, and receipts on both clients;
5. prove offline/reconnect/restart and duplicate-suppression behavior;
6. finish the practical OpenCode-parity Desktop workbench and port the useful
   Khala Code MVP into a compact Effect Native mobile remote workbench;
7. prove mobile-originated coding in a real brokered remote workroom, including
   files, changes, bounded terminal, managed preview, safe writeback, and
   cross-device continuation;
8. package, release, dogfood, and close with exact receipts.

The agent implements small, already-decided leaves. It does **not** invent a
new architecture, relax authority, infer completion from UI text, or revive
paused Sarah/persona/A/V/presentation scope.

## Read this before touching code

At this revision, GitHub reports **15** open `roadmap:sol` issues. #8652 closed
after the initial Revision 24 table, then reopened without `roadmap:sol`; it is
outside this dispatch set and portal expansion remains paused. Live issue labels and bodies still use
older Sarah-first/P1 wording. Revision 25 controls priority and product shape
until those records are reconciled.

Several lanes are concurrently owned:

- #8597's Sarah-removal/native-composer claim released at `e8bf6b8603`. Do not
  assume the lane remains free: refresh comments before each M0–M7 leaf.
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
- the issue is still open and active under Revision 25;
- no live claim overlaps files or contracts;
- the leaf has a falsifiable acceptance test;
- required upstream contract decisions already exist.

Post the exact `CLAIM` template from `CLAIM_PROTOCOL.md` before mutation. A
general “working on desktop/mobile” comment is not enough.

## Live issue disposition and what this agent may do

| Issue | Revision 25 disposition | Delegation rule |
| --- | --- | --- |
| #8566 | P0 parent | Do not claim the epic. Reconcile its stale body/labels when authorized; close only after R0–R7 and child exits reconcile. |
| #8574 | P0 Desktop | Primary implementation lane. Take one D0–D6 leaf only after the active claim releases or explicitly hands off disjoint paths. |
| #8597 | P0 mobile | Primary implementation lane. The prior claim released; refresh live comments, then take one M0–M7/R0–R6 leaf from the mobile port ledger. |
| #8638 | P0 Fleet substrate parent | Do not rebuild Fleet or claim the epic. Integrate existing contracts; close only after #8640, R3/R7, and accepted follow-on disposition. |
| #8640 | P0 live proof | Coordinator/owner-gated. Do not take credentials, scanner, type-gate, deploy, or live burn work. Consume its receipts after release. |
| #8547 | P0 remote execution | Minimum real remote-workroom path is required for mobile R6/R7. A senior integration owner must freeze Firecracker/workroom, grant, isolation, writeback, and reclaim contracts before delegation. |
| #8636 | P0 remote routing | Minimum explicit local/remote target contract is required for R3/R6/R7. Never invent fallback or grant authority without the approved contract; advanced placement breadth follows R7. |
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
| 3 | R3/M3 | Real Fleet start/control/outcome plus remote-workroom lifecycle on both clients | #8640 live proof and approved #8547/#8636 contract work |
| 4 | R4 | Offline/restart/replay/refetch convergence | Desktop D2/D3 and mobile read-only supervision |
| 5 | R5–R6/M4–M6 | Complete Desktop workbench and mobile remote coding/fleet client | Packaging prep and diagnostics |
| 6 | R7/M7 | Signed/installable releases and mobile-originated remote-coding dogfood receipt | None that changes the acceptance baseline |
| 7 | Follow-ons | Advanced cloud/provider/placement breadth, then explicitly reactivated work | Only after R7 |

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
- #8652 is not part of the labeled roadmap count; its later reopen does not
  reactivate portal product scope under Revision 25.

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
  approval, command, outcome, receipt, repository, remote-workroom, preview,
  artifact, and writeback entity names/keys;
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
- Build the neutral coding/fleet home: recent work, repositories, sync health,
  attention, active threads/workrooms/runs, outcomes, and handoff.
- Do not restore Sarah/demo modes, invent a backing worker/model, or add raw
  **local device** filesystem/process/credential capability. Typed remote
  editor/terminal/preview projections belong to later M3–M5 leaves.

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

## M3 remote-workroom contract freeze: senior decision required

The lower-reasoning agent must not invent isolation, credential, workroom, Git
writeback, port, or target-fallback authority. Before M3 production code, a
senior integration owner must freeze:

- canonical lifecycle states and create/resume/stop/destroy/reclaim outcomes;
- owner/repository/thread/workroom/run/snapshot refs and TTL/recovery rules;
- provider/Git grant issue, redemption, scope, expiry, revocation, and replay
  defense;
- isolated workspace/account homes and the accepted production isolation rung;
- bounded file IO, run/spawn/PTY, output/timeout/reconnect/teardown behavior;
- managed preview-port discovery/gateway/auth/expiry and network policy;
- exact pre/post image, branch/PR writeback, no-force policy, verification, and
  failure recovery;
- target selection/eligibility/fallback history and one cross-target claim
  registry; and
- safe projections, usage/compute receipts, reclaim evidence, migrations, and
  integration owner.

After freeze, the delegated agent may take one codec/fixture, read-only mobile
projection, bounded Effect Native surface, or deterministic fault-test leaf at
a time. Real grants, production isolation, deployment, credentials, live burns,
and final acceptance remain senior/owner-gated.

## R3 fleet-control leaves

Start only after R1–R2 read-only continuity is green and the senior owner has
frozen mutation outcomes.

### FLEET-1 — authoritative start

- Replace Desktop's local `stageFleet` presentation with an authenticated,
  server-authoritative start adapter.
- Return a durable `runRef` plus typed accepted/rejected/failed/unknown result.
- Keep local Pylon staging available only as clearly labeled diagnostic
  substrate until retired by proof.
- Mobile receives the same start contract and must explicitly select or accept
  the typed execution target. Start cannot silently choose a provider account,
  Pylon, or remote workroom.

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

## R6 / M0–M7 mobile coding and fleet queue

The Sarah-removal claim released at `e8bf6b8603`; rebase onto its landed state
and confirm no successor claim overlaps. Use the exhaustive
[`mobile port ledger`](./2026-07-10-khala-code-mvp-to-openagents-mobile-port-plan.md);
do not dispatch from this abbreviated queue alone.

1. **M0 — freeze/inventory:** capability manifest, source/destination locks,
   legacy test-vector inventory, and honest missing states.
2. **M1 — identity/Sync/repos:** owner session, secure recovery, device,
   repository picker/binding, shared threads, cursor/offline state.
3. **M2 — authoritative turns:** rich events, composer context, queue/steer/
   interrupt/retry, named account/model/target readiness, push/deep links.
4. **M3 — workroom lifecycle:** after senior contract freeze, integrate #8547/
   #8636 create/resume/stop/reclaim, brokered grants, isolation rung, TTL/
   snapshot, and exact target/fallback projection.
5. **M4 — files/changes/writeback:** bounded tree/read/edit, exact diff,
   artifacts/verification, safe branch/PR refs, no force writeback.
6. **M5 — terminal/preview:** bounded remote PTY/run/spawn, managed ports,
   authenticated preview, reconnect/teardown, stop/reclaim evidence.
7. **M6 — Fleet/release hardening:** run/work/attempt detail, attention/
   approvals, shared controls, background/offline faults, accessibility,
   physical iOS/Android, local builds/updates, rewritten QA gates. Never use
   EAS.
8. **M7 — dogfood/retirement:** mobile-originated remote task, Desktop
   continuation, safe writeback, exact receipt, migration, and inability of the
   legacy product/install/update path to ship.

Do not add raw **local device** filesystem/process/credential/port authority,
import the legacy app package/UI tree, restore Sarah/persona/voice/video/
Minerals/demo pricing, or add presentation-only polish. Remote files, changes,
terminal, preview, artifacts, and writeback are required through the approved
owner-scoped workroom contract. Desktop handoff is optional, not the only way
to finish useful coding.

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
Master Revision 25, the mobile port ledger when applicable, the live issue
body/comments, and the named source/tests. Post an
exact CLAIM. Do not touch overlapping claims, paused scope, schemas/migrations,
authority, credentials, lockfiles, shared catalogs, or external systems unless
listed above. Preserve unrelated work. If blocked or uncertain after two
attempts/15 minutes, post a bounded blocker receipt and take the next READY
leaf. On success, stage only scoped files, commit, rebase safely, push main,
and post CLAIM-RELEASE with exact results and residuals.
```

## Recommended next dispatch at this timestamp

1. Refresh the active claim states (#8574, #8597, #8640, #8547, #8636).
2. FAST-1 and FAST-2 are already landed; do not repeat them. If #8597 releases,
   rebase and run FAST-3 mobile verification before adding Sync/workroom UI.
3. While shared contracts are blocked, take only M0 inventory/test-vector work
   or app-local honest-state/accessibility leaves with disjoint claims.
4. The senior integration owner freezes R1–R2 and the M3 workroom/target/grant/
   writeback contract. Then dispatch SYNC-1/2/3 and M1/M2 leaves separately,
   integrate through SYNC-4, and keep schema/migration changes serialized.
5. After real #8547/#8636 substrate is available, dispatch M3–M5 read-only
   projections first, then controls, fault tests, physical-device receipts, and
   the M7 cross-device dogfood.

The fastest route is not maximum parallel edits. It is a frozen shared contract,
small disjoint leaves, immediate receipts, and zero rework from invented
authority or competing local state.
