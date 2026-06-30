# Codex → Khala Code Porting Audit

Date: 2026-06-30

Status: planning / direction audit. Reads `openai/codex` (the Rust coding agent,
vendored read-only at `projects/repos/codex`) against what OpenAgents already
ships as **Khala Code**, and proposes a concrete, parallelizable "port Codex
into our coding app overnight" plan. This doc flips no promise state, changes no
runtime authority, and broadens no public copy.

Inputs:

- `docs/research/terminal-agents/codex.md` — the deep Codex tool-layer study.
- `docs/research/terminal-agents/2026-06-29-openagents-khala-tool-decisions.md`
  — the decision record that turned the Codex/OpenCode/Gemini/Hermes/Pi studies
  into Khala tool decisions.
- `docs/research/terminal-agents/openagents-current-state.md` — the current
  62-system implementation inventory.
- `docs/adr/0012-adopt-khala-native-terminal-tool-runtime.md` — the accepted
  decision for `@openagentsinc/khala-tools`.
- Direct reads of `packages/khala-tools/src/*`,
  `clients/khala-code-desktop/src/bun/khala-chat-runtime.ts`,
  `clients/khala-cli/*`, and `projects/repos/codex/codex-rs/*`.

---

## 0. TL;DR

**We are much further along than "we should build a coding agent."** Khala Code
is a working agentic coding app: the desktop runs a real tool loop over a native
Effect/Effect-Schema tool runtime (`@openagentsinc/khala-tools`) that already
implements the full Codex-equivalent core catalog (`read`, `ls`, `glob`, `grep`,
`edit`, `write`, `apply_patch`, `exec_command`, `write_stdin`, `ask_user`,
`todo_write`, `view_image`, plus `web_fetch`/`web_search`/`browser` presets),
four-lane results, an OpenAI-compatible tool adapter, scoped permission
requests, and default-on Rampart PII redaction. ADR 0012's accepted plan has
**shipped**.

So the porting question is not "copy Codex's tool catalog" — we have it. It is:
**which of Codex's harder, lower-down execution-boundary machinery should we
port next, where we currently have honest stubs or nothing.** Those are, in
priority order: (1) real sandbox enforcement, (2) a central tool dispatcher with
hooks/lifecycle/telemetry, (3) durable session persistence + resume/fork
(rollout-style JSONL), (4) atomic `apply_patch`, (5) compaction, (6) a headless
JSONL event schema, (7) MCP, (8) the planner/feature/config/provider polish, and
— **last** — (9) a session-scoped approval cache + real product permission policy
to replace `allowAllKhalaPermissionService`.

**Deliberate stance for now: Khala Code runs permit-all ("YOLO"), for trusted
local operators only.** The desktop keeps `allowAllKhalaPermissionService` as the
default, so the permission/approval-cache work is intentionally deprioritized to
the end of the list. Sandbox enforcement (item 1) still lands early because it
contains *what* a permitted command can do, independent of *whether* we prompt;
the prompting/cache layer (item 9) is the thing we defer.

The rest of this doc ranks those, maps each to exact files on both sides, and
lays out an overnight multi-agent porting plan with lanes that don't collide.

---

## 1. What Khala Code is today (the baseline to port *into*)

"Khala Code" names three things; keep them distinct.

1. **The Khala model / coding lane** — hosted `openagents/khala`, OpenAI-compatible
   at `POST /api/v1/chat/completions`. The desktop and CLI are *clients* of this
   hosted brain. (`docs/khala/khala.md`.)
2. **Khala Code Desktop** — `clients/khala-code-desktop`, an Electrobun app whose
   Bun host owns model transport + native tool execution.
   (`clients/khala-code-desktop/src/bun/khala-chat-runtime.ts`.)
3. **`@openagentsinc/khala-tools`** — `packages/khala-tools`, the provider-neutral
   Effect-Schema native tool runtime shared by desktop, CLI, and future Pylon
   fallback.

### Already built and working

| Capability | Where | Notes |
| --- | --- | --- |
| Native tool catalog (16 tools) | `packages/khala-tools/src/{read,ls,glob,grep,edit,write,apply-patch,exec-command,write-stdin,ask-user,todo-write,view-image,web-fetch,web-search,browser}.ts` | Each Effect-Schema-typed, co-located `.test.ts`. `glob`/`grep` are ripgrep-backed with ignore-aware fallbacks. |
| Tool contracts / registry / executor | `packages/khala-tools/src/index.ts` | `KhalaToolDefinition/Invocation/Result/Event`, `makeKhalaToolRegistry`, `executeKhalaTool`, four result lanes (`modelOutput`, `ui`, `privateDataRefs`/`artifacts`, `publicSummary`), `toOpenAiCompatibleTools`. |
| Permission request model | `index.ts` (`KhalaPermissionRequest`, `KhalaPermissionService`) | Typed authority enum, `saveScope: once/session/project`, decisions `allow/deny/always`. **Contract exists; product policy + caching do not yet.** |
| Desktop turn loop | `khala-chat-runtime.ts` | OpenAI-compatible loop, `MAX_TOOL_ROUNDS=8`, `MAX_TOTAL_TOOL_CALLS=32`, SSE streaming, novel anti-hallucination "grounding corrections" (≤3) and vacuous-answer detection. |
| Codex-fleet delegation tools | `clients/khala-code-desktop/src/bun/khala-codex-fleet-tools.ts` | `pylon_ensure`, `codex_fleet_status`, `codex_spawn`. |
| Default-on PII redaction | `packages/khala-tools/src/redaction.ts` + `docs/khala/2026-06-30-khala-code-desktop-redaction.md` | Rampart per-session placeholder table, fail-soft model→heuristics→regex. Privacy prefilter, not a security boundary. |
| Khala CLI | `clients/khala-cli/*` (shipped npm `@openagentsinc/khala`) | Interactive + headless, BYOK, `khala fleet connect`, `khala spawn`, Artanis channel. |
| Pylon delegation lanes | `apps/pylon/src/{codex,claude}-agent-executor.ts`, `workspace-materializer.ts`, `node/approval-queue.ts`, `node/auto-approval-policy.ts` | Workspace materialization, owner-local Codex `danger-full-access` + post-hoc `fileChangeEscapesWorkspace()`, exact token accounting, owner-only ATIF traces. |

### Honest stubs / gaps (these are what Codex can teach)

- **No sandbox enforcement.** `defaultKhalaProcessService` runs `Bun.spawn` and
  truthfully returns `sandbox: { enforced: false, kind: "none" }`. Tests pin that
  it does **not** claim sandboxing (`exec-command.test.ts`).
- **Permit-all by design (for now); no approval cache.** The desktop wires
  `allowAllKhalaPermissionService` on purpose — Khala Code is "YOLO," for trusted
  local operators only at this stage. `executeKhalaTool` evaluates permission
  per-call with no "always allow this action/resource for the session" memory.
  This is the deliberately last-priority item (§3 Tier 4): we are not adding
  prompts/cache until the rest of the execution boundary lands.
- **`executeKhalaTool` is minimal.** No pre/post-tool hooks, no lifecycle
  events, no telemetry tags, no per-turn tool-call accounting, no streamed
  argument-diff consumption — Codex's `ToolRegistry::dispatch_*` has all of these.
- **`apply_patch` is non-atomic.** V1 applies operations sequentially and returns
  a partial-failure receipt (`apply-patch.ts`, `atomic: false`).
- **No durable session persistence / resume / fork / rewind** in the desktop or
  `khala-tools`. Turn state is in-session only. (Pylon has session records for
  the delegated lane; the native lane has none.)
- **No compaction** in the native loop.
- **No MCP** client or server in `khala-tools`/desktop.
- **No tool planner / progressive disclosure.** `registry.materialize(availability)`
  filters by preset, but there is no model/feature/env-sensitive planner and no
  deferred/searchable tools.
- **`exec_command` uses pipes, not a PTY.** `write_stdin` sessions are `Bun.spawn`
  stdio, not a real terminal; no process groups.
- **No headless structured event schema** for the desktop (the CLI has a simpler
  SSE path).

---

## 2. Codex, mapped to what we still need

Codex's value is **less its catalog and more its execution boundary**: typed
host tool-spec, central dispatcher, grammar `apply_patch`, sandbox/approval
framework, bounded streaming, rollout persistence, and a clean headless event
schema. The relevant crates in `projects/repos/codex/codex-rs/`:

| Codex crate / file | What it owns | Maps to our gap |
| --- | --- | --- |
| `core/src/tools/registry.rs` | Central `ToolRegistry` dispatcher: per-turn accounting, lookup, lifecycle notifications, `PreToolUse`/`PostToolUse` hooks, input rewrite, telemetry, dispatch trace. | §1 "executeKhalaTool is minimal". |
| `core/src/tools/spec_plan.rs` | Per-turn tool planner: layers shell/MCP/core/collab/dynamic/hosted tools by mode, model metadata, env, feature flags. | §1 "no planner". |
| `tools/src/{tool_spec,tool_definition,responses_api,tool_search,dynamic_tool,code_mode}.rs` | Host tool-spec model; deferred/searchable tools; namespaced MCP/dynamic tools. | progressive disclosure. |
| `apply-patch/` + `core/src/apply_patch.rs` + `core/src/safety.rs` | Lark-grammar patch tool; `assess_patch_safety`; writable-root checks; delegate-to-runtime; hard-link caveat. | §1 atomic/safe `apply_patch`. |
| `sandboxing/`, `bwrap/`, `linux-sandbox/`, `execpolicy/` (+ Seatbelt path) | Platform sandbox transforms (Seatbelt/macOS, Landlock+seccomp/bubblewrap/Linux), command policy, managed network proxy. | §1 no sandbox enforcement. |
| `core/src/tools/sandboxing.rs` (`ApprovalStore`, `with_cached_approval`, `ExecApprovalRequirement`) | Session approval cache keyed by serialized approval keys; denied-read preservation. | §1 approval cache. |
| `core/src/exec.rs` | Process runtime: output byte caps, ≤10k delta events, timeout/drain bounds, sandbox transform, network proxy, PTY/unified exec. | harden `exec_command`, add PTY. |
| rollout / `external-agent-sessions/` + `codex exec resume` | JSONL session rollouts; resume/fork by id. | §1 persistence/resume. |
| `codex-mcp/`, `mcp-client`, `mcp-types` | MCP client + `codex mcp-server` exposing Codex as an MCP server. | §1 MCP. |
| `exec/src/exec_events.rs` | Stable, TS-exported `ThreadEvent` JSONL schema; stderr-for-progress / stdout-for-final split. | §1 headless schema. |
| `config/` (`config_toml.rs`, `loader/mod.rs`) | Layered config (admin→system→cloud→user→profile→project→`-c`), `requirements.toml` constraints, trusted-project gating, denylist. | config/profiles. |
| `model-provider-info/` | `ModelProviderInfo` TOML provider schema. | provider abstraction. |
| `features/` | Static feature registry + `[features]` config + `--enable/--disable`. | feature flags. |
| `app-server*` | JSON-RPC engine↔frontend seam (TUI and `exec` both consume it). | a normalized event seam if we unify surfaces. |

Two corrections worth recording from the latest read of Codex `main`:

- Codex is now **Responses-API-only on the wire** (`wire_api = "chat"` is a hard
  error). We do not need to mirror that; hosted Khala owns our wire. But it means
  Codex's provider code is Responses-shaped.
- The TUI and `exec` consume the **app-server** JSON-RPC notification stream, not
  raw core events. The reusable seam is a protocol, not an event enum.

---

## 3. High-impact-sooner ranking

Ranked by (safety/credibility value) × (leverage across desktop + CLI + Pylon) ÷
(port effort). Each item is sized S/M/L.

### Tier 1 — do first (everything else rides on these)

1. **Sandbox enforcement for `exec_command` (L, macOS-first).**
   Port Codex's Seatbelt path first (`sandboxing/` macOS profile) so local shell
   runs under a real workspace-scoped sandbox on Mac (our primary desktop
   platform), with the existing honest `sandbox.enforced` flag finally able to
   report `true`. Add Linux (Landlock/seccomp via `linux-sandbox`/`bwrap`) as a
   fast-follow. Preserve Codex's **denied-read preservation** rule (don't drop the
   sandbox to escalate if that would also drop a read-deny). This contains *what*
   a permitted command can do — valuable even while we stay permit-all, because it
   bounds blast radius without prompting.
2. **Central dispatcher with hooks + lifecycle + per-turn accounting (M).**
   Grow `executeKhalaTool` into a `ToolRegistry::dispatch`-equivalent choke point:
   pre/post-tool hooks, `tool_started/progress/completed/failed` lifecycle events
   (the `KhalaToolEvent` kinds already exist), telemetry tags, per-turn tool-call
   counters, and typed model-visible errors. Everything below plugs into this.

### Tier 2 — high leverage, do this week

3. **Durable session persistence + resume/fork (M).**
   Adopt a rollout-style append-only JSONL per session (model items + tool events
   + approvals) under the desktop's owner-local state dir, plus `khala resume <id>`
   / `khala fork <id>` and a desktop "Resume" surface. Reuse the existing
   `KhalaToolEvent`/transcript shapes as the on-disk record. (Codex: rollout
   crates + `codex exec resume`.)
4. **Atomic `apply_patch` (S–M).**
   Add an atomic backend: stage all touched paths, validate, write-or-rollback,
   so a mid-patch failure leaves the tree unchanged. Port `assess_patch_safety`'s
   writable-root + hard-link checks from `core/src/safety.rs`. Keep our grammar.
5. **Headless JSONL event schema for the desktop host (S–M).**
   Define a Khala equivalent of `exec/src/exec_events.rs` `ThreadEvent`
   (`thread.started`, `turn.*`, `item.*` with `command_execution|file_change|
   mcp_tool_call|todo_list|error`, `Usage`) and a `khala code --json` headless
   mode with the stderr-progress / stdout-final split. This makes Khala Code
   scriptable and eval-harness-friendly (feeds the Gym lane).
6. **Compaction (M).**
   Add a context-budget + compaction step to the native loop so long coding
   sessions don't blow the window. (Codex: token-budget tools + summarization;
   our Pylon `tas/compaction.ts` has decision helpers to reuse.)

### Tier 3 — extensibility & polish, after Tier 1–2 land

7. **MCP client (M) then `khala mcp-server` (M).**
   Namespaced, deferred, policy-scoped — must not shadow built-ins (ADR 0012).
   Reuse `packages/mcp-contract` and Pylon `tas/mcp-*.ts`.
8. **Tool planner + progressive disclosure (M).**
   A `spec_plan`-style planner that materializes the visible tool subset by mode,
   model metadata, env, and feature flags; deferred/searchable external tools.
9. **PTY-backed `exec_command`/`write_stdin` (M) + feature-flag registry (S) +
   layered config/profiles (L) + provider schema (M).** Move `exec_command`/
   `write_stdin` from `Bun.spawn` pipes to a real PTY (Probe already has
   `node-pty` in the vendored snapshot) with process groups. Port `features/`
   first (cheap, high-leverage); config layering and the `ModelProviderInfo`
   schema only as far as a single-tenant owner-local app needs (skip
   MDM/enterprise — see §5).

### Tier 4 — deferred (we run permit-all / "YOLO" until then)

10. **Real product permission policy + session approval cache (M).**
    Replace `allowAllKhalaPermissionService` with a default
    `approval_required`-with-prompts policy and an `ApprovalStore` equivalent so
    "always allow" scopes to `(action, resource-pattern, session/project)`, not
    "all tools." Port the *shape* of `core/src/tools/sandboxing.rs`
    (`with_cached_approval`, typed approval keys, `saveScope`); we already have
    `KhalaPermissionRequest.saveScope` and decision `always` to wire in.
    **Deliberately last.** For now Khala Code is permit-all for trusted local
    operators, so this is a posture upgrade we take only once the rest of the
    boundary (sandbox, dispatcher, persistence, atomic patch) is in place and we
    want to open Khala Code to less-trusted or remote-driven use. Until then,
    sandbox containment (Tier 1 #1) — not prompting — is what bounds risk.

---

## 4. How we extend our tools in this direction

Concrete, additive moves that keep ADR 0012's boundaries:

- **Make the executor the only choke point.** Route every desktop/CLI/Pylon-native
  tool call through one dispatcher (Tier 1 #2). No surface should call a tool
  handler directly; this is where approval, sandbox decision, hooks, events, and
  bounding live.
- **Keep tool authority as data.** We already have `KhalaToolAuthority` enums and
  presets (`inspect`/`coding`/`owner_local_full`/`network`/`browser`/`extension`).
  Add the missing `external_directory`, `credential`, `owner_full_access` *prompts*
  and a sandbox requirement per tool, mirroring Codex's `Approvable`/`Sandboxable`
  split — but in Effect services, not Rust traits.
- **Never let the wire smuggle danger.** Owner-local full access stays a local
  config/UI toggle, visibly labeled; public/request payloads express permission
  *requests*, never authority overrides. (Already an invariant; the sandbox work
  must not regress it.)
- **One event stream, four lanes, everywhere.** The headless JSONL schema (Tier 2
  #5) and the desktop UI should both project the same `KhalaToolEvent` stream;
  large outputs spill to private artifacts with a bounded model preview (already
  the contract — enforce it in the dispatcher).
- **Reuse, don't rebuild.** Probe's PTY/browser/scoped-FS slices, Pylon's
  workspace materializer + approval queue + `tas/*` evidence modules, and
  `packages/ui` diff rendering are the donor parts ADR 0012 already named.

---

## 5. What we explicitly do NOT care about

Cut these from any overnight plan; they are effort sinks for our product shape.

- **Windows sandbox support** (AppContainer, the Windows arg0 re-exec path). Not a
  target platform. macOS first, Linux second.
- **The full ratatui TUI** (`tui/`, ~150-variant AppEvent bus, inline-scrollback
  renderer). Our surfaces are the Electrobun desktop and an OpenTUI-class CLI; we
  borrow *concepts* (slash-command enum, modal approval overlay) at most, not the
  crate.
- **Enterprise/MDM config** (`requirements.toml`, admin/system/cloud config layers,
  trusted-project denylist for managed fleets). We are owner-local single-tenant;
  port at most user + profile + project-local layering, skip the MDM machinery.
- **Multi-provider wire abstraction** (Bedrock SigV4, Azure Responses detection,
  Ollama/LMStudio built-ins, Chat-vs-Responses selection). Desktop routes hosted
  Khala only; BYOK is OpenRouter metadata forwarded to hosted Khala, never called
  directly. Take the `ModelProviderInfo` TOML *shape* if/when we expose local
  models, nothing more.
- **Codex's own auth flows** (ChatGPT login, AgentIdentity, PAT, Bedrock API key).
  We have OpenAgents auth + the Codex-fleet device-login path already.
- **Codex cloud-tasks / app-server-daemon / remote execution infra**
  (`cloud-tasks*`, `app-server-daemon`). Pylon + the hosted Worker own remote
  execution and durable receipts for us.
- **Analytics/otel specifics** (`analytics/`, Codex's telemetry pipeline). We are
  Cloudflare-primitives-first (Analytics Engine / Tail Workers); don't import a
  parallel stack.
- **`code-mode` host / WASM tool-calling** as a near-term item — interesting, but
  not before Tier 1–2.

---

## 6. The "port Codex into Khala Code overnight" plan

The work parallelizes cleanly because the gaps touch mostly different files.
Set N coding agents on these lanes. Each lane is scoped to avoid collisions; the
**dispatcher lane (A) is the one shared seam**, so land it first or have every
other lane code against its interface.

### Wave 0 — the shared seam (land before fan-out, ~1 agent)

- **Lane A — Dispatcher.** Grow `executeKhalaTool` into a hooked dispatcher
  (pre/post hooks, lifecycle `KhalaToolEvent` emission, per-turn accounting, typed
  errors). Publish a stable `KhalaToolDispatcher` interface. Files:
  `packages/khala-tools/src/index.ts` (+ new `dispatcher.ts`). Acceptance: all
  existing `khala-tools` tests pass through the new path; desktop loop calls only
  the dispatcher.

### Wave 1 — fan out (lanes are independent; all code against Lane A's interface)

- **Lane C — macOS sandbox for exec.** New process service backed by a Seatbelt
  profile; flip `sandbox.enforced` to `true` when enforced; denied-read
  preservation. Donor: `sandboxing/` (Seatbelt). Files: `exec-command.ts` +
  new `process-sandbox-macos.ts`. Acceptance: a command that writes outside the
  workspace is blocked under sandbox; honest reporting test updated.
- **Lane D — Atomic apply_patch + safety.** Atomic staged write/rollback +
  `assess_patch_safety` port. Files: `apply-patch.ts`. Donor:
  `core/src/{apply_patch,safety}.rs`. Acceptance: mid-patch failure leaves tree
  unchanged; writable-root/hard-link checks covered.
- **Lane E — Session persistence + resume/fork.** Append-only JSONL rollout under
  owner-local state; `khala resume`/`khala fork`; desktop Resume surface. Files:
  new `packages/khala-tools/src/session-rollout.ts`, `clients/khala-cli/*`,
  desktop host. Donor: Codex rollout + `exec resume`. Acceptance: kill mid-session,
  resume, continue with intact context + tool history.
- **Lane F — Headless JSONL event schema.** Khala `ThreadEvent` + `khala code
  --json` + stderr/stdout split. Files: new `shared/headless-events.ts`, desktop
  host, CLI. Donor: `exec/src/exec_events.rs`. Acceptance: a scripted run emits a
  stable JSONL stream + a single final stdout message; schema test exported.
- **Lane G — Compaction.** Context-budget + summarize step in the native loop.
  Files: desktop `khala-chat-runtime.ts`, reuse Pylon `tas/compaction.ts`.
  Acceptance: a synthetic long session compacts instead of erroring; preserved/
  restored refs covered.

### Wave 2 — extensibility (after Wave 1 stabilizes)

- **Lane H — MCP client** (namespaced, deferred, policy-scoped) → **`khala
  mcp-server`** export. Donor: `codex-mcp/`, reuse `packages/mcp-contract`.
- **Lane I — Tool planner + progressive disclosure.** `spec_plan`-style planner +
  deferred/searchable external tools. Donor: `core/src/tools/spec_plan.rs`,
  `tools/src/{tool_search,dynamic_tool}.rs`.
- **Lane J — PTY exec + process groups.** Donor: `core/src/exec.rs`; reuse Probe
  `node-pty`. **Lane K — feature-flag registry** (cheap; `features/`).

### Wave 3 — deferred (we ship permit-all / "YOLO" until this lands)

- **Lane B — Permission policy + approval cache.** New
  `packages/khala-tools/src/permission-policy.ts` (`ApprovalStore`,
  `with-cached-approval`, real prompt service) + desktop wiring to replace
  `allowAllKhalaPermissionService`. Donor: `core/src/tools/sandboxing.rs`.
  Acceptance: edit/shell can prompt; "always allow" scopes to action+resource;
  tests cover denial, cached allow, scope leakage. **Intentionally last** — see
  §3 Tier 4. Until this lands, Khala Code stays permit-all for trusted local
  operators and relies on Lane C's sandbox containment to bound risk.

### Coordination rules for the agent fleet

- Land **Lane A** first; every other lane imports its interface. The Wave 1–2
  lanes touch mostly disjoint files (C=process-sandbox, D=apply-patch, E=rollout,
  F=headless-events, G=chat-runtime, H=mcp, I=planner, J=pty, K=features), so they
  fan out cleanly. The two contended files are `index.ts` (A owns the dispatcher
  edit; others add exports) and `khala-chat-runtime.ts` (G owns compaction;
  E adds resume wiring — sequence G→E or use a fresh worktree per agent). Lane B
  (deferred) also touches both `index.ts` and the desktop wiring, which is another
  reason to run it after the contended lanes have settled.
- Per workspace policy: each agent works in a **clean `origin/main` worktree**,
  commits its scoped lane, and pushes `main` independently. No cross-lane stashing.
- Every lane lands with its package tests + `check:deploy` green, and updates
  this doc's status table. No public-copy or promise changes ride along.
- Don't widen authority: sandbox/permission lanes must keep "no wire-level danger"
  and "prompt-unavailable never means allow" invariants.

---

## 7. Reference index

Khala Code (port *into*):

- `packages/khala-tools/src/index.ts` — contracts, registry, `executeKhalaTool`,
  process service, OpenAI adapter.
- `packages/khala-tools/src/{read,ls,glob,grep,edit,write,apply-patch,exec-command,write-stdin,ask-user,todo-write,view-image,web-fetch,web-search,browser,redaction}.ts`
- `packages/khala-tools/README.md` — per-tool contract reference.
- `clients/khala-code-desktop/src/bun/khala-chat-runtime.ts` — desktop turn loop.
- `clients/khala-code-desktop/src/bun/khala-codex-fleet-tools.ts` — fleet tools.
- `clients/khala-cli/*` — shipped CLI.
- `apps/pylon/src/{codex,claude}-agent-executor.ts`, `workspace-materializer.ts`,
  `node/{approval-queue,auto-approval-policy}.ts`, `tas/*.ts` — delegation +
  donor evidence modules.
- `docs/adr/0012-adopt-khala-native-terminal-tool-runtime.md`
- `docs/research/terminal-agents/2026-06-29-openagents-khala-tool-decisions.md`
- `docs/research/terminal-agents/openagents-current-state.md`
- `docs/khala/2026-06-30-khala-code-desktop-redaction.md`

Codex (port *from*, `projects/repos/codex/codex-rs/`):

- `core/src/tools/{registry,spec_plan,sandboxing}.rs`,
  `core/src/tools/handlers/*` — dispatcher, planner, approval cache, handlers.
- `tools/src/{tool_spec,tool_definition,responses_api,tool_search,dynamic_tool,code_mode}.rs`
- `apply-patch/`, `core/src/{apply_patch,safety,exec}.rs`
- `sandboxing/`, `bwrap/`, `linux-sandbox/`, `execpolicy/`
- `codex-mcp/`, `mcp-client`, `mcp-types`
- `exec/src/exec_events.rs`, `exec/` (headless), `config/`,
  `model-provider-info/`, `features/`, `app-server*`
- `docs/research/terminal-agents/codex.md` — the deep prior study.

---

## 8. Status

| Lane | System | Status |
| --- | --- | --- |
| A | Central hooked dispatcher | not started |
| C | macOS sandbox for exec | not started |
| D | Atomic apply_patch + safety | not started |
| E | Session persistence + resume/fork | not started |
| F | Headless JSONL event schema | not started |
| G | Compaction | not started |
| H | MCP client + server | not started |
| I | Tool planner + progressive disclosure | not started |
| J | PTY exec + process groups | not started |
| K | Feature-flag registry | not started |
| B | Permission policy + approval cache | deferred — permit-all ("YOLO") until then |

Update this table as lanes land. None of this changes runtime authority, public
copy, or promise state until a lane ships with tests + `check:deploy` green and
(where user-facing) the normal promise-evidence gate.
