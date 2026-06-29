# Artanis capabilities & agency audit — why he has no tools, and the concrete path to real capabilities

- **Date:** 2026-06-27
- **Scope:** The owner-only Artanis operator chat channel (#6363) and what it would take to make Artanis a real acting agent under epic #6359 (read repo files himself, hit Khala himself, dispatch the owner's linked Codex accounts, run work locally/in a sandbox).
- **Owning repo:** `OpenAgentsInc/openagents` (`apps/openagents.com/workers/api`).
- **Trigger:** The owner can now talk to Artanis, but when asked to read
  `docs/khala/2026-06-26-khala-open-issues-master-roadmap.md` and help parallelize the roadmap work
  (currently tasked to a single agent and going slowly), Artanis correctly answered that he "does not
  have direct access to read files." That answer is honest and exactly right given the current code —
  this audit explains why, and the concrete path to fix it.

> Authority discipline (carried through this whole doc): everything below keeps Artanis **owner-scoped**,
> **Khala-powered** (dogfood `openagents/khala`), and routes **spend / destructive** actions through
> `artanis-approval-gates`. No secrets, tokens, mnemonics, payout targets, or raw prompts appear here.

---

## 1. Current state — what Artanis is, exactly

The owner-facing "Talk to Artanis" channel is `POST /api/operator/artanis/chat`
(`apps/openagents.com/workers/api/src/artanis-operator-chat-routes.ts`), gated to the owner (admin API
token, or a browser session whose email is an OpenAgents admin). Per turn it:

1. Authenticates the owner and resolves an owner-scoped id (`owner:<userId>`).
2. Loads owner **memory** (`artanis-owner-memory.ts`, D1, owner-scoped) — prior turns + durable notes.
3. Builds **situational awareness** (`artanis-situational-awareness.ts`,
   `buildArtanisSituationalAwareness`) — a bounded, read-only context object.
4. Calls the operator **core** `artanisOperatorTurn` (`artanis-operator.ts`), which assembles a
   Blueprint-style grounded program (persona system prompt + grounded context block + the owner
   conversation) and makes **one non-streaming `openagents/khala` completion**.
5. Persists the owner message + Artanis's reply back to memory and returns the reply text.

So Artanis today is: **a stateless-per-turn, Khala-powered LLM reply, grounded with owner memory +
injected read-only situational awareness.** That is a chat persona with good context — not an actor.

### What Artanis CAN see (the awareness buckets — read-only context, not tools)

`buildArtanisSituationalAwareness` assembles three buckets from **injected reader functions** (it never
reads anything itself; the route wires the real readers, tests wire fakes):

- **`recentActions`** — recent commits, recent Pylon→Codex assignments (public-safe one-line objective
  summaries, never raw prompts), issues opened/closed, and the Artanis tick log.
- **`goals`** — the master roadmap ref + the open epics that define the current sequence (code-anchored
  default `ARTANIS_DEFAULT_GOALS`: #6359 autonomy, #6316 serving, #6303 product), overridable by a live
  reader.
- **`ongoingOps`** — active assignments, recent Worker deploys, GLM fleet readiness, and the public
  tokens-served counter.

Every reader is best-effort: an absent/failing source degrades that one bucket to empty/`null`
("honest absence over fabrication"), never a fabricated value. The result is bounded, owner-only, and
public-safe-internally.

**Crucially, this is a snapshot of *summaries* pre-computed by the route and dropped into the prompt as
text.** It is not a capability Artanis can invoke. He cannot ask for a file that wasn't pre-fetched, he
cannot follow a reference, and he cannot widen the window. If the roadmap doc's *contents* aren't in the
awareness block (and they are not — awareness only carries the roadmap *ref* and a one-line summary),
Artanis cannot read it.

### What Artanis CANNOT do (today)

- **Read an arbitrary repo file** (e.g. the roadmap doc, a source file, an issue body) on demand.
- **Call any tool / function** at all — there is no tool-use loop in the turn.
- **Dispatch work** to the owner's linked Codex accounts (the Khala→Pylon→Codex path).
- **Run code / open a sandbox / execute anything.**
- **Take any action** — open/close issues, comment, commit, merge, deploy. The core is explicitly
  "PROPOSE and CONVERSE; it never grants new authority."

The persona prompt enforces honesty about this: *"Never claim you filed, deployed, submitted,
published, paid, or merged something you did not actually perform."* That is why Artanis told the truth
about not being able to read files — the prompt is doing its job; the **capability simply isn't wired**.

---

## 2. Why — the precise gap

`artanisOperatorTurn` (`artanis-operator.ts`, lines ~406-442) does exactly one thing with the model:

```ts
const outcome = yield* Effect.exit(input.khalaClient(request))
// ... on success:
return { reply: served.content, servedVia: 'openagents_khala', ... }
```

It makes a **single** Khala call and returns `served.content` (text). There is:

- **No tool-use loop.** It never inspects `served.toolCalls`, never defines tools, never executes a
  tool and feeds the result back for another turn.
- **No repo/file read** wired into the turn or the route.
- **No dispatch seam** to Pylon/Codex from the operator turn (the route only injects read-only
  awareness readers and a memory store).
- **No execution.** The only "intent" handling is `mentionsSpendOrDestructive(...)` — a bounded,
  documented audit over the owner's last message that flips a `deferredToApprovalGate` *hint* for the
  UI. It routes nothing and grants nothing.

**Important enabling finding:** the inference substrate is *already tool-capable*. In
`inference/provider-adapter.ts`, `InferenceResult` carries an optional OpenAI-compatible
`toolCalls?: ReadonlyArray<InferenceToolCall>` (populated when `finishReason === 'tool_calls'`), and
`InferenceRequest.passthroughParams` is an open `Record<string, unknown>` that already forwards
provider params verbatim — so `tools` / `tool_choice` can be passed through to Khala today. The missing
piece is **not** the model or the wire format. The missing piece is the **agentic loop + the tool
definitions + the tool executors** inside the operator turn. That is the single primitive whose absence
explains every "Artanis can't…" above.

The substrate to *act* also already exists elsewhere in the repo — it's just not connected to the chat
turn:

- The **Khala→Pylon→Codex Coding Delegation Runbook** (`AGENTS.md`, "Khala -> Pylon -> Codex Coding
  Delegation Runbook") is the documented mechanism to route real coding/repo work to the owner's linked
  Codex accounts and run it locally or in a sandbox (`pylon codex accounts list`, `pylon khala request
  --workflow codex_agent_task --repo OpenAgentsInc/openagents --branch main --commit <sha> --verify
  ...`, `pylon assignment run-no-spend`). The local Codex runner already executes with sandbox
  `danger-full-access` / approval `never` as an **owner-local executor invariant** — exactly "run work
  locally or in a sandbox."
- The **#6355 parallel backlog-burndown loop** is the operator runner that, given the roadmap or an
  explicit list, dispatches `codex_agent_task` across the connected Codex accounts and applies a verify
  → review → merge policy (the roadmap notes "Codex is the master coding agent for now").
- `artanis-labor-requester.ts` (`runArtanisLaborRequestTick`, `assertArtanisLaborPublicSafe`,
  `validateArtanisLaborProposal`) and `artanis-work-routing.ts` already model public-safe labor
  request/dispatch proposals.
- `artanis-approval-gates.ts` already defines the typed risky-action ledger
  (`ARTANIS_RISKY_ACTION_KINDS`: `pylon_job_dispatch`, `deployment`, `wallet_spend`, `settlement`,
  `runtime_promotion`, …) with operator-approval requirements and rollback posture — the boundary that
  must wrap any acting tool that spends or is destructive.

In short: **awareness reads state, the runbook/#6355 loop can act, the approval gates can gate — but
the chat turn wires none of these to Artanis.** He is a well-grounded narrator of a machine he cannot
touch.

---

## 3. The path to real agency (mapped to the existing architecture)

The whole path hangs off **one** net-new primitive: a typed tool-calling loop in `artanisOperatorTurn`.
Everything else is a tool plugged into that loop, reusing code that already exists.

### 3.1 Tool-calling loop in the operator turn (the missing primitive)

Turn `artanisOperatorTurn`'s single completion into a bounded agentic loop:

1. Build the request as today, **plus** a typed `tools` array in `passthroughParams` (the substrate
   already forwards it; `InferenceResult.toolCalls` already returns calls).
2. If `served.toolCalls` is present: execute each tool via an **owner-scoped, typed dispatch table**,
   append the tool result as a `tool` message, and call Khala again.
3. Repeat up to a bounded max-iteration cap; otherwise return `served.content` as the final reply.

Constraints (preserve the existing invariants):

- **Owner-scoped only.** Tools run under the authenticated owner id; same gating as the route today.
- **Khala-powered.** Still `openagents/khala` every iteration — the loop *increases* dogfood usage,
  it doesn't bypass it.
- **Read tools run free; spend/destructive tools do not execute** — they produce an
  `artanis-approval-gates` pending gate and report it back, exactly like `deferredToApprovalGate` does
  today but as a real, typed gate record rather than a hint string.
- **Honest absence preserved.** A tool that finds nothing returns "(none)", never invention.

This is the concrete realization of the #6359 roadmap line: *"a tick action that selects + dispatches
the #6355 work within bounded authority (read / dispatch own-capacity Codex / verify / merge non-spend
code / open issues), escalating only spend/destructive via approval gates."* The chat turn is the
human-facing front of that same authority.

### 3.2 Repo / file read tool (fixes the immediate complaint)

Give Artanis a `read_repo_file(path)` / `list_repo_dir(path)` tool, scoped to the **public**
`OpenAgentsInc/openagents` repo. Two viable backends, not mutually exclusive:

- **GitHub contents API** (read-only, public repo) for direct, low-latency reads of files like
  `docs/khala/2026-06-26-khala-open-issues-master-roadmap.md`. Simplest first step; no Pylon needed.
- **A Pylon/Codex read task** for anything needing a working tree (grep across files, follow imports).

Either way, the moment this tool exists, the owner's original ask — *"read the roadmap and help me
parallelize"* — works: Artanis reads the doc himself, then reasons over it.

Bound it: public repo only, path allow-list/size cap, never read `.secrets/`, `.env`, wallet/mnemonic
material, or private repos (mirror the redaction discipline already in `artanis-approval-gates.ts`'s
`unsafeApprovalRefPattern`).

### 3.3 Khala self-calls (already true; the loop makes it useful)

Artanis is already Khala-powered. A tool loop simply lets him **chain** Khala calls — read a file,
reason, query issues, reason again — instead of one shot. No new provider wiring; pure reuse of the
existing `khalaClient` seam, still metered as Khala usage.

### 3.4 Dispatch to the owner's linked Codex accounts (the parallelization engine)

Give Artanis a `dispatch_codex_task` tool that drives the **Khala→Pylon→Codex runbook** / the **#6355
loop**:

- Inputs are **public-safe and bounded**: a public issue number, public file paths, a public
  verification command, pinned `repo`/`branch`/`commit` (per the runbook's "real repository work"
  form).
- It selects an available linked Codex account (`pylon codex accounts list` →
  `capability.pylon.local_codex`, `readiness.state: ready`), publishes capacity, dispatches
  `codex_agent_task`, and runs `assignment run-no-spend` — i.e. work runs **locally / in the owner's
  sandbox** under the owner-local `danger-full-access` executor invariant.
- **`pylon_job_dispatch` is a risky-action kind** in `artanis-approval-gates.ts`. So the first dispatch
  in a window (or any dispatch beyond a pre-approved scope) produces a pending approval gate; once the
  owner approves the scope, Artanis dispatches within it autonomously — which is precisely the #6359
  design (autonomous within bounded authority, escalate only the gated edge).
- Verify → review → merge non-spend code follows the #6355 verify/merge policy; merges of non-spend
  code are inside bounded authority, deploys/settlement stay gated.

This is how a tool-enabled Artanis **parallelizes the roadmap**: instead of one agent serially chewing
the backlog, Artanis fans out N `codex_agent_task` assignments across the linked Codex accounts (the
runbook's `OPENAGENTS_PYLON_CODEX_CONCURRENCY` parallel form), verifies each closeout against exact
token rows + traces, merges the green ones, and refills — keeping the public counter honestly
incrementing. That is the #6355 loop, driven from the operator channel.

### 3.5 Sandbox / local execution

Covered by 3.4 for coding work: the Pylon/Codex runner *is* the sandboxed/local executor
(`danger-full-access`, approval `never`, owner-local). A separate "Artanis runs arbitrary shell" tool is
**not** recommended — it would bypass the verify/merge discipline and the approval ledger. Keep
execution flowing through the typed `codex_agent_task` path so every action has a closeout, exact token
rows, and a public-safe trace.

---

## 4. Status of the parallelization ask

- The roadmap (`docs/khala/2026-06-26-khala-open-issues-master-roadmap.md`) is the live backlog: the
  #6316 serving track (#6320 throughput, #6311 durable fleet, #6323 NVFP4 pilot, #6318 scheduler,
  #6317 stress, #6312 benchmark) plus #6355/#6356/#6357/#6358 and the Artanis autonomy epic #6359.
- It is currently advanced largely by a single supervised dispatch lane (one agent driving Pylon/Codex
  slices), which is why it is going slowly relative to the owner's intent.
- A **tool-enabled, Codex-dispatching Artanis is exactly the fix**: he reads the roadmap himself
  (3.2), selects the next bounded issues, and fans them out across the linked Codex accounts via the
  #6355 loop (3.4) — parallel instead of serial — verifying and merging as closeouts land. This is
  not new scope; it is the literal #6359 mandate ("Drive the parallel burndown loop (#6355) — dispatch
  / verify / merge across connected Codex accounts, keep the counter honestly incrementing") finally
  given the hands to do it.

Until the tool loop lands, the honest status is: **Artanis can advise on parallelization but cannot
execute it; the burndown remains gated on a human/agent manually running the runbook.**

---

## 5. Recommendations & issue triage

Ordered by leverage. All under epic **#6359**; all owner-scoped, Khala-powered, with spend/destructive
gated by `artanis-approval-gates`.

1. **#6364 — Artanis tool-calling loop in `artanisOperatorTurn`** (the enabling primitive). Typed
   owner-scoped tool dispatch table; bounded max iterations; read-tools execute, spend/destructive
   tools emit a pending approval gate instead of executing. Reuses the existing
   `passthroughParams.tools` + `InferenceResult.toolCalls` substrate.
2. **#6365 — Artanis repo-read tool** (`read_repo_file` / `list_repo_dir`, public
   `OpenAgentsInc/openagents` only, path/size bounded, secret-path denylist). Directly fixes the
   owner's complaint; smallest first slice; can ship on the GitHub contents API before the full loop is
   polished.
3. **#6366 — Artanis dispatches the #6355 Codex burndown loop** (`dispatch_codex_task` tool driving
   the Khala→Pylon→Codex runbook across linked Codex accounts, parallel; verify → review → merge
   non-spend; `pylon_job_dispatch` gated). This is the parallelization engine for the roadmap.
4. **#6367 — Artanis sandbox / local execution policy** — document that owner-local sandboxed
   execution flows *only* through the typed `codex_agent_task` Pylon/Codex path (no arbitrary-shell
   tool), with closeout + exact-token + trace evidence, and wire the verify step into the loop.

The recommended **next concrete step**: ship #6365 first (repo-read tool on the GitHub contents API)
behind the minimal #6364 loop, so the owner can immediately ask Artanis to read the roadmap and reason
over it; then land #6366 to turn that reasoning into parallel Codex dispatch.

---

## 6. Invariants honored

- **Owner-scoped:** all tools run under the authenticated owner id; the channel stays admin-gated.
- **Khala-powered:** every loop iteration is an `openagents/khala` call (more dogfood usage, never a
  bypass).
- **Spend/destructive owner-gated:** dispatch, deploy, settlement, wallet, runtime-promotion remain
  `artanis-approval-gates` risky-action kinds; read tools run free, acting tools escalate.
- **No secrets:** no tokens, mnemonics, payout targets, raw prompts, or private-repo content in tools'
  inputs/outputs or in this doc; redaction discipline mirrors the existing approval-gate patterns.
- **Honest absence over fabrication:** preserved end-to-end; a tool that finds nothing says so.
