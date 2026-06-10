# Claude Agent SDK × Pylon — "Pylon Can Talk To Your Local Claude"

Date: 2026-06-10 (evening)

Status: originally a design-only audit; the design is now implemented.
The build sequence below was executed the same evening as issues #4718
(SDK dependency, BYOK probe, capability declaration), #4719 (bounded
executor gate), and #4720 (dispatch work class + bounded real-task
smoke), all merged to main under epic #4717. The companion product
promise `pylon.local_claude_agent_bridge.v1` (added at registry
`2026-06-10.21`) remains yellow on one blocker: the live-device run of
the bounded task smoke (`apps/pylon/docs/claude-agent-task-smoke.md`,
leg 2 — needs an operator-credentialed machine). The design content
below is kept as written; the implementation followed it closely.

## The directive

Owner directive tonight: add support for the Claude Agent SDK into Pylon
so Pylon can command the user's local Claude using the TypeScript SDK.

This is the execution leg of the Autopilot Coder target sentence audited
earlier today
(`2026-06-10-autopilot-coder-full-flow-audit.md`, leg 6 — "THE GAP"):
the control plane can take, place, deliver, and review a coding work
order on production, but no lane anywhere can actually do the coding.
The Claude Agent SDK is the most direct way to close that gap on the
requester-Pylon lane, because it turns "command a local coding agent"
from process-wrangling a CLI into a typed, in-process library call with
exactly the controls (tool allowlists, hooks, permissions, sessions) the
assignment contract already demands.

## What the Claude Agent SDK is (source: code.claude.com docs)

Reference index: `https://code.claude.com/docs/llms.txt` (the complete
documentation index; fetch it before deeper exploration). Key facts as of
2026-06-10:

- **Library, not service.** `npm install @anthropic-ai/claude-agent-sdk`
  gives the same tools, agent loop, and context management that power
  Claude Code, programmable from TypeScript (Python exists too; Pylon is
  a Bun/TypeScript app, so the TypeScript SDK is the fit). The agent loop
  runs inside our process on the user's machine — which is precisely the
  requester-Pylon privacy model.
- **Bundled binary.** The TypeScript SDK bundles a native Claude Code
  binary for the host platform as an optional dependency; no separate
  Claude Code install is required.
- **Core surface.** `query({ prompt, options })` returns an async
  iterator of messages; options include `allowedTools` (e.g. `Read`,
  `Write`, `Edit`, `Bash`, `Glob`, `Grep`, `WebSearch`, `WebFetch`),
  `permissionMode`, `hooks` (`PreToolUse`, `PostToolUse`, `Stop`,
  `SessionStart`, `SessionEnd`, `UserPromptSubmit`, …), `agents`
  (subagent definitions), `mcpServers`, and `resume` (session id) for
  multi-turn context. Session state is JSONL on the local filesystem.
- **Filesystem config.** With default options the SDK loads `.claude/`
  skills/commands/CLAUDE.md from the working directory and `~/.claude/`;
  `settingSources` restricts which sources load. For a bounded worker
  lane we restrict this deliberately (see boundaries).
- **Auth.** `ANTHROPIC_API_KEY` (Console), or Bedrock / Vertex /
  Azure / Claude-Platform-on-AWS via env switches. Anthropic does **not**
  permit third parties to offer claude.ai login or rate limits in their
  products — so the Pylon lane is BYOK by policy, the same pattern sprint
  issue #4713 already sets for the user's Gemini key. Starting
  2026-06-15, Agent SDK usage on subscription plans draws from a separate
  monthly Agent SDK credit — relevant to contributor cost expectations
  and to honest copy about who pays for inference.
- **Branding law.** Product copy may say "Claude Agent", "Claude" inside
  an agents menu, or "Powered by Claude". It may **not** say
  "Claude Code" or imitate Claude Code's visual identity. Pylon copy for
  this lane should say "your local Claude" / "Claude Agent".

## Why this SDK and not another wrapper

The gap audit's open execution issue (#4661) is written "Codex-backed"
because Codex CLI was the local agent assumed available. The Claude Agent
SDK changes the calculus in three ways:

1. **Typed control instead of process scraping.** `allowedTools`,
   `permissionMode`, and `PreToolUse` hooks let the Pylon worker enforce
   the assignment contract (bounded working directory, no network tools
   unless granted, no spend) *in code*, not by trusting a CLI flag
   surface. The normalized coding assignment payload
   (`openagents.autopilot_coding_assignment.v1`) already carries
   `allowedToolKinds`, budget/timeout, and trace policy — they map almost
   field-for-field onto SDK options.
2. **In-process events.** Every tool use streams back as a message with
   structure (including `parent_tool_use_id` for subagent attribution).
   That is the raw material for honest progress submissions
   (`submitAssignmentProgress`) and for digesting public-safe closeout
   refs, without parsing terminal output.
3. **Sessions.** `resume` gives the revision loop (`request_changes` on a
   delivered work order) a real continuation primitive: the follow-up
   task can resume the session that did the work, with context intact,
   on the same device.

The lane should still be adapter-shaped: `local_claude_agent` joins
`local_codex` as peer adapters behind one execution gate, consistent with
`autopilot.codex_cloudcode_wrapper.v1` (wrap coding agents; don't marry
one).

## Where it plugs into Pylon (exact seams, current code)

The Pylon worker loop already has the execution-gate chain this slots
into — `apps/pylon/src/assignment.ts`, inside `runNoSpendAssignment`:

```ts
const runtimeGate =
  (await executeTassadarAssignment(lease, observedAtDate)) ??
  (await executeRuntimeGate(state, lease, observedAtDate))
```

Each gate inspects `lease.codingAssignment`, returns `null` if the work
class is not its own, and otherwise returns the closeout-shaped record
(artifact/blocker/build/preview/proof/result/summary/test refs plus
status and message) that flows into `AssignmentCloseout`. The Tassadar
gate (green, ran live on a real Pylon) is the proven template, including
its typed-refusal error arm.

The bridge is therefore:

- **`executeClaudeAgentAssignment(state, lease, now)`** — a third gate in
  the chain. It recognizes a coding work class in the normalized payload
  (e.g. a `runtimeGate.agentKind` of `claude_agent_sdk`, or the
  repo-change task kinds once the server dispatches them), materializes a
  bounded workspace under `state.paths.cache` (the `executeRuntimeGate`
  pattern), runs `query()` with assignment-derived options, and digests
  the message stream into refs.
- **Capability ref: `capability.pylon.local_claude_agent`** — declared in
  the runtime capability defaults when the SDK and credentials are
  actually present, alongside the existing
  `capability.pylon.local_codex` / `capability.pylon.local_coding_agent`
  refs that OA-AUTO-015 already modeled on the placement side. Admission
  then comes free: `computeAssignmentAdmission` already blocks
  `wrong_capability`, and the Tassadar readiness audit's
  admission-hardening lesson applies — the dispatch payload must carry
  the capability in `requiredCapabilityRefs`, not rely on server-side
  gating alone.
- **Typed degradation.** When the SDK is not installed, the key is
  absent, or the platform is unsupported, the gate returns a
  `blocker.assignment.claude_agent_unavailable`-class refusal — never a
  silent fallthrough to a fake closeout.
- **Server side already done.** Placement, lease creation, the normalized
  payload, closeout ingestion to `delivered`, and review all exist and
  ran live (#4633). No Worker-side schema change is required to pilot
  this lane in no-spend mode; the dispatch script just needs to emit the
  work class and capability ref.

## Option mapping (assignment contract → SDK)

| Assignment payload field | SDK option |
| --- | --- |
| `allowedToolKinds` | `allowedTools` (default bounded set: `Read`, `Edit`, `Write`, `Bash`, `Glob`, `Grep`; no `WebSearch`/`WebFetch` unless the assignment grants network) |
| bounded workspace / repo refs | `cwd` of the materialized workspace + `PreToolUse` hook rejecting any `file_path`/command escaping it |
| budget / `timeoutSeconds` | turn/iteration cap on the message loop + wall-clock abort |
| trace policy (no raw prompts/payloads off-device) | digest messages to refs; full JSONL session stays local as operator evidence |
| acceptance criteria refs | test command run via `Bash` after the agent finishes; exit code → proof/test refs (the `executeRuntimeGate` `runCommand` pattern) |
| closeout schema | `PostToolUse` hooks accumulate edited-file and command evidence for artifact/build refs |
| revision follow-up | `resume: sessionId` (session ref stored locally, only a hashed ref leaves the device) |
| config isolation | `settingSources` restricted so the worker does not load `~/.claude` user config into delegated work by default |

## Boundaries (the law for this lane)

1. **The user's Claude, the user's key, the user's machine.** Pylon never
   ships or proxies platform credentials to the device; BYOK only
   (`ANTHROPIC_API_KEY` or the user's Bedrock/Vertex/Azure config). No
   claude.ai login brokering — Anthropic's terms forbid it and the
   sprint's identity law (#4713: "the user's identity AND the user's
   inference") already says the same thing for Gemini.
2. **Redaction law unchanged.** `assertPublicProjectionSafe` already
   guards every progress/artifact/closeout POST in `assignment.ts`. Raw
   SDK messages, prompts, file contents, provider payloads, and local
   paths never leave the device; closeouts carry refs only. The SDK
   session JSONL is operator-local evidence, exactly like the gap
   audit's "operator-only evidence retention" line.
3. **Authority unchanged.** The gate inherits every existing boundary:
   worker closeout is not accepted work; no settlement, payout, deploy,
   spend, or Forum publication authority. `payoutClaimAllowed: false`
   in no-spend mode, wallet-readiness admission in paid mode — all
   already enforced by the loop.
4. **Copy law.** "Pylon can talk to your local Claude" / "Claude Agent" —
   never "Claude Code", never "autonomous", never "shipped" before the
   receipts exist. The promise's `unsafeCopy` binds this.
5. **Packaging honesty.** The SDK (and its bundled binary) is heavy and
   platform-scoped; it must be an optional/lazy dependency so the
   packaged Pylon stays installable everywhere, and the npm publish
   story (#4654, shared with nip90/tassadar-executor) must answer for it
   before the lane ships in a release. A Pylon without the SDK simply
   never declares the capability.

## Relation to open issues and promises

- **#4661** (packaged-binary real-task runtime smoke): this lane is a
  second adapter for the same acceptance — a real repo checkout, change,
  and verifiable test output executed by the installed binary's worker
  loop. Recommendation: keep #4661's work class adapter-agnostic and
  let `local_codex` and `local_claude_agent` both satisfy it; whichever
  lands first clears
  `blocker.product_promises.live_probe_pylon_runtime_gates_incomplete`.
- **#4713** (agent identity, memories, model adapters): the Claude Agent
  SDK is also a *model adapter* candidate for the Pylon agent surface
  (compose forum posts with local Claude instead of Gemini), but that is
  a separate, smaller use than the coding-executor lane; do not conflate
  them in implementation.
- **Promises:** `pylon.local_claude_agent_bridge.v1` (new, yellow) is the
  named promise for this lane. It stands on
  `compute.tassadar_executor_poc.v1` (green — the execution spine) and
  feeds `autopilot.codex_probe_pylon_successor.v1` (yellow — runtime
  gates) and `autopilot.codex_cloudcode_wrapper.v1` (yellow — wrap
  coding agents). It does not by itself move
  `autopilot.free_coding_task_beta.v1` or the paid lane; those still
  need the payment-movement and writeback work in the full-flow audit.

## Build sequence (when implementation is authorized)

1. Add `@anthropic-ai/claude-agent-sdk` to `apps/pylon` as an
   optional/lazily-imported dependency; capability auto-declared only
   when import + credential probe succeed.
2. `executeClaudeAgentAssignment` gate behind the existing chain, with
   the bounded-workspace, hook-enforced sandbox, ref-digesting closeout,
   and typed refusal arms; unit tests mirroring the five Tassadar gate
   tests (recognize, execute, refuse, passthrough, redaction).
3. A repeatable no-spend smoke in `apps/pylon/docs/` driving one real
   fixture-repair task through the live assignment API from the packaged
   binary (this is #4661's acceptance, satisfied via this adapter).
4. Release-gate integration: install smoke must pass with and without
   the SDK present; launch-gates copy line added.
5. Then, and only then, flip the promise blockers with receipts.

## One-sentence truth

The Claude Agent SDK gives Pylon a typed steering wheel for the user's
local Claude, the worker loop already has the socket it bolts into, and
tonight this lane exists as an audited design plus a yellow promise —
not as shipped software.
