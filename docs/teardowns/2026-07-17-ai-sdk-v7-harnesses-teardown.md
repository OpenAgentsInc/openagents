# AI SDK v7 Harnesses Teardown — 2026-07-17

Read-only architecture and implementation audit of the experimental AI SDK v7
harness system in the public `vercel/ai` repository. The audit used an exact,
freshly fast-forwarded commit and followed the implementation from
`HarnessAgent` through the versioned adapter contract, sandboxes, bridge
transport, five adapters, approval and continuation state, AI SDK stream
projection, Workflow DevKit slices, UI, and TUI. It did not create a Vercel
sandbox, start a harness runtime, send a model request, or execute upstream
tests.

## TL;DR

AI SDK v7 Harnesses introduce a second runtime abstraction beside models. A
provider gives `generateText` or `streamText` a model call. A harness adapter
gives `HarnessAgent` a complete stateful coding-agent runtime—Claude Code,
Codex, OpenCode, Deep Agents, or Pi—with native history, built-in tools,
permissions, compaction, workspace behavior, and resume semantics. `HarnessAgent`
then projects that runtime into familiar AI SDK `GenerateTextResult`,
`StreamTextResult`, tool, usage, telemetry, UI-message, and TUI shapes. [source]

```text
application / useChat / TUI / Workflow DevKit
                      |
                 HarnessAgent
          config only; no live session state
                      |
              HarnessAgentSession
       sandbox + adapter session + approvals
                      |
      +---------------+----------------+
      |                                |
bridge-backed adapters             host adapter
Claude / Codex / OpenCode /         Pi runtime
Deep Agents inside sandbox          in host Node
      |                                |
authenticated WebSocket          sandbox-backed VFS,
event/replay bridge              files and shell tools
      +---------------+----------------+
                      |
       HarnessV1NetworkSandboxSession
 files + processes + ports + lifecycle + policy
```

The abstraction is more rigorous than a wrapper around agent CLIs:

- `HarnessV1` is a versioned adapter spec, separate from model-provider specs;
- `HarnessAgent` is a stateless definition and every call requires an explicit
  live `HarnessAgentSession`;
- the framework owns sandbox acquisition and lifecycle, while the adapter owns
  the native runtime and translation;
- native history is authoritative, so only the newest user input crosses each
  prompt turn instead of replaying the application's full message history;
- built-in runtime tools and host-executed AI SDK tools share typed tool shapes
  but retain an execution-origin distinction;
- lifecycle state separates between-turn resume from active-turn continuation;
- bridge adapters support attach, cursor replay, disk replay, and finally lossy
  rerun; and
- workflow slices preserve a single UI message across function boundaries by
  suspending near the platform deadline and reopening active stream parts in
  the next slice. [schema] [source] [test]

The strongest reusable seams for OpenAgents are the provider/harness
separation, explicit session object, adapter-owned native semantics, sandbox
provider contract, restricted tool view, content-addressed bootstrap recipes,
uniform stream projection, pending-approval continuation state, and explicit
lossless-versus-lossy recovery language. [source]

The important boundaries are just as concrete:

- built-in permissions default to `allow-all`; Codex supports no built-in
  approval or filtering and therefore requires `allow-all`; [source]
- “runs in a sandbox” depends on the selected provider. Network policy is
  optional, `HarnessAgent` does not impose one, and `just-bash` is a JavaScript
  virtual environment without network or kernel containment; [source]
- host-defined AI SDK tools execute in the host process. They receive a
  restricted sandbox handle, but their own implementation is trusted host
  code; [source]
- Pi's agent runtime, credentials, model calls, session journal, and a scoped
  workspace mirror live in the host process even though its coding tools target
  the sandbox; [source]
- bridge credentials are forwarded from the host into sandbox processes and a
  256-bit bridge token travels in the WebSocket query string; [source]
- bridge replay is best-effort, per-turn, and only as durable as an append-only
  sandbox file; corruption degrades to rerun; [source]
- lossy continuation may recompute in-flight work and therefore can repeat
  side effects; [source]
- the snapshot identity truncates SHA-256 to 16 hex characters and marker
  presence, rather than a signed artifact manifest, establishes bootstrap
  reuse; [source]
- `file-change` and compaction are projected as dynamic provider-executed tools
  for UI compatibility, which is useful but semantically lossy; [source]
- the feature has changed rapidly since June 10, including fixes for relay
  authentication, approvals, path traversal and symlink escape, usage,
  step-ending, replay, OIDC, resume credentials, and long-turn timeouts.
  [history]

The central OpenAgents decision is: **adapt the separate harness plane,
explicit session and continuation model, adapter/runtime boundary, sandbox
provider interface, restricted tool surface, typed stream projection, and
slice-aware recovery; reject permissive defaults, provider-optional
containment, host tool execution as sandboxed execution, best-effort replay as
exactly-once recovery, opaque resume state as a receipt, and UI-compatible
projection as canonical execution truth.**

## 1. Snapshot and provenance

Before inspection, the reference clone was clean on `main`. A fast-forward-only
pull confirmed it already matched `origin/main`.

| Artifact             | Identity                                                                              | What it establishes                   |
| -------------------- | ------------------------------------------------------------------------------------- | ------------------------------------- |
| Public repository    | `https://github.com/vercel/ai`                                                        | Public source and history             |
| Audited commit       | `6cd7c74acf0d7ec84dd58a841fc0e20970d6f2e8` on `main`                                  | Exact snapshot used here              |
| Commit time          | `2026-07-17T16:30:46-04:00`                                                           | Freshness of the audited tip          |
| Commit subject       | `fix: correct misleading onToolCall return-value documentation (#17260)`              | Latest repository change              |
| AI SDK version       | `ai@7.0.31`                                                                           | v7 release line                       |
| Core harness version | `@ai-sdk/harness@1.0.36`                                                              | Experimental harness release          |
| Adapter versions     | Claude `1.0.37`; Codex `1.0.38`; OpenCode `1.0.37`; Pi `1.0.36`; Deep Agents `1.0.35` | Independently released adapters       |
| License              | Apache License 2.0                                                                    | Permissive source-study boundary      |
| Repository scale     | 7,072 tracked files                                                                   | Overall monorepo scale                |
| Harness corpus       | 307 tracked implementation, test, architecture, and documentation files               | Audited feature surface               |
| TypeScript scale     | About 40,791 lines in core harness, adapters, sandboxes, and workflow harness         | Approximate implementation scale      |
| Test surface         | 72 tracked test files in those package families                                       | Executable evidence surface           |
| Development history  | 115 harness-area commits since the first specification landed on `2026-06-10`         | Very high early-stage change velocity |

The versioned specification landed June 10, initial Claude Code/Codex/Pi
adapters followed the same day, v7 shipped June 25, OpenCode and Deep Agents
arrived around the release, and durable workflow slicing landed June 23. The
audited tree is therefore a real public release, but also a system only about
five weeks old whose documentation explicitly warns of breaking changes.
[history]

### Evidence labels and limits

- **`[source]`** — tracked implementation, documentation, or manifest;
- **`[schema]`** — TypeScript, Zod, bridge, sandbox, lifecycle, or tool contract;
- **`[test]`** — a tracked unit, type, integration, or end-to-end test;
- **`[history]`** — Git history at or before the audited commit;
- **`[target]`** — current OpenAgents source at the audited target revision;
- **`[inferred]`** — reasoned from multiple observations; and
- **`[limitation]`** — a source-only audit boundary.

There are intentionally no `[runtime]` observations. Source cannot prove
production isolation, provider behavior, snapshot integrity, bridge
availability, workflow exactly-once behavior, model quality, credential
handling by external runtimes, or end-to-end user approval safety. [limitation]

## 2. Harnesses are peers to providers

The central design choice is conceptual, not syntactic. `LanguageModelV4`
describes a lower-level model interface. `HarnessV1` describes an agent runtime
that owns state and capabilities larger than a model invocation. The two reuse
AI SDK primitives where they genuinely overlap—tool calls, results, approvals,
usage, finish reasons, streams, telemetry—but the harness is not modeled as a
provider. Its metadata is named `harnessMetadata` inside the adapter contract
and rebound only when projecting into existing AI SDK result types. [schema]

`HarnessV1` has one entry point, `doStart`, plus a spec version, stable
`harnessId`, typed native built-ins, approval/filtering support flags, optional
lifecycle-state schema, and optional content-addressed bootstrap recipe. There
is deliberately no universal static capability object. Unsupported behavior is
reported at the operation that needs it. This avoids claiming uniform semantics
that runtimes do not have, though it moves some discovery from negotiation time
to failure time. [source] [inferred]

OpenAgents should preserve the peer relationship. A native agent runtime is
not a model string and should not be flattened into the provider selector. It
needs its own exact version, protocol, tool, permission, state, containment,
and recovery declarations. [inferred]

## 3. `HarnessAgent` is stateless configuration

`HarnessAgent` implements AI SDK `Agent` with version `agent-v1`, but holds no
live conversation. Construction fixes the harness, sandbox provider,
instructions, skills, merged tool surface, filtering, permission mode,
telemetry, diagnostics, and bootstrap hooks. Every `generate`, `stream`,
`continueGenerate`, or `continueStream` call requires a session from
`agent.createSession()`. [source]

```text
HarnessAgent
  immutable configuration
  merged tool types
  sandbox provider
        |
        +-- createSession(chat A) -> sandbox A + native session A
        +-- createSession(chat B) -> sandbox B + native session B
```

User tools override built-ins on name collision in the merged type and
validation surface. Filtering is computed once. If a permission mode needs
native approvals and the adapter does not support them, the setup fails rather
than pretending enforcement. [source]

The prompt contract differs from model calls. A harness owns native history. A
message array is reduced to the newest user message for a new turn; prior UI
history is not replayed. Instructions are prepended only to the first user
input of a fresh session and not re-applied after resume. That creates two
histories: opaque native state and projected application history. OpenAgents
should retain both with explicit lineage and loss accounting. [schema]

## 4. Session lifecycle is the real API

`HarnessAgentSession` owns the adapter session, network sandbox session, and a
bridge-port lease when sessions share a caller-owned sandbox. Its local states
are active, detached, stopped, or destroyed; turn states are idle, running,
awaiting approval, or suspended. A turn sequence prevents stale completion
callbacks from changing the state of a newer turn. [source]

| Operation       | Runtime                         | Sandbox         | Returned state                             |
| --------------- | ------------------------------- | --------------- | ------------------------------------------ |
| `detach()`      | parked when possible            | kept running    | resume state, possibly nested continuation |
| `stop()`        | persisted then stopped          | stopped         | resume state, possibly nested continuation |
| `suspendTurn()` | kept or interrupted per adapter | kept running    | active-turn continuation at cursor         |
| `destroy()`     | destroyed best-effort           | deleted/stopped | none                                       |

`resumeFrom` means the previous turn is complete. `continueFrom` means a turn
is unfinished and must continue without a new prompt. The framework rejects
supplying both, checks harness ID, spec version, state type, and adapter schema,
then passes the opaque state to the adapter. Framework-owned pending approvals
live outside adapter `data` in continuation state. This is a strong division,
but the payload remains a resume handle—not proof of admission, policy,
effects, or non-duplication. [schema] [test] [inferred]

## 5. The sandbox is a two-tier capability

The basic `Experimental_SandboxSession` exposes files and processes. The
harness `HarnessV1NetworkSandboxSession` adds stable resource ID, default work
directory, ports, URL resolution, lifecycle, optional port replacement,
optional network policy, and `restricted()`. A provider creates or resumes it.
[schema]

`restricted()` returns only file/process methods to host-executed tools. That
prevents a tool from stopping the sandbox, changing egress, or exposing a port
through the received object. It does not itself enforce filesystem scope,
process isolation, network isolation, quotas, or secret boundaries. Those are
provider responsibilities. [source] [inferred]

| Provider       | Files/processes                            | Ports/resume | Containment meaning                                   |
| -------------- | ------------------------------------------ | ------------ | ----------------------------------------------------- |
| Vercel Sandbox | remote VM/session API                      | supported    | platform isolation and optional native network policy |
| Just Bash      | in-memory JS filesystem and bash emulation | unsupported  | local emulation, not kernel or network isolation      |

Vercel sessions default to 30 minutes. Named sessions permit cross-process
lookup. Snapshot creation uses a persistent template keyed by bootstrap
identity and forks a per-session sandbox. Caller-owned sandboxes retain
lifecycle ownership and may declare a bridge-port pool. [source]

Network policy supports allow-all, deny-all, and custom host/CIDR allowlists
with CIDR deny precedence, but the method is optional and `HarnessAgent` does
not require or set it. Thus the docs' sandbox claim is conditional on exact
provider and configuration. [source] [inferred]

## 6. Bootstrap identity and templates

Bridge adapters declare a recipe with adapter ID, infrastructure directory,
files, and sequential commands. The framework hashes recipe contents and a
schema version, uses the first eight bytes of SHA-256 as a 16-character
identity, and writes `.bootstrap-<identity>.ok` after success. Consumer
`onBootstrap` requires an explicit `bootstrapHash`; `onSession` runs after each
fresh or resumed acquisition. Infrastructure belongs outside the user's work
directory. [source]

This is a clean cache invalidation mechanism, not supply-chain proof. It does
not bind base image, package resolution, downloaded artifacts, command output,
runtime binary, signer, or policy. OpenAgents already strengthens the seam by
combining the upstream recipe identity with exact base image, repository,
toolchain, agent setup, sandbox profile, and lockfile references. It should add
signed provenance and observed bootstrap receipts. [source] [target]

## 7. Bridge-backed runtime architecture

Claude Code, Codex, OpenCode, and Deep Agents install a Node bridge inside the
sandbox. The host obtains an exposed port, applies the recipe, generates a
random 32-byte token, spawns the bridge with runtime credentials, resolves a
WebSocket URL, authenticates with `agent_bridge_token` in the query, and
translates typed frames. [source]

The shared bridge listens on `0.0.0.0`, accepts only the token, allows one
active authorized socket, and replaces a stale socket on reconnect. It routes
tool results, approvals, user messages, abort, interrupt, detach, shutdown, and
resume while the adapter drives its native SDK or CLI. [source]

```text
host HarnessAgent                     sandbox
      |                                  |
      | wss://port/?agent_bridge_token   |
      +------------------------------> bridge
      |<-- seq event log / replay -------+
      |                                  +--> native SDK/CLI
      |-- tool result / approval -------->
```

The token is strong and rotates per spawn, but query-string transport risks URL
logging, and forwarding model credentials gives the sandbox runtime access to
them. OIDC reduces static-secret lifetime without removing the boundary.
OpenAgents should prefer private authenticated transport, proof-of-possession
workload identity, audience-limited credentials, and redaction tests.
[inferred]

## 8. Reconnect, replay, and rerun

The bridge assigns process-wide monotonic sequence numbers. Host
`SandboxChannel` validates frames, serializes dispatch, buffers until listeners
exist, tracks `lastSeenEventId`, retries transient disconnects for 30 seconds,
and sends `resume(lastSeenEventId)` after reconnect. [source]

The bridge retains only the current or just-finished turn in memory. It also
appends events asynchronously to `event-log.ndjson` in the sandbox. Replay-mode
respawn loads this file before the first resume. A new turn clears both replay
logs but keeps sequence numbers monotonic. Corrupt or partial disk logs are
discarded and recovery falls back to rerun. [source]

Recovery therefore has four levels:

1. **attach** to the same live bridge and replay after the cursor;
2. **disk replay** from the persisted event tail;
3. **rerun** by resuming native thread state and re-driving work; or
4. failure when insufficient state remains.

`SandboxChannel.suspend()` freezes dispatch, drains queued frames, closes the
host socket, and returns the last delivered cursor while the bridge continues.
This is a thoughtful gap/duplicate boundary. But rerun is explicitly lossy and
may repeat commands, writes, or external calls. OpenAgents should require an
effect ledger, lease/fence, exact durable cursor, and owner-visible replay
decision before re-driving privileged work. [source] [test] [inferred]

## 9. Pi is a different architecture

Pi runs as a host library, not an in-sandbox bridge. Auth, model registry,
settings, and journals live under a temporary host root. A process-global VFS
maps the sandbox work path to a scoped host mirror so Pi can discover `.pi`,
`.agents`, and `AGENTS.md`; the full tree is not mirrored. File, search, edit,
and bash tools are reimplemented against the restricted sandbox with path
mapping and realpath checks. [source]

Recent history is material: path traversal was fixed June 26 and the audited
tip contains a July 17 symlink-escape fix. Same-process approval pauses can park
a live Pi session in a module-global map; cross-process continuation persists a
journal to the sandbox and reruns the in-flight tail. [history] [source]

Thus “operates in a sandbox” means Pi's workspace effects target the sandbox;
the runtime and model call remain on the host. Security claims should say so.
[inferred]

## 10. Tools, filtering, and approvals

Adapters declare native built-ins with common names where possible.
`providerExecuted: true` means the runtime already performed a call. Host AI
SDK tools are validated and executed in the host, receive the restricted
sandbox in their context, and send results back to the runtime. [schema]

Filtering either hides a built-in natively or auto-denies it through the
approval path. If neither works, filtering throws. Inactive host tools are not
sent and unexpected calls receive execution-denied results. [source]

Built-in permission modes are `allow-all` (the default), `allow-edits`, and
`allow-reads`. Custom tools separately use AI SDK approval statuses. A required
approval emits a stream request, pauses with finish reason `tool-calls`, stores
the obligation in continuation state, and resumes after a later message
contains the response. [source] [test]

These are interoperability mechanisms, not target authorization. Tool kind,
approval, and a restricted object do not prove deterministic policy or OS
enforcement. OpenAgents should compile them into its own capability and
containment policy and receipt every decision. [inferred]

## 11. Stream projection

`HarnessV1StreamPart` mirrors AI SDK model parts for text, reasoning, tools,
approvals, step finish, turn finish, usage, and errors. Harness additions are
native tool name, opaque file changes, compaction, warnings, and metadata.
`runPrompt` validates calls, executes host tools, coordinates approvals, strips
work-directory prefixes from display paths, builds telemetry, and fills a
custom `StreamTextResult`; `generate()` drains the same machinery. [schema]

File changes and compaction lack native UI parts, so translation emits dynamic,
provider-executed tool parts. That is effective compatibility but a lossy
semantic envelope. A visible `fileChange` does not establish a complete diff,
cause, authorization, or durable receipt. [source] [inferred]

## 12. Adapter matrix

| Adapter     | Placement      | Native state                     | Approval/filtering          | Continuation notes                                       |
| ----------- | -------------- | -------------------------------- | --------------------------- | -------------------------------------------------------- |
| Claude Code | sandbox bridge | Claude SDK conversation/workdir  | both supported              | attach/replay then SDK continue; manual compaction       |
| Codex       | sandbox bridge | Codex thread ID/workdir          | neither for built-ins       | thread resume/rerun; automatic compaction only           |
| OpenCode    | sandbox bridge | OpenCode session ID/server       | approvals; denial filtering | attach/replay or session resume                          |
| Deep Agents | sandbox bridge | live LangGraph `MemorySaver`     | approvals; denial filtering | stopped conversation cannot resume; no manual compaction |
| Pi          | host process   | host journal + sandbox workspace | both supported              | same-process park; cross-process journal rerun           |

All five accept custom host tools and skills. Uniform APIs do not imply uniform
guarantees; every run should retain adapter, version, placement, approval,
filtering, compaction, stop/resume, and continuation-loss facts. [source]

## 13. Workflow slices fit long turns into serverless execution

`@ai-sdk/workflow-harness` provides a serializable state machine and slice
runner. State holds session ID, prompt, approval messages, status, resume state,
continuation, stream context, final result, and error. [schema]

The default slice budget is 750 seconds, leaving margin before an expected
roughly 800-second Fluid Compute recycle. At timeout it suspends the turn,
keeps the sandbox/runtime live, persists continuation, and leaves workflow
output open. The next step creates from `continueFrom` and calls
`continueStream()` without resending the prompt. [source]

To preserve one UI assistant message, it drops repeated starts and intermediate
finishes, records active text/reasoning/tool-input parts, emits temporary closes
at boundaries, and reopens required parts before later deltas. Only terminal
completion or approval closes the client stream. Normal completion detaches for
the next user turn unless one-shot destruction is requested. [source] [test]

This is an excellent projection state machine, but durability belongs partly to
Workflow state and partly to a warm sandbox. A cursor does not make effects
exactly once, and rerun may duplicate them. OpenAgents should put slicing atop
durable admitted work, leases, fencing, idempotent effects, and replayable
canonical events. [inferred]

## 14. UI, TUI, telemetry, and diagnostics

Harness streams feed `toUIMessageStream`, `useChat`, and typed tool renderers.
The chat ID becomes the stable harness/sandbox ID and the server persists opaque
resume state rather than replaying all messages. `@ai-sdk/tui` needs a small
wrapper that injects one live session into the base Agent interface. [source]

Turns also drive AI SDK-compatible operation, step, and tool telemetry. Optional
bridge diagnostics forward structured events and captured sandbox console
lines. Recent fixes for step numbering, finish-step boundaries, usage totals,
opaque errors, missing reasoning, approvals, and long turns show both the value
and fragility of normalization. OpenAgents should retain native events beside
versioned projection events and explicitly redact diagnostics. [history]

## 15. OpenAgents reconciliation

OpenAgents already has two target-native sandbox-provider implementations:

- `@openagentsinc/ai-sdk-sandbox-local` is explicitly an owner-local fixture,
  scopes paths to a temporary workspace, supplies explicit account homes, and
  disclaims kernel, network, and multi-tenant containment.
- `@openagentsinc/ai-sdk-sandbox-openagents` delegates lifecycle, files,
  processes, ports, and egress to `openagents.sandbox.v1`; requires explicit
  account homes; binds snapshot identity to base image, repo, toolchain, agent
  setup, profile, lockfiles, and bridge recipe; scopes paths to the workspace;
  and rejects missing or allow-all egress in public/untrusted lanes. [target]

Both are pinned to `@ai-sdk/harness@1.0.18`; audited upstream is `1.0.36`.
Intervening history includes filtering, replay, approval, CLI relay, resume
credential, error, finish-step, and Pi symlink hardening. Interface compatibility
does not prove semantic compatibility across that gap. [history] [target]

The next step requires a separately admitted upgrade packet, not a research-lane
version bump. It should diff every contract and behavior from `1.0.18` to
`1.0.36`, test all adapters against both target providers, preserve public-lane
denials, and exercise attach/replay/rerun and approval continuation. [inferred]

## 16. Adapt / study / reject

| Mechanism                             | Stance                         | OpenAgents-native interpretation                                                          |
| ------------------------------------- | ------------------------------ | ----------------------------------------------------------------------------------------- |
| Provider/harness separation           | Adapt                          | Distinct model and runtime identity, version, policy, events, and receipts                |
| Stateless agent plus explicit session | Adapt                          | One engine definition; explicit work/session handle per conversation                      |
| `HarnessV1` adapter seam              | Adapt with stronger boundaries | Version negotiation, native event retention, declared loss and capabilities               |
| Sandbox provider/session split        | Adapt                          | Thin adapter over authoritative workroom lifecycle and isolation                          |
| Restricted tool view                  | Adapt with stronger boundaries | Object attenuation plus path, process, network, secret, quota, and target policy          |
| Content-addressed bootstrap           | Adapt with stronger boundaries | Full manifest, exact refs, signatures, observed setup, and provenance receipts            |
| Attach/replay/rerun classification    | Adapt                          | Recovery class plus effect ledger, fence, cursor, and owner-visible loss                  |
| Framework-owned pending approvals     | Adapt                          | Durable Interaction state independent from opaque native runtime state                    |
| AI SDK-compatible streams             | Adapt                          | Portable projection beside a lossless native plane                                        |
| Workflow slice continuity             | Adapt                          | Durable work slicing with leases/fences and idempotent effects                            |
| Default `allow-all`                   | Reject                         | Fail closed by lane and capability                                                        |
| Codex requiring built-in `allow-all`  | Reject for public work         | Enforce beneath adapter or disable until exact approval/filtering exists                  |
| Optional network enforcement          | Reject as containment claim    | Public lanes require explicit deny-first egress below adapter                             |
| Host tools described as sandboxed     | Reject                         | Trusted host code or execution moved into admitted isolated workload                      |
| Best-effort NDJSON replay             | Reject as durability           | Transactional canonical event log and receipted cursor                                    |
| Opaque resume state                   | Reject as proof                | Resume handle plus admission, identity, effects, recovery, and outcome receipts           |
| Dynamic tool projection               | Reject as canonical fact       | UI compatibility only; exact file/diff/compaction lineage remains authoritative elsewhere |

## 17. Recommended follow-up

1. Pin exact source and package tarballs for `1.0.18` and `1.0.36`; produce a
   spec, lifecycle, and behavior compatibility matrix.
2. Upgrade OpenAgents adapters only under a separately admitted packet with
   exact package digests and generated contract tests.
3. Exercise all five adapters independently; do not generalize one adapter's
   approval, compaction, or recovery proof.
4. Require public/untrusted egress policy below the adapter and verify bridge
   ingress is private, authenticated, short-lived, and redacted.
5. Bind sessions to work unit, sandbox generation, exact ref/tree, workload
   identity, capability set, and engine generation.
6. Preserve native and projected events with exact translator version and loss
   records.
7. Fault-test socket drop, bridge crash, corrupt log, process recycle, expired
   sandbox, approval pause, stale continuation, and duplicate rerun effects.
8. Make rerun an explicit owner-visible recovery decision with idempotency
   evidence.
9. Keep owner-local fixture claims separate from public containment claims.
10. Gate “runs in a sandbox” on exact adapter placement, tool location,
    credential path, and enforced profile.

## 18. Bottom line

AI SDK v7 Harnesses are the strongest public attempt yet to make complete
coding agents first-class AI SDK agents without pretending they are model
providers. The architecture correctly centers explicit sessions, native state,
adapter translation, sandbox ownership, resumable lifecycle, tool-origin
distinctions, and compatible output. Its replay and workflow slicing show
unusually careful thought about long-running serverless turns.

The abstraction does not erase runtime differences. Codex cannot enforce the
same built-in permissions as Claude Code or Pi. Deep Agents loses stopped
conversation state. Pi runs on the host. Attach can be lossless while rerun is
not. Vercel Sandbox and Just Bash do not mean the same thing by sandbox. The key
lesson is not “one API makes agents interchangeable.” It is **one API can carry
useful common structure while exact native placement, authority, lifecycle,
loss, and recovery guarantees remain adapter-specific facts.**

OpenAgents already chose the right seam by implementing the sandbox-provider
contract behind stricter local and public lanes. It should close the `1.0.18`
to `1.0.36` gap deliberately, preserve stronger egress and snapshot identity,
and layer harness interoperability beneath—not in place of—admitted work,
capabilities, isolation, leases, fences, canonical events, and receipts.

## Source map

Primary evidence included:

- `architecture/harness-abstraction.md` and `architecture/sandbox-abstraction.md`;
- `packages/harness/src/v1`, agent/session, bridge, `SandboxChannel`, stream
  projection, bootstrap, validation, telemetry, diagnostics, and tests;
- the Claude Code, Codex, OpenCode, Deep Agents, and Pi harness packages;
- Vercel and Just Bash sandbox packages, workflow harness, and TUI;
- `content/docs/03-ai-sdk-harnesses` and provider adapter documentation;
- harness examples for functions, Next.js, workflows, and TUI; and
- Git history through `6cd7c74acf0d7ec84dd58a841fc0e20970d6f2e8`.
