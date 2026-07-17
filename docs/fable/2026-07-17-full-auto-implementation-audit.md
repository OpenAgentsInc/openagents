# Full Auto — Implementation Audit and Design-Gap Analysis

- **Date:** 2026-07-17
- **Author:** Fable coordinator session (audit requested by owner)
- **Scope:** How "Full Auto" is currently implemented on `main` of the openagents repo, versus what the design (ProductSpec rev 9 + hardening epic #8873 + lane epics #8901/#8902) says it should be.
- **Method:** Three parallel research sweeps (implementation code, design/spec/issue record, handoff/dogfood context), followed by inline verification of file paths, commits-on-main, smoke scripts, and release tags in this session.

---

## 1. Executive summary

Full Auto is a **durable, restart-survivable autonomous continuation loop** for OpenAgents Desktop. A per-thread toggle (or the local control API) enables it; the main process then keeps re-dispatching "continue" turns against the granted workspace until the user stops it, a safety cap (20) is hit, failures accumulate (5), or the workspace no longer matches.

The implementation on `main` is **substantially complete against the rev-9 ProductSpec**: all 13 hardening children of epic #8873 are merged, provider-lane generalization (L6 #8901) and lane-independent spec projection (L7 #8902) are merged, live proofs exist for Codex + Claude + ACP Grok, and post-handoff stall/resume fixes have landed (`d3ad8424da`, `8cb900bbf9`, `4dfd1e834e`, `2ae33f3e09`, all verified on `main`).

The gaps that remain are **almost entirely proof/release gaps, not code gaps**:

1. **No Desktop release tag contains Full Auto.** Verified this session: `git tag --contains d480f779aa` returns nothing; latest tag is `openagents-desktop-v0.1.0-rc.12`. The feature exists only on `main`/dev builds.
2. **The owner packaged-app restart observation has never been performed** — the ProductSpec's single success metric (`owner_observed_restart_resume_sessions >= 1` before any release claim) is unmet.
3. **AssuranceSpec is `proposed` with 37/37 obligations `needs_design`** — no proof design, execution, or admission has happened; the assurance program is honest about this but entirely unstarted.
4. **ACP release claim is blocked** on the ACP-10 matrix (#8897): Grok live-loop proof achieved, Cursor pending; FA-AC-33 retained as an explicit residual.
5. **Usage metrics (#8911) are plumbed but gated off** — owner consent copy approved, live rollout receipts pending; the ProductSpec's original quality metrics (repo-grounded first actions, median consecutive turns, stop reliability) remain unmeasured.

---

## 2. Current implementation (verified on `main`)

All paths below verified to exist this session under `apps/openagents-desktop/`.

### 2.1 Core modules (main process)

| Module | Role |
|---|---|
| `src/full-auto-registry.ts` | Durable per-thread state: `enabled`, `continuationCount`, workspace binding, execution profile (lane/account/model/effort), dispatch lease, failure state, typed `disabledBy` attribution. Atomic write (`.pending` + rename); corrupt files quarantined to `registry.json.quarantined-<ISO>` and the app continues with Full Auto off. Schema `openagents.desktop.full_auto_registry.v1`; 128-record eviction cap that never evicts enabled records. |
| `src/full-auto-reconcile.ts` | The single continuation decision function, `reconcileFullAutoThreads()`, invoked from exactly two places: after any Full-Auto-flagged turn completes, and once at startup after interrupted-turn recovery. Promise-chain mutex (`makeSerialTaskQueue`) + durable per-thread lease = exactly-once dispatch. Constants: 20-continuation cap, 5-consecutive-failure disable, exponential backoff `min(2^n × 30s, 30min)`. |
| `src/full-auto-lane.ts` | Lane-keyed instruction policies (`codex-local` default, `fable-local`, `acp:grok-cli`, `acp:cursor-agent`). Unknown lanes fail closed (`null` policy). `autoResolveQuestions: true` for admitted lanes; Claude background `AskUserQuestion` is denied immediately with proceed-with-judgment guidance rather than parking. |
| `src/full-auto-followup.ts` | Destructive one-shot handoff of promoted followups from background turns into the next dispatch. |
| `src/full-auto-control-contract.ts` / `src/full-auto-control-openapi.ts` / `src/full-auto-control-server.ts` | Phase-1 local control surface: loopback-only (127.0.0.1) node:http server, off by default (`OPENAGENTS_DESKTOP_FULL_AUTO_CONTROL=1` to enable), bearer-gated on every route via environment-auth narrowing-only credential (scopes `operator_read` + `coding_session_control`), constant-time token compare, 0600 connection file at `{userData}/full-auto/control.json`. Hand-authored OpenAPI 3.1 doc is the canonical surface; a bidirectional parity test enforces server↔doc equivalence. |

Registry file lives at `{userData}/full-auto/registry.json`.

### 2.2 Control API routes

```
GET  /v1/openapi.json
GET  /v1/lanes
GET  /v1/full-auto
POST /v1/full-auto/start                      (bootstrap: mint thread, bind workspace, enable, reconcile)
GET  /v1/full-auto/{threadRef}
POST /v1/full-auto/{threadRef}/enable
POST /v1/full-auto/{threadRef}/disable
POST /v1/full-auto/{threadRef}/continue-now   (fire-and-forget reconcile trigger)
GET  /v1/full-auto/{threadRef}/turns          (last 20 turn.full-auto.* rows, metadata only)
```

Enable/start require the caller to name the expected workspace and refuse 409 `workspace_mismatch` without touching the registry. Every mutation appends an owner-visible system note attributed `caller: control-api`. Clients: `scripts/full-auto-cli.ts` (argv), `scripts/full-auto-mcp.ts` (stdio MCP), over shared `scripts/full-auto-control-client.ts`.

### 2.3 IPC surface (renderer ↔ main)

- `CodexLocalFullAutoSetChannel` — toggle persists immediately to the durable registry.
- `CodexLocalFullAutoGetChannel` — `{ enabled, state, turnRef }` for hydration.
- `CodexLocalFullAutoStateChannel` — broadcast of coarse live state: `idle | turn_running | turn_completed | turn_failed | cap_reached | blocked`.
- `CodexLocalFullAutoInterruptChannel` — thread-scoped Stop targeting the actual background turn.

Renderer (`src/renderer/shell.ts`) keeps `fullAutoByThread`, hydrates from the durable registry at attach, fences manual sends while a background turn is running.

### 2.4 Safety model (as implemented)

- **Workspace binding (FA-H2):** enable captures the main-resolved workspace; reconciliation refuses and disables (`disabledBy: "workspace_guard"`) on mismatch; pre-upgrade unbound records fail closed.
- **Exactly-once (FA-H3):** serialized reconciliation + durable lease claimed before dispatch; stale leases cleared only by the startup pass.
- **Failure policy (FA-H5):** thrown errors and `ok:false` both persist typed failure state; exponential backoff; auto-disable at 5 consecutive failures. Failures never consume cap slots.
- **Profile continuity (FA-H6):** initiating turn's lane/account/model/effort bound and replayed; profile revalidated against live contract enums on read.
- **Cap (FA-H7):** 20 successful continuations → durable self-disable with note; resets only on toggle-off.
- **Attribution (#8928):** `disabledBy ∈ {ui_toggle, control_api, workspace_guard, continuation_cap, dispatch_failure_limit}` + `disabledAt`, closing the shared-Mac "records flip to disabled mysteriously" incident.
- **Approval policy:** `approvalPolicy: "never"` forced on Full Auto turns; sandbox stays the existing danger-full-access default — Full Auto deliberately inherits the same trust boundary as any Codex turn (no new permission system, per scope cut).

### 2.5 Tests and smokes (verified present)

- `tests/full-auto-registry.test.ts`, `tests/full-auto-restart.e2e.test.ts` (on-disk two-registry restart semantics, ~600 LOC), `src/full-auto-control-server.test.ts` (auth, parity, projections), `src/full-auto-lane.test.ts`, `src/full-auto-followup.test.ts`, `src/full-auto-hydration.integration.test.ts`, plus renderer coverage in `shell.test.ts` / `react-composer.test.tsx`.
- Real-Electron smokes wired in `package.json`: `smoke:full-auto-restart` (two-process seed→resume, incl. workspace-mismatch and Claude-lane fixtures) and `smoke:full-auto-control` (second-OS-process CLI against a live control server).
- No TODO/FIXME/stubs found in the Full Auto core paths.

### 2.6 Post-handoff fixes already on `main`

The 2026-07-16 dogfood window surfaced a resume/stall class of bugs; the fixes are merged (verified `git branch --contains` → `main`):

- `d3ad8424da` — resume Full Auto conversations canonically (stall fix)
- `8cb900bbf9` — retain active chats for Full Auto
- `4dfd1e834e` — start Full Auto on toggle
- `2ae33f3e09` — restore controls for resumed Full Auto
- `83c136dead` — attribute Full Auto stops (#8928)

---

## 3. The design record

- **ProductSpec:** `specs/desktop/full-auto.product-spec.md` — rev 9 (2026-07-16), 37 acceptance criteria FA-AC-01…37, explicit in/out/cut lists, one success metric, owner gates, per-rev receipts.
- **AssuranceSpec:** `specs/desktop/full-auto.assurance-spec.md` — rev 1, bound to ProductSpec rev 9, lifecycle `proposed`.
- **Issue lineage:** #8852 (rev 1, renderer loop) → #8853 (rev 2, durable main-owned loop) → epic **#8873** (13 hardening children #8874–#8886, all closed) → **#8901** L6 lane generalization → **#8902** L7 spec-workflow projection → **#8928** attribution guard. Supporting: #8911 (usage plumbing), #8897 (ACP-10 release gate).
- **Historical design:** `docs/fable/2026-07-15-full-auto-repo-intent-to-dispatch-loop.product-spec.md`; deep-dive audit `docs/sol/2026-07-16-openagents-desktop-full-auto-deep-dive.md` (10 findings); dogfood runbook `docs/sol/2026-07-16-full-auto-shared-mac-dogfood-runbook.md`; terminal handoff `docs/fable/2026-07-16-fable-session-final-closeout.md`.

Design intent, condensed: *one toggle + a repo-grounding instruction (not a new permission model) + durable main-owned goal state* — press the button, get repo-grounded work that survives quit/relaunch, with a plain Stop and a 20-turn cap as the only autonomy policy (CUT-FA-01).

---

## 4. Design vs implementation — gap analysis

### 4.1 Closed gaps (deep-dive findings → hardened)

All ten findings of the 2026-07-16 deep-dive were addressed or explicitly re-scoped:

| Finding | Status |
|---|---|
| 1. Toggle hydration mismatch after restart | Fixed for hydration-at-attach + live state (FA-AC-19–21); arbitrary in-session thread-switch resync remains an explicit spec cut (CUT-FA-03) |
| 2. No workspace binding | Fixed (FA-H2, fail-closed) |
| 3. Overlapping dispatch possible | Fixed (FA-H3 mutex + lease) |
| 4. Background work invisible / no stop | Fixed (FA-H4 live state, Stop, send fencing) |
| 5. `ok:false` silent stall | Fixed (FA-H5 typed failures + backoff + auto-disable) |
| 6. Profile reset on continuation | Fixed (FA-H6 binding/replay) |
| 7. Cap-reset API drift | Clarified in spec + tested (toggle-off is the only reset) |
| 8. Documentation rot | Largely fixed — GUARANTEES.md no longer claims a renderer-owned loop (grep-verified this session); older docs/sol and docs/fable design records remain historical, correctly superseded by the ProductSpec |
| 9. Unmeasurable metrics | Plumbing landed default-off (#8911); live measurement still pending (see 4.2) |
| 10. Corrupt registry crash | Fixed (quarantine + fail-closed feature, app boots) |

### 4.2 Open gaps (current, ranked)

1. **Release gap — no distributed artifact.** No Desktop tag contains any Full Auto commit (verified: `git tag --contains d480f779aa` is empty; newest tag `openagents-desktop-v0.1.0-rc.12`). Every proof so far is dev-build. Full Auto cannot be claimed "shipped" until the DIST chain (#8917 coordinator, #8922 feed, #8926 `pnpm run release` real-port wiring + RC run) produces a tag containing it.
2. **Owner observation gap.** The ProductSpec's sole success metric — at least one owner-observed packaged-app toggle-on → send → quit → relaunch → resume session, receipted in an issue or NEEDS_OWNER — has not been recorded. The module-level e2e and the two-process smoke exercise the same durable modules, but the spec explicitly reserves release claims on the real observation. This is an owner gate, not an engineering task; per UI-first policy, the ask is "run the packaged app, toggle Full Auto, quit, relaunch, watch it resume."
3. **Assurance gap.** AssuranceSpec rev 1 is `proposed`: 37/37 obligations `needs_design`, execution unauthorized. That is the honest state per #8902's closure, but it means the formal assurance program (proof design → execution → admission) for Full Auto has not begun. Until it does, "spec-covered" means product-spec-covered only.
4. **ACP residual (FA-AC-33).** Grok ACP live loop proven (real turn, real commit, durable disable); Cursor pending; both gated behind the ACP-10 pinned compatibility matrix (#8897) before any non-experimental ACP Full Auto claim. Current honest status: ACP lanes are *experimental*.
5. **Metrics gap.** #8911 landed authenticated pre-turn admission + durable idempotent outbox, but gates are default-off pending agent-side enablement and a live receipt. Consequently the original design-quality metrics (≥80% repo-grounded first actions, ≥3 median consecutive turns, 100% stop reliability) are still unmeasured — we cannot yet say quantitatively whether Full Auto does *good* work, only that the loop mechanics are sound.
6. **Cross-machine control (Phase 2) — designed-out, not built.** The control surface is same-machine loopback by design; relay through openagents.com/Khala Sync is an explicit out-of-scope Phase 2. No gap against rev 9, but worth naming since "control Full Auto from anywhere" is the obvious next owner expectation.
7. **Promise registry.** `autopilot.desktop_full_auto_guidance.v1` remains red; the spec notes shipping this feature does not flip it — a separate registry pass with real-use evidence is required.
8. **Minor known cut:** toggle state does not resync when switching to a previously-enabled thread mid-session (CUT-FA-03, acknowledged; the toggle is truthful for the thread you send from).

### 4.3 Assessment

The build phase of Full Auto is done and hardened to a standard well above the original rev-1 sketch: durable, exactly-once, fail-closed on workspace, attributed on every disable, controllable programmatically with an OpenAPI-parity-tested surface, and generalized across provider lanes. The distance between "implemented" and "designed" is now concentrated in the **evidence ladder**: tag it, have the owner watch one real restart-resume, turn on the metrics, finish the ACP matrix, and start the assurance program. None of these are code gaps on the loop itself.

---

## 5. Recommended next steps

1. Finish the DIST chain (#8917/#8922/#8926) and cut the first Desktop tag containing Full Auto — this is the single biggest blocker to any release claim.
2. Owner gate: one packaged-app restart-resume observation, receipted (screen recording or issue receipt satisfies the metric).
3. Enable the #8911 usage gates for the owner's dogfood profile and capture the live receipt; then wire the three design-quality metrics onto that plumbing.
4. Drive FA-AC-33/#8897 to a verdict: Cursor live loop + pinned matrix, or formally re-scope ACP Full Auto as experimental in the spec.
5. Begin AssuranceSpec proof design for the highest-risk obligations first (exactly-once FA-AC-15, workspace fail-closed FA-AC-13/14, stop FA-AC-20).
6. Registry pass on `autopilot.desktop_full_auto_guidance.v1` once real dogfood use exists.
