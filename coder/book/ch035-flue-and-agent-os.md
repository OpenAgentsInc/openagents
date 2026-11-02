# 35. Flue and Agent OS

> **Status: written.** Read at withastro/flue@deee93b40d275ecc9a50c50af2158f20089d54bc and rivet-dev/agent-os@c7462927ec452bbd5def5c0fc50a3fd74574d13a on 2026-11-02.

Sources:

- [withastro/flue](https://github.com/withastro/flue/tree/deee93b40d275ecc9a50c50af2158f20089d54bc)
  at `deee93b40d275ecc9a50c50af2158f20089d54bc` (Apache-2.0, release v2.3.0,
  2026-10-12).
- [rivet-dev/agent-os](https://github.com/rivet-dev/agent-os/tree/c7462927ec452bbd5def5c0fc50a3fd74574d13a)
  at `c7462927ec452bbd5def5c0fc50a3fd74574d13a` (Apache-2.0, last commit
  2026-10-28).

Shallow clones were read under `projects/repos/flue` and
`projects/repos/agent-os`; nothing was installed or run. Paths below are
relative to each repository's root.

## Why study them

Both projects build the layer under an agent rather than an agent itself, and
they split it in two places Coder also splits it.

Flue is a TypeScript framework for headless agents. An agent is a function
that declares its model, sandbox, skills and tools, and the runtime gives it
sessions, durable recovery, subagents and channels (`README.md`). Its
durability contract is the most careful written statement of "what happens
to accepted work after a crash" among the agents this part reads, and that is
the question Coder chapters 2 and 20 must answer.

Agent OS is a library that boots Linux-like virtual machines inside the host
process, on V8 isolates and WebAssembly instead of microVMs (`README.md`).
Each VM has a kernel-owned filesystem, process table, sockets and permission
policy, and the host binds trusted functions into it. It is a different
answer to Coder chapter 5's sandbox, and its host functions are close to
chapter 8's bound host functions.

The workspace's reference list describes Agent OS as an agent runtime with
ACP sessions and durable transcripts. At this revision the repository has
neither: it is a VM and sandbox library (`docs/content/docs/architecture.mdx`).
This chapter reads what the revision contains.

## What Flue does

| Feature | File at the revision | Coder chapter |
| --- | --- | --- |
| An agent is a function marked `'use agent'` that calls `useModel`, `useSandbox`, `useSkill` and `useTool`, and returns its instructions | `README.md`, `apps/docs/src/content/docs/guide/building-agents.md` | 1, 4 |
| Every accepted submission reaches exactly one terminal outcome: completed, failed or aborted | `apps/docs/src/content/docs/guide/durability.md` | 2, 20 |
| Recovery from durable evidence only, with one action per kind of evidence | `apps/docs/src/content/docs/guide/durability.md`, `packages/runtime/src/submission-state.ts` | 2, 20 |
| Tool-batch repair: recorded results are kept, unresolved ordinary calls are not run again | `apps/docs/src/content/docs/guide/durability.md` | 2, 4 |
| Durable tools whose `step.do` steps are recorded once and skipped on replay | `apps/docs/src/content/docs/guide/durability.md` | 4, 20 |
| Versioned conversation records with submission, turn and attempt IDs | `packages/runtime/src/conversation-records.ts` | 2 |
| Fold checkpoints: a cache over the log, never authoritative, rebuilt by replay | `packages/runtime/src/conversation-fold-checkpoint.ts` | 2 |
| Compaction on a threshold, or after a context-overflow reply and then retried | `packages/runtime/src/compaction.ts` | 7 |
| Sandboxes: an in-memory filesystem with an emulated bash, a local directory, or a remote container; a sandbox adds `read`, `write`, `edit`, `bash`, `grep` and `glob` | `apps/docs/src/content/docs/guide/sandboxes.md`, `packages/runtime/src/sandbox.ts` | 5 |
| Skills in the Agent Skills format, disclosed progressively; `allowed-tools` parsed and not enforced | `apps/docs/src/content/docs/guide/skills.md`, `packages/runtime/src/skill-frontmatter.ts` | 15 |
| Subagents as child sessions with their own durable streams, sharing the parent's sandbox and retry budget | `apps/docs/src/content/docs/guide/subagents.md`, `apps/docs/src/content/docs/guide/durability.md` | 14 |
| On Cloudflare, one generated Durable Object class per agent; streams and submissions in its SQLite storage | `packages/runtime/src/cloudflare/flue-agent-class.ts`, `apps/docs/src/content/docs/guide/cloudflare-target.md` | 20 |

### The accepted-work contract

Every input that reaches an agent is a submission. Once admitted, the runtime
owes it exactly one durable terminal outcome, `completed`, `failed` or
`aborted`, "no matter how many crashes happen in between." The outcome is a
`submission_settled` record in the conversation's stream. Submissions for one
conversation form a durable queue processed in admission order, one at a
time (`apps/docs/src/content/docs/guide/durability.md`).

Recovery works only from what was recorded. It first closes any partially
streamed assistant output as an aborted entry, then acts on the evidence: an
input that was never persisted is requeued; a completed response settles as
completed; a partial response tells the model its stream was interrupted and
continues; a tool turn with unresolved calls is repaired; a transient
provider error is retried after a backoff; a context overflow compacts and
retries; a recorded abort settles as aborted. `classifySubmissionState` in
`packages/runtime/src/submission-state.ts` is the single classifier behind
that table.

Tool-batch repair is conservative. Results recorded before the crash are
kept and never run again. An unresolved ordinary call is not re-executed,
because the runtime cannot know whether it took effect; it settles with an
unknown-outcome error. A tool marked `durable: true` instead re-runs, and each
completed `step.do` returns its recorded value without executing. The guide
names the rule "at-least-once execution over exactly-once recording."

The defaults are 10 attempts and one hour per submission, set per agent with
a `durability` static.

### The log and its caches

Each record carries an envelope with a version, IDs for the conversation,
harness and session, and optional submission, operation, turn and attempt
IDs (`packages/runtime/src/conversation-records.ts`). State is folded from
the stream. A fold checkpoint stores a folded state at a stream offset, so a
load folds only the suffix; any validation failure falls back to a full
replay with one warning, and the file states that a checkpoint is "strictly a
cache over the log — never authoritative, safe to discard"
(`packages/runtime/src/conversation-fold-checkpoint.ts`).

### Sandboxes, skills and subagents

An agent has no file or shell tools until it gets a sandbox. The lightest
sandbox is an in-memory filesystem with a bash emulated in TypeScript
(`just-bash`), isolated from the host and rebuilt for each piece of work;
heavier ones are a local directory or a remote container
(`apps/docs/src/content/docs/guide/sandboxes.md`).

A skill is a directory with a `SKILL.md`. Each mounted skill costs one
catalog line (name and description) in the system prompt, and its
instructions load only when the model activates it. The `allowed-tools`
field is parsed for compatibility and not enforced
(`packages/runtime/src/skill-frontmatter.ts`); the skills guide tells authors
to enforce authorization in their own tools.

A subagent task runs as a child session with its own durable stream, shares
the parent's sandbox, and runs inside the parent's attempt, under the
parent's retry budget and timeout (`apps/docs/src/content/docs/guide/durability.md`,
"Delegated tasks").

## What Agent OS does

| Feature | File at the revision | Coder chapter |
| --- | --- | --- |
| A VM booted inside the host process; shell, filesystem, JavaScript and Python through one object | `README.md`, `packages/core/src/agent-os.ts` | 5 |
| A trusted sidecar owns the kernel, virtual filesystem, process and socket tables and permissions; guest code runs in untrusted executors | `docs/content/docs/architecture.mdx`, `docs/content/docs/security-model.mdx` | 5 |
| A Rust kernel: process table, file descriptors, pipes, pseudo-terminals, sockets, network policy, permissions, resource accounting | `crates/kernel/src/` | 5, 37 |
| Deny by default for network and host capabilities | `docs/content/docs/security-model.mdx`, `crates/kernel/src/network_policy.rs` | 5 |
| Bounded defaults for every queue, buffer and timeout; exhaustion fails with a typed error naming the limit | `docs/content/docs/resource-limits.mdx`, `docs/content/docs/security-model.mdx` | 5, 8 |
| Host functions as CLI commands and JavaScript globals, with flags generated from Zod schemas and a JSON result envelope | `docs/content/docs/host-functions.mdx` | 8, 16 |
| Persistence of the root filesystem in SQLite, and mounts of host directories and object stores | `README.md`, `docs/content/docs/persistence.mdx` | 20 |
| Limits: a registry of cross-compiled software, no arbitrary binaries, no container runtimes, no file watching, no hardware access | `docs/content/docs/limitations.mdx` | 5 |

### One trust boundary

The security model has one boundary: sidecar and kernel on the trusted side,
executors on the other. A guest that bypasses an applied permission, mount
boundary, egress rule or resource limit is in scope; a host that chooses a
permissive configuration is not a runtime escape
(`docs/content/docs/security-model.mdx`, "Trust boundary"). Guest processes
are kernel-managed and never become host processes. Network access and host
capabilities need explicit policy. Every queue, buffer, timeout and resource
collection has a bounded default (`docs/content/docs/resource-limits.mdx`),
and running out "fails with a typed error naming the limit and how to raise
it" (`docs/content/docs/security-model.mdx`).

### Host functions

The host passes `AgentOs.create({ hostFunctions })` a record of collections.
Each collection becomes a CLI binary `agentos-{name}` and a guest global; each
function becomes a subcommand and an async function. Zod fields become flags
(`z.string()` as `--name value`, `z.boolean()` as `--flag`/`--no-flag`). A
call exits 0 and writes `{"ok":true,"result":...}` on success, or exits
non-zero with the error on standard error. Host functions run as in-process
callbacks with a 30-second default timeout, and only in the library, not on
hosted VMs (`docs/content/docs/host-functions.mdx`).

## What Coder takes

**Exactly one terminal outcome per accepted task.** Coder chapter 20 states
the contract in Flue's words: once a Worker accepts a task, the log owes it
one settled record, finished, failed or stopped, across any number of
restarts. Chapter 23's `coder status ID` reads that record, and a task with
none is still in progress or still owed recovery.

**Recover from the log, by evidence kind.** Flue's recovery table fits
chapter 2's log directly, because Coder's log already records each step's
request, reply and tool output. Chapter 20 adopts the table: close a partial
reply as aborted, keep recorded tool results, and decide each remaining case
from what the log holds, never from what the process remembers.

**Never re-run a call whose outcome is unknown.** A tool call that was
running when the process died may have taken effect. Flue settles it with an
unknown-outcome error and lets the model decide. Chapter 4's tools carry an
effect level already; a call with an effect that was not recorded as done
settles as unknown in chapter 20, and only tools that declare themselves
safe to repeat run again.

**Checkpoints are caches.** A folded state stored at an offset speeds up
loading, and a bad one falls back to replay. Chapter 2's verifier already
rebuilds every request from events; a checkpoint is an optimization the
verifier can ignore.

**Typed limit errors.** Agent OS's rule that a limit fails with an error
naming the limit and how to raise it matches chapter 8's rule that limits
refuse rather than truncate. Chapter 5's sandbox returns the same shape: the
limit's name, its value and the setting that changes it.

**Host functions as commands with a fixed envelope.** Binding a host
function as both a shell command and a typed function, with flags generated
from its schema and one JSON result shape, is a clean way to give a
model-written program or a shell session access to Coder's capabilities.
Chapter 8 binds host functions to programs; chapter 16 can serve the same
registry to a sandboxed shell as commands, with arguments checked against the
same schemas as chapter 4's tools.

**Subagents under the parent's budget.** Flue runs a child's work inside the
parent's attempt. Chapter 14's scoped subagents draw from the parent's
budget the same way, when the person has set one.

## What Coder leaves, and why

**Authoring in TypeScript hooks.** Flue's agents are TypeScript functions
with hook calls. Coder's harness logic is Coax, and the host does only
processes, files and network ([plan rules](PLAN.md#rules-for-every-chapter)).

**Workers and Durable Objects as the home of the session.** Flue's
Cloudflare target, like chapter 34's control plane, puts each conversation in
a Durable Object. Coder's session is a log on the person's machine or on
OpenAgents' own machines (chapter 20), and nothing leaves a machine without
chapter 18's opt-in.

**Default retry budgets and timeouts.** Flue settles a submission after 10
attempts or one hour by default. Coder adds no step or time limit of its own
(`coax/lib/microcoder/loop.coax`, the endings comment); limits are the
person's to set.

**Skill fields that are not enforced.** Flue carries `allowed-tools` without
enforcing it. Coder chapter 15's plugins get a grant that chapter 4's
pipeline checks on every call, so a field that names tools either restricts
them or does not exist.

**An emulated or Wasm machine for coding work.** Flue's virtual sandbox
emulates bash in TypeScript, and Agent OS runs only software from its own
cross-compiled registry, without container runtimes or file watching
(`docs/content/docs/limitations.mdx`). Coder's tasks build and test real
repositories with their own toolchains, so chapter 5 confines native
processes instead. Agent OS's own documentation points to an external
sandbox mount when a workload needs a full Linux OS.

**Hosted VMs.** Agent OS's hosted mode runs each VM as a durable instance on
Rivet Cloud or a self-hosted Rivet deployment (`README.md`, "Quickstart").
Coder runs on the person's machines and OpenAgents' own.

## Where a borrowed design is measured

- The accepted-work contract and recovery table: chapter 20 kills a Worker
  at each step of a recorded task and checks that every accepted task settles
  exactly once and that no recorded tool result runs twice.
- Unknown-outcome settlement: chapter 20 checks that a call with effects
  interrupted mid-run is never re-executed; chapter 21 measures how often the
  model recovers on its own afterwards.
- Fold checkpoints: chapter 2's verifier checks that a log loaded from a
  checkpoint and one replayed from the start produce the same requests;
  chapter 21 measures the load time saved on long logs.
- Typed limit errors: chapter 5's checks assert the error names the limit.
- Host functions as commands: chapter 8 compares a model-written program
  calling host functions as commands against its typed program form, on the
  same tasks, for task success and calls per task.

## Sources

Flue:

- [README.md](https://github.com/withastro/flue/blob/deee93b40d275ecc9a50c50af2158f20089d54bc/README.md)
- [apps/docs/src/content/docs/guide/durability.md](https://github.com/withastro/flue/blob/deee93b40d275ecc9a50c50af2158f20089d54bc/apps/docs/src/content/docs/guide/durability.md)
- [apps/docs/src/content/docs/guide/sandboxes.md](https://github.com/withastro/flue/blob/deee93b40d275ecc9a50c50af2158f20089d54bc/apps/docs/src/content/docs/guide/sandboxes.md)
- [apps/docs/src/content/docs/guide/skills.md](https://github.com/withastro/flue/blob/deee93b40d275ecc9a50c50af2158f20089d54bc/apps/docs/src/content/docs/guide/skills.md)
- [apps/docs/src/content/docs/guide/subagents.md](https://github.com/withastro/flue/blob/deee93b40d275ecc9a50c50af2158f20089d54bc/apps/docs/src/content/docs/guide/subagents.md)
- [apps/docs/src/content/docs/guide/building-agents.md](https://github.com/withastro/flue/blob/deee93b40d275ecc9a50c50af2158f20089d54bc/apps/docs/src/content/docs/guide/building-agents.md)
- [apps/docs/src/content/docs/guide/cloudflare-target.md](https://github.com/withastro/flue/blob/deee93b40d275ecc9a50c50af2158f20089d54bc/apps/docs/src/content/docs/guide/cloudflare-target.md)
- [packages/runtime/src/submission-state.ts](https://github.com/withastro/flue/blob/deee93b40d275ecc9a50c50af2158f20089d54bc/packages/runtime/src/submission-state.ts)
- [packages/runtime/src/conversation-records.ts](https://github.com/withastro/flue/blob/deee93b40d275ecc9a50c50af2158f20089d54bc/packages/runtime/src/conversation-records.ts)
- [packages/runtime/src/conversation-fold-checkpoint.ts](https://github.com/withastro/flue/blob/deee93b40d275ecc9a50c50af2158f20089d54bc/packages/runtime/src/conversation-fold-checkpoint.ts)
- [packages/runtime/src/compaction.ts](https://github.com/withastro/flue/blob/deee93b40d275ecc9a50c50af2158f20089d54bc/packages/runtime/src/compaction.ts)
- [packages/runtime/src/sandbox.ts](https://github.com/withastro/flue/blob/deee93b40d275ecc9a50c50af2158f20089d54bc/packages/runtime/src/sandbox.ts)
- [packages/runtime/src/skill-frontmatter.ts](https://github.com/withastro/flue/blob/deee93b40d275ecc9a50c50af2158f20089d54bc/packages/runtime/src/skill-frontmatter.ts)
- [packages/runtime/src/cloudflare/flue-agent-class.ts](https://github.com/withastro/flue/blob/deee93b40d275ecc9a50c50af2158f20089d54bc/packages/runtime/src/cloudflare/flue-agent-class.ts)

Agent OS:

- [README.md](https://github.com/rivet-dev/agent-os/blob/c7462927ec452bbd5def5c0fc50a3fd74574d13a/README.md)
- [docs/content/docs/architecture.mdx](https://github.com/rivet-dev/agent-os/blob/c7462927ec452bbd5def5c0fc50a3fd74574d13a/docs/content/docs/architecture.mdx)
- [docs/content/docs/security-model.mdx](https://github.com/rivet-dev/agent-os/blob/c7462927ec452bbd5def5c0fc50a3fd74574d13a/docs/content/docs/security-model.mdx)
- [docs/content/docs/resource-limits.mdx](https://github.com/rivet-dev/agent-os/blob/c7462927ec452bbd5def5c0fc50a3fd74574d13a/docs/content/docs/resource-limits.mdx)
- [docs/content/docs/host-functions.mdx](https://github.com/rivet-dev/agent-os/blob/c7462927ec452bbd5def5c0fc50a3fd74574d13a/docs/content/docs/host-functions.mdx)
- [docs/content/docs/limitations.mdx](https://github.com/rivet-dev/agent-os/blob/c7462927ec452bbd5def5c0fc50a3fd74574d13a/docs/content/docs/limitations.mdx)
- [docs/content/docs/persistence.mdx](https://github.com/rivet-dev/agent-os/blob/c7462927ec452bbd5def5c0fc50a3fd74574d13a/docs/content/docs/persistence.mdx)
- [packages/core/src/agent-os.ts](https://github.com/rivet-dev/agent-os/blob/c7462927ec452bbd5def5c0fc50a3fd74574d13a/packages/core/src/agent-os.ts)
- [crates/kernel/src/](https://github.com/rivet-dev/agent-os/tree/c7462927ec452bbd5def5c0fc50a3fd74574d13a/crates/kernel/src)

Coder's side: [coax/lib/microcoder/loop.coax](../../coax/lib/microcoder/loop.coax), [psionic/lib/harness/sandbox.coax](../../psionic/lib/harness/sandbox.coax), [psionic/lib/harness/program.coax](../../psionic/lib/harness/program.coax). [Chapter plan](PLAN.md), Part VII.
