# Autopilot Desktop Coding Surface — Audit + Gap Analysis

Date: 2026-06-18
Scope: `apps/autopilot-desktop` (Electrobun), `packages/probe`, `apps/pylon`,
`packages/autopilot-control-protocol`, `packages/autopilot-ui`, and the
`docs/autopilot-coder/` audit set.
Type: audit + doc only. No feature build.

## The vision (the target we gap-analyze against)

Day-to-day coding should happen inside the **one** Autopilot Desktop (Electrobun)
app as the single surface — replacing direct use of the Claude Code and Codex
CLIs. That one window should expose everything the runtime layer has been built
to do:

- multiple coding-agent runtimes (Codex, Claude, local Apple FM) and lanes
  (local / cloud-gcp / cloud-shc), and a swarm/multi-session view;
- multiple provider accounts with explicit account management;
- live agent coding sessions with a transcript / turn / diff view;
- approvals (decision cards) in-window;
- a real "give it a repo + a task" entry point — the actual CLI replacement;
- the training run visualization (what is most visible today) staying alongside.

The honest one-line verdict appears in §"Is day-to-day coding in the app real
yet" below.

## What the desktop app actually is today

`apps/autopilot-desktop` is a Foldkit (Effect TEA) webview in an Electrobun
shell. The **Bun main process** (`src/bun/`) owns the control token, discovers
or launches a local Pylon node, polls it, and exposes a typed RPC bridge; the
**webview** (`src/ui/`) is a pure `Model`/`Message`/`update`/`view` app that
renders public-safe projections and dispatches scoped commands. The webview→Bun
contract is `DesktopRPCSchema` in `apps/autopilot-desktop/src/shared/rpc.ts`, and
the shared protocol types come from
`@openagentsinc/autopilot-control-protocol`.

The webview is materially richer than "only the training run." The pane router
(`src/ui/view.ts:3890` `paneView`, sidebar `NAV` at `view.ts:187`) renders:

`network` (default landing), `builtin-agent` (Agent), `nodes`, `training`,
`training-fullscreen` (Training Live), `sessions`, `decisions`, `spawn`,
`settings`, and `session-detail`.

So Sessions, Decisions/Approvals, Spawn (with a Codex/Claude adapter toggle and
an auto/local/cloud-gcp/cloud-shc lane picker), and a Session-detail event
timeline with Cancel **all exist as UI** and dispatch real RPC verbs
(`spawnSession`, `cancelSession`, `resolveApproval`, `submitIntent`).

### Why it "only shows the training run" in practice

Two structural facts explain the owner's perception:

1. **The default landing pane is `network`** — an immersive, full-screen
   `three-effect` proof-replay scene with no sidebar chrome
   (`view.ts:3918` `rootView`; `initial-state.ts` sets `pane: "network"` and
   boots only `LoadInstallReadiness` + `LoadProofReplayBundle`). The Training and
   Training Live panes are the next most-polished surfaces. The coding panes are
   one sidebar click away but are not the first thing rendered.

2. **Every coding pane is gated on a connected local Pylon node.** Sessions,
   Decisions, Session-detail, and the spawn result all read `modelNode(model)`,
   which is `null` until the Bun poller reaches a running control server. In a
   fresh **packaged** install with no running node, those panes render
   "Connecting…" / "No sessions" indefinitely (see the bundled-node gap below).
   So the only surfaces that show real content with no local node are the
   network/training scenes, which fetch public Worker/replay data directly.

The result: the coding surface is *built and wired to a contract*, but it has
never been the app's foreground, and it is dark unless a local node is up.

## The runtime underneath (what the desktop should drive)

There are **two** "Probe" runtimes; the desktop drives the second.

- `packages/probe/packages/runtime` (`@openagentsinc/probe-runtime`) is the
  original TUI coding agent: a 2k-line `cli.ts`, an LLM tool runtime
  (`src/llm/tool-runtime.ts`), and a multi-account provider contract
  (`src/contracts/provider-account.ts`, providers `chatgpt_codex` + `google_gemini`
  with real lease-selection and secret-redaction logic). It has **no control
  server**, and its permission handler is an explicit stub that always allows
  (`src/permission.ts`); it has **no transcript store**.

- `apps/pylon` bundles the active fork as `@openagentsinc/pylon-runtime` and is
  where the real driveable coding loop lives:
  - loopback control server `apps/pylon/src/node/control-server.ts`
    (`startControlServer`, bearer-token gated, `/command` + `/events` SSE);
  - session lifecycle `apps/pylon/src/node/control-sessions.ts`
    (`ControlSessionActions`: `spawn`/`list`/`cancel`/`events`);
  - real executors: `apps/pylon/src/codex-agent-executor.ts` (`runWithCodexSdk`,
    gated), `apps/pylon/src/claude-agent-executor.ts` (`runWithClaudeAgentSdk`,
    gated/refusable), `apps/pylon/src/node/apple-fm-local-session.ts`;
  - approvals with exactly-once `apps/pylon/src/node/approval-queue.ts`;
  - external/host session nesting `apps/pylon/src/node/external-sessions.ts`
    (`ExternalAgentKind = "claude" | "codex"`, `parentRef` nesting);
  - multi-session driving via `apps/pylon/scripts/multi-session-run.ts` and the
    runbook `docs/autopilot-coder/pylon-multi-session-agent-runbook.md`.

The contract the desktop imports is `packages/autopilot-control-protocol`
(`src/control.ts`, schema tag `openagents.pylon.control.v0.3`): `Adapter =
["codex","claude_agent","apple_fm"]`, `SessionSummary`, `SpawnCommand`,
`SessionEvent` (approvals modeled as `decision_requested`/`decision_resolved`).

Crucially, the **interactive composer coding loop** (type a prompt → Codex/Claude
edits the repo → focused checks → patch summary → reload/continue) lives in the
Pylon TUI / `pylon dev` (per `docs/autopilot-coder/2026-06-12-pylon-codex-day-to-day-readiness-audit.md`,
"Done in source": `codex-composer.ts` #4839, supervised danger mode #4840,
`pylon context/dev doctor/check/apply/reload`). It is **not** surfaced in the
Electrobun window — the desktop dispatches *bounded* spawn/cancel/approve, not an
interactive composer.

## The terminal-agent-systems well (#5107 / I1–I43)

The I1–I43 bucket (#5198–#5281, completing epic #5107 alongside G1–G7 / H1–H6)
is **fully landed but lives entirely in the web app**
(`apps/openagents.com/apps/web/src/page/loggedIn/autopilot-work/*.ts`). Each
lane is a **refs-only read projection** plus a render lane in the web `/autopilot`
Run-detail cockpit, with **no new runtime, execution, approval, mutation, or
settlement authority** by design. `rg "autopilot-desktop"` across
`docs/autopilot-coder/terminal-agent-systems/` returns nothing: none of these
lanes target the Electrobun app, and the desktop does not consume any `Forge*View`
projection. So "terminal-agent-systems is implemented" is true *as web read
projections* — it is not desktop coding capability and not new runtime.

## Gap table — BUILT (runtime) vs CONNECTED (desktop UI) vs MISSING

Legend: BUILT = runtime/backend exists; CONNECTED = a real desktop UI surface
dispatches/render it; MISSING = no real surface.

| # | Surface | BUILT (runtime / backend) | CONNECTED to desktop (UI) | Verdict |
|---|---------|---------------------------|---------------------------|---------|
| 1 | Agent coding sessions | Yes — Pylon `control-sessions.ts` spawn/list/cancel/events; executors for codex/claude/apple_fm; exactly-once approvals | Yes — `sessions`/`session-detail` panes (`view.ts:3084`,`:3814`) render `SessionList` + event timeline; `spawnSession`/`cancelSession` RPC wired | **Ready-to-wire / wired but node-gated.** Surfaces exist; dark without a live local node. No interactive composer loop in-window. |
| 2 | Multi-account management | Yes — `provider-account.ts` contract + lease selection + redaction; per-session `--account-ref` (#4868); concurrent spawner (#4869) | Read-only only — `AccountList` via `accountsSection` (`view.ts:441`) shows provider/ready from node-state; **no add/select/priority/quota management UI** | **Partial — backend built, desktop is read-only.** No account CRUD or per-session account picker in the app. |
| 3 | Provider / runtime picker (Codex/Claude/local/swarm) | Codex, Claude, Apple FM all built (executors above); "swarm" is not a runtime concept (no `swarm` in `apps/pylon/src`) | Partial — Spawn pane has a Codex/`claude_agent` adapter toggle + lane picker (`view.ts:3582`); Apple FM is a separate Agent-pane card (`view.ts:3479`), not a spawn option; **no swarm option** | **Partial.** Codex/Claude are pickable for spawn; Apple FM is its own flow; local-vs-cloud is a lane toggle; swarm absent. |
| 4 | Swarm view | Multi-session exists (concurrent spawner #4869, control `session.list`, external-session `parentRef` nesting); no first-class "swarm" abstraction | **No.** No swarm/fanout/lane-grid view; `rg "swarm\|fanout\|multi-agent"` over `apps/autopilot-desktop/src` returns nothing | **MISSING in desktop.** Backend can run N sessions; the app has no orchestration/swarm surface. |
| 5 | Approvals UI | Yes — `approval-queue.ts` exactly-once `list`/`resolve`; protocol `decision_requested`/`decision_resolved` | Yes — `decisions` pane + `approvalsCard` (`view.ts:390`,`:3143`); `resolveApproval` RPC; sidebar pending badge | **Wired (node-gated).** Approve/Deny exists; depends on a live node emitting decisions. |
| 6 | Transcript / turn / diff view | Partial — Pylon emits `SessionEvent` phases + bounded recent-events tail + artifact stats; diff-review lives in `apps/pylon/src/tas/diff-review.ts`; original Probe has no transcript store | Partial — `session-detail` shows an event timeline + click-to-expand + artifact line (`view.ts:3789`); **no diff/patch viewer, no live streaming token view** | **Partial.** Event/artifact timeline yes; rich diff/turn transcript no. |
| 7 | Day-to-day repo/task entry (the real CLI replacement) | The interactive composer loop is BUILT in Pylon TUI/`pylon dev`, not in a contract verb | Bounded only — Spawn pane sends one objective+verify via `spawnSession`; **no repo selection, no interactive composer/reply loop, no apply/reload in-window** | **MISSING as a CLI replacement.** The app can fire-and-forget a bounded session; it cannot host the iterative coding loop that replaces Codex/Claude Code CLI. |
| 8 | Training run (shows today) | Yes — public Worker projections + `three-effect` scene | Yes — `network` (default), `training`, `training-fullscreen` panes; the most polished surfaces | **Stays alongside.** This is the current foreground and should remain. |

Supporting shared assets already present: `@openagentsinc/autopilot-ui` exports
`AccountList`, `SessionList`, decision/steer/verify/artifacts/assignments/
cloud-quota/node-status components and dark tokens (`packages/autopilot-ui/src/index.ts`)
— a real reuse surface for any new wiring.

## Promise scope: `autopilot.desktop_gui_client.v1`

Defined in `apps/openagents.com/workers/api/src/product-promises.ts:2069`, state
**`yellow`**, scope explicitly **local-only, observe-and-bounded-steer**:
"a GUI client for observing and steering local Autopilot coding sessions …
local-only … cloud-lane sessions, remote/Tailnet control, full TUI parity, and
pricing/distribution are not wired or decided." authorityBoundary: "a
view-and-bounded-action client; it cannot supervise the Pylon node, reach
remote/cloud nodes, deploy, or mutate repository/provider access." Blockers:
`autopilot_desktop_live_runtimes_not_wired`,
`autopilot_desktop_remote_cloud_lane_not_wired`,
`autopilot_desktop_pricing_distribution_undecided`. Cloud coding sessions are a
separate **red** promise `autopilot.cloud_coding_sessions.v1`
(product-promises.ts:2102).

Net: the *vision* in this audit (the desktop as the day-to-day CLI replacement)
is **deliberately broader than the current yellow promise**. Surfaces 1–7 above
must not be described as GA; they sit inside / beyond a yellow, local-only
promise.

## Is day-to-day coding in the app real yet? — Verdict

**No — not as a Claude Code / Codex CLI replacement.** What is real today is a
yellow, local-only *observe-and-steer cockpit*: the runtime to run Codex/Claude/
Apple FM sessions, approvals, multi-account, and multi-session all exist in
Pylon, and the desktop already has Spawn / Sessions / Decisions / Session-detail
panes wired to the typed control protocol. But three things keep it from being
the single coding surface:

1. **No interactive composer loop in-window.** The day-to-day "type a request →
   agent edits the repo → checks → patch → reload/continue" loop lives in the
   Pylon TUI / `pylon dev`, not the Electrobun webview. The Spawn pane is
   fire-and-forget bounded sessions, with no repo picker, no reply turn, no
   diff/apply view.
2. **The coding panes are node-gated and not the foreground.** Default landing is
   the network/training scene; coding panes are dark until a local node is up.
3. **Packaged installs cannot reliably bring up that node yet.** The bundled
   headless Pylon node + launcher exist in source (#5011/#5025/#5027,
   `src/bun/node-launcher.ts`), but the shipped `.app` is **unsigned/un-notarized**;
   signing + notarization + a clean-machine first-run→node-online smoke is the
   live blocker (`docs/launch/JUNE15_LAUNCH_PLAN.md`).

## Biggest gap

**There is no interactive coding composer surface in the Electrobun window.** The
control protocol exposes bounded `session.spawn`/`cancel`/`events` and approvals,
but not the iterative composer turn loop (prompt → agent reply → tool/diff →
approve/continue/reload) that actually replaces a CLI. That loop is built in
Pylon's TUI and must be projected into a desktop pane (composer + streaming
transcript + diff + inline approvals + reply) before the app can be the
day-to-day coding surface. Everything else (multi-account UI, swarm view, cloud
lane) is secondary to this.

## Recommended first surface to wire

**A "Session" / coding composer pane built on the existing control protocol —
spawn → live streamed transcript → inline approvals → reply/continue → cancel —
driven against a local `pylon dev` node.**

Rationale: it is the highest-leverage, lowest-new-contract step. The protocol
verbs and SSE event stream already exist (`session.spawn`/`events`,
`decision_requested`/`decision_resolved`); the shared `SessionList`, decision,
and verify components already exist in `@openagentsinc/autopilot-ui`; and it
turns the existing Spawn + Session-detail panes from fire-and-forget into a real
loop. It can be proven against a source-checkout `pylon dev` node today, before
the packaged signing blocker is resolved.

## Sequenced wiring plan

Each phase notes what to **reuse** vs. **build**, and respects the yellow,
local-only scope of `autopilot.desktop_gui_client.v1`.

1. **Composer turn loop (first surface).**
   Reuse: `session.spawn`/`events` SSE, `approval-queue`, `autopilot-ui`
   `SessionList`/decision/verify components, the existing `session-detail`
   timeline. Build: a streaming transcript subscription in `src/bun/` that tails
   `/events` per session and pushes `SessionEvent`s as `Message`s; a composer
   pane (objective + reply turns) and a follow-up/continue verb if the protocol
   needs one. Prove against `pylon dev`. Keep diff rendering minimal first
   (text/patch), then port a richer diff view.
   Biggest gap closed: #7 (CLI replacement loop), strengthens #1 and #6.

2. **Repo / workspace entry.**
   Build a repo/worktree selector in the composer (the protocol already carries
   `worktreePath`/`workspaceRef`; Pylon `workspace-materializer.ts` does the
   `git_checkout`). Makes "give it a repo + a task" first-class.

3. **Provider/account picker for spawn.**
   Reuse the built per-session account selection (#4868, `--account-ref`) and the
   `provider-account` contract. Build a desktop account-management surface
   (add/select/priority/quota) on top of the read-only `AccountList`. Add Apple FM
   as a spawn adapter option alongside Codex/Claude rather than a separate card.
   Closes #2 and finishes #3.

4. **Swarm / multi-session view.**
   Reuse the concurrent spawner (#4869) and `session.list` + `parentRef` nesting.
   Build a lane/grid view over N concurrent sessions (status, account, repo,
   approvals roll-up). Closes #4.

5. **Diff / turn fidelity + transcript persistence.**
   Port `apps/pylon/src/tas/diff-review.ts` output into a desktop diff viewer; add
   a durable session-record read surface. Finishes #6.

6. **Cloud / remote lane (separate promise).**
   Only after 1–5 and explicitly under `autopilot.cloud_coding_sessions.v1`
   (red): broker desktop → cloud-gcp through Pylon as the single front door, per
   the recommended path in
   `docs/autopilot-coder/2026-06-14-cloud-desktop-mobile-coding-sessions-full-flow-audit.md`
   §5. Do not widen the yellow desktop promise to cover this.

Parallel, non-blocking: resolve the packaged-node signing/notarization blocker
(#5027) so the composer loop works from a downloaded build, not just a
source checkout.

## Source references

- Desktop app: `apps/autopilot-desktop/src/ui/{model,message,view,bridge,initial-state}.ts`,
  `apps/autopilot-desktop/src/shared/rpc.ts`,
  `apps/autopilot-desktop/src/bun/{node-launcher,pylon-control,node-state-poll}.ts`,
  `apps/autopilot-desktop/README.md`, `apps/autopilot-desktop/AGENTS.md`.
- Runtime: `packages/probe/packages/runtime/src/{cli,permission}.ts`,
  `packages/probe/packages/runtime/src/contracts/provider-account.ts`,
  `apps/pylon/src/node/{control-server,control-sessions,approval-queue,external-sessions}.ts`,
  `apps/pylon/src/{codex-agent-executor,claude-agent-executor}.ts`,
  `apps/pylon/src/node/apple-fm-local-session.ts`,
  `apps/pylon/scripts/multi-session-run.ts`.
- Contract / shared UI: `packages/autopilot-control-protocol/src/control.ts`,
  `packages/autopilot-ui/src/index.ts`.
- terminal-agent-systems: `docs/autopilot-coder/terminal-agent-systems/2026-06-16-forge-autopilot-coder-systems-roadmap.md`,
  `apps/openagents.com/apps/web/src/page/loggedIn/autopilot-work/*.ts`.
- Promise + audits: `apps/openagents.com/workers/api/src/product-promises.ts` (`:2069`, `:2102`),
  `docs/autopilot-coder/2026-06-13-autopilot-desktop-reality-vs-claim-status.md`,
  `docs/autopilot-coder/2026-06-12-pylon-codex-day-to-day-readiness-audit.md`,
  `docs/autopilot-coder/2026-06-14-cloud-desktop-mobile-coding-sessions-full-flow-audit.md`,
  `docs/autopilot-coder/2026-06-13-programmatic-multi-account-pylon-audit.md`,
  `docs/autopilot-coder/pylon-multi-session-agent-runbook.md`,
  `docs/launch/JUNE15_LAUNCH_PLAN.md`.
</content>
