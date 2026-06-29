# Orca orchestrator adaptation report — what to port into OpenAgents

- **Date:** 2026-06-27
- **Author:** ADE architecture review (Claude, opus-4.8 1M)
- **Subject repo (reference, read-only):** `stablyai/orca` — "The AI Orchestrator for 100x builders." Runs Codex / Claude Code / OpenCode / Pi / Grok / Cursor / Gemini / +30 more CLI agents side by side, each in its own git worktree, tracked in one desktop app, with a Swift/Expo mobile companion to monitor and steer agents.
- **Local clone reviewed:** `/Users/christopherdavid/work/projects/repos/orca/` @ `3ce184ca1` (v1.4.103-rc.3)
- **Owning repo for any ports:** `OpenAgentsInc/openagents`
- **Status of Orca in our tree:** External reference under `projects/repos/` — **study and adapt patterns, do not vendor code**. Orca is MIT licensed (see License note at the end).

> Discipline carried through this doc: ports must prefer Cloudflare primitives (Workers / Durable Objects / D1 / Queues), Bun + Effect + Foldkit on the web app, and native SwiftUI on mobile. We do **not** adopt Orca's Electron shell, embedded Chromium, xterm/Ghostty terminal, or native computer-use modules. The valuable, portable assets are the **abstractions and state models**, not the desktop runtime.

---

## 1. Executive summary

Orca is, almost line-for-line, the locally-installed Electron version of what we are building as a distributed, Cloudflare-mediated service: a system that fans one objective across N coding agents, each isolated in its own git worktree, tracks all of them in one place, and lets an operator review diffs and steer agents from a phone. The overlap with our active work is striking:

| Orca capability | Our active equivalent |
| --- | --- |
| Worktree-per-agent, fan-out across N agents | `codex-supervisor.sh` / `claude-supervisor.sh` N-worker pools + `workspace-materializer.ts` |
| Multi-runner abstraction (Codex/ClaudeCode/OpenCode/Pi interchangeable) | `codex-agent-executor.ts` + `claude-agent-executor.ts` (two parallel executors, not yet unified) |
| Desktop dashboard tracking N agents, diffs, status, steering | Operator dashboard (#6401-6403, designed, not built) |
| Swift/Expo mobile companion: finish notifications + follow-ups | Khala SwiftUI app (`clients/khala-ios/Khala`) — today a chat client, not a companion |
| Agent-driveable CLI + orchestration skill | `khala-cli`, Artanis tool surface (AaaS #6381-87, #6359) |

The single biggest lesson from Orca is **architectural discipline at the seam between "which agent" and "where it runs."** Orca cleanly separates four concerns — *which* agent (declarative per-agent config registry), *where* it runs (a provider interface for local vs SSH/remote), *how* it reports status (a uniform hook ingest pipeline), and *how* to resume it (per-agent resume-argv) — so that adding Grok, Cursor, Pi, or Droid is a config entry, not a new code path. Our two executors currently fork that logic per agent.

### Top 5 things to adapt

1. **Unify our two executors behind one `AgentRunner` interface + a declarative agent registry** (model: Orca's `src/main/providers/types.ts` + `src/shared/tui-agent-config.ts`). Today `claude-agent-executor.ts` and `codex-agent-executor.ts` define parallel `*Runner`/`*TaskPayload`/`*RunInput`/`*RunResult` types and share only `workspace-materializer.ts`. Collapse them so a new runner (OpenCode, Pi, local vLLM/Khala-as-runner) is a registry entry. **Lands in `apps/pylon/src/`. Priority: NOW.**

2. **Replace the bash supervisor's ad-hoc state with a typed coordinator + persisted task DAG** (model: Orca's `src/main/runtime/orchestration/{db,coordinator,groups}.ts`). Adopt the explicit state machine: `tasks` (with dependency DAG), `dispatch_contexts` (with circuit-breaker `failure_count`), heartbeat liveness (5-min beat / 10-min hung), base-drift guard (refuse dispatch >N commits behind), slot-based max-concurrency, and **group addressing** (`@all` / `@idle` / `@worktree:<id>`) for fan-out. **Lands in `apps/pylon/src/` (local sqlite) and/or the Worker D1 for fleet-scope. Priority: NOW/SOON.**

3. **Build the operator dashboard on Orca's agent-status store shape** (model: `src/renderer/src/store/slices/agent-status.ts`): live + retained entries, per-entry `stateHistory`, `stateStartedAt` distinct from `updatedAt`, decay-to-idle, and unread/ack tracking keyed by `stateStartedAt`. Add Orca's **annotate-diff-and-ship-back** loop (`diffComments.ts` + agent-send popover in `ui.ts`). **Lands in `apps/openagents.com` (Foldkit). Priority: SOON (#6401-6403).**

4. **Turn the Khala mobile app into a true operator companion via a Cloudflare-mediated relay** (model: Orca's `mobile/src/transport/*` + `src/main/runtime/rpc/*` + `src/relay/`). Orca uses an **E2EE WebSocket** (QR pairing, tweetnacl) and pushes finish notifications **over the existing socket — no APNs/Firebase** — plus `terminal.subscribe`/`terminal.send` to steer. We should adopt the pairing + subscribe/steer protocol but front it with a **Durable Object relay** so it works for remote fleets, not just LAN. **Lands in `clients/khala-ios/Khala` + a new Worker/DO. Priority: SOON (AaaS #6381-87).**

5. **Give Artanis a real, bounded action surface via the agent-hook status pipeline + agent-driveable CLI** (model: Orca's `src/main/agent-hooks/server.ts` OSC-9999 ingest, the `orca` CLI `src/cli/`, and `skills/orchestration/SKILL.md`). Artanis is today a read-only situational-awareness persona; Orca's hook pipeline + CLI specs show exactly how to give an agent a typed, allowlisted "drive the orchestrator" surface. **Lands in `apps/openagents.com/workers/api` (Artanis) + `clients/khala-cli`. Priority: SOON (#6359 autonomy, #6381-87).**

---

## 2. Architecture overview of Orca

Orca is an Electron app (main / preload / renderer) plus a remote relay daemon and a mobile app. File counts: `src/main` ~1351, `src/renderer` ~3339, `src/shared` ~473, `src/relay` ~98, `src/cli` ~98, `mobile` (Expo/RN).

- **`src/main/`** — Node main process. Owns git/worktree ops, the provider abstraction, per-agent config, the orchestration coordinator + SQLite, the agent-hook HTTP ingest, IPC, and the mobile/RPC runtime.
- **`src/main/runtime/`** — the RPC runtime (`orca-runtime.ts`), the WebSocket/Unix-socket transports, the device registry + E2EE pairing, and `runtime/orchestration/` (the multi-agent coordinator).
- **`src/main/providers/`** — `IPtyProvider` / `IFilesystemProvider` / `IGitProvider` + `IProviderRegistry` (routes by `connectionId`: null = local, set = SSH/remote).
- **`src/shared/`** — the cross-cutting, agent-neutral truth: `tui-agent-config.ts` (the declarative agent registry), `agent-session-resume.ts`, `agent-process-recognition.ts`, `agent-status-types.ts`.
- **`src/renderer/`** — React UI; the relevant parts are the Zustand store slices (`store/slices/`) and the sidebar/dashboard components.
- **`src/relay/`** — a standalone daemon deployed to remote SSH hosts; multiplexes terminal I/O and forwards agent-hook payloads back over the SSH channel.
- **`src/cli/`** — the `orca` CLI (specs + handlers) that lets agents script Orca (`worktree create`, `snapshot`, `click`, `fill`, `orchestration send`, …).
- **`mobile/`** — Expo SDK 55 / RN 0.83 companion; pairs over E2EE WebSocket, subscribes to terminals, receives finish notifications, sends follow-ups.
- **`skills/`** — six `SKILL.md` skills (computer-use, orca-cli, orchestration, orca-emulator, linear-tickets, orca-linear).

The recurring design rule: **agent-neutral truth lives in `src/shared`; execution location is a provider; everything else is declarative config.**

---

## 3. Per-area adaptation analysis

### 3.1 Multi-runner abstraction (highest value)

**What Orca does.** The "which agent" knowledge is a single declarative table, `src/shared/tui-agent-config.ts`:

```
TUI_AGENT_CONFIG: Record<TuiAgent, TuiAgentConfig> = {
  claude: { detectCmd, launchCmd, expectedProcess, promptInjectionMode: 'argv', draftPromptFlag: '--prefill' },
  codex:  { detectCmd, launchCmd, promptInjectionMode: 'argv', preflightTrust: 'codex', draftPasteReadySignal: 'codex-composer-prompt' },
  // pi, opencode, gemini, grok, cursor, ...
}
```

Each entry declares: how to **detect** the binary, how to **launch** it, how prompts are **injected** (argv flag / env var / stdin / stdin-after-start), per-agent **trust pre-flight**, and a **composer-ready signal**. Binary resolution is uniform (`src/main/codex-cli/command.ts` `resolveCliCommand` — searches PATH then nvm/volta/asdf/mise, platform-aware). Session **resume** is a separate uniform function (`src/shared/agent-session-resume.ts` `getAgentResumeArgv` → `claude --resume <id>` vs `codex resume <id>` vs `opencode --session <id>`), keyed by `AgentProviderSessionMetadata { key: 'session_id'|'conversation_id', id }`. Process **recognition** (`src/shared/agent-process-recognition.ts`) tokenizes a command line and walks node/python interpreters to identify which agent is actually running. *Where* it runs is an orthogonal axis: `src/main/providers/types.ts` defines `IPtyProvider` (`spawn`/`write`/`attach`/`onData`/`onExit`/`serialize`/`revive`) and the registry dispatches by `connectionId` (null → `LocalPtyProvider`, set → SSH provider).

**What we'd port.** Our `apps/pylon/src/claude-agent-executor.ts` and `codex-agent-executor.ts` are ~80% structurally identical (both: recognize work class → materialize `git_checkout` workspace via the shared `workspace-materializer.ts` → drive one SDK session → verify with fixture command → digest closeout refs) yet each redeclares `*TaskPayload`, `*RunInput`, `*RunResult`, `*Runner`. Introduce:

- A neutral `AgentRunner` interface (`run(input: AgentRunInput): Promise<AgentRunResult>`) and a single `AgentTaskPayload` schema that carries `agentKind` as a field, mirroring how `workspace-materializer.ts` already unified the workspace contract.
- A declarative `AGENT_RUNNER_REGISTRY` (our analog of `TUI_AGENT_CONFIG`) keyed by `agentKind` (`claude_agent_sdk`, `codex_sdk`, and future `opencode_sdk`, `khala_local`), each entry declaring SDK package, readiness probe, sandbox/approval policy, and turn-reporter wiring. Note our real per-agent delta is the **boundary law** (Claude has a `PreToolUse` hook; Codex uses owner-local `danger-full-access` + post-hoc workspace-escape validation) — that becomes a typed capability flag on the registry entry, not forked control flow.

**Effort / risk.** Medium effort, low risk — it is a refactor of code we own, behind existing smoke tests (`claude-agent-task-smoke.ts`, `codex-agent-task-smoke.ts`, `apps/pylon/tests/codex-agent-executor.test.ts`). High leverage: it is the prerequisite for the supervisor and dashboard ports treating all runners uniformly.

### 3.2 Worktree-per-agent orchestration

**What Orca does.** State lives in SQLite (`src/main/runtime/orchestration/db.ts`): `tasks {id, parent_id, spec, status, deps(JSON), result}` with a dependency **DAG** (status `pending → ready → dispatched → completed/failed/blocked`; completing a task `promoteReadyTasks` for dependents); `dispatch_contexts {task_id, assignee_handle, status, failure_count, last_heartbeat_at}` with a **circuit breaker** (3 failures → `circuit_broken`); `messages` (typed: `dispatch`/`worker_done`/`heartbeat`/`escalation`/`decision_gate`) with a shared `thread_id` for fan-out correlation; `coordinator_runs`; `decision_gates`. The coordinator (`coordinator.ts`) polls every 2-5s, enforces **slot-based concurrency** (`MAX_CONCURRENT_DEFAULT = 4`), does a **base-drift pre-flight** (refuse dispatch if the worktree base is >20 commits behind), detects hung dispatches at a 10-min no-heartbeat threshold, and teaches each agent the reporting CLI via an injected preamble. Worktrees are `git worktree add -b <prefix>/<sanitized-name>`; the `taskId` is stamped into the worktree comment field for lineage recovery (`orca-runtime.ts`). Fan-out is **group addressing** (`groups.ts`: `@all`, `@idle`, `@worktree:<id>`, `@claude`) → one message per recipient, shared thread. Teardown (`worktree-teardown.ts`) sweeps PTYs across runtime + daemon + registry, then prunes git worktrees and merged branches.

**What we'd port.** Our `codex-supervisor.sh`/`claude-supervisor.sh` are excellent operationally (account-aware N-worker pool, `SUP_PER_ACCOUNT` same-account parallelism, exponential backoff on 409/rate-limit, presence heartbeat, NEEDS_OWNER safety pause) but encode all state in shell variables and process liveness. Port the **typed state model** as the durable spine:

- A `tasks` DAG + `dispatch_contexts` with `failure_count` circuit-breaker and `last_heartbeat_at` — so a restart resumes the fleet instead of losing it, and a flapping account self-quarantines deterministically rather than via backoff alone.
- The **base-drift guard** before dispatch (we pin `origin/main` commit already; make "too far behind" an explicit refusal).
- **Group addressing** for "fan one backlog issue across K accounts/runners," correlated by a shared thread id — directly what `multi-session-campaign.ts` wants.

**Where it lands.** Local-fleet state in `apps/pylon/src/` (sqlite, like Orca); for cross-machine AaaS fleets, the same schema in **D1** behind the Worker. Keep the bash supervisor as the *process launcher*, but have it read/write the typed store rather than owning state. **Effort:** medium-high. **Risk:** medium — touches the live earning loop; stage behind the existing local proof runtime (#4385) before pointing it at production presence.

### 3.3 Tracking / observability / steering UI

**What Orca does.** `src/renderer/src/store/slices/agent-status.ts` keys agents by `${tabId}:${leafUuid}` with two maps: `agentStatusByPaneKey` (live) and `retainedAgentsByPaneKey` (finished snapshots that persist until dismissed). Each entry carries `state` (`working`/`blocked`/`waiting`/`done`), `prompt`, `updatedAt`, `stateStartedAt` (distinct, so a tool ping doesn't look like a state change), a rolling `stateHistory` (max 20), `interrupted`, `toolName`/`toolInput`/`lastAssistantMessage`, plus `orchestration` lineage and `providerSession`. Entries **decay to idle** after 30 min silence (terminal `done` excepted). Unread/attention is `acknowledgedAgentsByPaneKey[paneKey] < entry.stateStartedAt`, auto-acked when the user focuses the pane (`useAutoAckViewedAgent.ts`). The review loop: `store/slices/diffComments.ts` (`DiffComment {filePath, lineNumber, startLine?, body, selectedText, sentAt}`, optimistic add → per-worktree queue → persist → ship to agent) and the **agent-send popover** in `store/slices/ui.ts` (pick an eligible running agent, transition `open → sending → error/success`). Persistence: the agent-hook server writes `last-status.json` (7-day TTL) so the dashboard rehydrates after restart.

**What we'd port.** This store shape is the design spec for operator dashboard #6401-6403. Adopt: the live/retained split, `stateStartedAt`-based unread, `stateHistory` for an activity timeline, and especially the **annotate-diff-then-ship-back** loop — review N agents' diffs, drop line comments, send corrections without leaving the dashboard. **Where it lands:** `apps/openagents.com` in Foldkit/Effect (our state model is Effect services, not Zustand, but the *shape* transfers directly). Source the live status from §3.5's hook pipeline. **Effort:** medium. **Risk:** low (greenfield UI).

### 3.4 Mobile companion + relay

**What Orca does.** `mobile/` is Expo SDK 55 / RN 0.83. The desktop runtime listens on a WebSocket (`src/main/runtime/rpc/ws-transport.ts`); pairing (`mobile/src/transport/pairing.ts`) is a QR `orca://pair?code=…` carrying endpoint + device token + the server's static public key, then an E2EE handshake (`e2ee_hello`/`e2ee_ready`/`e2ee_auth`/`e2ee_authenticated`) with `tweetnacl`; all subsequent RPC frames are encrypted. The desktop enforces a strict ~140-method **RPC allowlist** (`src/main/runtime/runtime-rpc.ts`). Crucially, **finish notifications ride the existing encrypted socket** (`notifications.subscribe` RPC fanned out from `src/main/ipc/notifications.ts` → `mobile/src/notifications/mobile-notifications.ts` → `expo-notifications` local notification) — **no APNs/Firebase server**. Steering is `terminal.send(terminal, text)` (dictation/clipboard/image/PR-comment) and `terminal.subscribe(terminal, {viewport})` for live output. The `src/relay/` daemon is a separate concern: it tunnels Orca↔remote-SSH-host (framed protocol, 5s keepalive), and forwards agent hooks back; mobile talks to the desktop directly, not the relay.

**What we'd port.** Our `clients/khala-ios/Khala` today is a **chat client** to the public Khala HTTP API (`KhalaClient.swift` → `POST /api/v1/chat/completions`) — it cannot watch or steer the agent fleet. Add a **companion mode**: pairing handshake, a subscribe-to-agent-status stream, finish notifications, and a follow-up/steer send. Two deliberate divergences from Orca:

- **Cloudflare-mediated, not LAN-direct.** Orca's direct-LAN WS assumes desktop and phone share a network (or Tailscale). Our fleet is remote/distributed, so front the relay with a **Durable Object** (one DO per operator/fleet) that Pylons connect outward to and the phone subscribes to — no inbound ports, works anywhere. Reuse Orca's E2EE handshake and allowlist *shape* over that DO socket.
- **Keep APNs optional.** Orca's "notify over the existing socket" is elegant for a foregrounded app; for backgrounded iOS we will still want APNs for wake. Adopt the socket-push for foreground, layer APNs for background.

**Where it lands:** `clients/khala-ios/Khala` (SwiftUI companion mode) + a new `apps/openagents.com/workers/*` Durable-Object relay. This is the concrete substrate for **Artanis-as-a-Service** (#6381-87) — the phone becomes the operator console for user fleets. **Effort:** high. **Risk:** medium (auth/E2EE correctness; pairing UX).

### 3.5 Agent-hook status pipeline, CLI, and skills

**What Orca does.** Agents report status by emitting **OSC 9999** sequences with JSON payloads; a loopback HTTP listener (`src/main/agent-hooks/server.ts`, bearer-token auth) ingests them, fans out to the UI over IPC, and persists `last-status.json`. The Codex lane maps each hook event (UserPromptSubmit/PreToolUse/PermissionRequest/PostToolUse/Stop) to a managed script installed in a managed home dir. The `orca` CLI (`src/cli/specs/*`, `handlers/*`) gives agents a scriptable surface (`worktree create`, `terminal`, `orchestration send/check`, `task-create/list/update`, browser `snapshot/click/fill`), all `--json` for agent consumption, dispatched over Unix socket or pairing code. `skills/orchestration/SKILL.md` documents the coordinator protocol (messages, ask/reply, task DAGs, dispatch, decision gates) as a teachable skill.

**What we'd port.** This is the missing middle for **Artanis autonomy (#6359)**. Today `artanis-situational-awareness.ts` injects read-only context via injected reader functions — Artanis can *see* but not *act*. Orca shows the clean shape for action: a **typed, allowlisted CLI/RPC surface** (our `clients/khala-cli` already has the seed: `khala request --workflow codex_agent_task`) plus a **uniform status-ingest pipeline** so every runner reports `working/permission/done/interrupted` the same way regardless of agent. Port:

- A uniform agent-status event schema + ingest (our analog of OSC-9999 → hook server), feeding both the dashboard (§3.3) and the mobile companion (§3.4). We already have `codex-turn-reporter.ts` — generalize it to all runners via the §3.1 registry.
- Extend `khala-cli` toward orchestration verbs (`task create/list`, `dispatch`, group send) routed through `artanis-approval-gates` for spend/destructive actions — the bounded action surface the capabilities audit (`docs/artanis/2026-06-27-artanis-capabilities-and-agency-audit.md`) calls for.

**Where it lands:** `apps/openagents.com/workers/api` (Artanis), `clients/khala-cli`, `apps/pylon/src` (reporter). **Effort:** medium. **Risk:** medium — keep every new verb owner-scoped and gate spend/destructive paths.

### 3.6 Genuinely novel extras (note, mostly do-not-port)

- **Annotate-AI-diff** (`diffComments.ts`) — port (covered in §3.3); the standout review pattern.
- **Session/scrollback persistence + host partitioning** (`workspace-session-host-persistence.ts` splits local vs `runtime:*` panes) — relevant if we ever do SSH-backed remote worktrees; note as later.
- **Visual evidence** (`.visual-evidence/` before/after PNG pairs in PRs) — lightweight, cheap to adopt as a PR-evidence convention; not load-bearing.
- **Design Mode** (click a Chromium element → HTML/CSS/screenshot into prompt), **computer-use** (native a11y trees per OS), **iOS emulator bridge** — interesting but tied to the Electron/native shell; **do not port**.

---

## 4. At-a-glance: Orca pattern → OpenAgents target

| Orca pattern (file) | OpenAgents target | Priority | Related issue |
| --- | --- | --- | --- |
| Declarative agent registry (`src/shared/tui-agent-config.ts`) + provider iface (`src/main/providers/types.ts`) + resume argv (`src/shared/agent-session-resume.ts`) | Unify `claude-agent-executor.ts`+`codex-agent-executor.ts` behind one `AgentRunner` + registry in `apps/pylon/src/` | **NOW** | #6388-91 (Claude Code) |
| Typed coordinator + task DAG + circuit breaker + heartbeat + drift guard + group addressing (`runtime/orchestration/{db,coordinator,groups}.ts`) | Typed state spine under `codex/claude-supervisor.sh` (sqlite local, D1 for fleet) | **NOW/SOON** | #6386 (per-account), AaaS #6381-87 |
| Agent-status store: live/retained, stateHistory, unread-by-stateStartedAt (`store/slices/agent-status.ts`) | Operator dashboard state model in `apps/openagents.com` (Foldkit/Effect) | **SOON** | #6401-03 (dashboard) |
| Annotate-diff → ship to agent (`store/slices/diffComments.ts`, `ui.ts`) | Diff review + steer in operator dashboard | **SOON** | #6401-03 |
| E2EE WS pairing + subscribe/steer + socket-push notifications (`mobile/src/transport/*`, `src/main/runtime/rpc/*`) | Khala SwiftUI **companion mode** + Durable-Object relay | **SOON** | AaaS #6381-87, #6386 |
| Agent-hook status ingest (OSC-9999, `agent-hooks/server.ts`) + agent-driveable CLI (`src/cli/`) + orchestration skill | Artanis action surface + uniform status ingest; extend `khala-cli` | **SOON** | #6359, #6381-87 |
| SSH host-partitioned session persistence (`workspace-session-host-persistence.ts`) | Remote-worktree support (if/when) | **LATER** | — |
| `.visual-evidence/` PR before/after convention | PR-evidence convention | **LATER** | — |
| Design Mode / computer-use / emulator / Electron shell / xterm-Ghostty / embedded Chromium | **Do not adopt** | — | — |

---

## 5. What NOT to adopt (and why it doesn't fit us)

- **The Electron desktop shell.** Orca is a downloadable cross-platform desktop IDE (`electron.vite.config.ts`, main/preload/renderer). Our product is **Cloudflare Workers + Foldkit web + native SwiftUI + a local Pylon daemon**. We want Orca's orchestration *brains*, delivered as a service, not a second desktop app. Porting the shell would fork our product surface.
- **xterm/Ghostty WebGL terminal + embedded Chromium browser + Design Mode.** Heavy native/renderer surface tied to Electron; irrelevant to a Worker/web/SwiftUI product. Our agents run headless in materialized workspaces; we don't surface live terminals to end users.
- **Native computer-use modules** (`native/computer-use-{macos,linux,windows}`) and the **iOS emulator bridge.** OS-specific native code; outside our distributed-fleet model.
- **LAN-direct mobile pairing as the *only* transport.** Orca assumes phone and desktop share a network (or Tailscale). For distributed user fleets we must mediate through a **Durable Object** (outbound connections, no inbound ports). Adopt Orca's E2EE *handshake* and method *allowlist*, but not its direct-socket topology.
- **SQLite-on-the-desktop as the fleet store.** Fine for Orca's single-machine model; for us, local Pylon state can be sqlite but **fleet/AaaS state belongs in D1** behind the Worker so it survives and is queryable across machines.
- **Per-agent forked control flow.** Even Orca avoids this — and our current two executors partly violate it. The lesson is the opposite of "copy each agent's code": converge on the registry.

---

## 6. Recommended sequencing

1. **Unify the runner (NOW).** Refactor the two Pylon executors behind one `AgentRunner` + `AGENT_RUNNER_REGISTRY` keyed by `agentKind`, with the Claude/Codex boundary differences as typed capability flags. Lands in `apps/pylon/src/`; guarded by existing smoke tests. *This unblocks everything else.*
2. **Typed coordinator state (NOW/SOON).** Introduce the `tasks` DAG + `dispatch_contexts` (circuit breaker, heartbeat, drift guard) as the supervisor's durable spine; keep the bash launcher but make it state-driven. Add group addressing for fan-out. Validate on the local proof runtime (#4385) first.
3. **Uniform status ingest (SOON).** Generalize `codex-turn-reporter.ts` into an all-runner status event + ingest, feeding both dashboard and mobile.
4. **Operator dashboard (SOON).** Build #6401-6403 on the agent-status store shape + annotate-diff loop, sourced from (3).
5. **Mobile companion + DO relay (SOON).** Companion mode in Khala + Durable-Object relay with E2EE pairing; this is the AaaS operator console.
6. **Artanis action surface (SOON).** Extend `khala-cli` with orchestration verbs behind `artanis-approval-gates`; wire to the coordinator from (2).

### Suggested new GitHub issues

- **"Unify Pylon coding executors behind one AgentRunner + declarative agent registry"** — collapse `claude-agent-executor.ts`/`codex-agent-executor.ts` into a neutral `AgentRunner` interface, single `AgentTaskPayload` (carrying `agentKind`), and `AGENT_RUNNER_REGISTRY` (SDK pkg, readiness probe, sandbox/approval policy, turn-reporter, capability flags). Acceptance: existing smoke tests pass; adding a third runner (OpenCode or `khala_local`) is a registry entry + fixture, no new control flow.
- **"Typed orchestration coordinator + task DAG for the supervisor pool"** — port Orca's `tasks`/`dispatch_contexts` model (dependency DAG, 3-strike circuit breaker, heartbeat liveness, base-drift refusal, slot concurrency, group addressing) as the supervisor's durable store (sqlite local + D1 fleet). Acceptance: a restarted supervisor resumes in-flight tasks; a flapping account self-quarantines; `@idle` fan-out dispatches one shared-thread task to K runners.
- **"Operator dashboard agent-status model + annotate-diff steering"** (folds into #6401-6403) — live/retained entries, `stateStartedAt` unread, `stateHistory` timeline, diff line-comments shipped back to a chosen running agent.
- **"Khala mobile operator companion + Durable-Object relay"** — E2EE pairing, agent-status subscription, finish notifications (socket + APNs), follow-up/steer send, method allowlist; DO-mediated for remote fleets. AaaS console.
- **"Uniform agent-status ingest pipeline across all runners"** — generalize the Codex turn reporter into a runner-neutral status event consumed by dashboard + mobile + Artanis.

---

## 7. License / attribution note

Orca is **MIT licensed** — `LICENSE` reads `MIT License / Copyright (c) 2026 Lovecast Inc.` MIT permits use, modification, and reuse including for commercial purposes, with the only obligation being that the copyright + permission notice be retained **in copies or substantial portions of the Software**. Implications for us:

- **Adapting patterns/architecture (what this report recommends) carries no MIT obligation** — ideas and APIs are not copyrightable, and we are explicitly *not* vendoring code (consistent with our `projects/` read-only-reference rule).
- **If we ever copy a "substantial portion" verbatim** (e.g., lifting a specific TS module like `tui-agent-config.ts` or `groups.ts` largely intact), we must include Orca's MIT notice and Lovecast Inc. copyright in that file/derived work. Prefer clean-room re-implementation in our own idioms (Effect/Foldkit/Swift) to avoid the question entirely.
- No patent or trademark grant is implied; do not use the "Orca" name/branding.

**Recommendation:** treat Orca strictly as architectural reference. Re-implement the chosen patterns natively. If any non-trivial snippet is copied, add the MIT attribution header to that file.
