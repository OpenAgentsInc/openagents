# OpenAgents Coder TUI: agent-fleet port plan — 2026-08-23

Target: `packages/openagents-cli` (`@openagentsinc/cli` 0.3.2), the
`openagents coder` interface.

Short-term goal, from the owner: extend the Coder interface so it can delegate
to many coding agents at once, in the shape described in
[`docs/transcripts/275.md`](../transcripts/275.md) — a purpose-built chat
console that consumes the OpenRouter-backed chat API first, then launches
several coding-agent harnesses on sandbox computers and drives them against the
issue backlog, with a 15-way fan-out as the proof.

Source of the mechanics: the companion
[cc tool and agent-fleet rendering reproduction spec](./2026-08-23-cc-tool-and-agent-fleet-rendering-reproduction.md).
Section references below (`§`) point into it. Prior art for the delegation data
model is
[Claude subagent histories](./2026-07-10-claude-subagents-rendering-analysis.md)
and [OpenAgents subagent design](./2026-07-10-openagents-subagents-design.md);
this plan does not restate them.

Status: proposal. No product code is changed by this document.

## TL.DR

The Coder interface already has the hard parts of a terminal application — a
snapshot/renderer split, differential painting, an absolute scroll anchor,
resize handling, expansion state, and a `--plain` twin that renders the same
snapshot. What it lacks is a **second state store**: everything on screen is
derived from one flat `entries` array, and a fleet of coding agents is not
expressible as a flat array of settled/unsettled text entries.

The port is therefore three layers, in this order:

1. A `CoderTask` registry beside the transcript, with a stable child id, a
   status machine, and an aggregated progress record.
2. Fleet rendering at four densities (footer phrase → per-agent line → detail →
   nested transcript), all reading the registry.
3. A per-tool renderer registry, which the delegation renderer is then just the
   first client of.

Doing 3 first is the tempting order and the wrong one: the delegation surfaces
need state the transcript cannot hold, and the tool-renderer interface should be
designed against a real second client rather than guessed at.

## 1. What exists today (observed)

### 1.1 The session (`src/coder-session.ts`)

The reply source is an async iterable of four chunk kinds:

```ts
export type ReplyChunk =
  | { readonly type: "text"; readonly value: string }
  | { readonly type: "reasoning"; readonly value: string }
  | { readonly type: "tool_call"; readonly callId: string; readonly name: string; readonly arguments: string }
  | { readonly type: "tool_result"; readonly callId: string; readonly output: string | undefined; readonly error: string | undefined }
```

The transcript is a flat array of entries:

```ts
export interface CoderEntry {
  readonly role: "you" | "assistant" | "notice" | "tool" | "reasoning"
  text: string
  settled: boolean
  readonly tool?: CoderToolCall
}

export interface CoderToolCall {
  readonly callId: string
  readonly name: string
  readonly arguments: string
  output: string | undefined
  error: string | undefined
  status: "running" | "succeeded" | "failed"
}
```

and the renderer contract is one immutable snapshot:

```ts
export interface CoderSnapshot {
  readonly entries: ReadonlyArray<CoderEntry>
  readonly running: boolean
  readonly repository: string
  readonly branch: string
  readonly model: string
  readonly turns: number
}
```

`CoderSession` owns: one in-flight turn (`controller`), listener fan-out
(`onChange`), notices, backend cycling (refused mid-turn, with a notice), and
the chunk→entry state machine, including the withdrawn opening entry, settling
the previous entry when the chunk kind changes, `[interrupted]` on abort, a
`notice` entry on failure, and — in `finally` — marking every unsettled entry
settled and any still-`running` tool call `failed`.

### 1.2 The reply source (`src/coder-chat-api.ts`)

`ChatApiReplySource` submits `POST /api/v3/chat/turns` and then **polls**
`GET /api/v3/chat/events`, which returns the conversation's whole log, tracking
the highest delivered `sequence` per `run_id`. It maps
`text_delta` / `reasoning_delta` / `tool_call_started` /
`tool_call_completed` / `tool_call_failed` / `response_completed` /
`response_failed` onto `ReplyChunk`, preferring the server's own `tool_call`
projection (pretty-printed `arguments`, extracted `output`, structured `error`)
over the raw payload. `POLL_INTERVAL_MS = 250`, `TURN_TIMEOUT_MS = 300_000`.

Two shipped-contract constraints are load-bearing for this plan: one
conversation per account (`DATA-002`) and **one active turn per conversation**
(`TURN-001`) — a second concurrent turn is refused with `turn_in_progress`
(HTTP 409).

### 1.3 The interface (`src/coder-ui.ts`)

ANSI only, no rendering dependency; OpenTUI is unusable here because its FFI is
Bun-only and the CLI ships as an npm package run on Node. Layout is transcript /
status / composer, `STATUS_ROWS = 1`, `COMPOSER_ROWS = 3`, `GUTTER = 9`.

Established mechanics worth keeping unchanged:

- **Differential paint**: only changed rows are rewritten; nothing emits a
  newline, clears the screen, or writes past the last row, so the terminal never
  scrolls and scrollback is never polluted. Resize clears `painted` entirely
  because every row was laid out for the old width.
- **Absolute scroll anchor**: `anchor` holds a line index, not a distance from
  the bottom, so a scrolled-up reader stays put while output arrives;
  `anchor === undefined` means "follow".
- **Alternate scroll mode** (`\x1b[?1007h`) instead of mouse reporting, so text
  selection keeps working.
- **Byte-level key parsing** with a 40 ms `ESCAPE_WINDOW_MS` so a lone `Esc`
  interrupts and an arrow key does not.
- **Hints give way, state does not**: `hints()` drops key hints from the end
  until the row fits rather than dropping the counter.
- Only reachable keys are advertised: `tab to switch model` appears only when
  `session.canCycleBackend && !running`; `ctrl+o to expand` only when a tool
  call exists.
- A one-second ticker redraws the status line while a turn runs, so elapsed time
  advances between chunks.

Current tool rendering is a single local function, three glyphs
(`◐` yellow / `✗` red / `✓` green), and two densities (collapsed: name +
clipped args + clipped outcome; expanded: full argument lines and full output
with a `→` marker on the first line). Expansion is `Set<string>` of call ids,
and `focusedTool` is "the newest tool call in the transcript".

### 1.4 Gap table

| Capability | `cc` | Coder today |
| --- | --- | --- |
| Tool render contract | per-tool, 11 methods, partial input (§1.2) | one function, `switch`-free but also extension-free |
| Compact vs full | separate renderer outputs (§1.2, §8.3) | `open` boolean over the same text |
| Grouped fan-out row | `renderGroupedToolUse` + grouping pass (§4) | none |
| Task registry | `AppState.tasks`, 7 task types (§6.2) | none; transcript only |
| Child agent progress | counters + activity ring + summary (§6.1) | none |
| Fleet densities | 5 (§8) | 0 |
| Background/foreground promotion | `backgroundAgentTask` + signal (§6.3) | none |
| Durable child transcript | disk output path returned in the result (§5.2) | none |
| Completion notification | XML into the parent's queue (§6.4) | none |
| Concurrency accounting | mostly absent | absent |
| Row memoization | prop-equality gate (§3) | differential paint only |

## 2. Proposed data model

New module `src/coder-tasks.ts`, deliberately independent of both the transcript
and the renderer, mirroring `cc`'s registry (§6) with OpenAgents vocabulary.

```ts
/** Stable, minted before launch; also the transcript filename and progress key. */
export type CoderTaskId = string

export type CoderTaskStatus = "pending" | "running" | "completed" | "failed" | "stopped"

export interface CoderToolActivity {
  readonly toolName: string
  readonly input: Readonly<Record<string, unknown>>
  readonly label: string | undefined
}

export interface CoderTaskProgress {
  readonly toolUseCount: number
  /** latestInputTokens + cumulativeOutputTokens; see §6.1 for why. */
  readonly tokenCount: number
  readonly lastActivity: CoderToolActivity | undefined
  readonly recentActivities: ReadonlyArray<CoderToolActivity>
  readonly summary: string | undefined
}

export interface CoderTask {
  readonly id: CoderTaskId
  /** Display-only, 3-5 words, supplied by whoever requested the delegation. */
  readonly description: string
  readonly prompt: string
  readonly agent: string
  readonly model: string | undefined
  readonly startedAt: number
  readonly endedAt: number | undefined
  readonly status: CoderTaskStatus
  readonly background: boolean
  /** True until the result has been read by the requester. */
  readonly unread: boolean
  readonly progress: CoderTaskProgress
  readonly transcriptPath: string | undefined
  readonly result: string | undefined
  readonly error: string | undefined
}
```

The snapshot grows one field, and only one:

```ts
export interface CoderSnapshot {
  // ...existing
  readonly tasks: ReadonlyArray<CoderTask>
}
```

Rules carried over from `cc`, each with its reason:

1. **Register before launch.** A child that fails to start must still be
   visible, with a reason.
2. **Mint the id first.** One id for task, transcript path, progress events, and
   completion notice.
3. **No-op updates return the same reference**, so `onChange` does not repaint
   an unchanged fleet.
4. **Terminal tasks linger.** Completed tasks stay listed while `unread`;
   stopped tasks stay ~3 s (`cc`'s `STOPPED_DISPLAY_MS`) so the reader sees the
   transition.
5. **Counters are aggregated on write**, never recomputed from an event log at
   paint time.
6. **`recentActivities` is bounded** (a small ring, `cc` keeps a handful) —
   an unbounded list is a memory leak with a 15-way fan-out.

## 3. Proposed delegation contract

The console asks for N children; the request is one shape whether one or fifteen
are wanted.

```ts
export interface CoderDelegationRequest {
  readonly description: string        // 3-5 words, display only
  readonly prompt: string
  readonly agent?: string             // harness/agent definition id
  readonly model?: string
  readonly background?: boolean       // default true for fan-out
  readonly isolation?: "worktree" | "computer"
  readonly cwd?: string
}
```

and each launch answers with either the synchronous result or the async handle
(`cc` §5.2):

```ts
export type CoderDelegationResult =
  | { readonly status: "completed"; readonly taskId: CoderTaskId; readonly result: string }
  | { readonly status: "launched"; readonly taskId: CoderTaskId; readonly transcriptPath: string }
  | { readonly status: "refused"; readonly reason: string; readonly code: string }
```

`refused` is a first-class outcome rather than a thrown error, because the
refusals are expected and enumerable: concurrency cap reached, quota exhausted,
no sandbox computer available, recursion guard, unknown agent.

Guards to implement before any fan-out ships, following `cc` §5.3 (each returns
`refused` with remediation text, so the requesting agent can adapt):

- a child cannot fan out again beyond a configured depth;
- a fan-out cannot exceed the active-computer cap;
- abort propagates parent→child only, never child→parent.

## 4. Proposed rendering

### 4.1 The renderer contract, in rows not nodes

Because the interface paints ANSI rows, `cc`'s node-returning renderers become
row-returning functions. This is the one structural change the port must make:

```ts
export interface RenderContext {
  readonly width: number
  readonly expanded: boolean
  readonly verbose: boolean
  readonly transcript: boolean
  /** So a renderer can collapse itself rather than be truncated from outside. */
  readonly rows: number
  readonly busyCalls: number
}

export interface ToolRenderer {
  readonly name: string
  /** One-line label for an activity list: `Read(src/foo.ts)`. */
  label(input: Readonly<Record<string, unknown>>): string
  use(input: Readonly<Record<string, unknown>>, ctx: RenderContext): ReadonlyArray<string>
  progress?(events: ReadonlyArray<unknown>, ctx: RenderContext): ReadonlyArray<string>
  result?(output: string | undefined, error: string | undefined, ctx: RenderContext): ReadonlyArray<string>
  group?(calls: ReadonlyArray<CoderToolCall>, ctx: RenderContext): ReadonlyArray<string> | undefined
}
```

Registry rules, all from `cc` §8.2:

- lookup by name; **a missing renderer falls back to today's generic rows**, so
  an unknown or MCP tool still draws;
- arguments are parsed defensively and a parse failure renders the raw name;
- every renderer call is wrapped so a throwing renderer cannot break the frame;
- renderers see partial arguments (the chat API's `tool_call_started` may carry
  incomplete `arguments`).

### 4.2 Four densities

**(a) Status line phrase.** One `getPillLabel`-equivalent (`cc` §8.4) folded
into the existing status row, beside `working…`/`ready`:

```
  ● working… (1m 04s · streaming)   ⣿ 4 agents · 1 done, 1 unread    repo · main · model
```

Reduce N tasks to one phrase (`3 agents`, `1 agent`), append terminal counts,
and show a `· ↓ to view` call-to-action **only** for attention states —
`cc`'s `pillNeedsCta` exists precisely to stop a running fleet from nagging.

**(b) Fleet block in the transcript.** One `AgentProgressLine` equivalent per
child (`cc` §8.1), as a single transcript entry so it scrolls with the
conversation it belongs to:

```
  agents   4 running · 1 done
           ├─ coder  fix flaky auth test    ◐ Bash(pnpm vitest run) · 14 tools · 8.2k
           ├─ coder  port grep renderer     ◐ Read(src/coder-ui.ts) · 9 tools · 4.1k
           ├─ coder  update teardown index  ✓ Done (6 tool uses · 3.4k tokens · 1m 12s)
           └─ coder  reconcile issue 812    ✗ failed: worktree dirty
```

Rules, all from `cc` §8.1: three status cases only
(`lastActivity || "Initializing…"` / background summary / `Done`); counts shown
while running and hidden once a background child resolves; `├─`/`└─`; suppress
the repeated agent-type column when every child shares a type.

**(c) Detail.** `Esc`-dismissable overlay or a focused expansion of one child
(`cc` §8.5): `agent › description`, elapsed + tokens + tools, a **Progress**
list of `recentActivities` with the newest marked `›` and undimmed, the prompt
clipped to ~300 characters, the error, and `x` to stop while running.

**(d) Nested transcript.** The child's durable transcript, rendered with the
same entry renderers as the parent (`cc` §8.3 transcript mode). This is the
first real consumer of `transcriptPath`.

### 4.3 Interaction, extending what exists

- `ctrl+o` keeps its meaning (expand the focused thing) but `focusedTool` grows
  into a focus cursor over `tool | task`, defaulting to the newest.
- Fleet navigation reuses the pill-window arithmetic when the fleet is wider
  than the row (`cc` §8.4's `calculateHorizontalScrollWindow` →
  `startIndex/endIndex/showLeftArrow/showRightArrow`).
- New keys are advertised only when reachable, matching the existing rule:
  `↓ to view agents` only with tasks present, `x to stop` only on a running
  focused task.
- The existing one-second ticker already covers per-agent elapsed time; the
  fleet block must be re-rendered on it, so fleet rows must be cheap.

### 4.4 Repaint discipline

Differential painting handles *what changed on screen*, not *what was rebuilt in
memory*. With 15 children emitting progress, rebuilding every transcript row per
event is the predictable regression. Port `cc`'s memo gate (§3) as a row cache
keyed per entry, invalidated on exactly: entry identity, width, expansion,
verbosity, transcript mode, `settled`, and — for fleet rows — the child's
`(status, toolUseCount, tokenCount, lastActivity)` tuple. Settled text entries
must never be re-wrapped.

## 5. Execution: where the children actually run

Transcript 275's sequence is chat API first, then harnesses on sandbox
computers. That splits cleanly into two independently shippable backends behind
`CoderDelegationRequest`:

1. **Local child processes** (worktree isolation, same machine). Shortest path
   to a real fleet on screen, no new service, and it exercises the whole
   registry/rendering stack. `cc`'s worktree rules apply: remove the worktree
   when clean, retain it when it has changes.
2. **Sandbox computers** (the transcript's Firecracker / GKE Agent Sandbox
   isolation tiers, GCS checkpoints, recoverable commands). Same registry, a
   different launcher, plus quota state the local path does not need.

The `TURN-001` single-active-turn constraint (§1.2) is the first architectural
question this plan cannot answer from the source: N concurrent children cannot
each hold the account's single chat turn. Three options, needing an owner or
server-side decision rather than a CLI workaround:

- children do not use `/api/v3/chat` at all (they are harnesses with their own
  provider path, and the console's chat turn stays the operator's);
- the server admits a delegation turn kind that does not occupy the
  conversation's active-turn slot;
- children write into distinct nested threads, which is exactly the missing
  primitive the forge-side nested-thread delegation audit identified (a
  `threads` ledger plus `thread.spawn`/`resume`/`cancel`/`complete`).

The third is the coherent long-run answer and is not a CLI change. **Option one
is the honest first slice**: build the registry and rendering against local
child processes, keep the console's own turn on the chat API, and do not claim
durable nested-thread receipts until the ledger exists.

## 6. Proposed sequence

| Slice | Contents | Verification |
| --- | --- | --- |
| 1 | `src/coder-tasks.ts`: registry, status machine, progress aggregation, no-op-identity updates, eviction timers. `tasks` added to `CoderSnapshot`. | unit tests over transitions and the token rule; `coder-session.test.ts` unchanged |
| 2 | Fleet rendering: status-line phrase + fleet block; row cache. Fed by a fake launcher. | snapshot-style row assertions at several widths, in the style of `coder-ui.test.ts` |
| 3 | Local child launcher: worktree isolation, durable transcript path, completion notice into the transcript, concurrency cap and `refused` outcomes. | end-to-end 3-way fan-out against a trivial task |
| 4 | Detail view + nested transcript + `x to stop` + focus cursor over tool/task. | interaction tests; `--plain` parity |
| 5 | `ToolRenderer` registry with the generic fallback; port 3-4 renderers (shell, read, edit, delegation). | per-renderer row tests; no behavior change for unknown tools |
| 6 | Grouped fan-out row (`group()`), consuming several sibling delegations in one turn. | grouping-pass tests |
| 7 | Sandbox-computer launcher, quota/active-computer accounting, 15-way fan-out proof. | the transcript's 15-way proof, plus quota-refusal paths |

Slices 1-4 need no server change and no new authority. Slice 7 needs the
sandbox-computer path and the concurrency policy, and must reconcile with
whatever answers §5's turn question.

## 7. Explicit non-goals

- No React, Ink, or OpenTUI dependency. The FFI constraint recorded in
  `coder-ui.ts` stands until OpenTUI ships an N-API entry point.
- No `tmux`-backed teammates or pane splitting.
- No new claim of durable delegation receipts before the nested-thread ledger
  exists. Until then, a Coder task is process-local state plus an on-disk
  transcript, and the interface should not imply otherwise.
- No copying of `cc` source. The reproduction spec exists so this can be
  implemented from behavior and shapes.
