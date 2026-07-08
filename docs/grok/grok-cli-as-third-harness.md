# Grok CLI as the third Khala Code harness

Date: 2026-07-08
Status: Grok design analysis (flips no promise state)
Depends on: [`parallel-multi-harness-asap.md`](./parallel-multi-harness-asap.md)
CLI reference: `docs/grok-cli/`

## 1. Goal

Make **Grok Build CLI** a peer of **Codex** and **Claude Code** inside
Khala Code:

- **Axis A:** user can chat in Grok mode (thread, tools, approvals)
- **Axis B:** fleet can spawn Grok workers under claims + verify gates
- **Surfaces:** desktop + mobile via Khala Sync
- **UI:** new chrome is Effect Native (EN-3/EN-5), not throwaway shells

## 2. Integration choice: ACP primary

| Mode | Command | Use |
| --- | --- | --- |
| **Primary** | `grok agent stdio` | Chat runtime + rich workers; JSON-RPC on stdio |
| Secondary | `grok -p … --output-format streaming-json` | Simple workers, CI, fixtures |
| Avoid | Interactive TUI automation | Fragile; product owns UI |

Why ACP wins:

1. Client (Khala Code Bun host / agent computer) owns FS, terminal, MCP
2. Assistant text streams as `session/update` chunks — maps to
   `message_delta` / `message_done`
3. Explicit `session/new`, resume-capable session ids
4. Auth methods advertised (`cached_token`, `xai.api_key`)

See `docs/grok-cli/headless-scripting.md` and
`docs/grok-cli/examples/acp-stdio-hello.mjs`.

## 3. Schema extensions (Wave 0)

In `packages/agent-runtime-schema` (today):

```text
AgentDefinitionHarnessKind  includes: codex, claude_code, khala, …
AgentRuntimeAdapterKind     includes: claude_code, codex, opencode, hermes, …
```

**Add (proposed names):**

| Field | Proposed literal | Notes |
| --- | --- | --- |
| harness kind | `grok_cli` | Prefer explicit `_cli` — this is the Build CLI, not a vague "grok api" |
| adapter kind | `grok_cli` | Same |
| raw event kind | `grok_acp_event` \| `grok_cli_event` | Projector input |
| fleet workerKind | `grok` | UI-short form; map to harness kind in dispatch |
| runtime mode (desktop) | `grok_runtime` | Mirror `claude_runtime` |

Also extend:

- `FleetRun.workerKind`: `codex | claude | grok | auto`
- Role registry: `harness: codex | claude | grok | khala`
- Capacity ref pattern: `capacity.coding.grok.account.*` (mirror Claude)

## 4. Axis A — `GrokAcpChatRuntime`

### 4.1 Shape (mirror Claude/Codex)

Implement beside:

- `clients/khala-code-desktop/src/bun/codex-app-server-chat-runtime.ts`
- `clients/khala-code-desktop/src/bun/claude-app-sdk-chat-runtime.ts`

New:

- `…/grok-acp-chat-runtime.ts`
- `…/grok-acp-event-projector.ts` (ACP updates → `KhalaCodeDesktopChatTurnEvent`)
- `…/grok-session-store.ts` (`~/.khala-code/grok-sessions.json` mapping)

Minimal methods (headless proof set):

| Method | ACP / process mapping |
| --- | --- |
| `startThread` | `session/new` with `cwd`, `mcpServers` |
| `startTurn` | `session/prompt`; stream `session/update` |
| `interruptTurn` | process signal / future ACP cancel if available |
| `resumeThread` | new session with resume semantics if ACP exposes; else map store |

Process lifecycle:

```text
spawn: grok agent stdio [--debug-file …]
  initialize(protocolVersion=1, clientCapabilities)
  authenticate(methodId)
  session/new
  loop: session/prompt + session/update
  kill on Scope release
```

Always pass automation-safe flags at the outer CLI when using `-p` fallback:
`--no-auto-update`. For ACP, document `auto_update = false` in capacity host
`~/.grok/config.toml`.

### 4.2 Desktop selector

Extend `harness-setting.ts` / RPC `runtimeMode`:

```text
codex_harness | claude_runtime | grok_runtime | (legacy khala)
```

Composer pill: **Codex | Claude | Grok | Auto**
Auto (chat) = last-used or policy default — **never silent mid-turn swap**.

### 4.3 MCP bridge

Today `khala_fleet` injects into Codex config. For Grok chat harness:

- Register the same fleet tools via Grok MCP config
  (`grok mcp add` or session `mcpServers` in `session/new`)
- Approval mode must still surface in Khala Inbox (product approvals),
  not only Grok's internal permission prompts

## 5. Axis B — Grok fleet workers

### 5.1 Dispatch path (target)

```text
FleetRunSupervisor tick
  → claim work unit (unique)
  → prepare_work pins: repo, commit, branch, verify, claimRef
  → runnerKind = grok_agent
  → capacity host executes Grok worker
       preferred: ACP session with pinned prompt + worktree cwd
       fallback: grok -p --cwd <worktree> --output-format json
  → closeout: verify command + PR metadata + token/time rows
  → release claim
```

### 5.2 Isolation

| Layer | Mechanism |
| --- | --- |
| Git | Prefer Grok `--worktree` **or** monorepo worktree — pick **one** policy per FleetRun; document GC |
| Auth | Per-capacity-host Grok login / API key; never shared across owners |
| FS/network | Agent computer sandbox when on cloud; desktop host sandbox profile when local |
| Secrets | Brokered grants only; no keys in prompts or Sync |

### 5.3 Prompt contract (worker)

Every Grok worker prompt must include:

1. Issue / work unit id and claimRef
2. Pinned repo + commit + branch
3. Allowed paths
4. Named verify command
5. "Open one PR; do not expand scope"
6. Output machine-readable summary block for closeout parsing

### 5.4 Metering honesty

Grok may not emit OpenAI-identical token rows. Rules:

- Prefer exact fields when present in JSON output
- Otherwise record `compute_time` + `not_measured` for tokens
- Never synthesize token counts to make dashboards pretty
- Tag rows: `harness=grok_cli`, `role=?`, `runRef`, `claimRef`

## 6. Khala Sync projections

Mobile does not run three CLIs. Mobile **observes and steers**.

### 6.1 Scopes to add / extend

| Scope / collection | Writers | Readers |
| --- | --- | --- |
| Fleet runs | desktop / daemon | mobile, web |
| Workers (per harness) | supervisor | mobile |
| Approvals | harness bridges + supervisor | mobile Inbox |
| Thread summaries | chat runtimes | mobile chat list |

### 6.2 Mutators (mobile → authority)

| Mutator | Effect |
| --- | --- |
| `fleet_run_control` | pause / resume / drain / stop |
| `approval_decision` | allow / deny tool or merge gate |
| `steer_message` | queue prompt to worker or chat harness |

Authority stays server/desktop-daemon side; mobile is not a second
supervisor implementation.

### 6.3 Dogfood receipt (definition of done for Sync slice)

1. Desktop starts mixed FleetRun (codex+grok fixtures)
2. Phone shows workers + states via Sync within seconds
3. Phone pauses run; desktop supervisor observes pause
4. Phone approves a pending tool; worker continues
5. All events have receipt/changelog rows

## 7. Effect Native UI touchpoints

Do **not** invent a Grok-only theme. Same Protoss tokens.

| Component need | EN demand |
| --- | --- |
| Harness segmented control | catalog control / toggle group |
| Worker list (virtualized) | Phase 2 lists |
| Severity chips (advisor/judge) | badge / chip |
| Plan DAG card | custom composition of stack/list/button |
| Account readiness matrix | table/list + status |

Desktop: land under EN-5 conversion, not a parallel vanilla DOM panel if
avoidable. If a temporary RPC headless smoke is needed, keep UI minimal.

## 8. File / package touch list (implementation map)

| Area | Paths (expected) |
| --- | --- |
| Schema | `packages/agent-runtime-schema/src/index.ts` (+ tests/fixtures) |
| Desktop runtimes | `clients/khala-code-desktop/src/bun/grok-*.ts` |
| Selector | `harness-setting.ts`, `rpc-handlers.ts`, shared RPC schemas |
| Fleet | `fleet-run-supervisor.ts`, fleet MCP server, planner |
| Pylon executor | new `grok-agent-executor.ts` (mirror claude-agent-executor) |
| Sync | `packages/khala-sync*` projections + mutators |
| Mobile | harness pill + fleet peek screens (EN-3) |
| Docs | `docs/grok-cli/*` (done); promises later for public claims |
| QA | fixture ACP mock; scenario: mixed workerKind run |

## 9. Test ladder

| Tier | Proof |
| --- | --- |
| Unit | Projector maps ACP chunks → turn events |
| Fixture | Mock ACP process; full startTurn |
| Integration | Real `grok agent stdio` hello (env-armed, skip-safe) |
| Fleet fixture | Mixed workerKinds, claim uniqueness |
| Live smoke | One real Grok worker claimed issue, verify command, closeout |
| Mobile dogfood | Sync projection + one mutator |

## 10. ASAP issue board (suggested filing names)

These are **suggested** issue titles for the owner/fleet to file under
MASTER_ROADMAP — not filed by this doc:

1. `schema: add grok_cli harness + adapter kinds`
2. `desktop: GrokAcpChatRuntime (Axis A)`
3. `desktop: runtimeMode grok_runtime + composer pill`
4. `pylon: grok agent executor + capacity readiness`
5. `fleet: workerKind=grok + mixed FleetRun fixture`
6. `sync: fleet_run projection + mobile peek`
7. `sync: fleet_run_control + approval mutators`
8. `en: mobile harness pill (EN-3)`
9. `en: desktop fleet multi-harness cards (EN-5)`
10. `qa: multi-harness fixture matrix (codex/claude/grok)`

## 11. Non-goals (for the first green)

- Replacing Codex as default coder
- Public marketing of "Grok armies" without promises
- Full Grok TUI feature parity inside Khala
- Training/Tassadar coupling
- Sharing one Grok login across multi-tenant capacity

## 12. Bottom line

Grok is not a detour from the multi-harness plan — it is the **third
adapter** the Axis A/B design was always waiting for. Enter through
**ACP**, pin work through **claims + worktrees**, surface through
**Khala Sync**, chrome through **Effect Native**, and keep **Codex-on-
agent-computers (P2)** as the cloud backbone so parallelism has somewhere
real to run.
