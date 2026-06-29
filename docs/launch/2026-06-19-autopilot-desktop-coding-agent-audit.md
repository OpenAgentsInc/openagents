# Autopilot Desktop → Full Coding Agent — Audit + Roadmap

Date: 2026-06-19
Repo: `OpenAgentsInc/openagents`
Type: research/audit + GitHub roadmap. **No app code was changed.** The
concurrently-edited surfaces `apps/autopilot-desktop/src/ui/*`,
`apps/openagents.com/workers/api/src/blueprint/*` (Blueprint × Tassadar chat), and
`apps/openagents.com/workers/api` payout paths were read **read-only** and are NOT
modified by this audit; the roadmap issues it opens are explicitly scoped to avoid
colliding with that in-flight work (see §7).

All paths are repo-relative to `OpenAgentsInc/openagents` unless noted.

---

## 0. The question

> Turn the Autopilot Electrobun desktop app into a **full-fledged coding agent** —
> connect all the terminal-agent / coding-agent systems we built but never wired
> into the UI, plus the Blueprint/Tassadar chat we just spec'd, behind **clean
> navigation** (no UI clutter).

The honest headline, after reading the runtime, the desktop, and the recently
landed panes: **the desktop is much further along than the framing assumes.** The
runtime substrate (Pylon control protocol, Probe executors, approvals, accounts,
swarm, the autonomous coordinator loop, external-session nesting) is real, and as
of the last few days the desktop already has **composer, swarm, chat, spawn,
sessions, decisions, session-detail, accounts** panes wired to it. The work that
remains is not "build the panes from scratch" — it is:

1. **Make the Blueprint chat real** (it is currently presentational/seeded, not
   driven by live signature selection / module execution / replay verdicts).
2. **Surface the autonomous coordinator loop** (intent → fanout → ship) as a
   first-class desktop view (today it is only a small "ask" card + a header
   pause/resume toggle).
3. **Raise diff/transcript fidelity** and add **artifact/receipt depth**.
4. **Re-organize navigation** — the sidebar is now a flat 13-button wall and there
   is **no command palette and no keyboard shortcuts at all**. This is the literal
   "don't clutter the UI" requirement.
5. **Close the live-node + packaging gaps** that keep all coding panes dark on a
   fresh install (tracked by the auto-onboarding EPIC #5441; referenced, not
   duplicated).

This doc gives a per-system BUILT/PARTIAL/SCAFFOLD/NOT-CONNECTED table, a target
architecture, a clean navigation design, and a GitHub EPIC + child roadmap.

> Note on a prior audit: `docs/launch/2026-06-18-autopilot-desktop-coding-surface-audit.md`
> (yesterday) concluded "no interactive composer loop in-window; swarm MISSING."
> That was true **then**. Between that audit and this one, the **composer pane
> (#5355), swarm pane (#5362), chat pane (#5453), per-session account picker, and
> account-management card** all landed. This audit supersedes the pane-status
> findings of that one; its runtime findings and the yellow-promise scope
> (`autopilot.desktop_gui_client.v1`) still hold.

---

## 1. The substrate (what's BUILT on the runtime side)

These exist and work on the backend; the question for each is only "is it
surfaced in the desktop, and how well."

### 1.1 Probe coding-agent runtime — `packages/probe`
- The original Probe runtime (`packages/probe/packages/runtime/src/`) is a
  single-turn Gemini/Apple-FM backend + a benchmark/study harness with real
  file-mutation tools (`file-mutation.ts`: write/edit/patch with line-ending/BOM
  preservation, per-file locks, stale-content guard) and a **permission
  scaffold** (`permission.ts`) whose default handler **always allows** (interactive
  UX explicitly "for when that integration is built"). **No multi-turn session
  state machine, no control server, no transcript store** live here.
- **The real, driveable coding loop lives in Pylon, not Probe** (below). This is a
  load-bearing distinction: the desktop drives Pylon.

### 1.2 Pylon control surface — `apps/pylon`
- **Control server** `apps/pylon/src/node/control-server.ts` (`startControlServer`,
  loopback `127.0.0.1:4716`, bearer-token, schema `openagents.pylon.control.v0.3`).
  Two transports: dev-token `/command` (full verb set) and the capability-gated
  CL-14 `/bridge` (`/bridge`, `/bridge/pair`, SSE `/sessions/:ref/events`).
- **Session lifecycle** `apps/pylon/src/node/control-sessions.ts`: states
  `queued|running|completed|failed|cancelled`; event phases `queued, started,
  composer_event, dev_check_started, completed, failed, cancelled,
  redaction_blocked`; `spawn`/`reply` (reply = a child session with
  `parentSessionRef`, the multi-turn model)/`list`/`events`/`cancel`/`artifact`.
  Executors `codex`, `claude_agent`, `apple_fm`; lanes `auto|local|cloud-gcp|
  cloud-shc` (cloud lanes recorded but fall back to local without cloud config).
  **Session records are in-memory**; only redaction-scanned proof/failure
  artifacts + intents persist to disk.
- **Approvals** `apps/pylon/src/node/approval-queue.ts`: read-first
  `list`/`history`/`resolve`; decisions `approve|deny|answer` (`answer` requires a
  non-empty answer); **exactly-once** via the protocol approval ledger (duplicate
  resolve returns the original with `duplicate:true`).
- **Auto-approval policy** `apps/pylon/src/node/auto-approval-policy.ts`
  (`--on-approval auto`): bounded, fail-closed, allow-list of safe kinds, hard-deny
  patterns (destructive / spend / network-exfil) always win, out-of-scope paths
  escalate/deny, capped count + time window.
- **Control verbs (dev-token):** `session.spawn/reply/list/events/cancel/artifact`,
  `approvals.list/resolve`, `accounts.list`, `wallet.status` (+ spend verbs
  node-only), `assignments.poll/accept`, `deploy.cloud/status`,
  `coordinator.pause/resume/status`, `intent.submit/list`, `apple_fm.status`,
  `apple_fm.session.start`, `bridge.*`. **Bridge (capability-gated):**
  `capability.list`, `decision.resolve` (exactly-once relay), `session.list/
  snapshot/history/subscribe`, `artifact.read`, `session.cancel`.
- **CLI** `pylon sessions <list|spawn|reply|batch|exec|cancel>` with `--adapter`,
  `--objective`, `--verify`, `--worktree`/`--managed-worktree`/`--repo`/`--base-ref`,
  `--lane`, `--concurrency`, `--on-approval manual|deny|auto`,
  `--max-auto-approvals`, `--auto-window-seconds`, `--auto-out-of-bounds`.
  `sessions exec` is the blocking run-to-completion driver.
- **Concurrent spawner** `apps/pylon/scripts/multi-session-run.ts`: JSON plan, one
  isolated workspace per session, bounded concurrency, **account failover**
  (ordered `accountPool` per session + run-level pool, quota-block routing). In-node
  equivalent: `sessions batch`.
- **External-session / sub-agent nesting** `apps/pylon/src/node/external-sessions.ts`
  (+ `codex-sessions.ts`): tails `~/.claude/projects` (and Codex), nests sub-agents
  under `parentRef: claude:<sessionId>`, projects read-only views into
  `session.list`/`events` (`pylonManaged:false`).
- **Autonomous coordinator loop** `apps/pylon/src/coordinator/coordinator-runtime.ts`
  + `planner.ts` + `intent-intake.ts`: durable intent queue
  (`received→planning→fanning_out→shipping→shipped|failed`), `planIntent` splits an
  ask into parts (scope hint or checklist), fans out a session per part in a fresh
  worktree, reconciles to a terminal state. Enabled by default (`OA_COORDINATOR=0`
  to disable); ship step triple-gated (spend-gate eligible **AND** decision auto
  **AND** `OA_SHIP_AUTO_EXECUTE=1`; spend gate defaults to DENY → autonomous ship
  escalates rather than spends). This is the "AFK autonomous loop"
  (`docs/autopilot-coder/2026-06-13-afk-autonomous-loop.md`).
- **Diff review** `apps/pylon/src/tas/diff-review.ts` (ported to the desktop's
  shared `DiffReview` component).

### 1.3 Blueprint + Tassadar (the conversational front)
- **Blueprint** (`apps/openagents.com/workers/api/src/blueprint/`): a real
  DSPy-style typed-contract + governance framework (signatures, module versions,
  typed program runs, repositories, services like `program-run-authority` /
  `continuation-decision` / `release-gate`, fixtures, contract export, HTTP routes).
  Live consumer: `packages/probe/packages/runtime/src/blueprint/signature-lookup.ts`
  (semantic signature selection + tool scoping). **Optimizer/GEPA/RLM is
  schema+design only.**
- **Tassadar modules** (`packages/tassadar-executor/src/dense-weight-module-runtime.ts`,
  `linked-dense-module-runtime.ts`): real exact-execution units with
  replay-verifiable digests, proven against a live execute→replay→settle market
  (`apps/openagents.com/workers/api/src/tassadar-*.ts`). Modules compose **as steps
  inside Blueprint signatures** — there is **no Tassadar inference/serving seam**
  (that framing was explicitly rejected).
- **EPIC #5449 (CLOSED) + #5450–#5456 (CLOSED)** landed the backend seams (module
  registry service, module-as-signature-step binding, chat-program runtime, replay
  signature type) and a **presentational** desktop chat pane. See §3.2 for the
  remaining live-wiring gap.

---

## 2. Desktop pane inventory — full `PaneId` map

`PaneId` union (`apps/autopilot-desktop/src/ui/model.ts:50`) — **14 panes**:
`network, onboarding, builtin-agent, nodes, training, training-fullscreen,
sessions, decisions, spawn, composer, chat, swarm, settings, session-detail`.

Architecture: Foldkit TEA — `model.ts` (state) → `view.ts` (pure render,
`paneView` router at `view.ts:5162`, exhaustive over all 14) → `update.ts`
(reducer) → `commands.ts` (RPC effects) → `bun/*` (node-side handlers); the
webview→Bun contract is `DesktopRPCSchema` in `src/shared/rpc.ts`; inbound
node-state arrives via `subscriptions.ts`.

### 2.1 Per-pane status table

Legend: **BUILT** = full view + wired to runtime + functional · **PARTIAL** = view
exists but missing wiring or partial · **SCAFFOLD-ONLY** = stub view, no wiring ·
**NOT-CONNECTED** = runtime exists but pane doesn't call it · **NOT-BUILT** = no view.

| Pane | Status | Renderer (`view.ts`) | Runtime wiring | Notes |
|---|---|---|---|---|
| `network` | **BUILT** | `networkPane` (5198 immersive shell) | public Worker fetch + `LoadProofReplayBundle` | Default landing: Tassadar proof-replay scene + public-activity strip. Not node-gated. |
| `onboarding` | **BUILT** | `onboardingPane` (5108) | `onboardingStatus`/`identityChoiceState`/`chooseIdentity` | AO-3/AO-4. **Auto-advances to `chat` on completion.** Fail-soft. |
| `builtin-agent` | **BUILT** | `builtInAgentPane` (3823) | `builtinAgentReadiness`/`startBuiltInAgent`/`appleFmReadiness`/`startAppleFmSession`/`promiseSurfacingReadiness` | Hosted-compute + Apple-FM + promise-gap cards; start → `session-detail`. |
| `nodes` | **BUILT** | `nodesPane` (642) | `deployCloud`/`submitIntent`/`resolveApproval` + read projections | Aggregate dashboard: deploy, **ask (intent.submit)**, approvals, balance, assignments, cloud, accounts (read-only), notifications, session preview. |
| `training` | **BUILT** | `trainingPane` (3275) | 7-projection load + plan/activate/reconcile/lease/bootstrap/admit/build verbs; launch/closeout via `submitIntent` | Most heavily-wired pane. Public-safe admin lifecycle. |
| `training-fullscreen` | **BUILT** | `trainingFullscreenPane` (3330) | shared training projections | Immersive 3D (three-effect) over the same data. |
| `sessions` | **BUILT** | `sessionsPane` (3379) | read projection over `modelNode().sessions` + nav | Filter bar; selecting → `session-detail`. No own RPC (correct). |
| `decisions` | **BUILT** | `decisionsPane` (3585) | `resolveApproval` (optimistic) | Approvals queue; sidebar pending badge. |
| `spawn` | **BUILT** | `spawnPane` (4024) | `spawnSession` (+ `validateSpawnRequest`) | codex/claude + lane; no account picker (composer owns that). On success → `session-detail`. |
| `composer` (#5355) | **BUILT** | `composerPane` (4961) | `spawnSession`/`spawnAppleFmSession`, `listManagedAccounts`/`add`/`remove`/`setManagedAccountPriority` | **Most complete day-to-day surface:** spawn → live transcript (polled events) → inline approvals → reply/continue → cancel; codex/claude/apple_fm + per-account picker + lane; `DiffReview` card. |
| `chat` (#5453) | **PARTIAL** | `chatPane` (4908) | `SpawnChatTurn` → `spawnSession` only | **Presentational.** Submitting spawns a bounded session with a Blueprint objective **string**, but the assistant message + Blueprint step verdicts are **synthesized locally** from hardcoded refs/digests (`blueprintChatScopedSteps`, `model.ts:610–749`); on `SucceededChatTurn` the Tassadar step is set `"verified"` **regardless of real output** (`update.ts:1891`). No code reconciles `chatSessionRef` events back into the chat steps despite the status text "node-state poll will stream the turn." **This is the central chat gap.** |
| `swarm` (#5362) | **BUILT** | `swarmPane` (3534) | pure projection over `modelNode` sessions/accounts/events; per-cell open-in-composer / cancel; top roll-up → `decisions` | Grid over N concurrent sessions. No new verb (by design). |
| `settings` | **PARTIAL** | `settingsPane` (4148) | `installReadiness` (live) + connection read | Install-readiness + connection are live; **Theme / Notifications / Updates are static informational cards** (no controls). No keybinding/command settings. |
| `session-detail` | **BUILT** | `sessionDetailPane` (4256) | `cancelSession`, reads events/artifacts | Event timeline (expand), lane provenance, verify line, artifact line, `DiffReview`, cancel. Leaf pane (not in NAV). |

**Net:** 11 BUILT, 2 PARTIAL (`chat`, `settings`), 1 BUILT-leaf (`session-detail`).
**No SCAFFOLD-ONLY, NOT-CONNECTED, or NOT-BUILT panes.** Every PaneId has a real
renderer and router case. This is the corrected, current state.

### 2.2 What's BUILT on the runtime but NOT (or thinly) surfaced in desktop

These are the real "built but not wired into the UI" items the task targets:

| System | Runtime state | Desktop state | Verdict |
|---|---|---|---|
| Blueprint signature selection + program run | BUILT (`signature-lookup.ts`, Blueprint services) | chat sends an objective **string**; no signature/program/step round-trip | **NOT-CONNECTED** (chat is seeded, §3.2) |
| Tassadar module-as-step execution + replay verdict | BUILT (registry service #5451, binding #5450, runtime #5452) | chat step shows hardcoded digest, always "verified" | **NOT-CONNECTED** |
| Autonomous coordinator loop (intent → plan → fanout → reconcile → ship) | BUILT (`coordinator-runtime.ts`, default-on) | only `intent.submit` "ask" card (nodes) + header pause/resume + `coordinator.status` read | **PARTIAL** — no first-class loop view (queue, per-intent plan, fanned children, ship gate state) |
| `--on-approval auto` bounded auto-approval | BUILT (`auto-approval-policy.ts`) | manual approve/deny only in UI | **NOT-CONNECTED** — no per-session auto-approve toggle / audit view |
| `session.reply` continuation (multi-turn) | BUILT (child-session model) | composer reply uses it; chat does not; no thread history surface beyond composer | **PARTIAL** |
| `sessions batch` / concurrent spawner / account failover | BUILT (`multi-session-run.ts`, failover routing) | swarm shows running sessions; **no batch launch UI, no failover/routing view** | **PARTIAL** |
| Artifacts / receipts depth | BUILT (`session.artifact`, proof/failure JSON) | session-detail shows a single artifact line | **PARTIAL** — no artifact/receipt browser |
| Diff fidelity | BUILT (`tas/diff-review.ts`) | `DiffReview` card renders a derived ChangeSet | **PARTIAL** — text/patch level, no per-hunk / file-tree / side-by-side |
| External-session / sub-agent nesting | BUILT (`external-sessions.ts`, `parentRef`) | merged into `session.list`; swarm/sessions render flat | **PARTIAL** — nesting not visualized as a tree |
| Worktree / repo entry | BUILT (protocol `worktreePath`/`workspaceRef`, `workspace-materializer`) | composer/spawn take objective; **no repo/worktree picker** | **PARTIAL** |
| Live SSE event stream | BUILT (`/sessions/:ref/events`) | desktop **polls** node-state (`node-state-poll.ts`) | **PARTIAL** — works, but polled not streamed |

---

## 3. The two PARTIAL panes, in detail

### 3.1 `settings` — informational, not functional
`settingsPane` (`view.ts:4148`) renders live install-readiness + connection, but
Theme, Notifications, and Updates are **static copy** with no controls, and there
is **no place to configure** keybindings, command-palette behavior, default
adapter/lane, coordinator policy, or account defaults. As navigation grows (§5),
settings becomes the home for those preferences.

### 3.2 `chat` — presentational, must be made live
The chat pane (`chatPane`, `view.ts:4908`) has a **genuinely rich view** —
Blueprint program-step chips (signature / tool-scope / Tassadar module step /
replay module), verdict badges, and a proof-replay preview. But the data behind it
is **seeded**:
- The steps come from `blueprintChatScopedSteps` (`model.ts:637`) built from
  hardcoded constants (`BLUEPRINT_CHAT_SIGNATURE_REF`,
  `BLUEPRINT_CHAT_TASSADAR_DIGEST_REF = "sha256:0caa43…"`, etc., `model.ts:610–635`).
- `ClickedChatSubmit` (`update.ts:1845`) builds a Blueprint objective **string** and
  calls `SpawnChatTurn` → `spawnSession`. On `SucceededChatTurn` (`update.ts:1891`)
  it appends an assistant message whose Tassadar step is `status:"verified",
  verdict:"verified"` — **the moment a session ref exists, not when anything is
  actually verified.**
- Nothing links the spawned `chatSessionRef`'s live events back into
  `chatMessages`. The status line "node-state poll will stream the turn" describes
  behavior that isn't wired.

So #5449's children closed with the **backend seams real** (module registry, the
module-as-step binding, the chat-program runtime, the replay signature type) and a
**presentational pane that demos the shape**. The remaining work is to drive the
pane from the real Blueprint chat-program runtime: semantic signature selection
→ program run → real Tassadar-module step execution → real replay verdict →
reconcile into the rendered steps. That is the load-bearing connection issue in
this roadmap (BP-1…BP-3 in §6).

---

## 4. Navigation & command surface — current state

- **Sidebar `NAV`** (`view.ts:294`): a **flat 13-button list** — Composer, Chat,
  Get started, Network, Agent, Nodes, Training, Training Live, Sessions, Swarm,
  Decisions, Spawn, Settings. `decisions` carries a pending-approvals badge; the
  header has a coordinator pause/resume toggle + status line.
- **Every pane is reachable**; only `session-detail` is absent (intentional leaf).
- **No command palette. No keyboard shortcuts. No grouped menu.** A full-tree grep
  for `keydown|keyboard|shortcut|hotkey|cmd+|ctrl+|command-palette|accelerator`
  over `apps/autopilot-desktop/src` returns **zero hits**. `subscriptions.ts`
  registers only the inbound node-state stream — no keyboard subscription.

This flat 13-button wall **is** the "UI clutter" the task wants solved. Adding more
top-level buttons for the unsurfaced systems in §2.2 would make it worse. The fix
is structural (§5), not additive.

---

## 5. Target architecture + clean navigation design

### 5.1 What a full coding agent needs (the target)
A single Autopilot Desktop window where, past onboarding, you can:
1. **Talk to it** — a Blueprint-driven **chat** is the default conversational front
   (real signature selection, real Tassadar-module steps with inline replay
   receipts); the chat can hand off to a coding composer.
2. **Code with it** — the **composer** loop (objective + repo/worktree + adapter +
   account + lane → live streamed transcript → inline approvals → reply/continue →
   cancel → diff) is the day-to-day CLI replacement.
3. **Run many at once** — the **swarm** grid over N concurrent sessions, with
   batch launch, account-failover/routing visibility, and sub-agent nesting shown
   as a tree.
4. **Supervise it** — a unified **approvals/decisions** roll-up with optional
   bounded **auto-approve** (the `--on-approval auto` policy) surfaced honestly.
5. **Let it run AFK** — a first-class **Autonomous loop** view: the intent queue,
   each intent's plan, its fanned children, reconcile state, and the (default-DENY)
   ship gate.
6. **Inspect what it did** — richer **diff / transcript / artifacts / receipts**.
7. Keep **training / network** scenes alongside (the current foreground).

All of this stays inside the yellow, local-only promise
`autopilot.desktop_gui_client.v1` (observe-and-bounded-steer); cloud lanes remain
the separate red promise `autopilot.cloud_coding_sessions.v1`.

### 5.2 Clean navigation — the explicit anti-clutter design
Replace the flat 13-button wall with **three tiers**:

1. **A small primary sidebar (≈5 grouped destinations), not 13 buttons.** Group the
   panes into a few intent-named sections, each opening to its sub-panes:
   - **Chat** (the default post-onboarding home) — Blueprint chat.
   - **Code** — Composer · Swarm · Sessions · Spawn (the active coding cluster).
   - **Supervise** — Decisions/Approvals · Autonomous loop · Accounts.
   - **Explore** — Network · Training · Training Live (the scenes; immersive).
   - **Settings** (incl. the new keybindings/command + defaults).
   `session-detail` stays a leaf. The sidebar shows ~5 items; sub-panes appear as a
   secondary in-section tab strip or list, so the top level never grows past the
   group count.
2. **A command palette (Cmd-K)** — the primary "everything" surface, so depth lives
   in search, not buttons. Fuzzy over: navigate-to-pane, spawn a session, open a
   session, resolve the next approval, submit an intent, pause/resume the
   coordinator, toggle auto-approve, open a replay. This is how you expose the full
   §2.2 capability set **without adding sidebar buttons**.
3. **Keyboard shortcuts** — a small, discoverable set (Cmd-K palette, Cmd-Enter
   submit in chat/composer, j/k + Enter through approvals/sessions, Cmd-1..5 to the
   primary groups), listed in Settings.

**Anti-clutter rules** (apply to every connection issue below):
- Do not add a new top-level sidebar button per system; route new capability into a
  **group**, the **command palette**, or an existing pane's secondary strip.
- Prefer surfacing a capability inside the pane it belongs to (e.g. auto-approve in
  the approvals roll-up; batch launch in swarm) over a new pane.
- Keep immersive scenes (network/training-fullscreen) chrome-light as they are.

### 5.3 How the connections map onto the substrate (build = mostly wiring)
- **Chat live** = drive `chatPane` from the real Blueprint chat-program runtime
  (#5452) via `signature-lookup` (semantic, no keyword routing) → program run →
  Tassadar module step (#5450) resolved from the registry (#5451) → replay verdict
  (#5456) → reconcile `chatSessionRef` events into the rendered steps. Replace the
  seeded `blueprintChatScopedSteps`.
- **Autonomous loop view** = read `intent.list` + `coordinator.status` (BUILT) and
  render the queue/plan/children/ship-gate; reuse the header pause/resume.
- **Auto-approve** = expose `--on-approval auto` policy state per session in the
  approvals roll-up; surface the bounded audit trail (`autoApprovals[]`).
- **Batch / failover** = drive `sessions batch` + render the failover routing
  (`accountPool`, quota-block reasons) in swarm.
- **Diff/artifacts depth** = extend `DiffReview` (file tree / hunks) and add an
  artifact/receipt browser over `session.artifact`.
- **Sub-agent tree** = use `parentRef` from `external-sessions` to nest in
  swarm/sessions.
- **Live SSE** (optional, perf) = consume `/sessions/:ref/events` instead of/in
  addition to polling.
- **Repo/worktree picker** = expose `worktreePath`/`managed-worktree` in composer.

---

## 6. Roadmap — EPIC + child issues

All issues created with the `roadmap` label, neutral metadata, **no GitHub
Actions**, and an explicit "don't clutter the UI" UX constraint where relevant.
Created issue numbers are listed in §6.1 after creation.

Structure:
- **Top EPIC** — Autopilot Desktop → full coding agent.
- **Sub-EPIC: Navigation shell + command palette** (the anti-clutter foundation;
  most UI connections depend on it).
- **Per-system connection issues** — chat-live, autonomous-loop, auto-approve,
  swarm/batch/failover, diff/artifacts, sub-agent tree, repo picker, settings.
- **Cross-references** — Blueprint × Tassadar chat EPIC #5449 (closed; the live
  wiring continues here), auto-onboarding EPIC #5441 (the live-node/packaging
  dependency), yellow promise `autopilot.desktop_gui_client.v1`.

### 6.1 Created issues

**Top EPIC**
- **#5461** — EPIC: Autopilot Desktop → full coding agent.

**Navigation sub-EPIC (anti-clutter foundation)**
- **#5462** — Sub-EPIC: navigation shell + command palette.
  - **#5463** — group the flat 13-button sidebar into ~5 grouped sections.
  - **#5464** — command palette (Cmd-K) over a typed command registry.
  - **#5465** — keyboard shortcut layer + Settings shortcut listing.

**Per-system connection issues**
- **#5466** — Chat-live: drive the desktop chat pane from the real Blueprint
  chat-program runtime (continues #5449).
- **#5467** — Autonomous-loop view: surface intent → plan → fanout → reconcile →
  ship gate.
- **#5468** — Bounded auto-approve surface in the approvals roll-up
  (`--on-approval auto`).
- **#5469** — Swarm: batch launch + account-failover/routing visibility +
  sub-agent tree.
- **#5470** — Diff/transcript fidelity + artifact & receipt browser.
- **#5471** — Repo / worktree picker in composer (give it a repo + a task).
- **#5472** — Settings: functional preferences (defaults/theme/notifications) +
  shortcut listing.

**Cross-references (not duplicated)**
- **#5449** — Blueprint × Tassadar chat EPIC (CLOSED; #5466 continues its desktop
  live-wiring).
- **#5441** — Autopilot Desktop auto-onboarding EPIC (OPEN; the live-node +
  packaging dependency that keeps coding panes dark on a fresh install).

### 6.2 Dependency shape

- #5466 (chat-live) depends on the closed #5450/#5451/#5452/#5456 backend seams;
  its UI lands in the concurrently-owned chat pane — **coordinate, continue, do
  not rewrite**.
- #5464 (palette) + #5465 (shortcuts) layer on #5463 (nav shell); the three nav
  children land together as the anti-clutter foundation.
- #5467/#5468/#5469/#5470/#5471 each route their surface into a nav **group** or
  an existing pane (per #5463), the **command palette** (#5464), or a pane's
  secondary strip — never a new top-level button.
- #5472 (settings) consumes the shortcut source from #5465 and the default
  adapter/lane/account it defines are read by composer/chat/spawn.
- All coding panes remain dark on a fresh install until the live-node + packaging
  work in #5441 lands; this roadmap is provable today against a source-checkout
  `pylon dev` node.

---

## 7. Concurrency & scope guardrails (honest)

- This audit **wrote only this doc** and **created GitHub issues**. It changed no
  app code.
- A concurrent agent owns the **Blueprint chat pane** in `apps/autopilot-desktop`
  + `workers/api/src/blueprint`; another owns **`workers/api` payout paths**. The
  roadmap issues here are scoped so the desktop chat-live work (§3.2/§6) is framed
  as **continuing** the existing #5449 effort (not a parallel rewrite), and **no**
  issue touches payout paths. Implementers must coordinate on `src/ui/*` since it
  is a shared, hot directory.
- Marked-unverified items: per-handler internal correctness in `bun/index.ts`
  (verb names confirmed present, not line-audited); the per-tool turn loop inside
  `claude-agent-executor.ts` / `codex-agent-executor.ts` (deepest layer, not fully
  read); `decision-broadcast.ts` / `decision-consistency.ts` (not fully read).

---

## 8. Key file references

- Desktop: `apps/autopilot-desktop/src/ui/{model,view,update,commands,message,
  subscriptions,initial-state}.ts`, `src/shared/{rpc,proof-replays,
  onboarding-status}.ts`, `src/bun/{index,pylon-control,node-state-poll,
  node-launcher}.ts`, `apps/autopilot-desktop/AGENTS.md`.
- Runtime: `packages/probe/packages/runtime/src/{cli,permission,file-mutation}.ts`,
  `apps/pylon/src/node/{control-server,control-sessions,approval-queue,
  auto-approval-policy,external-sessions,codex-sessions}.ts`,
  `apps/pylon/src/coordinator/{coordinator-runtime,planner}.ts`,
  `apps/pylon/src/node/intent-intake.ts`, `apps/pylon/scripts/multi-session-run.ts`,
  `apps/pylon/src/tas/diff-review.ts`,
  `apps/pylon/src/{codex-agent-executor,claude-agent-executor}.ts`.
- Contract / shared UI: `packages/autopilot-control-protocol/src/control.ts`,
  `packages/autopilot-ui/src/index.ts`.
- Blueprint / Tassadar: `apps/openagents.com/workers/api/src/blueprint/`,
  `packages/probe/packages/runtime/src/blueprint/signature-lookup.ts`,
  `packages/tassadar-executor/src/{dense-weight-module-runtime,
  linked-dense-module-runtime}.ts`,
  `apps/openagents.com/workers/api/src/tassadar-*.ts`.
- Prior audits: `docs/launch/2026-06-18-autopilot-desktop-coding-surface-audit.md`,
  `docs/launch/2026-06-18-autopilot-tassadar-chat-blueprint-audit.md`,
  `docs/launch/2026-06-18-blueprint-tassadar-chat-delegation.md`,
  `docs/autopilot-coder/2026-06-13-afk-autonomous-loop.md`,
  `docs/autopilot-coder/2026-06-13-autopilot-desktop-app-audit.md`.
- Promise: `apps/openagents.com/workers/api/src/product-promises.ts`
  (`autopilot.desktop_gui_client.v1` ~`:2069`, `autopilot.cloud_coding_sessions.v1`
  ~`:2102`).
- EPICs: #5449 (Blueprint × Tassadar chat, closed), #5441 (auto-onboarding, open).
